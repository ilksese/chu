use chu::markdown_view::{self, Block};
use pulldown_cmark::{Event, Parser};
use slint::StyledText;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const MAX_DEPTH: usize = 48;
const MIN_DEEP_BYTES: usize = 64 * 1024;
const CHILD_MARKER: &str = "CHU_MARKDOWN_DEPTH_CHILD";
const BODY: &str = "正文";

fn quote_source(depth: usize) -> String {
    format!("{}{BODY}\n\n\n", "> ".repeat(depth - 1))
}

fn inline_source(depth: usize) -> String {
    let markers = "**".repeat(depth - 1);
    format!("{markers}{BODY}{markers}\n\n\n")
}

fn image_alt_source(depth: usize) -> String {
    let markers = "**".repeat(depth - 2);
    format!("![{markers}{BODY}{markers}](https://invalid.test/image.png)\n\n\n")
}

fn event_depth(source: &str) -> usize {
    let mut depth = 0;
    let mut maximum = 0;
    for event in Parser::new(source) {
        match event {
            Event::Start(_) => {
                depth += 1;
                maximum = maximum.max(depth);
            }
            Event::End(_) => depth -= 1,
            _ => {}
        }
    }
    assert_eq!(depth, 0);
    maximum
}

fn assert_plain_fallback(source: &str) {
    let blocks = markdown_view::parse(source);
    assert_eq!(blocks.len(), 1);
    let block = &blocks[0];
    assert_eq!(block.kind, "fallback");
    assert!(block.text == source, "回退必须保留整篇原文和尾随换行");
    assert!(block.inline_markdown.is_empty());
    assert!(block.rows.is_empty());
    assert!(block.links.is_empty());

    let projected = markdown_view::project(&blocks);
    assert_eq!(projected.len(), 1);
    assert!(projected[0].text.as_str() == source);
    assert!(projected[0].styled == StyledText::from_plain_text(source));
    // 显式完成投影与领域块析构；parse 返回前也已析构所有中间树节点。
    drop(projected);
    drop(blocks);
}

fn assert_deep_fallback(source: &str) {
    assert!(source.len() >= MIN_DEEP_BYTES);
    assert!(source.ends_with("\n\n\n"));
    assert_plain_fallback(source);
}

fn run_in_child(test_name: &str, test: impl FnOnce()) {
    if let Some(marker) = std::env::var_os(CHILD_MARKER) {
        // 标记必须匹配当前测试，子进程直接执行断言，避免无限派生。
        assert_eq!(marker.to_str(), Some(test_name));
        test();
        return;
    }

    // 子进程只解析、投影并析构内存数据，不读取或写入任何宿主配置。
    let mut child = Command::new(std::env::current_exe().expect("定位当前测试程序"))
        .args(["--exact", test_name, "--nocapture", "--test-threads=1"])
        .env(CHILD_MARKER, test_name)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("启动 Markdown 深度测试子进程");
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if child.try_wait().expect("检查测试子进程状态").is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("Markdown 深度测试子进程超时：{test_name}");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().expect("回收测试子进程");
    assert!(
        output.status.success(),
        "Markdown 深度测试必须正常退出，不能 abort：{test_name}，状态 {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn max_depth_boundary_preserves_grammar_and_plain_fallback() {
    run_in_child(
        "max_depth_boundary_preserves_grammar_and_plain_fallback",
        || {
            for depth in [MAX_DEPTH - 1, MAX_DEPTH, MAX_DEPTH + 1] {
                for (kind, source, text) in [
                    ("quote", quote_source(depth), BODY),
                    ("paragraph", inline_source(depth), BODY),
                    (
                        "paragraph",
                        image_alt_source(depth),
                        "[图片：正文 · https://invalid.test/image.png；未加载]",
                    ),
                ] {
                    // 段落、图片与行内样式都计入事件深度，而非仅计算引用层数。
                    assert_eq!(event_depth(&source), depth);
                    if depth > MAX_DEPTH {
                        assert_plain_fallback(&source);
                        continue;
                    }
                    let blocks = markdown_view::parse(&source);
                    assert_eq!(blocks.len(), 1);
                    assert_eq!(blocks[0].kind, kind);
                    assert_eq!(blocks[0].text, text);
                    assert!(!blocks[0].inline_markdown.is_empty());
                    let projected = markdown_view::project(&blocks);
                    assert_eq!(projected.len(), 1);
                    assert_eq!(projected[0].text.as_str(), text);
                    drop(projected);
                    drop(blocks);
                }
            }
        },
    );
}

#[test]
fn deep_blockquotes_parse_project_and_drop_without_abort() {
    run_in_child(
        "deep_blockquotes_parse_project_and_drop_without_abort",
        || assert_deep_fallback(&quote_source(40_000)),
    );
}

#[test]
fn deep_inline_parse_project_and_drop_without_abort() {
    run_in_child("deep_inline_parse_project_and_drop_without_abort", || {
        assert_deep_fallback(&inline_source(20_000))
    });
}

#[test]
fn deep_image_alt_parse_project_and_drop_without_abort() {
    run_in_child(
        "deep_image_alt_parse_project_and_drop_without_abort",
        || assert_deep_fallback(&image_alt_source(20_000)),
    );
}

#[test]
fn closed_subtrees_before_deep_input_keep_the_entire_source() {
    run_in_child(
        "closed_subtrees_before_deep_input_keep_the_entire_source",
        || {
            let prefix = format!(
                "{}\n# 已闭合标题 **粗体**\n\n{}",
                quote_source(MAX_DEPTH),
                inline_source(MAX_DEPTH)
            );
            assert_eq!(event_depth(&prefix), MAX_DEPTH);
            let source = format!(
                "{prefix}\n{}\n尾部 [链接](https://example.test) 与 <原始标签>\n\n\n",
                quote_source(40_000)
            );
            assert_deep_fallback(&source);
        },
    );
}

#[test]
fn empty_inline_fallback_projects_as_visible_plain_text() {
    let source = "# 原文 **不应渲染**\n\n![原始图片](asset.png) <原始标签>\n\n\n";
    let blocks = [Block {
        kind: "fallback".into(),
        text: source.into(),
        ..Block::default()
    }];
    let projected = markdown_view::project(&blocks);
    assert_eq!(projected.len(), 1);
    assert_eq!(projected[0].text.as_str(), source);
    assert!(projected[0].styled == StyledText::from_plain_text(source));
}
