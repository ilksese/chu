//! Small filesystem journal for the import/apply unit of work. Only mutated paths are watched.
use crate::fs::{atomic_write, exists, remove, symlink};
use anyhow::{Result, bail};
use std::{
    fs,
    path::{Path, PathBuf},
};
enum Original {
    Missing,
    File(Vec<u8>, fs::Permissions),
    Link(PathBuf),
}
pub(crate) struct Journal {
    entries: Vec<(PathBuf, Original)>,
    done: bool,
}
impl Journal {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            done: false,
        }
    }
    pub fn watch(&mut self, path: &Path) -> Result<()> {
        if self.entries.iter().any(|(p, _)| p == path) {
            return Ok(());
        }
        let original = match fs::symlink_metadata(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Original::Missing,
            Err(e) => return Err(e.into()),
            Ok(m) if m.file_type().is_symlink() => Original::Link(fs::read_link(path)?),
            Ok(m) if m.is_file() => Original::File(fs::read(path)?, m.permissions()),
            Ok(_) => bail!("目标路径不是可替换的普通文件或符号链接: {}", path.display()),
        };
        self.entries.push((path.into(), original));
        Ok(())
    }
    pub fn commit(&mut self) {
        self.done = true;
    }
    pub fn rollback(&mut self) -> Result<()> {
        let mut failed = false;
        for (path, original) in self.entries.iter().rev() {
            let result = (|| -> Result<()> {
                if exists(path) {
                    remove(path)?;
                }
                match original {
                    Original::Missing => {}
                    Original::File(data, permissions) => {
                        atomic_write(path, data)?;
                        fs::set_permissions(path, permissions.clone())?;
                    }
                    Original::Link(target) => symlink(target, path)?,
                }
                Ok(())
            })();
            failed |= result.is_err();
        }
        self.done = true;
        if failed {
            bail!("导入失败，部分路径无法回滚，请检查文件权限");
        }
        Ok(())
    }
}
impl Drop for Journal {
    fn drop(&mut self) {
        if !self.done {
            let _ = self.rollback();
        }
    }
}
