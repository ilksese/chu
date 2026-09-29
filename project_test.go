package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestProjectSkillLifecycleProtectsLocalChanges(t *testing.T) {
	root := t.TempDir()
	projectRoot := filepath.Join(root, "demo")
	source := filepath.Join(root, "skills", "review")
	if err := os.MkdirAll(projectRoot, 0o700); err != nil {
		t.Fatal(err)
	}
	writeSkill(t, source, "review", "Review changes.")
	app := &App{root: root, state: appState{Skills: []storedSkill{{ID: "review", Name: "review", SourceDir: source}}}}
	if _, err := app.addProjectLocked(projectRoot); err != nil {
		t.Fatal(err)
	}
	projectID := app.state.Projects[0].ID
	if _, err := app.ToggleProjectSkill(projectID, "review", "opencode", true); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(projectRoot, ".opencode", "skills", "review")
	if !fileExists(filepath.Join(target, "SKILL.md")) {
		t.Fatal("project skill was not copied")
	}
	if err := os.Remove(filepath.Join(target, "SKILL.md")); err != nil {
		t.Fatal(err)
	}
	views := app.projectViewsLocked([]HostView{{ID: "opencode"}})
	if status := views[0].Deployments["review"]["opencode"].Status; status != "missing" {
		t.Fatalf("status = %q, want missing", status)
	}
	if _, err := app.ResetProjectSkill(projectID, "review", "opencode"); err != nil {
		t.Fatal(err)
	}
	if !fileExists(filepath.Join(target, "SKILL.md")) {
		t.Fatal("reset did not restore missing skill files")
	}
	if err := os.WriteFile(filepath.Join(target, "note.txt"), []byte("local"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := app.ToggleProjectSkill(projectID, "review", "opencode", false); err == nil {
		t.Fatal("modified project skill was removed")
	}
	if !fileExists(target) {
		t.Fatal("modified project skill should remain")
	}
	if _, err := app.ResetProjectSkill(projectID, "review", "opencode"); err != nil {
		t.Fatal(err)
	}
	if fileExists(filepath.Join(target, "note.txt")) {
		t.Fatal("reset did not restore the central copy")
	}
	if err := os.WriteFile(filepath.Join(target, "note.txt"), []byte("local again"), 0o600); err != nil {
		t.Fatal(err)
	}
	result, err := app.DeleteProject(projectID, true)
	if err != nil {
		t.Fatal(err)
	}
	if len(result.Retained) != 1 || !fileExists(target) {
		t.Fatalf("retained = %#v", result.Retained)
	}
}

func TestDeleteSkillBlockedByProject(t *testing.T) {
	root := t.TempDir()
	app := &App{root: root, state: appState{
		Skills:   []storedSkill{{ID: "review", Name: "review", SourceDir: filepath.Join(root, "skills", "review")}},
		Projects: []storedProject{{ID: "project", Name: "Demo", Deployments: map[string]map[string]deployment{"review": {"opencode": {Enabled: true}}}}},
	}}
	if _, err := app.DeleteSkill("review"); err == nil {
		t.Fatal("referenced skill was deleted")
	}
}
