package main

import (
	"crypto/md5"
	"encoding/hex"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

type promptLockItem struct {
	Name        string                      `json:"name"`
	File        string                      `json:"file"`
	ContentHash string                      `json:"contentHash"`
	UpdatedAt   string                      `json:"updatedAt"`
	Deployments map[string]promptDeployment `json:"deployments,omitempty"`
}

type promptDeployment struct {
	Mode     string `json:"mode"`
	Target   string `json:"target"`
	LastHash string `json:"lastHash"`
	Backup   bool   `json:"backup"`
}

func (a *App) CreatePrompt(name, content string) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	name = strings.TrimSpace(name)
	if name == "" || strings.ContainsAny(name, `/\`) || name == "." || name == ".." {
		return a.snapshotLocked(), errors.New("提示词名称无效")
	}
	name = strings.TrimSuffix(name, ".md")
	if name == "" {
		return a.snapshotLocked(), errors.New("提示词名称无效")
	}
	lock, err := a.reconcilePromptsLocked()
	if err != nil {
		return a.snapshotLocked(), err
	}
	if lock.Prompts == nil {
		lock.Prompts = map[string]promptLockItem{}
	}
	id := slug(name)
	if id == "" {
		id = fmt.Sprintf("prompt-%d", time.Now().Unix())
	}
	if _, exists := lock.Prompts[id]; exists {
		return a.snapshotLocked(), fmt.Errorf("提示词 %q 已存在", name)
	}
	fileName := name + ".md"
	path := filepath.Join(a.promptsDir(), fileName)
	if fileExists(path) {
		return a.snapshotLocked(), fmt.Errorf("提示词文件已存在: %s", path)
	}
	if err := os.MkdirAll(a.promptsDir(), 0o700); err != nil {
		return a.snapshotLocked(), err
	}
	if err := atomicWrite(path, []byte(content), 0o600); err != nil {
		return a.snapshotLocked(), err
	}
	lock.Prompts[id] = promptLockItem{
		Name: name, File: fileName, ContentHash: fileMD5(path), UpdatedAt: time.Now().Format(time.RFC3339),
		Deployments: map[string]promptDeployment{},
	}
	if err := a.writeSkillLock(lock); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) ReadPrompt(id string) (string, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	lock, err := a.readSkillLock()
	if err != nil {
		return "", err
	}
	item, ok := lock.Prompts[id]
	if !ok {
		return "", errors.New("未知的提示词")
	}
	data, err := os.ReadFile(filepath.Join(a.promptsDir(), item.File))
	if err != nil {
		return "", err
	}
	return string(data), nil
}

func (a *App) UpdatePrompt(id, name, content string) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	name = strings.TrimSpace(strings.TrimSuffix(name, ".md"))
	if name == "" || strings.ContainsAny(name, `/\`) || name == "." || name == ".." {
		return a.snapshotLocked(), errors.New("提示词名称无效")
	}
	lock, err := a.reconcilePromptsLocked()
	if err != nil {
		return a.snapshotLocked(), err
	}
	item, ok := lock.Prompts[id]
	if !ok {
		return a.snapshotLocked(), errors.New("未知的提示词")
	}
	nextID := slug(name)
	if nextID == "" {
		return a.snapshotLocked(), errors.New("提示词名称无效")
	}
	if nextID != id {
		if _, exists := lock.Prompts[nextID]; exists {
			return a.snapshotLocked(), fmt.Errorf("提示词 %q 已存在", name)
		}
	}
	oldPath := filepath.Join(a.promptsDir(), item.File)
	nextFile := name + ".md"
	nextPath := filepath.Join(a.promptsDir(), nextFile)
	if nextPath != oldPath && fileExists(nextPath) {
		return a.snapshotLocked(), fmt.Errorf("提示词文件已存在: %s", nextPath)
	}
	if err := atomicWrite(oldPath, []byte(content), 0o600); err != nil {
		return a.snapshotLocked(), err
	}
	if nextPath != oldPath {
		if err := os.Rename(oldPath, nextPath); err != nil {
			return a.snapshotLocked(), err
		}
		for hostID, dep := range item.Deployments {
			if dep.Mode == "link" && fileExists(dep.Target) {
				_ = os.Remove(dep.Target)
				if err := os.Symlink(nextPath, dep.Target); err != nil {
					if copyErr := copyFile(nextPath, dep.Target); copyErr != nil {
						return a.snapshotLocked(), fmt.Errorf("重命名后重新分发失败: %w", copyErr)
					}
					dep.Mode = "copy"
				}
			}
			dep.LastHash = fileMD5(nextPath)
			item.Deployments[hostID] = dep
		}
	}
	item.Name = name
	item.File = nextFile
	item.ContentHash = fileMD5(nextPath)
	item.UpdatedAt = time.Now().Format(time.RFC3339)
	delete(lock.Prompts, id)
	lock.Prompts[nextID] = item
	if err := a.syncPromptCopiesLocked(lock, nextID); err != nil {
		return a.snapshotLocked(), err
	}
	if err := a.writeSkillLock(lock); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) DeletePrompt(id string) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	lock, err := a.reconcilePromptsLocked()
	if err != nil {
		return a.snapshotLocked(), err
	}
	item, ok := lock.Prompts[id]
	if !ok {
		return a.snapshotLocked(), errors.New("未知的提示词")
	}
	for hostID := range item.Deployments {
		if err := a.detachPromptLocked(lock, id, hostID); err != nil {
			return a.snapshotLocked(), err
		}
	}
	if err := os.Remove(filepath.Join(a.promptsDir(), item.File)); err != nil && !errors.Is(err, os.ErrNotExist) {
		return a.snapshotLocked(), err
	}
	delete(lock.Prompts, id)
	if err := a.writeSkillLock(lock); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) TogglePrompt(id, hostID string, enabled bool) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	lock, err := a.reconcilePromptsLocked()
	if err != nil {
		return a.snapshotLocked(), err
	}
	if _, ok := lock.Prompts[id]; !ok {
		return a.snapshotLocked(), errors.New("未知的提示词")
	}
	if _, ok := a.hostByID(hostID); !ok {
		return a.snapshotLocked(), errors.New("未知的 Agent 宿主")
	}
	if enabled {
		err = a.attachPromptLocked(lock, id, hostID)
	} else {
		err = a.detachPromptLocked(lock, id, hostID)
	}
	if err != nil {
		return a.snapshotLocked(), err
	}
	if err := a.writeSkillLock(lock); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) reconcilePromptsLocked() (*skillLockFile, error) {
	lock, err := a.readSkillLock()
	if err != nil {
		return nil, err
	}
	changed := a.adoptPromptFilesLocked(lock)
	for id, item := range lock.Prompts {
		source := filepath.Join(a.promptsDir(), item.File)
		sourceHash := fileMD5(source)
		if sourceHash == "" {
			for hostID := range item.Deployments {
				delete(item.Deployments, hostID)
				changed = true
			}
			lock.Prompts[id] = item
			continue
		}
		if sourceHash != item.ContentHash {
			item.ContentHash = sourceHash
			item.UpdatedAt = time.Now().Format(time.RFC3339)
			changed = true
		}
		for hostID, dep := range item.Deployments {
			if a.promptStillManaged(source, dep) {
				if dep.Mode == "copy" && fileMD5(dep.Target) != sourceHash {
					if err := copyFile(source, dep.Target); err != nil {
						return lock, err
					}
				}
				dep.LastHash = sourceHash
				item.Deployments[hostID] = dep
				changed = true
				continue
			}
			delete(item.Deployments, hostID)
			changed = true
		}
		lock.Prompts[id] = item
	}
	if changed {
		if err := a.writeSkillLock(lock); err != nil {
			return lock, err
		}
	}
	return lock, nil
}

func (a *App) adoptPromptFilesLocked(lock *skillLockFile) bool {
	entries, err := os.ReadDir(a.promptsDir())
	if err != nil {
		return false
	}
	known := map[string]bool{}
	for _, item := range lock.Prompts {
		known[item.File] = true
	}
	changed := false
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".md") || known[entry.Name()] {
			continue
		}
		name := strings.TrimSuffix(entry.Name(), ".md")
		id := slug(name)
		if id == "" || lock.Prompts[id].File != "" {
			continue
		}
		path := filepath.Join(a.promptsDir(), entry.Name())
		lock.Prompts[id] = promptLockItem{
			Name: name, File: entry.Name(), ContentHash: fileMD5(path), UpdatedAt: time.Now().Format(time.RFC3339),
			Deployments: map[string]promptDeployment{},
		}
		changed = true
	}
	return changed
}

func (a *App) promptStillManaged(source string, dep promptDeployment) bool {
	info, err := os.Lstat(dep.Target)
	if err != nil {
		return false
	}
	if info.Mode()&os.ModeSymlink != 0 {
		resolved, err := filepath.EvalSymlinks(dep.Target)
		sourceResolved, sourceErr := filepath.EvalSymlinks(source)
		return err == nil && sourceErr == nil && filepath.Clean(resolved) == filepath.Clean(sourceResolved)
	}
	return dep.Mode == "copy" && fileMD5(dep.Target) == dep.LastHash
}

func (a *App) attachPromptLocked(lock *skillLockFile, id, hostID string) error {
	item := lock.Prompts[id]
	if item.Deployments == nil {
		item.Deployments = map[string]promptDeployment{}
	}
	if dep, ok := item.Deployments[hostID]; ok && a.promptStillManaged(filepath.Join(a.promptsDir(), item.File), dep) {
		return nil
	}
	target, err := a.promptTarget(hostID)
	if err != nil {
		return err
	}
	source := filepath.Join(a.promptsDir(), item.File)
	if fileMD5(source) == "" {
		return errors.New("提示词文件不可用")
	}
	managed := false
	backup := false
	for otherID, other := range lock.Prompts {
		dep, ok := other.Deployments[hostID]
		if !ok || otherID == id {
			continue
		}
		managed = true
		if dep.Backup {
			backup = true
		}
		if fileExists(dep.Target) {
			if err := os.Remove(dep.Target); err != nil {
				return err
			}
		}
		delete(other.Deployments, hostID)
		lock.Prompts[otherID] = other
	}
	if fileExists(target + ".chubak") {
		backup = true
	}
	if fileExists(target) && !managed {
		if !backup {
			if err := os.Rename(target, target+".chubak"); err != nil {
				return fmt.Errorf("备份宿主提示词失败: %w", err)
			}
			backup = true
		} else if err := os.Remove(target); err != nil {
			return err
		}
	} else if fileExists(target) {
		if err := os.Remove(target); err != nil {
			return err
		}
	}
	if err := os.MkdirAll(filepath.Dir(target), 0o700); err != nil {
		return err
	}
	mode := "link"
	if err := os.Symlink(source, target); err != nil {
		if copyErr := copyFile(source, target); copyErr != nil {
			return fmt.Errorf("link 失败，copy 也失败: %w", copyErr)
		}
		mode = "copy"
	}
	item.ContentHash = fileMD5(source)
	item.Deployments[hostID] = promptDeployment{Mode: mode, Target: target, LastHash: item.ContentHash, Backup: backup}
	lock.Prompts[id] = item
	return nil
}

func (a *App) detachPromptLocked(lock *skillLockFile, id, hostID string) error {
	item, ok := lock.Prompts[id]
	if !ok {
		return nil
	}
	dep, ok := item.Deployments[hostID]
	if !ok {
		return nil
	}
	if fileExists(dep.Target) {
		if err := os.Remove(dep.Target); err != nil {
			return err
		}
	}
	backup := dep.Target + ".chubak"
	if dep.Backup && fileExists(backup) {
		if err := os.Rename(backup, dep.Target); err != nil {
			return fmt.Errorf("恢复宿主提示词失败: %w", err)
		}
	}
	delete(item.Deployments, hostID)
	lock.Prompts[id] = item
	return nil
}

func (a *App) syncPromptCopiesLocked(lock *skillLockFile, id string) error {
	item := lock.Prompts[id]
	source := filepath.Join(a.promptsDir(), item.File)
	for hostID, dep := range item.Deployments {
		if dep.Mode != "copy" || !a.promptStillManaged(source, dep) {
			dep.LastHash = item.ContentHash
			item.Deployments[hostID] = dep
			continue
		}
		if err := copyFile(source, dep.Target); err != nil {
			return err
		}
		dep.LastHash = item.ContentHash
		item.Deployments[hostID] = dep
	}
	lock.Prompts[id] = item
	return nil
}

func (a *App) promptViewsLocked(lock *skillLockFile, specs []hostSpec) []PromptView {
	if lock == nil {
		return nil
	}
	views := make([]PromptView, 0, len(lock.Prompts))
	for id, item := range lock.Prompts {
		enabled := map[string]bool{}
		modes := map[string]string{}
		for _, spec := range specs {
			if dep, ok := item.Deployments[spec.id]; ok {
				enabled[spec.id] = true
				modes[spec.id] = dep.Mode
			} else {
				enabled[spec.id] = false
			}
		}
		views = append(views, PromptView{
			ID: id, Name: item.Name, Source: filepath.Join(a.promptsDir(), item.File),
			Preview: markdownPreview(filepath.Join(a.promptsDir(), item.File)), EnabledOn: enabled, ModeByHost: modes,
		})
	}
	sort.Slice(views, func(i, j int) bool { return views[i].Name < views[j].Name })
	return views
}

func (a *App) promptTarget(hostID string) (string, error) {
	host, ok := a.hostByID(hostID)
	if !ok {
		return "", errors.New("未知的 Agent 宿主")
	}
	dir := filepath.Dir(host.ConfigPath)
	switch hostID {
	case "claude":
		return filepath.Join(dir, "CLAUDE.md"), nil
	case "opencode":
		if filepath.Base(dir) != "opencode" {
			dir = filepath.Join(dir, "opencode")
		}
		return filepath.Join(dir, "AGENTS.md"), nil
	case "jcode":
		return filepath.Join(dir, "prompt-overlay.md"), nil
	default:
		return filepath.Join(dir, "AGENTS.md"), nil
	}
}

func (a *App) promptsDir() string {
	return filepath.Join(a.root, "prompts")
}

func markdownPreview(path string) string {
	data, err := os.ReadFile(path)
	if err != nil {
		return ""
	}
	text := strings.TrimSpace(strings.ReplaceAll(string(data), "\r\n", "\n"))
	if len([]rune(text)) > 80 {
		return string([]rune(text)[:80])
	}
	return text
}

func fileMD5(path string) string {
	data, err := os.ReadFile(path)
	if err != nil {
		return ""
	}
	sum := md5.Sum(data)
	return hex.EncodeToString(sum[:])
}
