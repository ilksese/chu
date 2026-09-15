package main

import (
	"path/filepath"
	"testing"
)

func TestSlug(t *testing.T) {
	if got := slug("Code Review / v2"); got != "code-review-v2" {
		t.Fatalf("slug() = %q", got)
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
