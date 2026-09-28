package main

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

func (a *App) CreateReference(name, content string) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	name, err := normalizeReferenceName(name)
	if err != nil {
		return a.snapshotLocked(), err
	}
	path := filepath.Join(a.referencesDir(), name+".md")
	if fileExists(path) {
		return a.snapshotLocked(), fmt.Errorf("Reference %q 已存在", name)
	}
	if err := atomicWrite(path, []byte(content), 0o600); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) ReadReference(id string) (string, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	path, err := a.referencePath(id)
	if err != nil {
		return "", err
	}
	data, err := os.ReadFile(path)
	return string(data), err
}

func (a *App) UpdateReference(id, name, content string) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	oldPath, err := a.referencePath(id)
	if err != nil {
		return a.snapshotLocked(), err
	}
	name, err = normalizeReferenceName(name)
	if err != nil {
		return a.snapshotLocked(), err
	}
	nextPath := filepath.Join(a.referencesDir(), name+".md")
	if nextPath != oldPath && fileExists(nextPath) {
		oldInfo, oldErr := os.Lstat(oldPath)
		nextInfo, nextErr := os.Lstat(nextPath)
		if oldErr != nil || nextErr != nil || !os.SameFile(oldInfo, nextInfo) {
			return a.snapshotLocked(), fmt.Errorf("Reference %q 已存在", name)
		}
	}
	if err := atomicWrite(oldPath, []byte(content), 0o600); err != nil {
		return a.snapshotLocked(), err
	}
	if nextPath != oldPath {
		if err := os.Rename(oldPath, nextPath); err != nil {
			return a.snapshotLocked(), err
		}
	}
	return a.snapshotLocked(), nil
}

func (a *App) DeleteReference(id string) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	path, err := a.referencePath(id)
	if err != nil {
		return a.snapshotLocked(), err
	}
	if err := os.Remove(path); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) referenceViewsLocked() []ReferenceView {
	entries, err := os.ReadDir(a.referencesDir())
	if err != nil {
		return []ReferenceView{}
	}
	views := make([]ReferenceView, 0, len(entries))
	for _, entry := range entries {
		if !strings.HasSuffix(entry.Name(), ".md") {
			continue
		}
		info, err := entry.Info()
		if err != nil || !info.Mode().IsRegular() {
			continue
		}
		path := filepath.Join(a.referencesDir(), entry.Name())
		views = append(views, ReferenceView{
			ID: entry.Name(), Name: strings.TrimSuffix(entry.Name(), ".md"),
			Source: path, Preview: markdownPreview(path),
		})
	}
	sort.Slice(views, func(i, j int) bool { return views[i].Name < views[j].Name })
	return views
}

func (a *App) referencesDir() string {
	return filepath.Join(a.root, "references")
}

func (a *App) referencePath(id string) (string, error) {
	if filepath.Base(id) != id || strings.ContainsAny(id, `/\`) || !strings.HasSuffix(id, ".md") {
		return "", errors.New("未知的 Reference")
	}
	path := filepath.Join(a.referencesDir(), id)
	info, err := os.Lstat(path)
	if err != nil || !info.Mode().IsRegular() {
		return "", errors.New("未知的 Reference")
	}
	return path, nil
}

func normalizeReferenceName(name string) (string, error) {
	name = strings.TrimSpace(strings.TrimSuffix(strings.TrimSpace(name), ".md"))
	if name == "" || name == "." || name == ".." || strings.ContainsAny(name, `/\`) {
		return "", errors.New("Reference 名称无效")
	}
	return name, nil
}
