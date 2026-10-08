use crate::{App, fs::*, models::ReferenceView};
use anyhow::{Result, bail};
use std::{fs, path::PathBuf};
impl App {
    fn reference_path(&self, id: &str) -> Result<PathBuf> {
        if !id.ends_with(".md") {
            bail!("未知的 Reference");
        }
        safe_name(id)?;
        let path = self.root.join("references").join(id);
        if !fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file()) {
            bail!("未知的 Reference");
        }
        Ok(path)
    }
    pub fn create_reference(&mut self, name: &str, content: &str) -> Result<()> {
        let name = markdown_name(name)?;
        let path = self.root.join("references").join(format!("{name}.md"));
        if exists(&path) {
            bail!("Reference 已存在");
        }
        atomic_write(&path, content.as_bytes())
    }
    pub fn read_reference(&self, id: &str) -> Result<String> {
        Ok(fs::read_to_string(self.reference_path(id)?)?)
    }
    pub fn update_reference(&mut self, id: &str, name: &str, content: &str) -> Result<()> {
        let old = self.reference_path(id)?;
        let name = markdown_name(name)?;
        let next = self.root.join("references").join(format!("{name}.md"));
        if next != old && exists(&next) && !same_file::is_same_file(&old, &next).unwrap_or(false) {
            bail!("Reference 已存在");
        }
        atomic_write(&old, content.as_bytes())?;
        if next != old {
            fs::rename(old, next)?;
        }
        Ok(())
    }
    pub fn delete_reference(&mut self, id: &str) -> Result<()> {
        fs::remove_file(self.reference_path(id)?)?;
        Ok(())
    }
    pub(crate) fn reference_views(&self) -> Vec<ReferenceView> {
        let mut views = Vec::new();
        if let Ok(entries) = fs::read_dir(self.root.join("references")) {
            for e in entries.flatten() {
                let file = e.file_name().to_string_lossy().into_owned();
                if !file.ends_with(".md") || !e.file_type().is_ok_and(|t| t.is_file()) {
                    continue;
                }
                views.push(ReferenceView {
                    id: file.clone(),
                    name: file.strip_suffix(".md").unwrap_or(&file).into(),
                    source: e.path().to_string_lossy().into(),
                    preview: preview(&e.path()),
                });
            }
        }
        views.sort_by(|a, b| a.name.cmp(&b.name));
        views
    }
}
