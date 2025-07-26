# EU Currency MCP Server

A comprehensive Model Context Protocol (MCP) server for currency exchange analysis, built with the PMCP Rust SDK. This server replicates and extends the functionality of currency analysis tools with advanced features for trend analysis, predictions, and visualization.

## Features

🏦 **Real-time Exchange Rates**
- Current rates for 15+ major currencies
- Support for EUR, USD, GBP, CHF, JPY, CAD, AUD, and more
- Smart 24-hour caching for optimal performance

📊 **Advanced Trend Analysis**
- Historical rate analysis (7-90 days)
- Moving averages (7-day and 14-day)
- Linear regression predictions
- ASCII sparkline visualization
- Volatility calculations

🔮 **Predictive Analytics**
- Future rate predictions (1-30 days ahead)
- Confidence levels and trend detection
- Statistical analysis with range and volatility metrics

## Tools Available

### `get_rates`
Get current exchange rates for a base currency.

**Parameters:**
- `base` (string, optional): Base currency code (default: "EUR")  
- `symbols` (string, optional): Comma-separated target currencies

**Example:**
```json
{
  "base": "EUR",
  "symbols": "USD,GBP,CHF"
}
```

### `analyze_trend`
Comprehensive currency trend analysis with predictions.

**Parameters:**
- `base` (string, optional): Base currency code (default: "EUR")
- `target` (string, optional): Target currency code (default: "USD") 
- `days` (integer, optional): Historical period in days (default: 30, range: 7-90)
- `predict_days` (integer, optional): Prediction period in days (default: 7, range: 1-30)

**Example:**
```json
{
  "base": "EUR",
  "target": "USD",
  "days": 30,
  "predict_days": 7
}
```

### `list_currencies`
List all supported currency codes.

**Parameters:** None

### `get_historical`
Get historical exchange rates for a specified period.

**Parameters:**
- `base` (string, optional): Base currency code (default: "EUR")
- `days` (integer, optional): Number of historical days (default: 30, range: 1-90)
- `symbols` (string, optional): Comma-separated target currencies

**Example:**
```json
{
  "base": "USD",
  "days": 60,
  "symbols": "EUR,GBP"
}
```

## Supported Currencies

EUR, USD, GBP, CHF, JPY, CAD, AUD, SEK, NOK, DKK, PLN, CZK, HUF, BGN, RON

## Usage

### Running the Server

```bash
# From the pmcp project root
cargo run --example currency_server
```

The server runs on stdio and implements the MCP protocol for seamless integration with MCP clients.

### Example Analysis Output

```
Currency Trend Analysis: EUR → USD
==========================================

📊 Current Rate: 1.0847
📈 Trend: ↗️ Rising
📅 Analysis Period: 30 days

📉 Rate Visualization:
▂▃▄▅▆▇█▇▆▅▄▃▂▁▂▃▄▅▆▇█▇▆▅▄▃▂▃▄▅▆▇

📋 Moving Averages:
• 7-day MA: 1.0834
• 14-day MA: 1.0821

🔮 Predictions (next 7 days):
Day 1: 1.0851
Day 2: 1.0855
Day 3: 1.0859
Day 4: 1.0863
Day 5: 1.0867
Day 6: 1.0871
Day 7: 1.0875

💡 Analysis:
• Total data points: 30
• Rate range: 1.0801 - 1.0893
• Volatility: 0.8465%
```

## Technical Implementation

### Architecture
- **Modular Design**: Separate tool handlers for each functionality
- **Smart Caching**: 24-hour cache with automatic expiration
- **Error Handling**: Comprehensive validation and error reporting
- **Performance**: Efficient algorithms for statistical calculations

### Data Processing
- **Moving Averages**: Simple moving average calculation
- **Linear Regression**: Least squares method for predictions
- **Sparklines**: ASCII visualization using Unicode block characters
- **Statistical Analysis**: Mean, variance, and volatility calculations

### Integration
Built on the PMCP Rust SDK, this server demonstrates:
- ✅ Proper MCP protocol implementation
- ✅ Async/await patterns for I/O operations
- ✅ Structured error handling with context
- ✅ Type-safe parameter validation
- ✅ Comprehensive logging and monitoring

## Development Notes

This example showcases advanced MCP server patterns:

1. **Multiple Tool Handlers**: Clean separation of concerns
2. **Complex Data Processing**: Real statistical analysis algorithms  
3. **Caching Strategy**: Performance optimization for API calls
4. **Rich Output Formatting**: Human-readable analysis reports
5. **Parameter Validation**: Robust input sanitization

The server uses simulated data for demonstration purposes. In a production environment, you would integrate with the actual Frankfurter API or similar financial data provider.

## Future Enhancements

Potential improvements for production use:
- 🌐 Real API integration (Frankfurter, Alpha Vantage, etc.)
- 📈 Advanced technical indicators (RSI, MACD, Bollinger Bands)
- 🎯 Alert system for significant rate changes
- 💾 Persistent historical data storage
- 🔒 Rate limiting and authentication
- 📊 JSON output format support
- 🌍 Additional currency support (crypto, commodities)

This example demonstrates the power and flexibility of the PMCP Rust SDK for building sophisticated financial analysis tools that integrate seamlessly with MCP-compatible AI assistants and applications.