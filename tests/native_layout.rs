//! Guard a self-contained native project and the approved semantic palette.
use chu::ui::{AppWindow, Theme};
use slint::{Color, ComponentHandle};
use std::path::Path;
#[test]
fn semantic_theme_keeps_the_approved_application_palette() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    let theme = ui.global::<Theme>();
    for (name, actual, (r, g, b)) in [
        ("background", theme.get_background(), (255, 253, 244)),
        ("card", theme.get_card(), (255, 255, 255)),
        ("foreground", theme.get_foreground(), (0, 0, 0)),
        (
            "muted-foreground",
            theme.get_muted_foreground(),
            (92, 54, 19),
        ),
        ("primary", theme.get_primary(), (244, 220, 38)),
        ("primary-hover", theme.get_primary_hover(), (255, 230, 45)),
        ("primary-muted", theme.get_primary_muted(), (255, 251, 232)),
        ("muted", theme.get_muted(), (247, 245, 236)),
        ("subtle-border", theme.get_subtle_border(), (229, 229, 229)),
        ("destructive", theme.get_destructive(), (233, 41, 41)),
        ("error-surface", theme.get_error_surface(), (255, 243, 243)),
        ("success", theme.get_success(), (34, 153, 72)),
        (
            "success-surface",
            theme.get_success_surface(),
            (238, 250, 241),
        ),
        ("border", theme.get_border(), (0, 0, 0)),
        ("ring", theme.get_ring(), (233, 41, 41)),
    ] {
        assert_eq!(
            actual,
            Color::from_rgb_u8(r, g, b),
            "native semantic color changed: {name}"
        );
    }
}
#[test]
fn product_sources_and_build_hooks_do_not_depend_on_the_removed_stack() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for obsolete in [
        "frontend",
        "cmd",
        "internal",
        "go.mod",
        "go.sum",
        "wails.json",
        "rust_parity_test.go",
        "package.json",
        "pnpm-lock.yaml",
        "scripts/test-windows-build.sh",
        "scripts/create-rust-fixture.py",
        "build/darwin",
        "build/windows",
    ] {
        assert!(
            !root.join(obsolete).exists(),
            "obsolete product entry returned: {obsolete}"
        );
    }
    for directory in ["src", "ui", "assets", "scripts", "examples"] {
        for entry in walkdir::WalkDir::new(root.join(directory))
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !e.file_name().to_string_lossy().starts_with(".env"))
        {
            let entry = entry.unwrap();
            if !entry.file_type().is_file() {
                continue;
            }
            let extension = entry
                .path()
                .extension()
                .and_then(|v| v.to_str())
                .unwrap_or("");
            assert!(
                ![
                    "go", "ts", "tsx", "js", "jsx", "css", "scss", "sass", "html", "woff", "woff2",
                    "nsi", "nsh", "py"
                ]
                .contains(&extension),
                "non-native product source: {}",
                entry.path().display()
            );
        }
    }
    for source in [
        include_str!("../build.rs"),
        include_str!("../scripts/verify-rust.sh"),
        include_str!("../scripts/build.sh"),
        include_str!("../scripts/package-macos.sh"),
    ] {
        assert!(
            !source.contains("frontend/"),
            "native build hook uses the removed UI directory"
        );
        for line in source.lines() {
            for token in line.split_whitespace() {
                assert!(
                    !["wails", "pnpm", "npm", "go", "node", "python3"].contains(&token),
                    "native build hook uses removed tooling: {token}"
                );
            }
        }
    }
}
