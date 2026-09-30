package main

import (
	"crypto/sha256"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	wailsruntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

type storedProject struct {
	ID          string                           `json:"id"`
	Name        string                           `json:"name"`
	Path        string                           `json:"path"`
	CreatedAt   string                           `json:"createdAt"`
	Deployments map[string]map[string]deployment `json:"deployments,omitempty"`
}

type ProjectView struct {
	ID          string                                      `json:"id"`
	Name        string                                      `json:"name"`
	Path        string                                      `json:"path"`
	Available   bool                                        `json:"available"`
	CreatedAt   string                                      `json:"createdAt"`
	Deployments map[string]map[string]ProjectDeploymentView `json:"deployments"`
}

type ProjectDeploymentView struct {
	Enabled bool   `json:"enabled"`
	Status  string `json:"status"`
}

type ProjectRemovalResult struct {
	Snapshot Snapshot `json:"snapshot"`
	Retained []string `json:"retained"`
}

var projectSkillDirs = map[string]string{
	"opencode": filepath.Join(".opencode", "skills"),
	"claude":   filepath.Join(".claude", "skills"),
	"codex":    filepath.Join(".agents", "skills"),
	"jcode":    filepath.Join(".jcode", "skills"),
}

func (a *App) AddProject() (Snapshot, error) {
	path, err := wailsruntime.OpenDirectoryDialog(a.ctx, wailsruntime.OpenDialogOptions{
		Title:                "选择项目根目录",
		CanCreateDirectories: true,
		ResolvesAliases:      true,
	})
	if err != nil || path == "" {
		return a.GetSnapshot(), err
	}
	a.mu.Lock()
	defer a.mu.Unlock()
	return a.addProjectLocked(path)
}

func (a *App) RenameProject(projectID, name string) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	project, ok := a.projectByID(projectID)
	if !ok {
		return a.snapshotLocked(), errors.New("未知的项目")
	}
	name = strings.TrimSpace(name)
	if name == "" {
		return a.snapshotLocked(), errors.New("项目名称不能为空")
	}
	project.Name = name
	if err := a.saveStateLocked(); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) RelocateProject(projectID string) (Snapshot, error) {
	path, err := wailsruntime.OpenDirectoryDialog(a.ctx, wailsruntime.OpenDialogOptions{
		Title:                "重新选择项目根目录",
		CanCreateDirectories: true,
		ResolvesAliases:      true,
	})
	if err != nil || path == "" {
		return a.GetSnapshot(), err
	}
	path, err = canonicalProjectPath(path)
	if err != nil {
		return a.GetSnapshot(), err
	}

	a.mu.Lock()
	defer a.mu.Unlock()
	project, ok := a.projectByID(projectID)
	if !ok {
		return a.snapshotLocked(), errors.New("未知的项目")
	}
	for _, item := range a.state.Projects {
		if item.ID != projectID && item.Path == path {
			return a.snapshotLocked(), errors.New("该目录已关联为项目")
		}
	}
	project.Path = path
	for skillID, hosts := range project.Deployments {
		skill, exists := a.skillByID(skillID)
		for hostID := range hosts {
			target := projectSkillTarget(path, hostID, skillName(skill, skillID))
			dep := deployment{Enabled: true, Mode: "copy", Target: target}
			if fileExists(target) {
				dep.Mode = "conflict"
			} else if exists {
				if copyErr := copyDir(skill.SourceDir, target); copyErr == nil {
					dep.LastHash = hashPath(target)
				}
			}
			hosts[hostID] = dep
		}
	}
	if err := a.saveStateLocked(); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) ToggleProjectSkill(projectID, skillID, hostID string, enabled bool) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	project, ok := a.projectByID(projectID)
	if !ok {
		return a.snapshotLocked(), errors.New("未知的项目")
	}
	if !directoryExists(project.Path) {
		return a.snapshotLocked(), errors.New("项目根目录不可用")
	}
	skill, ok := a.skillByID(skillID)
	if !ok {
		return a.snapshotLocked(), errors.New("只能部署 Chu 管理的 skill")
	}
	if _, ok := projectSkillDirs[hostID]; !ok {
		return a.snapshotLocked(), errors.New("未知的 Agent 宿主")
	}
	if project.Deployments == nil {
		project.Deployments = map[string]map[string]deployment{}
	}
	if project.Deployments[skillID] == nil {
		project.Deployments[skillID] = map[string]deployment{}
	}
	dep, deployed := project.Deployments[skillID][hostID]
	target := projectSkillTarget(project.Path, hostID, skill.Name)
	if enabled {
		if deployed {
			return a.snapshotLocked(), nil
		}
		if !fileExists(filepath.Join(skill.SourceDir, "SKILL.md")) {
			return a.snapshotLocked(), errors.New("中央 skill 源目录不可用")
		}
		if fileExists(target) {
			return a.snapshotLocked(), fmt.Errorf("目标目录已存在，请先处理冲突: %s", target)
		}
		if err := copyDir(skill.SourceDir, target); err != nil {
			return a.snapshotLocked(), err
		}
		project.Deployments[skillID][hostID] = deployment{Enabled: true, Mode: "copy", Target: target, LastHash: hashPath(target)}
	} else {
		if !deployed {
			return a.snapshotLocked(), nil
		}
		if dep.Mode == "copy" && fileExists(dep.Target) {
			if hashPath(dep.Target) != dep.LastHash {
				return a.snapshotLocked(), errors.New("项目中的 skill 已被修改，已保护本地改动")
			}
			if err := os.RemoveAll(dep.Target); err != nil {
				return a.snapshotLocked(), err
			}
		}
		delete(project.Deployments[skillID], hostID)
		if len(project.Deployments[skillID]) == 0 {
			delete(project.Deployments, skillID)
		}
	}
	if err := a.saveStateLocked(); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) ResetProjectSkill(projectID, skillID, hostID string) (Snapshot, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	project, ok := a.projectByID(projectID)
	if !ok {
		return a.snapshotLocked(), errors.New("未知的项目")
	}
	skill, ok := a.skillByID(skillID)
	if !ok {
		return a.snapshotLocked(), errors.New("未知的 skill")
	}
	dep, ok := project.Deployments[skillID][hostID]
	if !ok || dep.Mode != "copy" {
		return a.snapshotLocked(), errors.New("该项目 Skill 不能重置")
	}
	if err := replaceDir(skill.SourceDir, dep.Target); err != nil {
		return a.snapshotLocked(), err
	}
	dep.LastHash = hashPath(dep.Target)
	project.Deployments[skillID][hostID] = dep
	if err := a.saveStateLocked(); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) DeleteProject(projectID string, cleanup bool) (ProjectRemovalResult, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	index := -1
	for i := range a.state.Projects {
		if a.state.Projects[i].ID == projectID {
			index = i
			break
		}
	}
	if index < 0 {
		return ProjectRemovalResult{Snapshot: a.snapshotLocked()}, errors.New("未知的项目")
	}
	var retained []string
	if cleanup {
		for _, hosts := range a.state.Projects[index].Deployments {
			for _, dep := range hosts {
				if dep.Mode != "copy" || !fileExists(dep.Target) {
					continue
				}
				if hashPath(dep.Target) != dep.LastHash {
					retained = append(retained, dep.Target)
					continue
				}
				if err := os.RemoveAll(dep.Target); err != nil {
					return ProjectRemovalResult{Snapshot: a.snapshotLocked()}, err
				}
			}
		}
	}
	a.state.Projects = append(a.state.Projects[:index], a.state.Projects[index+1:]...)
	if err := a.saveStateLocked(); err != nil {
		return ProjectRemovalResult{Snapshot: a.snapshotLocked()}, err
	}
	return ProjectRemovalResult{Snapshot: a.snapshotLocked(), Retained: retained}, nil
}

func (a *App) addProjectLocked(path string) (Snapshot, error) {
	path, err := canonicalProjectPath(path)
	if err != nil {
		return a.snapshotLocked(), err
	}
	for _, item := range a.state.Projects {
		if item.Path == path {
			return a.snapshotLocked(), errors.New("该目录已关联为项目")
		}
	}
	sum := sha256.Sum256([]byte(path))
	a.state.Projects = append(a.state.Projects, storedProject{
		ID: fmt.Sprintf("%x", sum[:8]), Name: filepath.Base(path), Path: path,
		CreatedAt: time.Now().UTC().Format(time.RFC3339), Deployments: map[string]map[string]deployment{},
	})
	if err := a.saveStateLocked(); err != nil {
		return a.snapshotLocked(), err
	}
	return a.snapshotLocked(), nil
}

func (a *App) projectViewsLocked(hosts []HostView) []ProjectView {
	views := make([]ProjectView, 0, len(a.state.Projects))
	for _, project := range a.state.Projects {
		states := map[string]map[string]ProjectDeploymentView{}
		for _, skill := range a.state.Skills {
			for _, host := range hosts {
				dep, deployed := project.Deployments[skill.ID][host.ID]
				target := projectSkillTarget(project.Path, host.ID, skill.Name)
				status := ""
				if deployed {
					switch {
					case dep.Mode == "conflict":
						status = "conflict"
					case !fileExists(filepath.Join(dep.Target, "SKILL.md")):
						status = "missing"
					case hashPath(dep.Target) != dep.LastHash:
						status = "modified"
					default:
						status = "enabled"
					}
				} else if fileExists(target) {
					status = "conflict"
				}
				if status != "" {
					if states[skill.ID] == nil {
						states[skill.ID] = map[string]ProjectDeploymentView{}
					}
					states[skill.ID][host.ID] = ProjectDeploymentView{Enabled: deployed, Status: status}
				}
			}
		}
		views = append(views, ProjectView{ID: project.ID, Name: project.Name, Path: project.Path, Available: directoryExists(project.Path), CreatedAt: project.CreatedAt, Deployments: states})
	}
	sort.SliceStable(views, func(i, j int) bool { return views[i].CreatedAt > views[j].CreatedAt })
	return views
}

func (a *App) projectByID(id string) (*storedProject, bool) {
	for i := range a.state.Projects {
		if a.state.Projects[i].ID == id {
			return &a.state.Projects[i], true
		}
	}
	return nil, false
}

func canonicalProjectPath(path string) (string, error) {
	path, err := filepath.Abs(filepath.Clean(path))
	if err != nil {
		return "", err
	}
	path, err = filepath.EvalSymlinks(path)
	if err != nil {
		return "", fmt.Errorf("项目目录不可用: %w", err)
	}
	if !directoryExists(path) {
		return "", errors.New("请选择有效的项目目录")
	}
	return path, nil
}

func projectSkillTarget(root, hostID, skillName string) string {
	return filepath.Join(root, projectSkillDirs[hostID], skillName)
}

func directoryExists(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.IsDir()
}

func skillName(skill *storedSkill, fallback string) string {
	if skill != nil {
		return skill.Name
	}
	return fallback
}

func (a *App) projectsUsingSkillLocked(skillID string) []string {
	var names []string
	for _, project := range a.state.Projects {
		if len(project.Deployments[skillID]) > 0 {
			names = append(names, project.Name)
		}
	}
	return names
}

func (a *App) syncProjectSkillCopiesLocked(skill *storedSkill) error {
	for i := range a.state.Projects {
		for hostID, dep := range a.state.Projects[i].Deployments[skill.ID] {
			if dep.Mode != "copy" || (fileExists(dep.Target) && hashPath(dep.Target) != dep.LastHash) {
				continue
			}
			if err := replaceDir(skill.SourceDir, dep.Target); err != nil {
				return fmt.Errorf("同步项目 %q 的 skill 失败: %w", a.state.Projects[i].Name, err)
			}
			dep.LastHash = hashPath(dep.Target)
			a.state.Projects[i].Deployments[skill.ID][hostID] = dep
		}
	}
	return nil
}
