//! Native and packaged branding must use the legacy application artwork unchanged.
use chu::ui::AppWindow;

#[test]
fn native_application_icon_matches_the_original_artwork() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    let icon = ui
        .get_application_icon()
        .to_rgba8()
        .expect("embedded application icon");
    let original = image::load_from_memory(include_bytes!("../assets/app-icon.png"))
        .unwrap()
        .to_rgba8();
    assert_eq!((icon.width(), icon.height()), original.dimensions());
    assert_eq!(icon.as_bytes(), original.as_raw());
}

#[cfg(target_os = "macos")]
#[test]
fn macos_bundle_contains_the_original_icon_in_all_required_sizes() {
    use std::{fs, path::Path, process::Command};
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("native binary");
    let app = temp.path().join("Chu Icon Test.app");
    fs::write(&source, b"fixture executable").unwrap();
    let packaged = Command::new("bash")
        .arg(root.join("scripts/package-macos.sh"))
        .arg(&source)
        .arg(&app)
        .output()
        .unwrap();
    assert!(
        packaged.status.success(),
        "package failed: {}",
        String::from_utf8_lossy(&packaged.stderr)
    );
    assert_eq!(
        fs::read(app.join("Contents/MacOS/chu")).unwrap(),
        b"fixture executable"
    );
    let plist = app.join("Contents/Info.plist");
    let metadata = Command::new("plutil")
        .args(["-extract", "CFBundleIconFile", "raw", "-o", "-"])
        .arg(&plist)
        .output()
        .unwrap();
    assert!(metadata.status.success());
    let filename = String::from_utf8(metadata.stdout).unwrap();
    assert_eq!(filename.trim(), "iconfile.icns");
    let icon = app.join("Contents/Resources").join(filename.trim());
    assert!(icon.is_file());
    let extracted = temp.path().join("verified.iconset");
    let decoded = Command::new("iconutil")
        .args(["--convert", "iconset"])
        .arg(&icon)
        .arg("--output")
        .arg(&extracted)
        .output()
        .unwrap();
    assert!(
        decoded.status.success(),
        "decode icon failed: {}",
        String::from_utf8_lossy(&decoded.stderr)
    );
    for size in [16, 32, 128, 256, 512] {
        for scale in [1, 2] {
            let suffix = if scale == 2 { "@2x" } else { "" };
            let filename = format!("icon_{size}x{size}{suffix}.png");
            assert_eq!(
                image::image_dimensions(extracted.join(filename)).unwrap(),
                (size * scale, size * scale)
            );
        }
    }
    let highest = image::open(extracted.join("icon_512x512@2x.png"))
        .unwrap()
        .to_rgba8();
    let original = image::load_from_memory(include_bytes!("../assets/app-icon.png"))
        .unwrap()
        .to_rgba8();
    assert_eq!(
        highest, original,
        "packaging must not redraw or recolor the original icon"
    );
}
