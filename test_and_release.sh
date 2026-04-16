#!/bin/bash
set -e

echo "=== mullande Test and Release ==="
echo ""

# Run tests first
echo "▶️  Running cargo test..."
cargo test
echo "✅ All tests passed"
echo ""

# Build for current platform (development)
echo "▶️  Building for current platform (debug)..."
cargo build
echo "✅ Debug build complete"
echo ""

# Build release for current platform
echo "▶️  Building release for current platform..."
cargo build --release
echo "✅ Release build complete"
echo ""

# Install to ~/.cargo/bin
echo "▶️  Installing to ~/.cargo/bin..."
mkdir -p "$HOME/.cargo/bin"
cp -f "target/release/mullande" "$HOME/.cargo/bin/"
echo "✅ Installed to $HOME/.cargo/bin/mullande"
echo ""

# Display current binary information
CURRENT=$(uname -s)-$(uname -m)
SIZE=$(du -h target/release/mullande | cut -f1)
echo "=== Summary ==="
echo "Current platform: $CURRENT"
echo "Binary size: $SIZE"
echo "Installed to: $HOME/.cargo/bin/mullande"
echo ""

# Build for other platforms if CROSS=1 is set
if [ "$CROSS" = "1" ]; then
    echo "📦 Building for other platforms (CROSS=1 set)..."
    echo ""

    mkdir -p releases

    # Check if Docker is running and cross is installed
    if command -v cross >/dev/null && docker info >/dev/null 2>&1; then
        echo "✅ Docker and cross are ready, building for all platforms..."
        echo ""

        # Linux x86_64
        echo "▶️  Building Linux x86_64..."
        if cross build --release --target x86_64-unknown-linux-gnu 2>&1; then
            cp -f target/x86_64-unknown-linux-gnu/release/mullande releases/mullande-x86_64-unknown-linux-gnu
            echo "✅ Linux x86_64 built: releases/mullande-x86_64-unknown-linux-gnu ($(du -h releases/mullande-x86_64-unknown-linux-gnu | cut -f1))"
        else
            echo "⚠️  Linux x86_64 build failed, skipping..."
        fi
        echo ""

        # Windows x86_64
        echo "▶️  Building Windows x86_64..."
        if cross build --release --target x86_64-pc-windows-msvc 2>&1; then
            cp -f target/x86_64-pc-windows-msvc/release/mullande.exe releases/mullande-x86_64-pc-windows-msvc.exe
            echo "✅ Windows x86_64 built: releases/mullande-x86_64-pc-windows-msvc.exe ($(du -h releases/mullande-x86_64-pc-windows-msvc.exe | cut -f1))"
        else
            echo "⚠️  Windows x86_64 build failed, skipping..."
        fi
        echo ""
    else
        echo "⚠️  Skipping cross compilation:"
        if ! command -v cross >/dev/null; then
            echo "   - cross is not installed (run: cargo install cross --git https://github.com/cross-rs/cross)"
        fi
        if ! docker info >/dev/null 2>&1; then
            echo "   - Docker is not running"
        fi
        echo ""
    fi

    echo "Available releases in releases/:"
    ls -lh releases/ 2>/dev/null || echo "  (none built yet)"
    echo ""
else
    echo "📦 Cross-platform compilation skipped (set CROSS=1 to enable)"
    echo "   To build for other platforms:"
    echo "     CROSS=1 ./test_and_release.sh"
    echo ""
fi

echo "🎉 Done! Run 'mullande --help' to get started."