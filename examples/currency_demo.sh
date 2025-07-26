#!/bin/bash

# EU Currency MCP Server Interactive Demo
# This script demonstrates the currency server by sending MCP protocol messages

echo "🏦 EU Currency MCP Server Interactive Demo"
echo "==========================================="
echo ""

# Start the server in background and capture its PID
echo "🚀 Starting currency server..."
cargo run --example currency_server &
SERVER_PID=$!

# Give server time to start
sleep 2

echo "✅ Server started (PID: $SERVER_PID)"
echo ""

# Function to send MCP messages to the server
send_mcp_message() {
    local message="$1"
    local description="$2"
    
    echo "📤 $description"
    echo "   Message: $message" 
    echo "   Response:"
    echo "$message" | nc -q 1 localhost 8080 2>/dev/null || echo "   (Server running on stdio - use MCP client to interact)"
    echo ""
}

echo "💡 The server is now running and waiting for MCP protocol messages."
echo "   It implements the Model Context Protocol over stdin/stdout."
echo ""

echo "🔧 To interact with the server, you need an MCP-compatible client that can:"
echo "   1. Send JSON-RPC initialization messages"
echo "   2. Call the available tools: get_rates, analyze_trend, list_currencies, get_historical"
echo "   3. Parse the structured responses"
echo ""

echo "📋 Available Tools:"
echo "   • get_rates: Get current exchange rates"
echo "   • analyze_trend: Analyze currency trends with predictions"  
echo "   • list_currencies: List supported currencies"
echo "   • get_historical: Get historical rate data"
echo ""

echo "🎯 Example tool calls the server expects:"
echo ""

echo "1️⃣  Initialize Client:"
cat << 'EOF'
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize", 
  "params": {
    "protocolVersion": "2024-11-05",
    "capabilities": {"tools": {}},
    "clientInfo": {"name": "demo-client", "version": "1.0.0"}
  }
}
EOF

echo ""
echo "2️⃣  List Available Tools:"
cat << 'EOF'
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/list",
  "params": {}
}
EOF

echo ""
echo "3️⃣  Get EUR Exchange Rates:"
cat << 'EOF'
{
  "jsonrpc": "2.0", 
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "get_rates",
    "arguments": {"base": "EUR"}
  }
}
EOF

echo ""
echo "4️⃣  Analyze EUR→USD Trend:"
cat << 'EOF'
{
  "jsonrpc": "2.0",
  "id": 4, 
  "method": "tools/call",
  "params": {
    "name": "analyze_trend",
    "arguments": {
      "base": "EUR",
      "target": "USD", 
      "days": 30,
      "predict_days": 7
    }
  }
}
EOF

echo ""
echo "💭 The server will respond with rich, formatted analysis including:"
echo "   • Current exchange rates with timestamps"
echo "   • Trend analysis with ASCII sparklines: ▁▂▃▄▅▆▇█" 
echo "   • Moving averages (7-day, 14-day)"
echo "   • Linear regression predictions"
echo "   • Statistical analysis (volatility, ranges)"
echo ""

echo "🔗 Integration Examples:"
echo "   • Claude Desktop with MCP configuration"
echo "   • Custom MCP clients using the PMCP SDK"
echo "   • AI assistants for financial analysis"
echo "   • Trading applications for trend detection"
echo ""

echo "⌨️  Press any key to stop the server..."
read -n 1 -s

# Stop the server
echo ""
echo "🛑 Stopping server..."
kill $SERVER_PID 2>/dev/null
wait $SERVER_PID 2>/dev/null

echo "✅ Demo completed!"
echo ""
echo "🎉 The EU Currency MCP Server provides comprehensive financial analysis"
echo "   tools through the Model Context Protocol, perfect for integration"
echo "   with AI assistants and financial applications!"