#!/bin/bash

# This script generates a test project from the template and builds it for Windows

set -e

echo "🏗️  Generating test project from template..."
TEMP_DIR=$(mktemp -d)
echo "📁 Temp directory: $TEMP_DIR"

# Generate project from template
wails init -n testapp -t "$(pwd)" -dir "$TEMP_DIR/testapp"

echo "📦 Installing frontend dependencies..."
cd "$TEMP_DIR/testapp/frontend"
pnpm install

echo "🔨 Building Windows executable..."
cd "$TEMP_DIR/testapp"
wails build -platform windows/amd64

echo "✅ Build complete!"
echo "📍 Executable location: $TEMP_DIR/testapp/build/bin/testapp.exe"
echo ""
echo "To copy to your Windows system, run:"
echo "cp $TEMP_DIR/testapp/build/bin/testapp.exe /mnt/c/Users/YourUsername/Desktop/"
