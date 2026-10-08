# Chu 品牌资源

- `app-icon.svg`：可编辑的图标源图，作为维护原始图形的依据。
- `app-icon.png`：1024×1024 应用图标，原生窗口编译时内嵌；macOS 打包从此图生成各尺寸 `.icns`。
- `windows/app.ico`：Windows 可执行文件的多尺寸原生图标资源。

三份资源均沿用既有图形，没有重绘或改色。PNG 与 ICO 分别被 Slint/build.rs 和原生打包脚本使用；SVG 是源素材，不是重复运行时负担。

不要把平台生成的图标集、应用 bundle、无用 Logo 或字体缓存放入此目录。生成产物位于忽略目录 `build/bin/`。
