#!/bin/bash

# Build script for PMCP WASM example
set -e

echo "🔨 Building PMCP WASM client..."

# Install wasm-pack if not already installed
if ! command -v wasm-pack &> /dev/null; then
    echo "📦 Installing wasm-pack..."
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

# Build the WASM module
echo "🏗️  Building WASM module..."
wasm-pack build --target web --out-dir pkg

echo "✅ Build complete!"
echo ""
echo "To run the example:"
echo "  1. Start an MCP server on ws://localhost:8080"
echo "  2. Serve this directory with a web server:"
echo "     python3 -m http.server 8000"
echo "  3. Open http://localhost:8000 in your browser"
echo ""
echo "For production builds, add --release flag:"
echo "  wasm-pack build --target web --out-dir pkg --release"