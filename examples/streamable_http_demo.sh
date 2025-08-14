#!/bin/bash

# Streamable HTTP MCP Demo Script
# This script demonstrates both stateful and stateless HTTP servers

set -e

echo "╔════════════════════════════════════════════════════════════╗"
echo "║           STREAMABLE HTTP MCP DEMO                        ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# Function to cleanup background processes
cleanup() {
    echo ""
    echo "🛑 Cleaning up..."
    if [ ! -z "$STATEFUL_PID" ]; then
        kill $STATEFUL_PID 2>/dev/null || true
    fi
    if [ ! -z "$STATELESS_PID" ]; then
        kill $STATELESS_PID 2>/dev/null || true
    fi
    exit 0
}

# Set trap for cleanup
trap cleanup INT TERM

# Check if cargo is available
if ! command -v cargo &> /dev/null; then
    echo "❌ Error: cargo is not installed"
    exit 1
fi

# Parse arguments
MODE="${1:-both}"

case "$MODE" in
    stateful)
        echo "🚀 Starting STATEFUL server demo..."
        echo ""
        
        # Start stateful server
        echo "📦 Building and starting stateful server on port 8080..."
        cargo build --example 22_streamable_http_server_stateful --features streamable-http 2>/dev/null
        cargo run --example 22_streamable_http_server_stateful --features streamable-http &
        STATEFUL_PID=$!
        
        # Wait for server to start
        echo "⏳ Waiting for server to start..."
        sleep 3
        
        # Run client against stateful server
        echo ""
        echo "🔗 Running client against stateful server..."
        echo "────────────────────────────────────────────"
        cargo run --example 24_streamable_http_client --features streamable-http
        
        # Cleanup
        kill $STATEFUL_PID 2>/dev/null
        ;;
        
    stateless)
        echo "🚀 Starting STATELESS server demo..."
        echo ""
        
        # Start stateless server
        echo "📦 Building and starting stateless server on port 8081..."
        cargo build --example 23_streamable_http_server_stateless --features streamable-http 2>/dev/null
        cargo run --example 23_streamable_http_server_stateless --features streamable-http &
        STATELESS_PID=$!
        
        # Wait for server to start
        echo "⏳ Waiting for server to start..."
        sleep 3
        
        # Run client against stateless server
        echo ""
        echo "🔗 Running client against stateless server..."
        echo "────────────────────────────────────────────"
        cargo run --example 24_streamable_http_client --features streamable-http -- stateless
        
        # Cleanup
        kill $STATELESS_PID 2>/dev/null
        ;;
        
    both|compare)
        echo "🚀 Starting COMPARISON demo (both servers)..."
        echo ""
        
        # Start both servers
        echo "📦 Building examples..."
        cargo build --example 22_streamable_http_server_stateful --features streamable-http 2>/dev/null
        cargo build --example 23_streamable_http_server_stateless --features streamable-http 2>/dev/null
        cargo build --example 24_streamable_http_client --features streamable-http 2>/dev/null
        
        echo "📡 Starting stateful server on port 8080..."
        cargo run --example 22_streamable_http_server_stateful --features streamable-http > /dev/null 2>&1 &
        STATEFUL_PID=$!
        
        echo "📡 Starting stateless server on port 8081..."
        cargo run --example 23_streamable_http_server_stateless --features streamable-http > /dev/null 2>&1 &
        STATELESS_PID=$!
        
        # Wait for servers to start
        echo "⏳ Waiting for servers to start..."
        sleep 4
        
        # Run client against stateful server
        echo ""
        echo "═══════════════════════════════════════════════════════════"
        echo "        PART 1: STATEFUL SERVER (Port 8080)"
        echo "═══════════════════════════════════════════════════════════"
        cargo run --example 24_streamable_http_client --features streamable-http
        
        echo ""
        echo "═══════════════════════════════════════════════════════════"
        echo "        PART 2: STATELESS SERVER (Port 8081)"
        echo "═══════════════════════════════════════════════════════════"
        cargo run --example 24_streamable_http_client --features streamable-http -- stateless
        
        # Cleanup
        kill $STATEFUL_PID 2>/dev/null
        kill $STATELESS_PID 2>/dev/null
        
        echo ""
        echo "═══════════════════════════════════════════════════════════"
        echo "                    COMPARISON SUMMARY"
        echo "═══════════════════════════════════════════════════════════"
        echo ""
        echo "STATEFUL SERVER (Port 8080):"
        echo "  ✅ Session IDs generated and tracked"
        echo "  ✅ Re-initialization prevented"
        echo "  ✅ Client state maintained across requests"
        echo "  📝 Best for: Long-running connections, complex workflows"
        echo ""
        echo "STATELESS SERVER (Port 8081):"
        echo "  ✅ No session overhead"
        echo "  ✅ Re-initialization allowed"
        echo "  ✅ Each request independent"
        echo "  📝 Best for: Serverless, microservices, simple APIs"
        echo ""
        ;;
        
    interactive)
        echo "🚀 Starting INTERACTIVE demo..."
        echo ""
        echo "Starting both servers for manual testing..."
        
        # Start both servers
        echo "📡 Starting stateful server on port 8080..."
        cargo run --example 22_streamable_http_server_stateful --features streamable-http &
        STATEFUL_PID=$!
        
        echo "📡 Starting stateless server on port 8081..."
        cargo run --example 23_streamable_http_server_stateless --features streamable-http &
        STATELESS_PID=$!
        
        echo ""
        echo "═══════════════════════════════════════════════════════════"
        echo "     SERVERS RUNNING - READY FOR MANUAL TESTING"
        echo "═══════════════════════════════════════════════════════════"
        echo ""
        echo "Stateful server:  http://localhost:8080"
        echo "Stateless server: http://localhost:8081"
        echo ""
        echo "You can now:"
        echo "1. Run the client manually:"
        echo "   cargo run --example 24_streamable_http_client"
        echo "   cargo run --example 24_streamable_http_client -- stateless"
        echo ""
        echo "2. Use curl to test directly:"
        echo "   curl -X POST http://localhost:8080 \\"
        echo "     -H 'Content-Type: application/json' \\"
        echo "     -H 'Accept: application/json' \\"
        echo "     -d '{\"id\":1,\"request\":{\"method\":\"initialize\",...}}'"
        echo ""
        echo "3. Connect with any MCP-compatible client"
        echo ""
        echo "Press Ctrl+C to stop servers..."
        
        # Wait for interrupt
        wait
        ;;
        
    *)
        echo "Usage: $0 [stateful|stateless|both|compare|interactive]"
        echo ""
        echo "Options:"
        echo "  stateful    - Run demo with stateful server only"
        echo "  stateless   - Run demo with stateless server only"
        echo "  both        - Run demo with both servers (default)"
        echo "  compare     - Same as 'both'"
        echo "  interactive - Start servers and wait for manual testing"
        exit 1
        ;;
esac

echo ""
echo "✅ Demo completed successfully!"