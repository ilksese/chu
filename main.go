package main

import (
	"embed"
	"fmt"
	"os"

	"github.com/wailsapp/wails/v2"
	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/assetserver"
	"github.com/wailsapp/wails/v2/pkg/options/linux"
)

//go:embed all:frontend/dist
var assets embed.FS

func main() {
	// WebKit's DMA-BUF renderer composites a black window on this X11 setup.
	_ = os.Setenv("WEBKIT_DISABLE_DMABUF_RENDERER", "1")
	_ = os.Setenv("WEBKIT_DISABLE_COMPOSITING_MODE", "1")
	app := NewApp()

	err := wails.Run(&options.App{
		Title:            "Chu Agent Manager",
		Width:            1360,
		Height:           860,
		MinWidth:         1080,
		MinHeight:        680,
		AssetServer:      &assetserver.Options{Assets: assets},
		BackgroundColour: &options.RGBA{R: 255, G: 253, B: 244, A: 255},
		Linux: &linux.Options{
			ProgramName:      "chu",
			WebviewGpuPolicy: linux.WebviewGpuPolicyNever,
		},
		OnStartup: app.startup,
		Bind:      []interface{}{app},
	})
	if err != nil {
		fmt.Println("Chu failed to start:", err)
	}
}
