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
cp "target/release/mullande" "$HOME/.cargo/bin/"
echo "✅ Installed to $HOME/.cargo/bin/mullande"
echo ""

# Show cross-compilation commands for other platforms
echo "📦 Cross compilation targets (run manually if you have the toolchains):"
echo ""
echo "For macOS ARM64:"
echo "  cargo build --release --target aarch64-apple-darwin"
echo "  cp target/aarch64-apple-darwin/release/mullande releases/mullande-aarch64-apple-darwin"
echo ""
echo "For Linux x86_64 (requires cross):"
echo "  cross build --release --target x86_64-unknown-linux-gnu"
echo "  cp target/x86_64-unknown-linux-gnu/release/mullande releases/mullande-x86_64-unknown-linux-gnu"
echo ""
echo "For Windows x86_64 (requires cross):"
echo "  cross build --release --target x86_64-pc-windows-msvc"
echo "  cp target/x86_64-pc-windows-msvc/release/mullande.exe releases/mullande-x86_64-pc-windows-msvc.exe"
echo ""

# Display current binary information
CURRENT=$(uname -s)-$(uname -m)
SIZE=$(du -h target/release/mullande | cut -f1)
echo "=== Summary ==="
echo "Current platform: $CURRENT"
echo "Binary size: $SIZE"
echo "Installed to: $HOME/.cargo/bin/mullande"
echo ""
echo "🎉 Done! Run 'mullande --help' to get started."
