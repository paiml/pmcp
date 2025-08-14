# PMCP Macros

Procedural macros for the PMCP (Production MCP) SDK, providing ergonomic tool and handler definitions with automatic schema generation.

## Features

- 🔧 **`#[tool]`** - Define individual tools with automatic schema generation
- 🚀 **`#[tool_router]`** - Collect tools from impl blocks for easy registration
- 📝 Type-safe parameter handling with compile-time validation
- 🔄 Automatic JSON schema generation from Rust types
- ⚡ Zero runtime overhead - all code generation happens at compile time

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
pmcp = { version = "1.1", features = ["macros"] }
serde = { version = "1.0", features = ["derive"] }
schemars = "1.0"
```

## Usage

### Basic Tool Definition

```rust
use pmcp_macros::{tool, tool_router};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[derive(Debug, Deserialize, JsonSchema)]
struct AddParams {
    a: i32,
    b: i32,
}

#[derive(Debug, Serialize, JsonSchema)]
struct AddResult {
    sum: i32,
}

#[tool(description = "Add two numbers")]
async fn add(params: AddParams) -> Result<AddResult, String> {
    Ok(AddResult {
        sum: params.a + params.b,
    })
}
```

### Tool Router for Multiple Tools

```rust
#[derive(Clone)]
struct Calculator;

#[tool_router]
impl Calculator {
    #[tool(description = "Add two numbers")]
    async fn add(&self, a: i32, b: i32) -> Result<i32, String> {
        Ok(a + b)
    }
    
    #[tool(description = "Multiply two numbers")]
    async fn multiply(&self, a: i32, b: i32) -> Result<i32, String> {
        Ok(a * b)
    }
    
    #[tool(name = "div", description = "Divide two numbers")]
    async fn divide(&self, a: f64, b: f64) -> Result<f64, String> {
        if b == 0.0 {
            Err("Division by zero".to_string())
        } else {
            Ok(a / b)
        }
    }
}
```

### Advanced Features

#### Optional Parameters

```rust
#[derive(Debug, Deserialize, JsonSchema)]
struct GreetParams {
    name: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    formal: bool,
}

#[tool(description = "Greet a person")]
fn greet(params: GreetParams) -> String {
    match (params.formal, params.title) {
        (true, Some(title)) => format!("Good day, {} {}!", title, params.name),
        (true, None) => format!("Good day, {}!", params.name),
        (false, _) => format!("Hey, {}!", params.name),
    }
}
```

#### Complex Types

```rust
#[derive(Debug, Deserialize, JsonSchema)]
struct ProcessRequest {
    items: Vec<String>,
    metadata: HashMap<String, Value>,
    config: ProcessConfig,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ProcessConfig {
    timeout_ms: u64,
    retry_count: u8,
}

#[tool(description = "Process complex data")]
async fn process(req: ProcessRequest) -> Result<Value, Error> {
    // Processing logic here
    Ok(json!({
        "processed": req.items.len(),
        "status": "success"
    }))
}
```

## Attributes

### `#[tool]` Attributes

- `name` - Custom tool name (defaults to function name)
- `description` - Tool description (required)
- `annotations` - Additional metadata (optional)

```rust
#[tool(
    name = "custom_name",
    description = "Tool description",
    annotations(
        category = "math",
        complexity = "simple",
        read_only = true
    )
)]
```

### `#[tool_router]` Attributes

- `router` - Name of the router field (defaults to "tool_router")
- `vis` - Visibility of generated methods (defaults to `pub`)

```rust
#[tool_router(router = "my_router", vis = "pub(crate)")]
impl MyServer {
    // tools...
}
```

## Schema Generation

The macros automatically generate JSON schemas for your tool parameters using the `schemars` crate. You can customize the generated schemas using schemars attributes:

```rust
#[derive(Deserialize, JsonSchema)]
struct Params {
    #[schemars(description = "User's age in years")]
    #[schemars(range(min = 0, max = 150))]
    age: u8,
    
    #[schemars(regex(pattern = r"^\w+@\w+\.\w+$"))]
    email: String,
    
    #[schemars(length(min = 8, max = 128))]
    password: String,
}
```

## Error Handling

Tools can return `Result<T, E>` where `E` implements `ToString`:

```rust
#[tool(description = "Divide two numbers")]
fn divide(a: f64, b: f64) -> Result<f64, MyError> {
    if b == 0.0 {
        Err(MyError::DivisionByZero)
    } else {
        Ok(a / b)
    }
}

#[derive(Debug)]
enum MyError {
    DivisionByZero,
    Overflow,
}

impl ToString for MyError {
    fn to_string(&self) -> String {
        match self {
            Self::DivisionByZero => "Cannot divide by zero".to_string(),
            Self::Overflow => "Operation would overflow".to_string(),
        }
    }
}
```

## Testing

The macro-generated code can be tested like regular Rust code:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_add_tool() {
        let params = AddParams { a: 5, b: 3 };
        let result = add(params).await.unwrap();
        assert_eq!(result.sum, 8);
    }
}
```

## Performance

- Zero runtime overhead - all code generation happens at compile time
- Automatic schema caching for repeated tool calls
- Efficient parameter parsing with serde
- No reflection or runtime type information needed

## Limitations

- Currently only supports tools (prompts and resources coming soon)
- Requires `schemars` for schema generation
- Async tools require `tokio` runtime

## Future Plans

- `#[prompt]` macro for prompt templates
- `#[resource]` macro for resource handlers
- Custom validation attributes
- Automatic OpenAPI spec generation
- Integration with popular web frameworks

## License

MIT - See parent project for details