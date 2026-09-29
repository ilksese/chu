package main

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

type skillLockFile struct {
	Version int                       `json:"version"`
	Skills  map[string]skillLockItem  `json:"skills"`
	Prompts map[string]promptLockItem `json:"prompts,omitempty"`
}

type skillLockItem struct {
	Source      string `json:"source"`
	SourceType  string `json:"sourceType"`
	Ref         string `json:"ref"`
	SkillPath   string `json:"skillPath"`
	ContentHash string `json:"contentHash"`
	InstalledAt string `json:"installedAt"`
	UpdatedAt   string `json:"updatedAt"`
}

type SkillCandidate struct {
	Name        string `json:"name"`
	Description string `json:"description"`
	Path        string `json:"path"`
	Installed   bool   `json:"installed"`
}

type SkillUpdate struct {
	ID     string `json:"id"`
	Name   string `json:"name"`
	Status string `json:"status"`
}

func (a *App) PreviewSkills(repository string) ([]SkillCandidate, error) {
	parsed, err := parseSkillSource(repository)
	if err != nil {
		return nil, err
	}
	cacheDir, err := a.checkoutSkillSource(parsed.URL, parsed.Ref)
	if err != nil {
		return nil, err
	}
	found := filterSkillCandidates(discoverSkillCandidates(filepath.Join(cacheDir, parsed.Subpath)), parsed.Filter)
	a.mu.Lock()
	defer a.mu.Unlock()
	installed := map[string]bool{}
	for _, item := range a.state.Skills {
		installed[item.Name] = true
	}
	for i := range found {
		found[i].Installed = installed[found[i].Name]
	}
	return found, nil
}

func (a *App) InstallSkills(repository string, paths []string) (Snapshot, error) {
	parsed, err := parseSkillSource(repository)
	if err != nil {
		return Snapshot{}, err
	}
	if len(paths) == 0 {
		return Snapshot{}, errors.New("请至少选择一个 skill")
	}
	cacheDir, err := a.checkoutSkillSource(parsed.URL, parsed.Ref)
	if err != nil {
		return Snapshot{}, err
	}
	a.mu.Lock()
	defer a.mu.Unlock()
	lock, err := a.readSkillLock()
	if err != nil {
		return a.snapshotLocked(), err
	}
	var failed []string
	now := time.Now().UTC().Format(time.RFC3339)
	for _, relative := range paths {
		if err := a.installSkillPath(cacheDir, parsed, relative, lock, now); err != nil {
			failed = append(failed, err.Error())
		}
	}
	if err := a.saveStateLocked(); err != nil {
		return a.snapshotLocked(), err
	}
	if err := a.writeSkillLock(lock); err != nil {
		return a.snapshotLocked(), err
	}
	if len(failed) > 0 {
		return a.snapshotLocked(), fmt.Errorf("部分 skill 未安装: %s", strings.Join(failed, "; "))
	}
	return a.snapshotLocked(), nil
}

func (a *App) CheckSkillUpdates() ([]SkillUpdate, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	lock, err := a.readSkillLock()
	if err != nil {
		return nil, err
	}
	grouped := map[string][]string{}
	for name, item := range lock.Skills {
		grouped[item.Source+"\n"+item.Ref] = append(grouped[item.Source+"\n"+item.Ref], name)
	}
	var updates []SkillUpdate
	for key, names := range grouped {
		source, ref, _ := strings.Cut(key, "\n")
		cacheDir, err := a.checkoutSkillSource(source, ref)
		if err != nil {
			return nil, err
		}
		found := discoverSkillCandidates(cacheDir)
		byPath := map[string]SkillCandidate{}
		byName := map[string][]SkillCandidate{}
		for _, candidate := range found {
			byPath[candidate.Path] = candidate
			byName[candidate.Name] = append(byName[candidate.Name], candidate)
		}
		for _, name := range names {
			item := lock.Skills[name]
			installedName := name
			for _, stored := range a.state.Skills {
				if stored.Name == name {
					if front, _, ok := readSkillFrontmatter(stored.SourceDir); ok {
						installedName = front
					}
				}
			}
			matchName := name
			if byName[name] == nil && byName[installedName] != nil {
				matchName = installedName
			}
			status := skillUpdateStatus(cacheDir, matchName, item, byPath, byName[matchName])
			if status == "" {
				continue
			}
			id := name
			for _, stored := range a.state.Skills {
				if stored.Name == name {
					id = stored.ID
				}
			}
			updates = append(updates, SkillUpdate{ID: id, Name: name, Status: status})
		}
	}
	sort.Slice(updates, func(i, j int) bool { return updates[i].Name < updates[j].Name })
	return updates, nil
}

func (a *App) UpdateSkill(skillID string) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	stored, ok := a.skillByID(skillID)
	if !ok {
		return a.snapshotLocked(), errors.New("未知的 skill")
	}
	lock, err := a.readSkillLock()
	if err != nil {
		return a.snapshotLocked(), err
	}
	itemKey, item, ok := skillLockItemFor(lock, stored.Name, stored.ID)
	if !ok {
		return a.snapshotLocked(), errors.New("该 skill 没有来源记录，需要重新安装后才能更新")
	}
	cacheDir, err := a.checkoutSkillSource(item.Source, item.Ref)
	if err != nil {
		return a.snapshotLocked(), err
	}
	found := discoverSkillCandidates(cacheDir)
	relative := resolveSkillPath(item, stored.Name, stored.SourceDir, found)
	if relative == "" {
		return a.snapshotLocked(), errors.New("来源仓库里找不到这个 skill")
	}
	source := filepath.Join(cacheDir, filepath.FromSlash(relative))
	if err := replaceDir(source, stored.SourceDir); err != nil {
		return a.snapshotLocked(), err
	}
	if name, description, ok := readSkillFrontmatter(stored.SourceDir); ok {
		stored.Name = name
		stored.Description = description
	} else {
		stored.Description = readSkillDescription(stored.SourceDir)
	}
	delete(lock.Skills, itemKey)
	item.SkillPath = relative
	item.ContentHash = hashPath(stored.SourceDir)
	item.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
	lock.Skills[stored.Name] = item
	for hostID, dep := range stored.Deployments {
		if dep.Mode != "copy" || dep.Target == "" {
			continue
		}
		if hashPath(dep.Target) != dep.LastHash {
			continue
		}
		if err := replaceDir(stored.SourceDir, dep.Target); err != nil {
			return a.snapshotLocked(), err
		}
		dep.LastHash = hashPath(dep.Target)
		stored.Deployments[hostID] = dep
	}
	if err := a.syncProjectSkillCopiesLocked(stored); err != nil {
		return a.snapshotLocked(), err
	}
	if err := a.saveStateLocked(); err != nil {
		return a.snapshotLocked(), err
	}
	if err := a.writeSkillLock(lock); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) RemoveSkill(skillID string) (Snapshot, error) {
	return a.DeleteSkill(skillID)
}

func (a *App) DeleteSkill(skillID string) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	index := -1
	for i := range a.state.Skills {
		if a.state.Skills[i].ID == skillID {
			index = i
		}
	}
	if index < 0 {
		return a.snapshotLocked(), errors.New("只能删除 Chu 管理的 skill")
	}
	stored := a.state.Skills[index]
	if projects := a.projectsUsingSkillLocked(stored.ID); len(projects) > 0 {
		return a.snapshotLocked(), fmt.Errorf("该 skill 仍被项目使用: %s", strings.Join(projects, "、"))
	}
	for _, dep := range stored.Deployments {
		if dep.Mode == "copy" && hashPath(dep.Target) != dep.LastHash {
			return a.snapshotLocked(), errors.New("目标 copy 已被修改，已保护用户改动")
		}
	}
	for _, dep := range stored.Deployments {
		if dep.Target != "" && fileExists(dep.Target) {
			if err := os.RemoveAll(dep.Target); err != nil {
				return a.snapshotLocked(), err
			}
		}
	}
	if stored.SourceDir != "" {
		if err := os.RemoveAll(stored.SourceDir); err != nil {
			return a.snapshotLocked(), err
		}
	}
	a.state.Skills = append(a.state.Skills[:index], a.state.Skills[index+1:]...)
	lock, err := a.readSkillLock()
	if err != nil {
		return a.snapshotLocked(), err
	}
	delete(lock.Skills, stored.Name)
	if err := a.saveStateLocked(); err != nil {
		return a.snapshotLocked(), err
	}
	if err := a.writeSkillLock(lock); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) installSkillPath(cacheDir string, parsed skillSource, relative string, lock *skillLockFile, now string) error {
	relative = filepath.ToSlash(filepath.Clean(relative))
	if relative == "." || strings.HasPrefix(relative, "../") || strings.Contains(relative, "/../") {
		return fmt.Errorf("%s: 非法路径", relative)
	}
	origin := filepath.Join(cacheDir, filepath.FromSlash(relative))
	name, description, ok := readSkillFrontmatter(origin)
	if !ok {
		return fmt.Errorf("%s: 不是合法 skill", relative)
	}
	for _, item := range a.state.Skills {
		if item.Name == name {
			return fmt.Errorf("skill %q 已存在", name)
		}
	}
	dirName := slug(name)
	target := filepath.Join(a.root, "skills", dirName)
	if fileExists(target) {
		return fmt.Errorf("中央 skill 目录已存在: %s", target)
	}
	if err := copyDir(origin, target); err != nil {
		return err
	}
	a.state.Skills = append(a.state.Skills, storedSkill{
		ID: dirName, Name: name, Description: description, SourceDir: target,
		Deployments: map[string]deployment{},
	})
	lock.Skills[name] = skillLockItem{
		Source: parsed.URL, SourceType: parsed.Type, Ref: parsed.Ref, SkillPath: relative,
		ContentHash: hashPath(target), InstalledAt: now, UpdatedAt: now,
	}
	return nil
}

func (a *App) checkoutSkillSource(source, ref string) (string, error) {
	if ref == "" {
		ref = "main"
	}
	sum := sha256.Sum256([]byte(source + "\n" + ref))
	cacheDir := filepath.Join(a.root, "sources", hex.EncodeToString(sum[:]))
	if fileExists(filepath.Join(cacheDir, ".git")) {
		cmd := exec.Command("git", "-C", cacheDir, "fetch", "--depth", "1", "origin", ref)
		if output, err := cmd.CombinedOutput(); err != nil {
			return "", fmt.Errorf("Git fetch 失败: %s", strings.TrimSpace(string(output)))
		}
		cmd = exec.Command("git", "-C", cacheDir, "checkout", "--detach", "FETCH_HEAD")
		if output, err := cmd.CombinedOutput(); err != nil {
			return "", fmt.Errorf("Git checkout 失败: %s", strings.TrimSpace(string(output)))
		}
		return cacheDir, nil
	}
	if err := os.RemoveAll(cacheDir); err != nil {
		return "", err
	}
	if err := os.MkdirAll(filepath.Dir(cacheDir), 0o700); err != nil {
		return "", err
	}
	cmd := exec.Command("git", "clone", "--depth", "1", "--branch", ref, source, cacheDir)
	if output, err := cmd.CombinedOutput(); err != nil {
		_ = os.RemoveAll(cacheDir)
		return "", fmt.Errorf("Git clone 失败: %s", strings.TrimSpace(string(output)))
	}
	return cacheDir, nil
}

func (a *App) readSkillLock() (*skillLockFile, error) {
	lock := &skillLockFile{Version: 1, Skills: map[string]skillLockItem{}}
	data, err := os.ReadFile(a.skillLockPath())
	if errors.Is(err, os.ErrNotExist) {
		return lock, nil
	}
	if err != nil {
		return nil, err
	}
	if err := json.Unmarshal(data, lock); err != nil {
		return nil, err
	}
	if lock.Skills == nil {
		lock.Skills = map[string]skillLockItem{}
	}
	if lock.Prompts == nil {
		lock.Prompts = map[string]promptLockItem{}
	}
	lock.Version = 1
	return lock, nil
}

func (a *App) writeSkillLock(lock *skillLockFile) error {
	lock.Version = 1
	if lock.Skills == nil {
		lock.Skills = map[string]skillLockItem{}
	}
	if lock.Prompts == nil {
		lock.Prompts = map[string]promptLockItem{}
	}
	data, err := json.MarshalIndent(lock, "", "  ")
	if err != nil {
		return err
	}
	return atomicWrite(a.skillLockPath(), append(data, '\n'), 0o600)
}

func (a *App) skillLockPath() string {
	return filepath.Join(a.root, "chu-lock.json")
}

type legacySkill struct {
	Name       string `json:"name"`
	Repository string `json:"repository"`
	SourceDir  string `json:"sourceDir"`
	Version    string `json:"version"`
}

func (a *App) migrateSkillSources(data []byte) error {
	var legacy struct {
		Skills []legacySkill `json:"skills"`
	}
	if json.Unmarshal(data, &legacy) != nil {
		return nil
	}
	lock, err := a.readSkillLock()
	if err != nil {
		return err
	}
	changed := false
	now := time.Now().UTC().Format(time.RFC3339)
	for _, item := range legacy.Skills {
		if item.Repository == "" || lock.Skills[item.Name].Source != "" {
			continue
		}
		parsed, err := parseSkillSource(item.Repository)
		if err != nil {
			continue
		}
		path := "."
		if item.Name == "grill-me" && strings.Contains(parsed.URL, "mattpocock/skills") {
			path = "skills/productivity/grill-me"
		}
		lock.Skills[item.Name] = skillLockItem{
			Source: parsed.URL, SourceType: parsed.Type, Ref: firstNonEmpty(item.Version, parsed.Ref),
			SkillPath: path, ContentHash: hashPath(item.SourceDir), InstalledAt: now, UpdatedAt: now,
		}
		changed = true
	}
	if !changed {
		return nil
	}
	if err := a.writeSkillLock(lock); err != nil {
		return err
	}
	return a.saveStateLocked()
}

type skillSource struct {
	Type    string
	URL     string
	Ref     string
	Subpath string
	Filter  string
}

func parseSkillSource(input string) (skillSource, error) {
	input = strings.TrimSpace(input)
	if input == "" {
		return skillSource{}, errors.New("请填写 skill 来源")
	}
	base, fragment, _ := strings.Cut(input, "#")
	ref, filter := fragment, ""
	if i := strings.Index(fragment, "@"); i >= 0 {
		ref, filter = fragment[:i], fragment[i+1:]
	}
	if strings.HasPrefix(base, "github:") {
		base = strings.TrimPrefix(base, "github:")
	}
	if strings.HasPrefix(base, "gitlab:") {
		base = "https://gitlab.com/" + strings.TrimPrefix(base, "gitlab:")
	}
	if owner, repo, skill, ok := cutAtSkill(base); ok {
		return skillSource{Type: "github", URL: "https://github.com/" + owner + "/" + repo, Ref: defaultRef(ref), Filter: firstNonEmpty(filter, skill)}, nil
	}
	if parsed, ok := parseGitURL(base, ref, filter); ok {
		return parsed, nil
	}
	if !strings.Contains(base, ":") && !strings.HasPrefix(base, ".") && !strings.HasPrefix(base, "/") {
		parts := strings.Split(strings.Trim(base, "/"), "/")
		if len(parts) >= 2 && parts[0] != "" && parts[1] != "" {
			subpath := strings.Join(parts[2:], "/")
			return skillSource{Type: "github", URL: "https://github.com/" + parts[0] + "/" + strings.TrimSuffix(parts[1], ".git"), Ref: defaultRef(ref), Subpath: cleanSubpath(subpath), Filter: filter}, nil
		}
	}
	return skillSource{}, errors.New("skill 来源必须是公开 HTTPS Git 仓库或 owner/repo")
}

func parseGitURL(input, ref, filter string) (skillSource, bool) {
	parsed, err := url.Parse(input)
	if err != nil || parsed.Scheme != "https" || parsed.Host == "" {
		return skillSource{}, false
	}
	parts := strings.Split(strings.Trim(parsed.Path, "/"), "/")
	if parsed.Host == "github.com" && len(parts) >= 2 {
		source := skillSource{Type: "github", URL: "https://github.com/" + parts[0] + "/" + strings.TrimSuffix(parts[1], ".git"), Ref: defaultRef(ref), Filter: filter}
		if len(parts) >= 4 && parts[2] == "tree" {
			source.Ref = parts[3]
			source.Subpath = cleanSubpath(strings.Join(parts[4:], "/"))
		}
		return source, true
	}
	if parsed.Host == "gitlab.com" && len(parts) >= 2 {
		gitAt := indexOf(parts, "-")
		repoEnd := len(parts)
		source := skillSource{Type: "gitlab", Ref: defaultRef(ref), Filter: filter}
		if gitAt > 1 && gitAt+2 < len(parts) && parts[gitAt+1] == "tree" {
			repoEnd = gitAt
			source.Ref = parts[gitAt+2]
			source.Subpath = cleanSubpath(strings.Join(parts[gitAt+3:], "/"))
		}
		source.URL = "https://gitlab.com/" + strings.TrimSuffix(strings.Join(parts[:repoEnd], "/"), ".git")
		return source, true
	}
	if strings.HasSuffix(parsed.Path, ".git") {
		parsed.Path = strings.TrimSuffix(parsed.Path, ".git")
		return skillSource{Type: "git", URL: parsed.String(), Ref: defaultRef(ref), Filter: filter}, true
	}
	return skillSource{}, false
}

func cutAtSkill(input string) (string, string, string, bool) {
	owner, rest, ok := strings.Cut(input, "/")
	if !ok || strings.Contains(input, ":") || strings.Contains(owner, "@") {
		return "", "", "", false
	}
	repo, skill, ok := strings.Cut(rest, "@")
	return owner, repo, skill, ok && repo != "" && skill != "" && !strings.Contains(repo, "/")
}

func cleanSubpath(value string) string {
	value = strings.Trim(filepath.ToSlash(filepath.Clean(value)), "/")
	if value == "." || strings.HasPrefix(value, "../") || strings.Contains(value, "/../") {
		return ""
	}
	return value
}

func skillRepository(item skillLockItem) string {
	if item.Source == "" {
		return ""
	}
	repository := item.Source
	if item.Ref != "" && item.Ref != "main" {
		repository += "#" + item.Ref
	}
	return repository
}

func defaultRef(value string) string {
	if value == "" {
		return "main"
	}
	return value
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if value != "" {
			return value
		}
	}
	return ""
}

func indexOf(values []string, want string) int {
	for i, value := range values {
		if value == want {
			return i
		}
	}
	return -1
}

var skillSearchDirs = []string{
	"skills", "skills/.curated", "skills/.experimental", "skills/.system",
	".agent/skills", ".agents/skills", ".claude/skills", ".cline/skills", ".codebuddy/skills",
	".codex/skills", ".commandcode/skills", ".continue/skills", ".cursor/skills", ".factory/skills",
	".github/skills", ".goose/skills", ".grok/skills", ".hermes/skills", ".iflow/skills",
	".junie/skills", ".kilo/skills", ".kilocode/skills", ".kimchi/skills", ".kiro/skills",
	".minimax/skills", ".mux/skills", ".neovate/skills", ".opencode/skills", ".openhands/skills",
	".pi/skills", ".qoder/skills", ".roo/skills", ".trae/skills", ".windsurf/skills",
	".zcode/skills", ".zencoder/skills",
}

func discoverSkillCandidates(root string) []SkillCandidate {
	if name, description, ok := readSkillFrontmatter(root); ok && !skillInternal(root) {
		return []SkillCandidate{{Name: name, Description: description, Path: "."}}
	}
	var found []SkillCandidate
	seen := map[string]bool{}
	scanSkillDir(root, root, 1, seen, &found)
	for _, dir := range skillSearchDirs {
		scanSkillDir(root, filepath.Join(root, dir), 3, seen, &found)
	}
	for _, dir := range pluginSkillDirs(root) {
		scanSkillDir(root, dir, 1, seen, &found)
	}
	if len(found) == 0 {
		scanSkillDir(root, root, 5, seen, &found)
	}
	sort.Slice(found, func(i, j int) bool { return found[i].Path < found[j].Path })
	return found
}

func filterSkillCandidates(found []SkillCandidate, filter string) []SkillCandidate {
	if filter == "" || filter == "*" {
		return found
	}
	var matched []SkillCandidate
	for _, item := range found {
		if strings.EqualFold(item.Name, filter) {
			matched = append(matched, item)
		}
	}
	return matched
}

func pluginSkillDirs(root string) []string {
	var dirs []string
	for _, manifest := range []string{".claude-plugin/marketplace.json", ".claude-plugin/plugin.json"} {
		data, err := os.ReadFile(filepath.Join(root, manifest))
		if err != nil {
			continue
		}
		var parsed map[string]any
		if json.Unmarshal(data, &parsed) != nil {
			continue
		}
		collectPluginSkills(root, parsed, &dirs)
	}
	return dirs
}

func collectPluginSkills(root string, value any, dirs *[]string) {
	switch item := value.(type) {
	case map[string]any:
		if skills, ok := item["skills"].([]any); ok {
			for _, skill := range skills {
				path, ok := skill.(string)
				if ok && strings.HasPrefix(path, "./") {
					*dirs = append(*dirs, filepath.Dir(filepath.Join(root, path)))
				}
			}
		}
		for _, child := range item {
			collectPluginSkills(root, child, dirs)
		}
	case []any:
		for _, child := range item {
			collectPluginSkills(root, child, dirs)
		}
	}
}

func scanSkillDir(root, dir string, depth int, seen map[string]bool, found *[]SkillCandidate) {
	if depth < 0 || !fileExists(dir) {
		return
	}
	if !skillInternal(dir) {
		if name, description, ok := readSkillFrontmatter(dir); ok && !seen[name] {
			relative, err := filepath.Rel(root, dir)
			if err == nil {
				seen[name] = true
				*found = append(*found, SkillCandidate{
					Name: name, Description: description, Path: filepath.ToSlash(relative),
				})
			}
			return
		}
	}
	entries, err := os.ReadDir(dir)
	if err != nil {
		return
	}
	for _, entry := range entries {
		if entry.IsDir() && entry.Name() != "node_modules" && entry.Name() != ".git" {
			scanSkillDir(root, filepath.Join(dir, entry.Name()), depth-1, seen, found)
		}
	}
}

func skillLockItemFor(lock *skillLockFile, name, id string) (string, skillLockItem, bool) {
	if item, ok := lock.Skills[name]; ok {
		return name, item, true
	}
	if item, ok := lock.Skills[id]; ok {
		return id, item, true
	}
	return "", skillLockItem{}, false
}

func resolveSkillPath(item skillLockItem, name, installed string, found []SkillCandidate) string {
	byPath := map[string]SkillCandidate{}
	byName := map[string][]SkillCandidate{}
	for _, candidate := range found {
		byPath[candidate.Path] = candidate
		byName[candidate.Name] = append(byName[candidate.Name], candidate)
	}
	if candidate, ok := byPath[item.SkillPath]; ok && (candidate.Name == name || item.SkillPath != ".") {
		return candidate.Path
	}
	if front, _, ok := readSkillFrontmatter(installed); ok {
		if matches := byName[front]; len(matches) == 1 {
			return matches[0].Path
		}
	}
	if matches := byName[name]; len(matches) == 1 {
		return matches[0].Path
	}
	slugName := slug(name)
	var slugMatches []SkillCandidate
	for _, candidate := range found {
		if slug(candidate.Name) == slugName || slug(candidate.Path) == slugName || slug(filepath.Base(candidate.Path)) == slugName {
			slugMatches = append(slugMatches, candidate)
		}
	}
	if len(slugMatches) == 1 {
		return slugMatches[0].Path
	}
	return ""
}

func skillUpdateStatus(root, name string, item skillLockItem, byPath map[string]SkillCandidate, sameName []SkillCandidate) string {
	if candidate, ok := byPath[item.SkillPath]; ok && candidate.Name == name {
		if hashPath(filepath.Join(root, filepath.FromSlash(item.SkillPath))) != item.ContentHash {
			return "update"
		}
		return ""
	}
	if len(sameName) >= 1 {
		return "update"
	}
	if len(sameName) > 1 {
		return "ambiguous"
	}
	return "deleted"
}

func skillInternal(path string) bool {
	data, err := os.ReadFile(filepath.Join(path, "SKILL.md"))
	if err != nil {
		return false
	}
	text := strings.ReplaceAll(string(data), "\r\n", "\n")
	return strings.Contains(text, "internal: true")
}

func readSkillFrontmatter(path string) (string, string, bool) {
	data, err := os.ReadFile(filepath.Join(path, "SKILL.md"))
	if err != nil {
		return "", "", false
	}
	text := strings.ReplaceAll(string(data), "\r\n", "\n")
	if !strings.HasPrefix(text, "---\n") {
		return "", "", false
	}
	end := strings.Index(text, "\n---")
	if end < 0 {
		return "", "", false
	}
	var name, description string
	lines := strings.Split(text[4:end], "\n")
	for i := 0; i < len(lines); i++ {
		key, value, ok := strings.Cut(strings.TrimSpace(lines[i]), ":")
		if !ok {
			continue
		}
		value = strings.Trim(strings.TrimSpace(value), `"'`)
		if value == "|" || value == ">" || value == "|-" || value == ">-" {
			var block []string
			for i+1 < len(lines) && (strings.HasPrefix(lines[i+1], " ") || strings.HasPrefix(lines[i+1], "\t")) {
				i++
				block = append(block, strings.TrimSpace(lines[i]))
			}
			if value[0] == '>' {
				value = strings.Join(block, " ")
			} else {
				value = strings.Join(block, "\n")
			}
		}
		switch strings.ToLower(strings.TrimSpace(key)) {
		case "name":
			name = value
		case "description":
			description = value
		}
	}
	if name == "" || description == "" {
		return "", "", false
	}
	return name, description, true
}

func replaceDir(source, target string) error {
	if err := os.RemoveAll(target); err != nil {
		return err
	}
	return copyDir(source, target)
}
