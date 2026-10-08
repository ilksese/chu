//! Generate persistent, isolated QA data using the same native application commands as the UI.
use anyhow::{Context, Result, bail};
use chu::{App, StoredSkill, fs::*};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Serialize)]
struct FixturePaths {
    base: PathBuf,
    root: PathBuf,
    home: PathBuf,
}
fn skill(path: &Path, name: &str, description: &str) -> Result<()> {
    atomic_write(&path.join("SKILL.md"),format!("---\nname: {name}\ndescription: {description}\n---\n\n# {name}\n\n这是隔离验收目录内的真实 Skill 文件。\n").as_bytes())
}
fn run(app: &mut App, method: &str, args: Value) -> Result<Value> {
    app.execute(method, args)
        .with_context(|| format!("验收数据操作失败: {method}"))
}
fn create_fixture(base: &Path) -> Result<FixturePaths> {
    if exists(base) {
        bail!("验收目录已经存在，拒绝覆盖；请选择新目录。");
    }
    mkdir(base)?;
    let base = base.canonicalize()?;
    let paths = FixturePaths {
        root: base.join("chu"),
        home: base.join("home"),
        base,
    };
    mkdir(&paths.home)?;
    let mut app = App::open(&paths.root, &paths.home)?;
    for host in app.hosts() {
        let mut root = json!({"preserve":"fixture"});
        if host.id == "claude" {
            root["mcpServers"] = json!({
                "host-filesystem":{"command":"echo","args":["external fixture"],"env":{"FIXTURE":"yes"}},
                "host-knowledge":{"type":"http","url":"https://fixture.invalid/external"}
            });
        }
        let data = if host.format == "toml" {
            b"model = \"fixture-model\"\n".to_vec()
        } else {
            serde_json::to_vec_pretty(&root)?
        };
        atomic_write(Path::new(&host.config_path), &data)?;
    }
    for (host, name, description) in [
        (
            "claude",
            "host-design-review",
            "宿主已有的设计审查技能，可通过导入纳入 Chu。",
        ),
        (
            "opencode",
            "host-git-workflow",
            "宿主已有的 Git 工作流技能，默认不出现在 Chu 列表中。",
        ),
    ] {
        skill(
            &Path::new(&app.host(host)?.skill_path).join(name),
            name,
            description,
        )?;
    }
    for (name, description) in [
        ("code-review", "检查代码质量、识别风险并给出具体改进建议。"),
        (
            "test-driven-development",
            "以测试驱动开发，保持项目行为可验证。",
        ),
    ] {
        let source = paths.root.join("skills").join(name);
        skill(&source, name, description)?;
        app.state.skills.push(StoredSkill {
            id: name.into(),
            name: name.into(),
            description: description.into(),
            source_dir: source.to_string_lossy().into(),
            ..Default::default()
        });
    }
    app.save()?;
    for host in chu::hosts::HOST_IDS {
        run(&mut app, "ToggleSkill", json!(["code-review", host, true]))?;
    }
    run(
        &mut app,
        "AddMCP",
        json!([{"name":"filesystem","description":"本地文件系统工具服务","type":"stdio","command":"echo","args":["fixture"]}]),
    )?;
    run(
        &mut app,
        "AddMCP",
        json!([{"name":"knowledge-base","description":"团队知识库检索（验收配置）","type":"http","endpoint":"https://fixture.invalid/mcp"}]),
    )?;
    run(&mut app, "ToggleMCP", json!(["filesystem", "claude", true]))?;
    run(
        &mut app,
        "AddAgent",
        json!([{"name":"reviewer","description":"专注于安全与可维护性的代码审查助手。","prompt":"检查潜在缺陷，遵守项目规范，保护用户改动。","model":"fixture-model"}]),
    )?;
    run(&mut app, "ToggleAgent", json!(["reviewer", "codex", true]))?;
    run(
        &mut app,
        "CreatePrompt",
        json!([
            "工程规范",
            "# 工程规范\n\n- 先验证需求与现有行为。\n- 修改必须有回归测试。\n- 不覆盖用户在宿主中的本地改动。"
        ]),
    )?;
    let prompt = app.snapshot()?.prompts[0].id.clone();
    run(&mut app, "TogglePrompt", json!([prompt, "claude", true]))?;
    run(
        &mut app,
        "CreateReference",
        json!([
            "API 约定",
            "# API 约定\n\n所有写入必须使用原子替换。配置变更之前创建备份。\n\n## 验收\n每个宿主独立验证部署与撤销。"
        ]),
    )?;
    let project = paths.base.join("sample-project");
    mkdir(&project)?;
    run(&mut app, "AddProject", json!([project]))?;
    let project = app.state.projects[0].id.clone();
    run(
        &mut app,
        "ToggleProjectSkill",
        json!([project, "code-review", "codex", true]),
    )?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let url = format!("http://{}", listener.local_addr()?);
    let server = thread::spawn(move || -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(10))
                }
                Err(e) => return Err(e.into()),
            }
        };
        // macOS accepted sockets inherit the nonblocking listener mode. Read the request normally.
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;
        let mut request = Vec::new();
        let mut buffer = [0; 1024];
        while !request.windows(4).any(|w| w == [13, 10, 13, 10]) {
            let n = stream.read(&mut buffer).context("读取本地模型请求失败")?;
            if n == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..n]);
        }
        let body =
            br#"{"data":[{"id":"fixture-model"},{"id":"fixture-review"},{"id":"fixture-fast"}]}"#;
        let crlf = std::str::from_utf8(&[13, 10])?;
        write!(
            stream,
            "HTTP/1.1 200 OK{crlf}Content-Type: application/json{crlf}Content-Length: {}{crlf}Connection: close{crlf}{crlf}",
            body.len()
        )?;
        stream.write_all(body)?;
        Ok(())
    });
    let provider = run(
        &mut app,
        "CreateProvider",
        json!([{"name":"本地验收供应商","baseUrl":url}]),
    );
    server
        .join()
        .map_err(|_| anyhow::anyhow!("本地模型服务异常退出"))??;
    provider?;
    Ok(paths)
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "cargo run --example create_fixture -- [新的输出目录]\n不传路径时生成可保留的临时验收目录；不会覆盖已有目录。"
        );
        return Ok(());
    }
    if args.len() > 1 {
        bail!("只接受一个可选的输出目录；使用 --help 查看用法。");
    }
    let base = if let Some(path) = args.first() {
        std::path::absolute(Path::new(path))?
    } else {
        tempfile::Builder::new()
            .prefix("chu-native-qa-")
            .tempdir()?
            .keep()
            .join("data")
    };
    let paths = create_fixture(&base)?;
    println!("{}", serde_json::to_string(&paths)?);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_exercises_native_actions_and_stays_inside_its_isolated_home() {
        let temp = tempfile::tempdir().unwrap();
        let paths = create_fixture(&temp.path().join("qa")).unwrap();
        let mut app = App::open(&paths.root, &paths.home).unwrap();
        let snapshot = app.snapshot().unwrap();
        assert_eq!(snapshot.skills.iter().filter(|s| s.managed).count(), 2);
        assert_eq!(snapshot.mcps.iter().filter(|m| m.managed).count(), 2);
        assert_eq!(snapshot.providers[0].models.len(), 3);
        assert_eq!(snapshot.projects.len(), 1);
        assert_eq!(snapshot.prompts.len(), 1);
        assert!(
            snapshot
                .hosts
                .iter()
                .all(|h| Path::new(&h.config_path).starts_with(&paths.home)
                    && Path::new(&h.skill_path).starts_with(&paths.home))
        );
        assert!(
            app.list_import_candidates("skills")
                .unwrap()
                .candidates
                .iter()
                .any(|c| c.name == "host-design-review" && c.available)
        );
        assert!(
            app.list_import_candidates("mcps")
                .unwrap()
                .candidates
                .iter()
                .any(|c| c.name == "host-filesystem" && c.available)
        );
    }
    #[test]
    fn fixture_refuses_to_overwrite_existing_data() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().join("qa");
        mkdir(&base).unwrap();
        atomic_write(&base.join("keep"), b"user data").unwrap();
        assert!(create_fixture(&base).is_err());
        assert_eq!(std::fs::read(base.join("keep")).unwrap(), b"user data");
        assert!(!base.join("chu").exists());
    }
}
