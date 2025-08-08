# PMCP WASM Client Example

This example demonstrates how to use PMCP in a web browser via WebAssembly.

## Features

- 🌐 **Browser-native**: Runs entirely in the browser using WASM
- 🔌 **WebSocket transport**: Connect to MCP servers via WebSocket
- 🛠️ **Full MCP support**: Tools, resources, and prompts
- 🎨 **Interactive UI**: Modern web interface for testing
- ⚡ **High performance**: Near-native speed with WASM

## Prerequisites

- Rust toolchain with `wasm32-unknown-unknown` target
- wasm-pack (`cargo install wasm-pack`)
- A web server (Python's http.server or similar)
- An MCP server with WebSocket support

## Building

1. Install the WASM target:
```bash
rustup target add wasm32-unknown-unknown
```

2. Build the WASM module:
```bash
./build.sh
```

Or manually:
```bash
wasm-pack build --target web --out-dir pkg
```

## Running

1. Start an MCP server with WebSocket support on `ws://localhost:8080`

2. Serve this directory with a web server:
```bash
# Using Python
python3 -m http.server 8000

# Or using Node.js
npx http-server -p 8000

# Or using Rust
cargo install basic-http-server
basic-http-server -p 8000
```

3. Open http://localhost:8000 in your browser

## Project Structure

```
wasm/
├── Cargo.toml          # WASM library configuration
├── src/
│   └── lib.rs          # WASM client implementation
├── index.html          # Web interface
├── pkg/                # Generated WASM output (after build)
│   ├── pmcp_wasm.js    # JavaScript bindings
│   └── pmcp_wasm_bg.wasm # WASM binary
└── build.sh            # Build script
```

## API Usage

### JavaScript API

```javascript
import init, { WasmClient } from './pkg/pmcp_wasm.js';

// Initialize WASM module
await init();

// Create client
const client = new WasmClient("ws://localhost:8080");

// Connect to server
await client.connect();

// List available tools
const tools = await client.list_tools();
console.log("Available tools:", tools);

// Call a tool
const result = await client.call_tool("add", { a: 5, b: 3 });
console.log("Result:", result);

// Disconnect
await client.disconnect();
```

### TypeScript Support

The generated `pkg/pmcp_wasm.d.ts` file provides full TypeScript definitions:

```typescript
export class WasmClient {
    constructor(url: string);
    connect(): Promise<any>;
    disconnect(): Promise<void>;
    list_tools(): Promise<any>;
    call_tool(name: string, args: any): Promise<any>;
    list_resources(): Promise<any>;
    read_resource(uri: string): Promise<any>;
    list_prompts(): Promise<any>;
    get_prompt(name: string, arguments: any): Promise<any>;
}
```

## Advanced Features

### Custom Transport Configuration

```rust
// In lib.rs
let config = WasmWebSocketConfig {
    url: "wss://example.com/mcp".to_string(),
    auto_reconnect: true,
    max_reconnect_attempts: 10,
    reconnect_delay_ms: 2000,
};
```

### Error Handling

```javascript
try {
    await client.connect();
} catch (error) {
    console.error("Connection failed:", error);
}
```

### Progress Notifications

```javascript
// Subscribe to progress events
client.on_progress((progress) => {
    console.log(`Progress: ${progress.percentage}% - ${progress.message}`);
});
```

## Browser Compatibility

- ✅ Chrome/Edge 90+
- ✅ Firefox 89+
- ✅ Safari 15+
- ⚠️ Requires WebAssembly and WebSocket support

## Performance Tips

1. **Use production builds**: Add `--release` flag for optimized builds
2. **Enable compression**: Use WebSocket compression when available
3. **Batch operations**: Group multiple tool calls when possible
4. **Cache results**: Store frequently accessed resources locally

## Troubleshooting

### CORS Issues

If you encounter CORS errors, ensure your MCP server includes proper headers:
```
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: GET, POST
Access-Control-Allow-Headers: Content-Type
```

### WebSocket Connection Failed

1. Check the server is running and accessible
2. Verify the WebSocket URL is correct
3. Check browser console for detailed error messages
4. Ensure no firewall/proxy is blocking WebSocket connections

### WASM Loading Issues

1. Ensure the web server serves `.wasm` files with correct MIME type: `application/wasm`
2. Check that all files in `pkg/` are accessible
3. Verify the import path in your HTML/JS is correct

## Production Deployment

For production use:

1. Build with optimizations:
```bash
wasm-pack build --target web --out-dir pkg --release
```

2. Use a CDN for static assets
3. Enable gzip/brotli compression
4. Consider using a WebSocket proxy for SSL termination
5. Implement proper error handling and retry logic

## License

MIT - See LICENSE file in the root directory