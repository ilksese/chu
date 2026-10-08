use crate::{App, fs::*, hosts::project_dir, models::*};
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
fn canonical(path: &str) -> Result<PathBuf> {
    let path = Path::new(path).canonicalize().context("项目目录不可用")?;
    if !path.is_dir() {
        bail!("请选择有效的项目目录");
    }
    Ok(path)
}
fn target(root: &str, host: &str, name: &str) -> Result<PathBuf> {
    Ok(Path::new(root)
        .join(project_dir(host).context("未知的 Agent 宿主")?)
        .join(name))
}
impl App {
    fn project_index(&self, id: &str) -> Result<usize> {
        self.state
            .projects
            .iter()
            .position(|p| p.id == id)
            .context("未知的项目")
    }
    pub fn add_project(&mut self, path: &str) -> Result<()> {
        let path = canonical(path)?;
        let path = path.to_string_lossy().into_owned();
        if self.state.projects.iter().any(|p| p.path == path) {
            bail!("该目录已关联为项目");
        }
        let id = format!("{:x}", Sha256::digest(path.as_bytes()))[..16].to_string();
        let name = Path::new(&path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into();
        self.state.projects.push(StoredProject {
            id,
            name,
            path,
            created_at: now(),
            deployments: Map::new(),
        });
        self.save()
    }
    pub fn rename_project(&mut self, id: &str, name: &str) -> Result<()> {
        let i = self.project_index(id)?;
        let name = name.trim();
        if name.is_empty() {
            bail!("项目名称不能为空");
        }
        self.state.projects[i].name = name.into();
        self.save()
    }
    pub fn relocate_project(&mut self, id: &str, path: &str) -> Result<()> {
        let i = self.project_index(id)?;
        let path = canonical(path)?.to_string_lossy().into_owned();
        if self
            .state
            .projects
            .iter()
            .any(|p| p.id != id && p.path == path)
        {
            bail!("该目录已关联为项目");
        }
        let mut project = self.state.projects[i].clone();
        project.path = path;
        for (skill_id, hosts) in &mut project.deployments {
            let skill = self.skill(skill_id).ok();
            for (host, dep) in hosts {
                let path = target(
                    &project.path,
                    host,
                    skill.as_ref().map(|s| s.name.as_str()).unwrap_or(skill_id),
                )?;
                *dep = Deployment {
                    enabled: true,
                    mode: "copy".into(),
                    target: path.to_string_lossy().into(),
                    last_hash: String::new(),
                };
                if exists(&path) {
                    dep.mode = "conflict".into();
                } else if let Some(skill) = &skill {
                    copy_dir(Path::new(&skill.source_dir), &path)?;
                    dep.last_hash = hash_path(&path);
                }
            }
        }
        self.state.projects[i] = project;
        self.save()
    }
    pub fn toggle_project_skill(
        &mut self,
        project: &str,
        skill: &str,
        host: &str,
        enabled: bool,
    ) -> Result<()> {
        let i = self.project_index(project)?;
        let p = &self.state.projects[i];
        if !Path::new(&p.path).is_dir() {
            bail!("项目根目录不可用");
        }
        let skill = self.skill(skill)?;
        let path = target(&p.path, host, &skill.name)?;
        let dep = p
            .deployments
            .get(&skill.id)
            .and_then(|d| d.get(host))
            .cloned();
        if enabled {
            if dep.is_some() {
                return Ok(());
            }
            if !Path::new(&skill.source_dir).join("SKILL.md").is_file() {
                bail!("中央 skill 源目录不可用");
            }
            if exists(&path) {
                bail!("目标目录已存在，请先处理冲突");
            }
            copy_dir(Path::new(&skill.source_dir), &path)?;
            self.state.projects[i]
                .deployments
                .entry(skill.id)
                .or_default()
                .insert(
                    host.into(),
                    Deployment {
                        enabled: true,
                        mode: "copy".into(),
                        target: path.to_string_lossy().into(),
                        last_hash: hash_path(&path),
                    },
                );
        } else {
            let Some(dep) = dep else {
                return Ok(());
            };
            if dep.mode == "conflict" {
                bail!("目标目录存在冲突，已保护用户文件");
            }
            if exists(&dep.target) && hash_path(Path::new(&dep.target)) != dep.last_hash {
                bail!("项目 skill 已被修改，已保护用户改动");
            }
            remove(Path::new(&dep.target))?;
            let deps = self.state.projects[i]
                .deployments
                .get_mut(&skill.id)
                .unwrap();
            deps.remove(host);
            if deps.is_empty() {
                self.state.projects[i].deployments.remove(&skill.id);
            }
        }
        self.save()
    }
    pub fn reset_project_skill(&mut self, project: &str, skill: &str, host: &str) -> Result<()> {
        let i = self.project_index(project)?;
        let skill = self.skill(skill)?;
        let mut dep = self.state.projects[i]
            .deployments
            .get(&skill.id)
            .and_then(|d| d.get(host))
            .cloned()
            .context("该 skill 尚未部署")?;
        if dep.mode == "conflict" {
            bail!("冲突目录不能重置");
        }
        if !Path::new(&self.state.projects[i].path).is_dir() {
            bail!("项目根目录不可用");
        }
        replace_dir(Path::new(&skill.source_dir), Path::new(&dep.target))?;
        dep.last_hash = hash_path(Path::new(&dep.target));
        self.state.projects[i]
            .deployments
            .get_mut(&skill.id)
            .unwrap()
            .insert(host.into(), dep);
        self.save()
    }
    pub fn delete_project(&mut self, id: &str, cleanup: bool) -> Result<Vec<String>> {
        let i = self.project_index(id)?;
        let mut retained = Vec::new();
        if cleanup {
            for dep in self.state.projects[i]
                .deployments
                .values()
                .flat_map(|h| h.values())
            {
                if !exists(&dep.target) {
                    continue;
                }
                if dep.mode != "copy" || hash_path(Path::new(&dep.target)) != dep.last_hash {
                    retained.push(dep.target.clone());
                    continue;
                }
                remove(Path::new(&dep.target))?;
            }
        }
        self.state.projects.remove(i);
        self.save()?;
        Ok(retained)
    }
    pub(crate) fn project_views(&self, hosts: &[Host]) -> Vec<ProjectView> {
        let mut views = Vec::new();
        for project in &self.state.projects {
            let mut deployments: Map<Map<ProjectDeploymentView>> = Map::new();
            for skill in &self.state.skills {
                for host in hosts {
                    let dep = project
                        .deployments
                        .get(&skill.id)
                        .and_then(|h| h.get(&host.id));
                    let status = if let Some(dep) = dep {
                        if dep.mode == "conflict" {
                            "conflict"
                        } else if !Path::new(&dep.target).join("SKILL.md").is_file() {
                            "missing"
                        } else if hash_path(Path::new(&dep.target)) != dep.last_hash {
                            "modified"
                        } else {
                            "enabled"
                        }
                    } else if target(&project.path, &host.id, &skill.name).is_ok_and(exists) {
                        "conflict"
                    } else {
                        ""
                    };
                    if !status.is_empty() {
                        deployments.entry(skill.id.clone()).or_default().insert(
                            host.id.clone(),
                            ProjectDeploymentView {
                                enabled: dep.is_some(),
                                status: status.into(),
                            },
                        );
                    }
                }
            }
            views.push(ProjectView {
                id: project.id.clone(),
                name: project.name.clone(),
                path: project.path.clone(),
                created_at: project.created_at.clone(),
                available: Path::new(&project.path).is_dir(),
                deployments,
            });
        }
        views.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        views
    }
    pub(crate) fn sync_project_copies(&mut self, skill: &StoredSkill) -> Result<()> {
        for project in &mut self.state.projects {
            if let Some(hosts) = project.deployments.get_mut(&skill.id) {
                for dep in hosts.values_mut() {
                    if dep.mode != "copy"
                        || (exists(&dep.target)
                            && hash_path(Path::new(&dep.target)) != dep.last_hash)
                    {
                        continue;
                    }
                    if !Path::new(&project.path).is_dir() {
                        continue;
                    }
                    replace_dir(Path::new(&skill.source_dir), Path::new(&dep.target))?;
                    dep.last_hash = hash_path(Path::new(&dep.target));
                }
            }
        }
        Ok(())
    }
}
