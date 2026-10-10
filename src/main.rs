use anyhow::{Context, Result, bail};
use chu::{App, controller::Controller, ui::AppWindow};
use slint::{ComponentHandle, Timer, TimerMode};
use std::{path::PathBuf, time::Duration};
fn main() -> Result<()> {
    let mut root = None;
    let mut home = None;
    let mut request = None;
    let mut snapshot = false;
    let mut screenshot = None;
    let mut page = 0;
    let mut import_dialog = false;
    let mut width = None;
    let mut height = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = Some(PathBuf::from(args.next().context("--root 缺少路径")?)),
            "--home" => home = Some(PathBuf::from(args.next().context("--home 缺少路径")?)),
            "--request" => request = Some(args.next().context("--request 缺少 JSON")?),
            "--snapshot" => snapshot = true,
            "--screenshot" => {
                screenshot = Some(PathBuf::from(args.next().context("--screenshot 缺少路径")?))
            }
            "--import-dialog" => import_dialog = true,
            "--page" => page = args.next().context("--page 缺少编号")?.parse()?,
            "--width" => width = Some(args.next().context("--width 缺少尺寸")?.parse::<u32>()?),
            "--height" => height = Some(args.next().context("--height 缺少尺寸")?.parse::<u32>()?),
            "--help" | "-h" => {
                println!(
                    "Chu · Rust + Slint\n默认启动原生桌面应用。\n隔离运行: chu --root /tmp/chu/state --home /tmp/chu/home\n自动化: --snapshot 或 --request '{{\"method\":\"Refresh\",\"args\":[]}}'\n截图: --screenshot /tmp/chu.png --page 0 --width 1080 --height 680\n导入弹窗截图: 追加 --import-dialog --page 2（或 3）\n页面: 0 总览 / 1 项目 / 2 Skills / 3 MCP / 4 Providers / 5 Agents / 6 提示词 / 7 References / 8 设置"
                );
                return Ok(());
            }
            _ => bail!("未知参数: {arg}"),
        }
    }
    if import_dialog && !matches!(page, 2 | 3) {
        bail!("--import-dialog 仅支持 --page 2（Skills）或 3（MCP）");
    }
    let mut app = match (root, home) {
        (Some(root), Some(home)) => App::open(root, home)?,
        (None, None) => App::system()?,
        _ => bail!("隔离模式必须同时指定 --root 和 --home，以免操作真实宿主目录"),
    };
    if let Some(request) = request {
        let value: serde_json::Value = serde_json::from_str(&request)?;
        let method = value["method"].as_str().context("请求缺少 method")?;
        println!(
            "{}",
            serde_json::to_string_pretty(&app.execute(method, value["args"].clone())?)?
        );
        return Ok(());
    }
    if snapshot {
        println!("{}", serde_json::to_string_pretty(&app.snapshot()?)?);
        return Ok(());
    }
    let ui = AppWindow::new()?;
    if width.is_some() || height.is_some() {
        let size = ui.window().size();
        ui.window().set_size(slint::PhysicalSize::new(
            width.unwrap_or(size.width),
            height.unwrap_or(size.height),
        ));
    }
    let controller = Controller::attach(&ui, app)?;
    controller.navigate(page);
    if import_dialog {
        ui.invoke_action("import".into());
    }
    let timer = Timer::default();
    if let Some(path) = screenshot {
        let weak = ui.as_weak();
        let started = std::time::Instant::now();
        let captured = std::cell::Cell::new(false);
        timer.start(TimerMode::Repeated, Duration::from_millis(250), move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if ui.get_busy() || started.elapsed() < Duration::from_millis(750) {
                return;
            }
            if captured.replace(true) {
                return;
            }
            let result = (|| -> Result<()> {
                let pixels = ui.window().take_snapshot()?;
                image::save_buffer(
                    &path,
                    pixels.as_bytes(),
                    pixels.width(),
                    pixels.height(),
                    image::ColorType::Rgba8,
                )?;
                Ok(())
            })();
            if let Err(e) = result {
                eprintln!("截图失败: {e}");
            } else {
                println!("截图已保存: {}", path.display());
            }
            let _ = slint::quit_event_loop();
        });
    }
    let run_result = ui.run();
    let shutdown_result = controller.shutdown();
    drop(controller);
    shutdown_result?;
    run_result?;
    Ok(())
}
