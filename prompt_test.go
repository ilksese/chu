package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestPromptReplaceKeepsOriginalBackup(t *testing.T) {
	root := t.TempDir()
	hostDir := filepath.Join(root, "opencode")
	if err := os.MkdirAll(hostDir, 0o700); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(hostDir, "AGENTS.md")
	if err := os.WriteFile(target, []byte("host original"), 0o600); err != nil {
		t.Fatal(err)
	}
	app := &App{root: root, state: appState{Paths: map[string]hostPath{
		"opencode": {Config: filepath.Join(hostDir, "opencode.json")},
	}}}

	if _, err := app.CreatePrompt("alpha", "alpha body"); err != nil {
		t.Fatal(err)
	}
	if _, err := app.TogglePrompt("alpha", "opencode", true); err != nil {
		t.Fatal(err)
	}
	if _, err := app.CreatePrompt("beta", "beta body"); err != nil {
		t.Fatal(err)
	}
	if _, err := app.TogglePrompt("beta", "opencode", true); err != nil {
		t.Fatal(err)
	}

	backup, err := os.ReadFile(target + ".chubak")
	if err != nil || string(backup) != "host original" {
		t.Fatalf("backup = %q %v", backup, err)
	}
	linked, err := filepath.EvalSymlinks(target)
	if err != nil || filepath.Base(filepath.Dir(linked)) != "prompts" {
		t.Fatalf("target link = %s %v", linked, err)
	}
	if body, _ := os.ReadFile(target); string(body) != "beta body" {
		t.Fatalf("deployed = %q", body)
	}

	if _, err := app.TogglePrompt("beta", "opencode", false); err != nil {
		t.Fatal(err)
	}
	restored, err := os.ReadFile(target)
	if err != nil || string(restored) != "host original" || fileExists(target+".chubak") {
		t.Fatalf("restored = %q %v", restored, err)
	}
}

func TestPromptDriftClosesCopyManagement(t *testing.T) {
	root := t.TempDir()
	hostDir := filepath.Join(root, "codex")
	if err := os.MkdirAll(hostDir, 0o700); err != nil {
		t.Fatal(err)
	}
	app := &App{root: root, state: appState{Paths: map[string]hostPath{
		"codex": {Config: filepath.Join(hostDir, "config.toml")},
	}}}
	if _, err := app.CreatePrompt("rules", "chu rules"); err != nil {
		t.Fatal(err)
	}
	lock, err := app.readSkillLock()
	if err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(hostDir, "AGENTS.md")
	if err := os.WriteFile(target, []byte("edited outside"), 0o600); err != nil {
		t.Fatal(err)
	}
	item := lock.Prompts["rules"]
	item.Deployments = map[string]promptDeployment{
		"codex": {Mode: "copy", Target: target, LastHash: item.ContentHash},
	}
	lock.Prompts["rules"] = item
	if err := app.writeSkillLock(lock); err != nil {
		t.Fatal(err)
	}

	snapshot := app.GetSnapshot()
	if snapshot.Prompts[0].EnabledOn["codex"] {
		t.Fatal("drifted copy stayed managed")
	}
	if body, _ := os.ReadFile(target); string(body) != "edited outside" {
		t.Fatalf("user edit replaced = %q", body)
	}
}

func TestPromptSourceChangeFollowsSymlink(t *testing.T) {
	root := t.TempDir()
	hostDir := filepath.Join(root, "claude")
	if err := os.MkdirAll(hostDir, 0o700); err != nil {
		t.Fatal(err)
	}
	app := &App{root: root, state: appState{Paths: map[string]hostPath{
		"claude": {Config: filepath.Join(hostDir, "settings.json")},
	}}}
	if _, err := app.CreatePrompt("guide", "old"); err != nil {
		t.Fatal(err)
	}
	if _, err := app.TogglePrompt("guide", "claude", true); err != nil {
		t.Fatal(err)
	}
	source := filepath.Join(root, "prompts", "guide.md")
	if err := os.WriteFile(source, []byte("new"), 0o600); err != nil {
		t.Fatal(err)
	}

	snapshot := app.GetSnapshot()
	if !snapshot.Prompts[0].EnabledOn["claude"] {
		t.Fatal("live symlink was closed")
	}
	target := filepath.Join(hostDir, "CLAUDE.md")
	if body, _ := os.ReadFile(target); string(body) != "new" {
		t.Fatalf("followed source = %q", body)
	}
	lock, err := app.readSkillLock()
	if err != nil || lock.Prompts["guide"].ContentHash != fileMD5(source) {
		t.Fatalf("hash not updated: %#v %v", lock.Prompts["guide"], err)
	}
}
