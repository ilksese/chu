//! Compatible, non-mutating parser for existing credential records. No process-environment expansion while reading.
use crate::models::Map;
use anyhow::{Result, bail};
pub fn parse(data: &[u8]) -> Result<Map<String>> {
    let text = std::str::from_utf8(data).map_err(|_| anyhow::anyhow!("凭证文件格式无效"))?;
    let text = text.trim_start_matches('\u{feff}');
    let mut p = 0;
    let mut values = Map::new();
    while p < text.len() {
        skip_space(text, &mut p);
        if p == text.len() {
            break;
        }
        if text.as_bytes()[p] == b'#' {
            skip_comment(text, &mut p);
            continue;
        }
        if text[p..].starts_with("export ") || text[p..].starts_with("export\t") {
            p += 6;
            skip_horizontal(text, &mut p);
        }
        let start = p;
        while p < text.len()
            && (text.as_bytes()[p].is_ascii_alphanumeric() || b"_.".contains(&text.as_bytes()[p]))
        {
            p += 1;
        }
        if p == start {
            bail!("凭证文件格式无效");
        }
        let key = &text[start..p];
        skip_horizontal(text, &mut p);
        if p >= text.len() || !b"=:".contains(&text.as_bytes()[p]) {
            bail!("凭证文件格式无效");
        }
        p += 1;
        skip_horizontal(text, &mut p);
        let value = if p < text.len() && b"\"'".contains(&text.as_bytes()[p]) {
            let quote = text.as_bytes()[p];
            p += 1;
            let start = p;
            let mut closed = false;
            while p < text.len() {
                if text.as_bytes()[p] == b'\\' {
                    p += 1;
                    if p < text.len() {
                        p += 1;
                    }
                    continue;
                }
                if text.as_bytes()[p] == quote {
                    closed = true;
                    break;
                }
                p += 1;
            }
            if !closed {
                bail!("凭证文件格式无效");
            }
            let raw = &text[start..p];
            p += 1;
            if quote == b'\'' {
                raw.to_owned()
            } else {
                expand(raw, &values, true)
            }
        } else {
            let start = p;
            while p < text.len() && !b"\r\n".contains(&text.as_bytes()[p]) {
                p += 1;
            }
            let mut line = &text[start..p];
            if let Some((i, _)) = line.char_indices().find(|(i, c)| {
                *c == '#' && (*i == 0 || line.as_bytes()[i - 1].is_ascii_whitespace())
            }) {
                line = &line[..i];
            }
            expand(line.trim_end(), &values, false)
        };
        values.insert(key.to_owned(), value);
        skip_horizontal(text, &mut p);
        if p < text.len() && text.as_bytes()[p] == b'#' {
            skip_comment(text, &mut p);
        }
    }
    Ok(values)
}
fn skip_space(text: &str, p: &mut usize) {
    while *p < text.len() && text.as_bytes()[*p].is_ascii_whitespace() {
        *p += 1;
    }
}
fn skip_horizontal(text: &str, p: &mut usize) {
    while *p < text.len() && b" \t".contains(&text.as_bytes()[*p]) {
        *p += 1;
    }
}
fn skip_comment(text: &str, p: &mut usize) {
    while *p < text.len() && text.as_bytes()[*p] != b'\n' {
        *p += 1;
    }
}
fn expand(raw: &str, values: &Map<String>, quoted: bool) -> String {
    let mut out = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                match next {
                    'n' if quoted => out.push('\n'),
                    'r' if quoted => out.push('\r'),
                    '$' => out.push('$'),
                    _ if quoted => out.push(next),
                    _ => {
                        out.push('\\');
                        out.push(next);
                    }
                }
            } else {
                out.push(c);
            }
            continue;
        }
        if c == '$' {
            let braces = chars.peek() == Some(&'{');
            if braces {
                chars.next();
            }
            let mut name = String::new();
            while chars
                .peek()
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
            {
                name.push(chars.next().unwrap());
            }
            if braces && chars.peek() == Some(&'}') {
                chars.next();
            }
            if name.is_empty() {
                out.push('$');
                if braces {
                    out.push('{');
                }
            } else if let Some(value) = values.get(&name) {
                out.push_str(value);
            }
            continue;
        }
        out.push(c);
    }
    out
}
