package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestReferenceCRUDAndExternalDiscovery(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, "references")
	if err := os.MkdirAll(filepath.Join(dir, "nested.md"), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "manual.md"), []byte("manual body"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "ignored.txt"), []byte("ignored"), 0o600); err != nil {
		t.Fatal(err)
	}
	app := &App{root: root}

	snapshot := app.GetSnapshot()
	if len(snapshot.References) != 1 || snapshot.References[0].Name != "manual" || snapshot.References[0].Preview != "manual body" {
		t.Fatalf("discovered references = %#v", snapshot.References)
	}

	if _, err := app.CreateReference("guide.md", "first body"); err != nil {
		t.Fatal(err)
	}
	body, err := app.ReadReference("guide.md")
	if err != nil || body != "first body" {
		t.Fatalf("read reference = %q %v", body, err)
	}
	if _, err := app.UpdateReference("guide.md", "renamed", "second body"); err != nil {
		t.Fatal(err)
	}
	if fileExists(filepath.Join(dir, "guide.md")) {
		t.Fatal("old reference remains after rename")
	}
	body, err = app.ReadReference("renamed.md")
	if err != nil || body != "second body" {
		t.Fatalf("updated reference = %q %v", body, err)
	}
	if _, err := app.DeleteReference("renamed.md"); err != nil {
		t.Fatal(err)
	}
	if fileExists(filepath.Join(dir, "renamed.md")) {
		t.Fatal("deleted reference remains")
	}

	if _, err := app.CreateReference("../escape", "bad"); err == nil {
		t.Fatal("path traversal name accepted")
	}
	if _, err := app.ReadReference("../manual.md"); err == nil {
		t.Fatal("path traversal id accepted")
	}
}
