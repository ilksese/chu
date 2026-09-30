package main

import (
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/joho/godotenv"
)

func TestProviderEnvPreservesAndEscapes(t *testing.T) {
	original := []byte("# keep comment\nOTHER='untouched # value'\nMULTI=\"first\nsecond\"\nKEY=first\nKEY=second # retain comment\n")
	for _, key := range []string{"plain", "123", "00123", `quotes'\"$HOME\backslash# =!`, "first\nsecond\rthird", " spaces ", "tab\tkey", `escaped\nkey`} {
		updated, err := providerEnvUpdate(original, "KEY", key)
		if err != nil || !strings.HasPrefix(string(updated), "# keep comment\nOTHER='untouched # value'\nMULTI=\"first\nsecond\"\n") || !strings.Contains(string(updated), "# retain comment\n") {
			t.Fatalf("update damaged content for %q: %v", key, err)
		}
		values, err := godotenv.Unmarshal(string(updated))
		if err != nil || values["KEY"] != key || values["OTHER"] != "untouched # value" || values["MULTI"] != "first\nsecond" {
			t.Fatalf("round trip: %#v %v", values, err)
		}
	}
	if _, err := providerEnvUpdate([]byte("BROKEN=\"unterminated"), "KEY", "value"); err == nil {
		t.Fatal("accepted malformed dotenv")
	}
	a := providerTestApp(t)
	if err := os.MkdirAll(a.root, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(a.root, ".env"), original, 0o644); err != nil {
		t.Fatal(err)
	}
	if err := saveProviderCredential(a.root, "KEY", "new", []byte("{}\n")); err != nil {
		t.Fatal(err)
	}
	for _, file := range []string{".env", "state.json"} {
		info, err := os.Stat(filepath.Join(a.root, file))
		if err != nil || info.Mode().Perm() != 0o600 {
			t.Fatalf("permissions %s: %v %v", file, info, err)
		}
	}
	t.Setenv("KEY", "fallback")
	if err := os.WriteFile(filepath.Join(a.root, ".env"), []byte("KEY=\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := providerKey(a.root, "KEY"); err == nil {
		t.Fatal("empty file key incorrectly fell back to process environment")
	}
}

func TestProviderEnvReplaceRecords(t *testing.T) {
	for _, newline := range []string{"\n", "\r\n"} {
		t.Run(strings.ReplaceAll(newline, "\n", "LF"), func(t *testing.T) {
			head := "# independent comment" + newline + "OTHER=untouched # retain exactly" + newline + "MULTI=\"first" + newline + "KEY=hidden-in-value" + newline + "# also inside value" + newline + "last\"" + newline
			tail := "NEXT='unchanged' # trailing comment" + newline
			original := []byte(head + "export KEY='old-first' # first note" + newline + "# between duplicates" + newline + "KEY=\"old-second" + newline + "still-old\" # second note" + newline + tail)
			updated, err := providerEnvUpdate(original, "KEY", "new-secret")
			if err != nil {
				t.Fatal(err)
			}
			want := head + "KEY=\"new-secret\" # first note" + newline + "# between duplicates" + newline + "# second note" + newline + tail
			if string(updated) != want {
				t.Fatalf("record preservation mismatch:\ngot  %q\nwant %q", updated, want)
			}
			for i := 0; i < 10; i++ {
				previous := "new-secret"
				if i > 0 {
					previous = strings.Repeat("x", i)
				}
				key := strings.Repeat("x", i+1)
				updated, err = providerEnvUpdate(updated, "KEY", key)
				if err != nil {
					t.Fatal(err)
				}
				want = strings.Replace(want, "KEY=\""+previous+"\"", "KEY=\""+key+"\"", 1)
				if string(updated) != want {
					t.Fatalf("repeated update accumulated assignments or changed records: %q", updated)
				}
				values, err := godotenv.Unmarshal(string(updated))
				if err != nil || values["KEY"] != key || values["MULTI"] != "first\nKEY=hidden-in-value\n# also inside value\nlast" {
					t.Fatalf("multiline value damaged: %#v %v", values, err)
				}
			}
		})
	}
}

func TestProviderEnvInlineCommentsAndMissingTarget(t *testing.T) {
	for _, original := range []string{
		`KEY="hash # inside" # outside`,
		`KEY='hash # inside' # outside`,
		`KEY=value#inside # outside`,
		"KEY=\"first\n# inside\nlast\" # outside",
	} {
		updated, err := providerEnvUpdate([]byte(original), "KEY", "new")
		if err != nil || string(updated) != `KEY="new" # outside` {
			t.Fatalf("inline comment lost or value mistaken for comment: %q %v", updated, err)
		}
	}
	updated, err := providerEnvUpdate([]byte("KEY=value # outside # nested note\n"), "KEY", "new")
	if err != nil || string(updated) != "KEY=\"new\" # outside # nested note\n" {
		t.Fatalf("multiple comment hashes lost: %q %v", updated, err)
	}
	original := "OTHER=\"start\r\nKEY=not-an-assignment\r\nend\"\r\n# retain"
	updated, err = providerEnvUpdate([]byte(original), "KEY", "new")
	if err != nil || string(updated) != original+"\r\nKEY=\"new\"\r\n" {
		t.Fatalf("missing target/CRLF append: %q %v", updated, err)
	}
	if _, err := providerEnvUpdate([]byte(`KEY="old" OTHER="keep"`), "KEY", "new"); err == nil {
		t.Fatal("ambiguous shared record should fail rather than drop another variable")
	}
}

func TestProviderPersistenceFailureIsConsistent(t *testing.T) {
	a := providerTestApp(t)
	if err := os.MkdirAll(filepath.Join(a.root, "state.json"), 0o700); err != nil {
		t.Fatal(err)
	}
	original := []byte("# comment\nKEY=old\n")
	if err := os.WriteFile(filepath.Join(a.root, ".env"), original, 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := a.CreateProvider(ProviderInput{Name: "Example", BaseURL: "http://127.0.0.1:1", EnvAPIKey: "KEY", APIKey: "new"}); err == nil {
		t.Fatal("state save unexpectedly succeeded")
	}
	actual, err := os.ReadFile(filepath.Join(a.root, ".env"))
	if err != nil || string(actual) != string(original) || len(a.state.Providers) != 0 {
		t.Fatalf("failed save changed credential/state: %v", err)
	}
	if err := os.Remove(filepath.Join(a.root, ".env")); err != nil {
		t.Fatal(err)
	}
	if _, err := a.CreateProvider(ProviderInput{Name: "Example", BaseURL: "http://127.0.0.1:1", EnvAPIKey: "KEY", APIKey: "new"}); err == nil {
		t.Fatal("state save unexpectedly succeeded without original credential")
	}
	if _, err := os.Stat(filepath.Join(a.root, ".env")); !os.IsNotExist(err) {
		t.Fatal("failed save left newly created credentials")
	}
	if err := os.RemoveAll(filepath.Join(a.root, "state.json")); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(filepath.Join(a.root, ".env"), 0o700); err != nil {
		t.Fatal(err)
	}
	if _, err := a.CreateProvider(ProviderInput{Name: "Example", BaseURL: "http://127.0.0.1:1", EnvAPIKey: "KEY", APIKey: "new"}); err == nil || len(a.state.Providers) != 0 {
		t.Fatal("credential failure changed state")
	}
	if err := os.Remove(filepath.Join(a.root, ".env")); err != nil {
		t.Fatal(err)
	}
	malformed := []byte("KEY=\"unterminated\n")
	if err := os.WriteFile(filepath.Join(a.root, ".env"), malformed, 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := a.CreateProvider(ProviderInput{Name: "Example", BaseURL: "http://127.0.0.1:1", EnvAPIKey: "KEY", APIKey: "new"}); err == nil {
		t.Fatal("malformed original file overwritten")
	}
	actual, err = os.ReadFile(filepath.Join(a.root, ".env"))
	if err != nil || string(actual) != string(malformed) || len(a.state.Providers) != 0 {
		t.Fatal("malformed original file or effective state changed")
	}
}

func TestProviderCacheSaveFailureKeepsMemoryAndDisk(t *testing.T) {
	a := providerTestApp(t)
	p := ProviderView{ID: "id", Name: "Example", BaseURL: "http://127.0.0.1:1", Models: []string{"cached"}, ModelsFetchedAt: "earlier"}
	if err := a.commitProvidersLocked([]ProviderView{p}, "", ""); err != nil {
		t.Fatal(err)
	}
	statePath := filepath.Join(a.root, "state.json")
	if err := os.Rename(statePath, statePath+".saved"); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(statePath, 0o700); err != nil {
		t.Fatal(err)
	}
	if _, err := a.RefreshProviderModels(p.ID); err == nil || !reflect.DeepEqual(a.state.Providers[0], p) {
		t.Fatal("cache save failure changed effective memory state")
	}
	if _, err := a.DeleteProvider(p.ID); err == nil || len(a.state.Providers) != 1 {
		t.Fatal("failed delete changed effective state")
	}
	if _, err := a.UpdateProvider(p.ID, ProviderInput{Name: "Renamed", BaseURL: p.BaseURL}); err == nil || a.state.Providers[0].Name != p.Name {
		t.Fatal("failed rename changed effective state")
	}
}
