use chu::{markdown_view, models::*, overview};
use i_slint_backend_testing::ElementHandle;
use slint::{ComponentHandle, Model, StyledText};

#[test]
fn project_matrix_last_host_and_reset_remain_reachable_by_local_horizontal_scroll() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = chu::ui::AppWindow::new().unwrap();
    ui.window().set_size(slint::LogicalSize::new(1080., 680.));
    ui.set_page(1);
    ui.set_selected_id("project".into());
    let hosts = ["OpenCode", "Claude Code", "Codex", "jcode"];
    ui.set_visible_hosts(chu::presenter::model(
        hosts
            .iter()
            .map(|name| chu::ui::HostRow {
                id: (*name).into(),
                name: (*name).into(),
                installed: true,
                visible: true,
                ..Default::default()
            })
            .collect(),
    ));
    ui.set_project_skills(chu::presenter::model(vec![chu::ui::ProjectSkillRow {
        id: "skill".into(),
        name: "Skill".into(),
        hosts: chu::presenter::model(
            hosts
                .iter()
                .map(|name| chu::ui::HostToggle {
                    id: (*name).into(),
                    name: (*name).into(),
                    available: true,
                    reset: true,
                    ..Default::default()
                })
                .collect(),
        ),
    }]));
    ui.show().unwrap();
    let matrix = ElementHandle::find_by_accessible_label(&ui, "项目部署表格")
        .next()
        .unwrap();
    let p = matrix.absolute_position();
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
            position: slint::LogicalPosition::new(p.x + 20., p.y + 85.),
            delta_x: -5000.,
            delta_y: 0.,
        });
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(500));
    for label in ["Skill · jcode 部署", "重置 Skill · jcode 副本"] {
        let target = ElementHandle::find_by_accessible_label(&ui, label)
            .find(|e| e.accessible_enabled() == Some(true))
            .unwrap();
        assert!(target.absolute_position().x >= p.x);
        assert!(
            target.absolute_position().x + target.size().width <= p.x + matrix.size().width,
            "项目末列不能只存在模型中而位于横滚之外：{label}"
        );
    }
}

#[test]
fn full_document_is_split_into_native_blocks_not_whole_document_styled_text() {
    let source = "---\nname: native\n---\n\n# 标题 **强调**\n\n正文 *斜体* 与 `代码`。\n\n> 引用内容\n\n- 第一项\n- [x] 完成项\n\n3. 第三项\n4. 第四项\n\n```rust\nfn main() {}\n```\n\n| 名称 | 值 |\n| --- | --- |\n| **粗体** | `42` |\n\n---\n";
    let blocks = markdown_view::parse(source);
    for kind in [
        "frontmatter",
        "heading",
        "paragraph",
        "quote",
        "list",
        "code",
        "table",
        "rule",
    ] {
        assert!(
            blocks.iter().any(|b| b.kind == kind),
            "missing {kind}: {blocks:?}"
        );
    }
    let heading = blocks.iter().find(|b| b.kind == "heading").unwrap();
    assert_eq!(heading.level, 1);
    assert_eq!(heading.text, "标题 强调");
    assert!(!heading.inline_markdown.starts_with('#'));
    assert!(blocks.iter().any(|b| b.prefix == "3."));
    assert!(blocks.iter().any(|b| b.prefix == "4."));
    assert!(blocks.iter().any(|b| b.text.contains("☑ 完成项")));
    let code = blocks.iter().find(|b| b.kind == "code").unwrap();
    assert_eq!(code.language, "rust");
    assert_eq!(code.text, "fn main() {}\n");
    let table = blocks.iter().find(|b| b.kind == "table").unwrap();
    assert_eq!(table.rows.len(), 2);
    assert!(table.rows[0].header);
    assert_eq!(table.rows[1].cells[0].text, "粗体");
    let projected = markdown_view::project(&blocks);
    assert_eq!(projected.len(), blocks.len());
    let table = projected.iter().find(|b| b.kind == "table").unwrap();
    assert_eq!(table.rows.row_count(), 2);
    assert_eq!(table.rows.row_data(0).unwrap().cells.row_count(), 2);
}

#[test]
fn unsafe_html_colors_images_and_schemes_never_become_active_markup() {
    let source = "<script>alert('x')</script>\n\n文字 <font color=\"red\">保留标签</font> <u>原始 HTML</u>。\n\n![品牌](https://invalid.test/image.png)\n\n[危险](javascript:alert) [站点](https://example.test/a) [位置](./README.md)";
    let blocks = markdown_view::parse(source);
    assert!(
        blocks
            .iter()
            .any(|b| b.kind == "fallback" && b.text.contains("<script>"))
    );
    let image = blocks
        .iter()
        .find(|b| b.text.contains("图片：品牌"))
        .unwrap();
    assert!(image.text.contains("未加载"));
    assert!(image.links.is_empty());
    for block in &blocks {
        assert!(!block.inline_markdown.contains("<font"));
        assert!(!block.inline_markdown.contains("<u>"));
        assert!(!block.inline_markdown.contains("<script>"));
        assert!(!block.inline_markdown.contains("](javascript:"));
        assert!(
            block
                .links
                .iter()
                .all(|l| !l.uri.starts_with("javascript:"))
        );
        if !block.inline_markdown.is_empty() {
            assert!(
                StyledText::from_markdown(&block.inline_markdown).is_ok(),
                "not safe inline: {}",
                block.inline_markdown
            );
        }
    }
    let links: Vec<_> = blocks.iter().flat_map(|b| &b.links).collect();
    assert_eq!(links.len(), 2);
    assert!(links.iter().any(|l| l.uri == "https://example.test/a"));
    assert!(links.iter().any(|l| l.uri == "./README.md"));
}

#[test]
fn nested_lists_quotes_inline_code_and_unsupported_syntax_stay_visible() {
    let source = "- 父条目\n  - 子条目 **内容**\n\n> 引用\n>\n> - 引用列表\n\n文字 ``a ` b`` 与 ~~删除~~\n\n脚注[^n]。\n\n[^n]: 脚注正文\n";
    let blocks = markdown_view::parse(source);
    assert!(
        blocks
            .iter()
            .any(|b| b.prefix.contains("  •") && b.text == "子条目 内容")
    );
    assert!(
        blocks
            .iter()
            .any(|b| b.kind == "quote" && b.text.contains("引用列表"))
    );
    assert!(blocks.iter().any(|b| b.text.contains("a ` b")));
    assert!(blocks.iter().any(|b| b.text.contains("[^n]")));
    assert!(blocks.iter().any(|b| b.text.contains("脚注正文")));
    for block in &blocks {
        if !block.inline_markdown.is_empty() {
            assert!(StyledText::from_markdown(&block.inline_markdown).is_ok());
        }
    }
}

#[test]
fn styled_projection_retains_plain_fallback_on_result_error() {
    let blocks = [markdown_view::Block {
        kind: "paragraph".into(),
        text: "仍可阅读".into(),
        inline_markdown: "# unsupported heading".into(),
        ..Default::default()
    }];
    let rows = markdown_view::project(&blocks);
    assert_eq!(rows[0].styled, StyledText::from_plain_text("仍可阅读"));
    assert!(markdown_view::parse("").is_empty());
}

#[test]
fn native_markdown_table_body_rows_are_inside_the_visible_table_geometry() {
    i_slint_backend_testing::init_no_event_loop();
    for (width, height) in [(1360., 860.), (1080., 680.)] {
        let ui = chu::ui::AppWindow::new().unwrap();
        ui.window().set_size(slint::LogicalSize::new(width, height));
        ui.set_page(7);
        ui.set_selected_id("table".into());
        ui.set_selected_name("表格阅读".into());
        let source = format!(
            "# 长文阅读\n\n{}| 宿主 | 状态 | 配置 | 备注 |\n| --- | --- | --- | --- |\n| OpenCode | 可用 | stdio | 保留配置 |\n| Codex | 已部署 | native | 本地横向滚动可达 |\n\n表格之后的正文。",
            "上文段落，验证外层纵向滚动。\n\n".repeat(20)
        );
        ui.set_selected_content(source.clone().into());
        ui.set_markdown_blocks(chu::presenter::model(markdown_view::project(
            &markdown_view::parse(&source),
        )));
        ui.show().unwrap();
        let preview = ElementHandle::find_by_accessible_label(&ui, "表格阅读 预览")
            .next()
            .unwrap();
        let p = preview.absolute_position();
        ui.window()
            .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                position: slint::LogicalPosition::new(p.x + 20., p.y + 20.),
                delta_x: 0.,
                delta_y: -5000.,
            });
        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(500));
        let find = |label| {
            ElementHandle::find_by_accessible_label(&ui, label)
                .find(|e| e.size().width > 0. && e.size().height > 0.)
                .unwrap_or_else(|| panic!("缺少表格正文 {label}"))
        };
        let header = find("宿主");
        let first = find("OpenCode");
        let second = find("Codex");
        let following = find("表格之后的正文。");
        assert!(first.absolute_position().y >= header.absolute_position().y + header.size().height);
        assert!(second.absolute_position().y >= first.absolute_position().y + first.size().height);
        assert!(
            following.absolute_position().y >= second.absolute_position().y + second.size().height,
            "表格之后的正文覆盖了第二行：{:?} {:?}",
            following.absolute_position(),
            second.absolute_position()
        );
        let preview = find("表格阅读 预览");
        let table = find("Markdown 表格");
        assert!(
            second.absolute_position().y + second.size().height
                <= table.absolute_position().y + table.size().height,
            "表格容器裁剪正文：表格{:?}/{:?}，第二行{:?}/{:?}",
            table.absolute_position(),
            table.size(),
            second.absolute_position(),
            second.size()
        );
        assert!(
            second.absolute_position().y + second.size().height
                <= preview.absolute_position().y + preview.size().height
        );
        let table_origin = table.absolute_position();
        ui.window()
            .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                position: slint::LogicalPosition::new(table_origin.x + 20., table_origin.y + 20.),
                delta_x: -5000.,
                delta_y: 0.,
            });
        i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(500));
        let last = find("本地横向滚动可达");
        assert!(last.absolute_position().x >= table.absolute_position().x);
        assert!(
            last.absolute_position().x + last.size().width
                <= table.absolute_position().x + table.size().width,
            "末列必须能通过表格局部横滚进入可视区：{width} 表格{:?}/{:?}，末列{:?}/{:?} {:?}",
            table.absolute_position(),
            table.size(),
            last.absolute_position(),
            last.size(),
            table.accessible_description()
        );
        assert!(
            (find("表格阅读 预览").absolute_position().x - preview.absolute_position().x).abs()
                < 1.
        );
    }
}

#[test]
fn overview_is_snapshot_only_managed_counts_and_explicit_navigation_targets() {
    let snapshot = Snapshot {
        skills: vec![
            SkillView {
                id: "managed".into(),
                name: "Managed".into(),
                managed: true,
                ..Default::default()
            },
            SkillView {
                id: "host-only".into(),
                ..Default::default()
            },
        ],
        mcps: vec![
            McpView {
                managed: true,
                ..Default::default()
            },
            McpView::default(),
        ],
        hosts: vec![Host {
            id: "host".into(),
            name: "宿主".into(),
            installed: false,
            ..Default::default()
        }],
        projects: vec![ProjectView {
            id: "project".into(),
            name: "Project".into(),
            available: false,
            deployments: Map::from([(
                "managed".into(),
                Map::from([(
                    "host".into(),
                    ProjectDeploymentView {
                        enabled: true,
                        status: "modified".into(),
                    },
                )]),
            )]),
            ..Default::default()
        }],
        providers: vec![Provider {
            id: "provider".into(),
            name: "Provider".into(),
            models_error: "secret transport error".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let before = snapshot.clone();
    let view = overview::project(&snapshot);
    assert_eq!(snapshot, before);
    assert_eq!(view.counts.iter().find(|r| r.page == 2).unwrap().value, 1);
    assert_eq!(view.counts.iter().find(|r| r.page == 3).unwrap().value, 1);
    assert!(view.issues.iter().any(|r| r.page == 8 && r.host == "host"));
    assert!(
        view.issues
            .iter()
            .any(|r| r.page == 1 && r.id == "project" && r.host == "host")
    );
    assert!(
        view.issues
            .iter()
            .any(|r| r.page == 4 && r.id == "provider")
    );
    assert!(
        view.issues
            .iter()
            .all(|r| !r.detail.contains("secret transport"))
    );
}
