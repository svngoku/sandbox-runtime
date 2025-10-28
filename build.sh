#!/bin/bash
# Build script for Rust sandbox runtime

set -e

echo "Building Sandbox Runtime (Rust)..."

# Check for Rust installation
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust is not installed. Please install from https://rustup.rs/"
    exit 1
fi

# Check for required dependencies on Linux
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    echo "Checking Linux dependencies..."

    if ! command -v bwrap &> /dev/null; then
        echo "Warning: bubblewrap (bwrap) not found. Install with: sudo apt-get install bubblewrap"
    fi

    if ! command -v socat &> /dev/null; then
        echo "Warning: socat not found. Install with: sudo apt-get install socat"
    fi

    if ! command -v python3 &> /dev/null; then
        echo "Warning: python3 not found. Install with: sudo apt-get install python3"
    fi

    if ! command -v rg &> /dev/null; then
        echo "Warning: ripgrep (rg) not found. Install with: sudo apt-get install ripgrep"
    fi
fi

# Check for required dependencies on macOS
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "Checking macOS dependencies..."

    if ! command -v rg &> /dev/null; then
        echo "Warning: ripgrep (rg) not found. Install with: brew install ripgrep"
    fi
fi

# Build in release mode
echo "Building release binary..."
cargo build --release

echo ""
echo "Build complete!"
echo "Binary location: target/release/srt"
echo ""
echo "To install globally, run:"
echo "  cargo install --path ."
echo ""
echo "Or copy the binary:"
echo "  sudo cp target/release/srt /usr/local/bin/"
