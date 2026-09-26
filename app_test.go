package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestSlug(t *testing.T) {
	if got := slug("Code Review / v2"); got != "code-review-v2" {
		t.Fatalf("slug() = %q", got)
	}
}

func TestDiscoverCachedImpeccable(t *testing.T) {
	root := filepath.Join(os.Getenv("HOME"), ".chu", "sources", "0f1870cfd1835454918b05ed1b3966ecba6f1f04c02260f8fe6e66f15a271d1e")
	if !fileExists(root) {
		t.Skip("cache missing")
	}
	found := discoverSkillCandidates(root)
	if len(found) == 0 || found[0].Name != "impeccable" {
		t.Fatalf("cached candidates = %#v", found)
	}
}

func TestParseSkillSource(t *testing.T) {
	parsed, err := parseSkillSource("pbakaus/impeccable")
	if err != nil || parsed.URL != "https://github.com/pbakaus/impeccable" || parsed.Ref != "main" {
		t.Fatalf("shorthand = %#v %v", parsed, err)
	}
	parsed, err = parseSkillSource("https://github.com/mattpocock/skills/tree/main/skills/productivity/grill-me")
	if err != nil || parsed.Ref != "main" || parsed.Subpath != "skills/productivity/grill-me" {
		t.Fatalf("tree = %#v %v", parsed, err)
	}
	parsed, err = parseSkillSource("vercel-labs/agent-skills@web-design-guidelines")
	if err != nil || parsed.Filter != "web-design-guidelines" {
		t.Fatalf("filter = %#v %v", parsed, err)
	}
}

func TestDiscoverSkillCandidates(t *testing.T) {
	root := t.TempDir()
	writeSkill(t, filepath.Join(root, "skills", "productivity", "grill-me"), "grill-me", "A relentless interview.")
	writeSkill(t, filepath.Join(root, "skills", "productivity"), "not-a-skill", "")
	found := discoverSkillCandidates(root)
	if len(found) != 1 || found[0].Name != "grill-me" || found[0].Path != "skills/productivity/grill-me" {
		t.Fatalf("candidates = %#v", found)
	}

	repoRoot := t.TempDir()
	writeSkill(t, repoRoot, "taste-skill", "Design skill.")
	found = discoverSkillCandidates(repoRoot)
	if len(found) != 1 || found[0].Path != "." {
		t.Fatalf("root candidates = %#v", found)
	}

	agentRoot := t.TempDir()
	writeSkill(t, filepath.Join(agentRoot, ".agents", "skills", "impeccable"), "impeccable", "Design skill.")
	writeSkill(t, filepath.Join(agentRoot, ".claude", "skills", "impeccable"), "impeccable", "Duplicate.")
	found = discoverSkillCandidates(agentRoot)
	if len(found) != 1 || found[0].Path != ".agents/skills/impeccable" {
		t.Fatalf("agent candidates = %#v", found)
	}
}

func writeSkill(t *testing.T, dir, name, description string) {
	t.Helper()
	if err := os.MkdirAll(dir, 0o700); err != nil {
		t.Fatal(err)
	}
	body := "---\nname: " + name + "\n"
	if description != "" {
		body += "description: " + description + "\n"
	}
	body += "---\n"
	if err := os.WriteFile(filepath.Join(dir, "SKILL.md"), []byte(body), 0o600); err != nil {
		t.Fatal(err)
	}
}

func TestSkillUpdateStatus(t *testing.T) {
	root := t.TempDir()
	writeSkill(t, filepath.Join(root, ".agents", "skills", "impeccable"), "impeccable", "Updated.")
	item := skillLockItem{SkillPath: ".agent/skills/impeccable", ContentHash: "old"}
	same := []SkillCandidate{{Name: "impeccable", Path: ".agents/skills/impeccable"}}
	if got := skillUpdateStatus(root, "impeccable", item, map[string]SkillCandidate{}, same); got != "update" {
		t.Fatalf("relocated skill = %q", got)
	}
	renamed := []SkillCandidate{{Name: "design-taste-frontend", Path: "skills/taste-skill"}}
	if got := skillUpdateStatus(root, "design-taste-frontend", skillLockItem{SkillPath: ".", ContentHash: "old"}, map[string]SkillCandidate{}, renamed); got != "update" {
		t.Fatalf("renamed skill = %q", got)
	}
}

func TestDeleteManagedSkill(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(root, "skills", "tdd")
	host := filepath.Join(root, "host")
	writeSkill(t, source, "tdd", "Delete me.")
	if err := os.MkdirAll(host, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(source, filepath.Join(host, "tdd")); err != nil {
		t.Fatal(err)
	}
	app := &App{root: root, state: appState{Skills: []storedSkill{{
		ID: "tdd", Name: "tdd", SourceDir: source,
		Deployments: map[string]deployment{"opencode": {Enabled: true, Mode: "link", Target: filepath.Join(host, "tdd")}},
	}}}}
	if err := os.WriteFile(filepath.Join(root, "chu-lock.json"), []byte(`{"version":1,"skills":{"tdd":{"source":"https://github.com/org/skills"}}}`), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := app.DeleteSkill("missing"); err == nil {
		t.Fatal("unmanaged skill deleted")
	}
	if _, err := app.DeleteSkill("tdd"); err != nil {
		t.Fatal(err)
	}
	if fileExists(source) || fileExists(filepath.Join(host, "tdd")) || len(app.state.Skills) != 0 {
		t.Fatal("skill files or state remain")
	}
	lock, err := app.readSkillLock()
	if err != nil || len(lock.Skills) != 0 {
		t.Fatalf("lock = %#v %v", lock, err)
	}
}

func TestSkillRepository(t *testing.T) {
	if got := skillRepository(skillLockItem{Source: "https://github.com/org/skills", Ref: "main"}); got != "https://github.com/org/skills" {
		t.Fatalf("repository = %s", got)
	}
	if got := skillRepository(skillLockItem{Source: "https://github.com/org/skills", Ref: "v1"}); got != "https://github.com/org/skills#v1" {
		t.Fatalf("repository = %s", got)
	}
}

func TestResolveSkillPath(t *testing.T) {
	found := []SkillCandidate{{Name: "design-taste-frontend", Path: "skills/taste-skill"}}
	got := resolveSkillPath(skillLockItem{SkillPath: "."}, "taste-skill", "", found)
	if got != "skills/taste-skill" {
		t.Fatalf("slug path = %q", got)
	}
	root := t.TempDir()
	writeSkill(t, root, "design-taste-frontend", "Design skill.")
	got = resolveSkillPath(skillLockItem{SkillPath: "."}, "taste-skill", root, found)
	if got != "skills/taste-skill" {
		t.Fatalf("resolved path = %q", got)
	}
}

func TestReadSkillDescription(t *testing.T) {
	directory := t.TempDir()
	frontmatter := "---\nname: grill-me\ndescription: A relentless interview.\n---\n# Ignored\n"
	if err := os.WriteFile(filepath.Join(directory, "SKILL.md"), []byte(frontmatter), 0o600); err != nil {
		t.Fatal(err)
	}
	if got := readSkillDescription(directory); got != "A relentless interview." {
		t.Fatalf("frontmatter description = %q", got)
	}

	folded := "---\nname: grill-me\ndescription: >\n  A relentless interview\n  to sharpen a plan.\n---\n"
	if err := os.WriteFile(filepath.Join(directory, "SKILL.md"), []byte(folded), 0o600); err != nil {
		t.Fatal(err)
	}
	if got := readSkillDescription(directory); got != "A relentless interview to sharpen a plan." {
		t.Fatalf("folded description = %q", got)
	}

	heading := "# Code Review\n\nDetails\n"
	if err := os.WriteFile(filepath.Join(directory, "SKILL.md"), []byte(heading), 0o600); err != nil {
		t.Fatal(err)
	}
	if got := readSkillDescription(directory); got != "Code Review" {
		t.Fatalf("heading description = %q", got)
	}
}

func TestConfigRoundTrip(t *testing.T) {
	directory := t.TempDir()
	jsonPath := filepath.Join(directory, "config.json")
	tomlPath := filepath.Join(directory, "config.toml")
	want := map[string]any{"mcpServers": map[string]any{"demo": map[string]any{"command": "npx", "args": []string{"demo"}}}}

	if err := writeConfig(jsonPath, "json", want); err != nil {
		t.Fatal(err)
	}
	gotJSON, err := readConfig(jsonPath, "json")
	if err != nil || gotJSON["mcpServers"] == nil {
		t.Fatalf("JSON round trip failed: %v", err)
	}

	if err := writeConfig(tomlPath, "toml", want); err != nil {
		t.Fatal(err)
	}
	gotTOML, err := readConfig(tomlPath, "toml")
	if err != nil || gotTOML["mcpServers"] == nil {
		t.Fatalf("TOML round trip failed: %v", err)
	}
}

func TestMCPConfigHashesStayInSync(t *testing.T) {
	app := &App{state: appState{MCPs: []storedMCP{
		{ID: "first", Deployments: map[string]deployment{"opencode": {Enabled: true}}},
		{ID: "second", Deployments: map[string]deployment{"opencode": {Enabled: true}}},
	}}}
	path := filepath.Join(t.TempDir(), "config.json")
	if err := atomicWrite(path, []byte("{}\n"), 0o600); err != nil {
		t.Fatal(err)
	}

	app.syncMCPConfigHashes("opencode", path)
	first := app.state.MCPs[0].Deployments["opencode"].LastHash
	second := app.state.MCPs[1].Deployments["opencode"].LastHash
	if first == "" || first != second {
		t.Fatalf("deployment hashes differ: %q != %q", first, second)
	}
	if err := app.validateMCPConfigHash("opencode", path); err != nil {
		t.Fatal(err)
	}
	if err := atomicWrite(path, []byte("{\"external\":true}\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := app.validateMCPConfigHash("opencode", path); err == nil {
		t.Fatal("external change was not detected")
	}
}
