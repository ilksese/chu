#![allow(dead_code)]
use chu::{App, fs::*, models::*};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    sync::{Arc, Mutex},
    thread,
};
pub struct Fixture {
    pub temp: tempfile::TempDir,
    pub app: App,
}
impl Fixture {
    pub fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let root = temp.path().join("chu");
        mkdir(&home).unwrap();
        let app = App::open(&root, &home).unwrap();
        for host in app.hosts() {
            atomic_write(
                Path::new(&host.config_path),
                if host.format == "toml" {
                    b"model = \"keep\"\n"
                } else {
                    b"{\"preserve\":{\"nested\":\"keep\"}}"
                },
            )
            .unwrap();
        }
        Self { temp, app }
    }
    pub fn skill(&mut self, name: &str) -> String {
        let id = identifier(name, "skill");
        let source = self.app.root.join("skills").join(&id);
        write_skill(&source, name, "Fixture skill");
        self.app.state.skills.push(StoredSkill {
            id: id.clone(),
            name: name.into(),
            description: "Fixture skill".into(),
            source_dir: source.to_string_lossy().into(),
            deployments: Map::new(),
        });
        self.app.save().unwrap();
        id
    }
    pub fn project(&mut self) -> String {
        let path = self.temp.path().join("project");
        mkdir(&path).unwrap();
        self.app.add_project(path.to_str().unwrap()).unwrap();
        self.app.state.projects.last().unwrap().id.clone()
    }
}
pub fn write_skill(path: &Path, name: &str, description: &str) {
    atomic_write(
        &path.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n\n# Fixture\n").as_bytes(),
    )
    .unwrap();
}
pub fn stdio(name: &str) -> McpInput {
    McpInput {
        name: name.into(),
        r#type: "stdio".into(),
        command: "echo --fixture".into(),
        args: vec!["extra".into()],
        ..McpInput::default()
    }
}
pub fn run_git(root: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "git fixture failed");
}
pub fn repo() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    run_git(temp.path(), &["init", "-b", "main"]);
    run_git(
        temp.path(),
        &["config", "user.email", "fixture@example.invalid"],
    );
    run_git(temp.path(), &["config", "user.name", "Fixture"]);
    write_skill(&temp.path().join("skills/tdd"), "tdd", "Version one");
    run_git(temp.path(), &["add", "."]);
    run_git(temp.path(), &["commit", "-m", "fixture"]);
    temp
}
type Response = (u16, String, Vec<(String, String)>);
pub struct Server {
    pub url: String,
    pub requests: Arc<Mutex<Vec<String>>>,
    pub handle: Option<thread::JoinHandle<()>>,
}
impl Server {
    pub fn new(responses: Vec<Response>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let out = requests.clone();
        let handle = thread::spawn(move || {
            for (status, body, headers) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                    .unwrap();
                let mut data = Vec::new();
                let mut buffer = [0; 1024];
                while let Ok(n) = stream.read(&mut buffer) {
                    if n == 0 {
                        break;
                    }
                    data.extend_from_slice(&buffer[..n]);
                    if data.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                out.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&data).into());
                let mut header = format!(
                    "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n",
                    body.len()
                );
                for (k, v) in headers {
                    header.push_str(&format!("{k}: {v}\r\n"));
                }
                header.push_str("\r\n");
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(body.as_bytes());
            }
        });
        Self {
            url,
            requests,
            handle: Some(handle),
        }
    }
    pub fn models() -> Self {
        Self::new(vec![(
            200,
            r#"{"data":[{"id":"model-a"},{"id":"model-b"}]}"#.into(),
            vec![],
        )])
    }
    pub fn finish(&mut self) {
        if let Some(h) = self.handle.take() {
            h.join().unwrap();
        }
    }
}
