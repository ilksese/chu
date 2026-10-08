//! Credential handling is app runtime only; secrets never enter snapshots or error messages.
use crate::dotenv::parse;
use crate::fs::*;
use anyhow::{Context, Result, bail};
use std::{fs, io::Write, path::Path};
pub fn same_env(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}
fn read(root: &Path) -> Result<Option<Vec<u8>>> {
    let path = root.join(".env");
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Ok(m) if m.file_type().is_file() => Ok(Some(
            fs::read(path).map_err(|_| anyhow::anyhow!("无法读取凭证文件"))?,
        )),
        _ => bail!("无法读取凭证文件：必须是普通文件"),
    }
}
pub fn key(root: &Path, name: &str) -> Result<String> {
    if name.is_empty() {
        return Ok(String::new());
    }
    let values = parse(&read(root)?.unwrap_or_default())?;
    let matches: Vec<_> = values.iter().filter(|(k, _)| same_env(k, name)).collect();
    if matches.len() > 1 && matches.windows(2).any(|w| w[0].1 != w[1].1) {
        bail!("凭证文件包含大小写冲突的环境变量");
    }
    let value = values
        .get(name)
        .cloned()
        .or_else(|| matches.first().map(|(_, v)| (*v).clone()))
        .or_else(|| std::env::var(name).ok())
        .filter(|v| !v.is_empty())
        .context("API Key 环境变量未定义或为空")?;
    Ok(value)
}
fn assignment(name: &str, key: &str) -> String {
    let escaped = key
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('\n', "\\n")
        .replace('\r', "\\r");
    format!("{name}=\"{escaped}\"")
}
/// Replace complete logical records (including multiline quotes), preserving unrelated bytes/comments.
pub fn update(original: &[u8], name: &str, key: &str) -> Result<Vec<u8>> {
    parse(original)?;
    let original = std::str::from_utf8(original).context("凭证文件格式无效")?;
    let newline = if original.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let assignment = assignment(name, key);
    let mut record = String::new();
    let mut output = String::new();
    let mut replaced = false;
    for line in original.split_inclusive('\n') {
        record.push_str(line);
        let Ok(values) = parse(record.as_bytes()) else {
            continue;
        };
        let target = values.keys().find(|k| same_env(k, name));
        if let Some(target) = target {
            if values.len() != 1 {
                bail!("目标凭证变量必须独占一个逻辑记录，未写入");
            }
            let ending = if record.ends_with("\r\n") {
                "\r\n"
            } else if record.ends_with('\n') {
                "\n"
            } else {
                ""
            };
            let body = record.trim_end_matches(['\r', '\n']);
            let mut comment = "";
            for (i, c) in body.char_indices() {
                if c == '#'
                    && let Ok(prefix) = parse(&body.as_bytes()[..i])
                    && prefix.len() == 1
                    && prefix.get(target) == values.get(target)
                {
                    comment = &body[i..];
                    break;
                }
            }
            if !replaced {
                output.push_str(&assignment);
                if !comment.is_empty() {
                    output.push(' ');
                    output.push_str(comment);
                }
                output.push_str(ending);
                replaced = true;
            } else if !comment.is_empty() {
                output.push_str(comment);
                output.push_str(ending);
            }
        } else {
            output.push_str(&record);
        }
        record.clear();
    }
    if !record.is_empty() {
        bail!("凭证文件逻辑记录不完整，未写入");
    }
    if !replaced {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push_str(newline);
        }
        output.push_str(&assignment);
        output.push_str(newline);
    }
    if parse(output.as_bytes())?.get(name).map(String::as_str) != Some(key) {
        bail!("API Key 无法无损编码，未写入");
    }
    Ok(output.into_bytes())
}
fn stage(root: &Path, data: &[u8]) -> Result<tempfile::NamedTempFile> {
    mkdir(root)?;
    let mut file = tempfile::Builder::new()
        .prefix(".chu-provider-")
        .tempfile_in(root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(data)?;
    file.as_file().sync_all()?;
    Ok(file)
}
pub fn save(root: &Path, name: &str, key: &str, state: &[u8]) -> Result<()> {
    let original = read(root)?;
    let updated = update(original.as_deref().unwrap_or_default(), name, key)?;
    let env = stage(root, &updated)?;
    let state_file = stage(root, state)?;
    let backup = original
        .as_ref()
        .map(|data| stage(root, data))
        .transpose()?;
    let env_path = root.join(".env");
    env.persist(&env_path)
        .map_err(|e| e.error)
        .context("保存凭证失败")?;
    if let Err(err) = state_file.persist(root.join("state.json")) {
        let rollback = if let Some(backup) = backup {
            backup.persist(&env_path).map(|_| ()).map_err(|e| e.error)
        } else {
            fs::remove_file(&env_path)
        };
        if rollback.is_err() {
            bail!("保存状态失败且凭证回滚失败");
        }
        return Err(err.error.into());
    }
    Ok(())
}
