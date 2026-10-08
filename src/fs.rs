//! Private, atomic writes and compatible deployment hashes. Never follow directory links on removal.
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};

pub fn exists(p: impl AsRef<Path>) -> bool {
    fs::symlink_metadata(p).is_ok()
}
pub fn mkdir(p: &Path) -> Result<()> {
    #[cfg(unix)]
    let missing: Vec<_> = p
        .ancestors()
        .take_while(|p| !p.as_os_str().is_empty() && !p.exists())
        .map(Path::to_path_buf)
        .collect();
    fs::create_dir_all(p)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for directory in missing {
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
        }
    }
    Ok(())
}
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path.parent().context("文件缺少父目录")?;
    mkdir(parent)?;
    let mut temp = tempfile::Builder::new()
        .prefix(".chu-write-")
        .tempfile_in(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temp.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    temp.write_all(data)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}
pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut data = serde_json::to_vec_pretty(value)?;
    data.push(b'\n');
    atomic_write(path, &data)
}
pub fn copy_file(source: &Path, target: &Path) -> Result<()> {
    atomic_write(target, &fs::read(source)?)
}
pub fn symlink(source: &Path, target: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, target)?;
    }
    #[cfg(windows)]
    {
        if source.is_dir() {
            std::os::windows::fs::symlink_dir(source, target)?;
        } else {
            std::os::windows::fs::symlink_file(source, target)?;
        }
    }
    Ok(())
}
pub fn copy_dir(source: &Path, target: &Path) -> Result<()> {
    copy_tree(source, target, false)
}
pub fn copy_skill_dir(source: &Path, target: &Path) -> Result<()> {
    copy_tree(source, target, true)
}
fn copy_tree(source: &Path, target: &Path, skip_git: bool) -> Result<()> {
    mkdir(target)?;
    for entry in walkdir::WalkDir::new(source)
        .min_depth(1)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| !skip_git || e.depth() != 1 || e.file_name() != ".git")
    {
        let entry = entry?;
        let to = target.join(entry.path().strip_prefix(source)?);
        if entry.file_type().is_symlink() {
            if let Some(p) = to.parent() {
                mkdir(p)?;
            }
            symlink(&fs::read_link(entry.path())?, &to)?;
        } else if entry.file_type().is_dir() {
            mkdir(&to)?;
        } else {
            copy_file(entry.path(), &to)?;
        }
    }
    Ok(())
}
pub fn remove(path: &Path) -> Result<()> {
    let Ok(info) = fs::symlink_metadata(path) else {
        return Ok(());
    };
    if info.is_dir() && !info.file_type().is_symlink() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}
pub fn replace_dir(source: &Path, target: &Path) -> Result<()> {
    // Stage before removing so copy failures cannot destroy the last good version.
    let parent = target.parent().context("目标缺少父目录")?;
    mkdir(parent)?;
    let stage = tempfile::Builder::new()
        .prefix(".chu-stage-")
        .tempdir_in(parent)?;
    let staged = stage.path().join("next");
    copy_dir(source, &staged)?;
    let old = stage.path().join("previous");
    if exists(target) {
        fs::rename(target, &old)?;
    }
    if let Err(err) = fs::rename(&staged, target) {
        if exists(&old) {
            fs::rename(&old, target)?;
        }
        return Err(err.into());
    }
    Ok(())
}
pub fn hash_path(path: &Path) -> String {
    hash_tree(path, false)
}
pub fn hash_skill_path(path: &Path) -> String {
    hash_tree(path, true)
}
fn hash_tree(path: &Path, skip_git: bool) -> String {
    let Ok(info) = fs::symlink_metadata(path) else {
        return String::new();
    };
    let mut hash = Sha256::new();
    if info.is_dir() {
        for entry in walkdir::WalkDir::new(path)
            .min_depth(1)
            .follow_links(false)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|e| !skip_git || e.depth() != 1 || e.file_name() != ".git")
            .flatten()
        {
            if let Ok(relative) = entry.path().strip_prefix(path) {
                hash.update(relative.to_string_lossy().as_bytes());
            }
            if entry.file_type().is_symlink() {
                if let Ok(link) = fs::read_link(entry.path()) {
                    hash.update(link.to_string_lossy().as_bytes());
                }
            } else if entry.file_type().is_file()
                && let Ok(data) = fs::read(entry.path())
            {
                hash.update(data);
            }
        }
    } else if let Ok(data) = fs::read(path) {
        hash.update(data);
    }
    format!("{:x}", hash.finalize())
}
pub fn md5_file(path: &Path) -> String {
    fs::read(path)
        .map(|d| format!("{:x}", md5::compute(d)))
        .unwrap_or_default()
}
pub fn slug(value: &str) -> String {
    let mut out = String::new();
    for c in value.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_owned()
}
pub fn identifier(name: &str, prefix: &str) -> String {
    let s = slug(name);
    if s.is_empty() {
        format!(
            "{prefix}-{}",
            &format!("{:x}", Sha256::digest(name.as_bytes()))[..16]
        )
    } else {
        s
    }
}
pub fn safe_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\', '\0']) {
        bail!("名称无效");
    }
    Ok(name.into())
}
pub fn markdown_name(name: &str) -> Result<String> {
    let name = name.trim();
    safe_name(name.strip_suffix(".md").unwrap_or(name))
}
pub fn display_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.contains('\0') {
        bail!("名称不能为空");
    }
    Ok(name.into())
}
pub fn safe_relative(root: &Path, relative: &str) -> Result<PathBuf> {
    let p = Path::new(relative);
    if p.is_absolute()
        || p.components()
            .any(|c| matches!(c, Component::ParentDir | Component::Prefix(_)))
    {
        bail!("非法路径");
    }
    let target = root.join(p);
    let canonical = target.canonicalize()?;
    if !canonical.starts_with(root.canonicalize()?) {
        bail!("路径越出来源仓库");
    }
    Ok(target)
}
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
pub fn preview(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_default()
        .replace("\r\n", "\n")
        .trim()
        .chars()
        .take(80)
        .collect()
}
