//! 纯 Markdown 解析与原生 Slint 块投影，不执行 I/O 或激活 URI。
use crate::ui;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};
use slint::{ModelRc, StyledText, VecModel};
use std::{ops::Range, rc::Rc};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Link {
    pub label: String,
    pub uri: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    pub inline_markdown: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TableRow {
    pub cells: Vec<Cell>,
    pub header: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Block {
    pub kind: String,
    pub text: String,
    pub inline_markdown: String,
    pub level: i32,
    pub prefix: String,
    pub language: String,
    pub rows: Vec<TableRow>,
    pub links: Vec<Link>,
}
struct Node<'a> {
    event: Event<'a>,
    span: Range<usize>,
    children: Vec<Node<'a>>,
}
#[derive(Default)]
struct Inline {
    text: String,
    markdown: String,
    links: Vec<Link>,
}
#[derive(Clone, Default)]
struct Context {
    quote: usize,
    list_depth: usize,
    prefix: String,
}

const MAX_DEPTH: usize = 48;

/// 整篇文档拆分为布局块，只有安全的行内片段交给 StyledText。
pub fn parse(source: &str) -> Vec<Block> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS;
    let mut roots = Vec::new();
    let mut stack: Vec<Node<'_>> = Vec::new();
    for (event, span) in Parser::new_ext(source, options).into_offset_iter() {
        match event {
            Event::Start(_) => {
                if stack.len() >= MAX_DEPTH {
                    // 建树前限深，保证已闭合子树和未闭合栈的递归析构也安全。
                    return vec![Block {
                        kind: "fallback".into(),
                        text: source.into(),
                        ..Block::default()
                    }];
                }
                stack.push(Node {
                    event,
                    span,
                    children: Vec::new(),
                });
            }
            Event::End(_) => {
                if let Some(mut node) = stack.pop() {
                    node.span.end = node.span.end.max(span.end);
                    append_node(&mut roots, &mut stack, node);
                }
            }
            _ => append_node(
                &mut roots,
                &mut stack,
                Node {
                    event,
                    span,
                    children: Vec::new(),
                },
            ),
        }
    }
    let mut output = Vec::new();
    render(&roots, source, &mut Context::default(), &mut output, 0);
    output
}
fn append_node<'a>(roots: &mut Vec<Node<'a>>, stack: &mut [Node<'a>], node: Node<'a>) {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    } else {
        roots.push(node);
    }
}
fn literal(source: &str, node: &Node<'_>) -> String {
    source
        .get(node.span.clone())
        .unwrap_or_default()
        .trim_end()
        .into()
}
fn plain_block(kind: &str, text: String) -> Block {
    Block {
        kind: kind.into(),
        inline_markdown: escape(&text),
        text,
        ..Block::default()
    }
}
fn push_inline(
    kind: &str,
    nodes: &[Node<'_>],
    source: &str,
    ctx: &mut Context,
    output: &mut Vec<Block>,
    level: i32,
) {
    let inline = inline(nodes, source, 0);
    if inline.text.trim().is_empty() {
        return;
    }
    let kind = if kind == "heading" {
        kind
    } else if ctx.quote > 0 {
        "quote"
    } else if ctx.list_depth > 0 {
        "list"
    } else {
        kind
    };
    output.push(Block {
        kind: kind.into(),
        text: inline.text,
        inline_markdown: inline.markdown,
        prefix: std::mem::take(&mut ctx.prefix),
        level,
        links: inline.links,
        ..Block::default()
    });
}
fn render(nodes: &[Node<'_>], source: &str, ctx: &mut Context, out: &mut Vec<Block>, depth: usize) {
    for node in nodes {
        if depth > MAX_DEPTH {
            out.push(plain_block("fallback", literal(source, node)));
            continue;
        }
        match &node.event {
            Event::Start(Tag::Paragraph) => {
                push_inline("paragraph", &node.children, source, ctx, out, 0)
            }
            Event::Start(Tag::Heading { level, .. }) => {
                push_inline("heading", &node.children, source, ctx, out, *level as i32)
            }
            Event::Start(Tag::List(start)) => {
                let mut next = *start;
                for item in &node.children {
                    let mut nested = ctx.clone();
                    nested.list_depth += 1;
                    nested.prefix = format!(
                        "{}{}",
                        "  ".repeat(ctx.list_depth.min(12)),
                        next.map(|n| format!("{n}.")).unwrap_or_else(|| "•".into())
                    );
                    render(
                        std::slice::from_ref(item),
                        source,
                        &mut nested,
                        out,
                        depth + 1,
                    );
                    if let Some(n) = next.as_mut() {
                        *n = n.saturating_add(1);
                    }
                }
            }
            Event::Start(Tag::Item) => {
                let mut pending = Vec::new();
                for child in &node.children {
                    if matches!(
                        &child.event,
                        Event::Start(
                            Tag::Paragraph
                                | Tag::List(_)
                                | Tag::CodeBlock(_)
                                | Tag::BlockQuote(_)
                                | Tag::Table(_)
                        )
                    ) {
                        let value = inline_refs(&pending, source, 0);
                        if !value.text.trim().is_empty() {
                            out.push(Block {
                                kind: if ctx.quote > 0 { "quote" } else { "list" }.into(),
                                text: value.text,
                                inline_markdown: value.markdown,
                                prefix: std::mem::take(&mut ctx.prefix),
                                links: value.links,
                                ..Block::default()
                            });
                        }
                        pending.clear();
                        render(std::slice::from_ref(child), source, ctx, out, depth + 1);
                    } else {
                        pending.push(child);
                    }
                }
                let inline = inline_refs(&pending, source, 0);
                if !inline.text.trim().is_empty() {
                    out.push(Block {
                        kind: if ctx.quote > 0 { "quote" } else { "list" }.into(),
                        text: inline.text,
                        inline_markdown: inline.markdown,
                        prefix: std::mem::take(&mut ctx.prefix),
                        links: inline.links,
                        ..Block::default()
                    });
                }
            }
            Event::Start(Tag::BlockQuote(_)) => {
                let mut nested = ctx.clone();
                nested.quote += 1;
                render(&node.children, source, &mut nested, out, depth + 1);
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let language = match kind {
                    CodeBlockKind::Fenced(info) => {
                        info.split_whitespace().next().unwrap_or_default().into()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
                out.push(Block {
                    kind: "code".into(),
                    text: raw_text(&node.children),
                    language,
                    ..Block::default()
                });
            }
            Event::Start(Tag::MetadataBlock(_)) => out.push(Block {
                kind: "frontmatter".into(),
                text: raw_text(&node.children),
                language: "metadata".into(),
                ..Block::default()
            }),
            Event::Start(Tag::Table(_)) => out.push(table(node, source)),
            Event::Start(Tag::HtmlBlock) | Event::Html(_) => {
                out.push(plain_block("fallback", literal(source, node)))
            }
            Event::Start(Tag::FootnoteDefinition(label)) => {
                let mut nested = ctx.clone();
                nested.prefix = format!("[^{label}]");
                render(&node.children, source, &mut nested, out, depth + 1);
            }
            Event::Rule => out.push(Block {
                kind: "rule".into(),
                ..Block::default()
            }),
            Event::Start(_) => out.push(plain_block("fallback", literal(source, node))),
            _ => push_inline("paragraph", std::slice::from_ref(node), source, ctx, out, 0),
        }
    }
}
fn raw_text(nodes: &[Node<'_>]) -> String {
    let mut result = String::new();
    for node in nodes {
        match &node.event {
            Event::Text(text) | Event::Code(text) | Event::Html(text) | Event::InlineHtml(text) => {
                result.push_str(text)
            }
            Event::SoftBreak | Event::HardBreak => result.push('\n'),
            _ => result.push_str(&raw_text(&node.children)),
        }
    }
    result
}
fn table(node: &Node<'_>, source: &str) -> Block {
    let mut block = Block {
        kind: "table".into(),
        ..Block::default()
    };
    for row in &node.children {
        let header = matches!(&row.event, Event::Start(Tag::TableHead));
        let mut cells = Vec::new();
        for cell in &row.children {
            let value = inline(&cell.children, source, 0);
            block.links.extend(value.links);
            cells.push(Cell {
                text: value.text,
                inline_markdown: value.markdown,
            });
        }
        block.text.push_str(
            &cells
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>()
                .join(" | "),
        );
        block.text.push('\n');
        block.rows.push(TableRow { cells, header });
    }
    block
}
fn inline(nodes: &[Node<'_>], source: &str, depth: usize) -> Inline {
    inline_refs(&nodes.iter().collect::<Vec<_>>(), source, depth)
}
fn inline_refs(nodes: &[&Node<'_>], source: &str, depth: usize) -> Inline {
    let mut result = Inline::default();
    for node in nodes {
        if depth > MAX_DEPTH {
            append_plain(&mut result, &literal(source, node));
            continue;
        }
        match &node.event {
            Event::Text(text) => append_plain(&mut result, text),
            Event::Code(text) => {
                result.text.push_str(text);
                let fence =
                    "`".repeat(text.split(|c| c != '`').map(str::len).max().unwrap_or(0) + 1);
                result.markdown.push_str(&format!("{fence} {text} {fence}"));
            }
            Event::SoftBreak => append_plain(&mut result, " "),
            Event::HardBreak => {
                result.text.push('\n');
                result.markdown.push_str("  \n");
            }
            Event::TaskListMarker(checked) => {
                append_plain(&mut result, if *checked { "☑ " } else { "☐ " })
            }
            Event::Start(Tag::Emphasis | Tag::Strong | Tag::Strikethrough) => {
                let value = inline(&node.children, source, depth + 1);
                let marker = match &node.event {
                    Event::Start(Tag::Emphasis) => "*",
                    Event::Start(Tag::Strong) => "**",
                    _ => "~~",
                };
                result.text.push_str(&value.text);
                result
                    .markdown
                    .push_str(&format!("{marker}{}{marker}", value.markdown));
                result.links.extend(value.links);
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                let value = inline(&node.children, source, depth + 1);
                result.text.push_str(&value.text);
                if safe_uri(dest_url) {
                    result.markdown.push_str(&format!(
                        "[{}](<{}>)",
                        value.markdown,
                        dest_url.replace('>', "%3E").replace('<', "%3C")
                    ));
                    result.links.push(Link {
                        label: value.text,
                        uri: dest_url.to_string(),
                    });
                } else {
                    result.markdown.push_str(&value.markdown);
                    append_plain(&mut result, &format!("（链接未启用：{dest_url}）"));
                }
                result.links.extend(value.links);
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                let alt = raw_text(&node.children);
                append_plain(&mut result, &format!("[图片：{alt} · {dest_url}；未加载]"));
            }
            Event::InlineHtml(text) | Event::Html(text) => append_plain(&mut result, text),
            Event::FootnoteReference(label) => append_plain(&mut result, &format!("[^{label}]")),
            _ => append_plain(&mut result, &literal(source, node)),
        }
    }
    result
}
fn safe_uri(uri: &str) -> bool {
    if uri.chars().any(|c| c.is_control()) {
        return false;
    }
    if let Some((scheme, _)) = uri.split_once(':') {
        matches!(
            scheme.to_ascii_lowercase().as_str(),
            "http" | "https" | "mailto"
        )
    } else {
        !uri.trim_start().starts_with("//")
    }
}
fn append_plain(target: &mut Inline, text: &str) {
    target.text.push_str(text);
    target.markdown.push_str(&escape(text));
}
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if c == '<' {
            escaped.push_str("&lt;");
            continue;
        }
        if c == '>' {
            escaped.push_str("&gt;");
            continue;
        }
        if c.is_ascii_punctuation() {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}
fn styled(markdown: &str, plain: &str) -> StyledText {
    // 1.18.1 返回 Result 而非 Option；不支持的片段保留为可见纯文本。
    StyledText::from_markdown(markdown).unwrap_or_else(|_| StyledText::from_plain_text(plain))
}
fn model<T: Clone + 'static>(rows: Vec<T>) -> ModelRc<T> {
    Rc::new(VecModel::from(rows)).into()
}
/// 投影已解析的安全领域块，不读取文件，也不改变应用状态。
pub fn project(blocks: &[Block]) -> Vec<ui::MarkdownBlock> {
    blocks
        .iter()
        .map(|b| {
            let markdown = if b.kind == "heading" {
                format!("**{}**", b.inline_markdown)
            } else {
                b.inline_markdown.clone()
            };
            ui::MarkdownBlock {
                kind: b.kind.clone().into(),
                text: b.text.clone().into(),
                styled: if b.kind == "fallback" {
                    // 纯文本回退不再进入 Markdown 解析，空行内片段也不会吞掉正文。
                    StyledText::from_plain_text(&b.text)
                } else {
                    styled(&markdown, &b.text)
                },
                level: b.level,
                prefix: b.prefix.clone().into(),
                language: b.language.clone().into(),
                rows: model(
                    b.rows
                        .iter()
                        .map(|r| ui::MarkdownTableRow {
                            header: r.header,
                            cells: model(
                                r.cells
                                    .iter()
                                    .map(|c| ui::MarkdownCell {
                                        text: c.text.clone().into(),
                                        styled: styled(
                                            &if r.header {
                                                format!("**{}**", c.inline_markdown)
                                            } else {
                                                c.inline_markdown.clone()
                                            },
                                            &c.text,
                                        ),
                                    })
                                    .collect(),
                            ),
                        })
                        .collect(),
                ),
                links: model(
                    b.links
                        .iter()
                        .map(|l| ui::MarkdownLink {
                            label: l.label.clone().into(),
                            uri: l.uri.clone().into(),
                        })
                        .collect(),
                ),
            }
        })
        .collect()
}
