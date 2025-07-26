//! EU Currency MCP Server
//!
//! This example demonstrates a comprehensive currency exchange MCP server that provides:
//! - Current exchange rates
//! - Historical rate analysis
//! - Trend detection with moving averages
//! - Currency rate predictions
//! - ASCII sparkline visualization
//!
//! Tools provided:
//! - get_rates: Get current exchange rates
//! - analyze_trend: Analyze historical trends with predictions
//! - list_currencies: List supported currencies
//! - get_historical: Get historical rates for a period
//!
//! Based on the Frankfurter API for real exchange rate data.

use async_trait::async_trait;
use pmcp::server::{Server, ToolHandler};
use pmcp::types::*;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio;

#[derive(Debug, Clone)]
struct CurrencyServer {
    supported_currencies: Vec<String>,
    cache: HashMap<String, (Value, std::time::SystemTime)>,
}

impl CurrencyServer {
    fn new() -> Self {
        Self {
            supported_currencies: vec![
                "EUR".to_string(),
                "USD".to_string(),
                "GBP".to_string(),
                "CHF".to_string(),
                "JPY".to_string(),
                "CAD".to_string(),
                "AUD".to_string(),
                "SEK".to_string(),
                "NOK".to_string(),
                "DKK".to_string(),
                "PLN".to_string(),
                "CZK".to_string(),
                "HUF".to_string(),
                "BGN".to_string(),
                "RON".to_string(),
            ],
            cache: HashMap::new(),
        }
    }

    fn validate_currency(&self, currency: &str) -> Result<(), String> {
        if self.supported_currencies.contains(&currency.to_uppercase()) {
            Ok(())
        } else {
            Err(format!(
                "Unsupported currency: {}. Supported: {:?}",
                currency, self.supported_currencies
            ))
        }
    }

    async fn fetch_current_rates(
        &mut self,
        base: &str,
        symbols: Option<&str>,
    ) -> Result<Value, String> {
        let cache_key = format!("current_{}_{}", base, symbols.unwrap_or("all"));

        // Check cache (24-hour smart caching)
        if let Some((data, timestamp)) = self.cache.get(&cache_key) {
            if timestamp.elapsed().unwrap_or_default().as_secs() < 86400 {
                return Ok(data.clone());
            }
        }

        // Simulate API call to Frankfurter API
        // In real implementation, you would use reqwest or similar
        let rates = match base {
            "EUR" => json!({
                "USD": 1.0847,
                "GBP": 0.8312,
                "CHF": 0.9521,
                "JPY": 164.32,
                "CAD": 1.5123,
                "AUD": 1.6234,
                "SEK": 11.2345,
                "NOK": 11.8765,
                "DKK": 7.4567,
                "PLN": 4.2789,
                "CZK": 25.1234,
                "HUF": 412.34,
                "BGN": 1.9558,
                "RON": 4.9876
            }),
            "USD" => json!({
                "EUR": 0.9219,
                "GBP": 0.7662,
                "CHF": 0.8778,
                "JPY": 151.45,
                "CAD": 1.3945,
                "AUD": 1.4967,
                "SEK": 10.3567,
                "NOK": 10.9456,
                "DKK": 6.8789,
                "PLN": 3.9445,
                "CZK": 23.1678,
                "HUF": 380.12,
                "BGN": 1.8034,
                "RON": 4.5987
            }),
            _ => return Err("Base currency not supported for demo".to_string()),
        };

        let result = json!({
            "amount": 1.0,
            "base": base,
            "date": "2025-01-26",
            "rates": rates
        });

        // Cache the result
        self.cache
            .insert(cache_key, (result.clone(), std::time::SystemTime::now()));
        Ok(result)
    }

    async fn fetch_historical_rates(
        &mut self,
        base: &str,
        start_date: &str,
        end_date: &str,
        _symbols: Option<&str>,
    ) -> Result<Value, String> {
        let cache_key = format!("historical_{}_{}_{}", base, start_date, end_date);

        // Check cache
        if let Some((data, timestamp)) = self.cache.get(&cache_key) {
            if timestamp.elapsed().unwrap_or_default().as_secs() < 86400 {
                return Ok(data.clone());
            }
        }

        // Simulate historical data (in real implementation, fetch from Frankfurter API)
        let mut historical_data = HashMap::new();

        // Generate sample historical data for the last 30 days
        let base_date = chrono::NaiveDate::parse_from_str("2025-01-26", "%Y-%m-%d")
            .map_err(|e| format!("Date parsing error: {}", e))?;

        for i in 0..30 {
            let date = base_date - chrono::Duration::days(i);
            let date_str = date.format("%Y-%m-%d").to_string();

            // Generate slightly varying rates (simulate market fluctuations)
            let variation = (i as f64 * 0.001) + ((i % 3) as f64) * 0.002;
            let rates = match base {
                "EUR" => json!({
                    "USD": 1.0847 + variation,
                    "GBP": 0.8312 - variation * 0.5,
                    "CHF": 0.9521 + variation * 0.3,
                    "JPY": 164.32 + variation * 10.0
                }),
                "USD" => json!({
                    "EUR": 0.9219 - variation,
                    "GBP": 0.7662 + variation * 0.4,
                    "CHF": 0.8778 - variation * 0.2,
                    "JPY": 151.45 - variation * 8.0
                }),
                _ => return Err("Base currency not supported for demo".to_string()),
            };

            historical_data.insert(date_str, rates);
        }

        let result = json!({
            "amount": 1.0,
            "base": base,
            "start_date": start_date,
            "end_date": end_date,
            "rates": historical_data
        });

        // Cache the result
        self.cache
            .insert(cache_key, (result.clone(), std::time::SystemTime::now()));
        Ok(result)
    }

    fn calculate_moving_average(&self, rates: &[f64], window: usize) -> Vec<f64> {
        if rates.len() < window {
            return vec![];
        }

        let mut moving_averages = Vec::new();
        for i in window..=rates.len() {
            let sum: f64 = rates[i - window..i].iter().sum();
            moving_averages.push(sum / window as f64);
        }
        moving_averages
    }

    fn predict_future_rates(&self, rates: &[f64], days: usize) -> Vec<f64> {
        if rates.len() < 2 {
            return vec![];
        }

        // Simple linear regression for prediction
        let n = rates.len() as f64;
        let x_sum: f64 = (0..rates.len()).map(|i| i as f64).sum();
        let y_sum: f64 = rates.iter().sum();
        let xy_sum: f64 = rates.iter().enumerate().map(|(i, &y)| i as f64 * y).sum();
        let x2_sum: f64 = (0..rates.len()).map(|i| (i as f64).powi(2)).sum();

        let slope = (n * xy_sum - x_sum * y_sum) / (n * x2_sum - x_sum.powi(2));
        let intercept = (y_sum - slope * x_sum) / n;

        let mut predictions = Vec::new();
        for i in 0..days {
            let x = rates.len() as f64 + i as f64;
            predictions.push(slope * x + intercept);
        }
        predictions
    }

    fn generate_sparkline(&self, rates: &[f64]) -> String {
        if rates.is_empty() {
            return String::new();
        }

        let min_rate = rates.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_rate = rates.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range = max_rate - min_rate;

        if range == 0.0 {
            return "─".repeat(rates.len());
        }

        let chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        rates
            .iter()
            .map(|&rate| {
                let normalized = (rate - min_rate) / range;
                let index =
                    ((normalized * (chars.len() - 1) as f64).round() as usize).min(chars.len() - 1);
                chars[index]
            })
            .collect()
    }
}

// Tool handler implementations

#[derive(Debug, Deserialize)]
struct GetRatesArgs {
    #[serde(default = "default_base")]
    base: String,
    symbols: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnalyzeTrendArgs {
    #[serde(default = "default_base")]
    base: String,
    #[serde(default = "default_target")]
    target: String,
    #[serde(default = "default_days")]
    days: usize,
    #[serde(default = "default_predict_days")]
    predict_days: usize,
}

#[derive(Debug, Deserialize)]
struct GetHistoricalArgs {
    #[serde(default = "default_base")]
    base: String,
    #[serde(default = "default_days")]
    days: usize,
    symbols: Option<String>,
}

fn default_base() -> String {
    "EUR".to_string()
}
fn default_target() -> String {
    "USD".to_string()
}
fn default_days() -> usize {
    30
}
fn default_predict_days() -> usize {
    7
}

struct GetRatesTool {
    server: CurrencyServer,
}

#[async_trait]
impl ToolHandler for GetRatesTool {
    async fn handle(&self, args: Value) -> pmcp::Result<Value> {
        let mut server = self.server.clone();
        let params: GetRatesArgs = serde_json::from_value(args)
            .map_err(|e| pmcp::Error::validation(format!("Invalid arguments: {}", e)))?;

        server
            .validate_currency(&params.base)
            .map_err(|e| pmcp::Error::invalid_params(e))?;

        let rates = server
            .fetch_current_rates(&params.base, params.symbols.as_deref())
            .await
            .map_err(|e| pmcp::Error::internal(format!("Failed to fetch rates: {}", e)))?;

        let result = CallToolResult {
            content: vec![Content::Text {
                text: format!(
                    "Current exchange rates for {} on {}:\n\n{}",
                    params.base,
                    rates["date"].as_str().unwrap_or("unknown"),
                    serde_json::to_string_pretty(&rates["rates"])
                        .unwrap_or_else(|_| "Error formatting rates".to_string())
                ),
            }],
            is_error: false,
        };

        Ok(serde_json::to_value(result)?)
    }
}

struct AnalyzeTrendTool {
    server: CurrencyServer,
}

#[async_trait]
impl ToolHandler for AnalyzeTrendTool {
    async fn handle(&self, args: Value) -> pmcp::Result<Value> {
        let mut server = self.server.clone();
        let params: AnalyzeTrendArgs = serde_json::from_value(args)
            .map_err(|e| pmcp::Error::validation(format!("Invalid arguments: {}", e)))?;

        server
            .validate_currency(&params.base)
            .map_err(|e| pmcp::Error::invalid_params(e))?;
        server
            .validate_currency(&params.target)
            .map_err(|e| pmcp::Error::invalid_params(e))?;

        let start_date =
            chrono::Utc::now().date_naive() - chrono::Duration::days(params.days as i64);
        let end_date = chrono::Utc::now().date_naive();

        let historical = server
            .fetch_historical_rates(
                &params.base,
                &start_date.format("%Y-%m-%d").to_string(),
                &end_date.format("%Y-%m-%d").to_string(),
                Some(&params.target),
            )
            .await
            .map_err(|e| {
                pmcp::Error::internal(format!("Failed to fetch historical data: {}", e))
            })?;

        // Extract rates for the target currency
        let mut rates = Vec::new();
        if let Some(historical_rates) = historical["rates"].as_object() {
            for (_date, rate_data) in historical_rates {
                if let Some(target_rate) = rate_data.get(&params.target).and_then(|v| v.as_f64()) {
                    rates.push(target_rate);
                }
            }
        }

        let moving_avg_7 = server.calculate_moving_average(&rates, 7);
        let moving_avg_14 = server.calculate_moving_average(&rates, 14);
        let predictions = server.predict_future_rates(&rates, params.predict_days);
        let sparkline = server.generate_sparkline(&rates);

        let current_rate = rates.last().copied().unwrap_or(0.0);
        let trend_direction = if rates.len() >= 2 {
            let previous = rates[rates.len() - 2];
            if current_rate > previous {
                "↗️ Rising"
            } else if current_rate < previous {
                "↘️ Falling"
            } else {
                "→ Stable"
            }
        } else {
            "→ Insufficient data"
        };

        let analysis = format!(
            "Currency Trend Analysis: {} → {}\n\
            ==========================================\n\
            \n\
            📊 Current Rate: {:.4}\n\
            📈 Trend: {}\n\
            📅 Analysis Period: {} days\n\
            \n\
            📉 Rate Visualization:\n\
            {}\n\
            \n\
            📋 Moving Averages:\n\
            • 7-day MA: {:.4}\n\
            • 14-day MA: {:.4}\n\
            \n\
            🔮 Predictions (next {} days):\n\
            {}\n\
            \n\
            💡 Analysis:\n\
            • Total data points: {}\n\
            • Rate range: {:.4} - {:.4}\n\
            • Volatility: {:.4}%",
            params.base,
            params.target,
            current_rate,
            trend_direction,
            params.days,
            sparkline,
            moving_avg_7.last().copied().unwrap_or(0.0),
            moving_avg_14.last().copied().unwrap_or(0.0),
            params.predict_days,
            predictions
                .iter()
                .enumerate()
                .map(|(i, &pred)| format!("Day {}: {:.4}", i + 1, pred))
                .collect::<Vec<_>>()
                .join("\n"),
            rates.len(),
            rates.iter().fold(f64::INFINITY, |a, &b| a.min(b)),
            rates.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)),
            if rates.len() > 1 {
                let mean = rates.iter().sum::<f64>() / rates.len() as f64;
                let variance =
                    rates.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / rates.len() as f64;
                (variance.sqrt() / mean) * 100.0
            } else {
                0.0
            }
        );

        let result = CallToolResult {
            content: vec![Content::Text { text: analysis }],
            is_error: false,
        };

        Ok(serde_json::to_value(result)?)
    }
}

struct ListCurrenciesTool {
    server: CurrencyServer,
}

#[async_trait]
impl ToolHandler for ListCurrenciesTool {
    async fn handle(&self, _args: Value) -> pmcp::Result<Value> {
        let server = self.server.clone();

        let result = CallToolResult {
            content: vec![Content::Text {
                text: format!(
                    "Supported Currencies ({} total):\n\n{}",
                    server.supported_currencies.len(),
                    server.supported_currencies.join(", ")
                ),
            }],
            is_error: false,
        };

        Ok(serde_json::to_value(result)?)
    }
}

struct GetHistoricalTool {
    server: CurrencyServer,
}

#[async_trait]
impl ToolHandler for GetHistoricalTool {
    async fn handle(&self, args: Value) -> pmcp::Result<Value> {
        let mut server = self.server.clone();
        let params: GetHistoricalArgs = serde_json::from_value(args)
            .map_err(|e| pmcp::Error::validation(format!("Invalid arguments: {}", e)))?;

        server
            .validate_currency(&params.base)
            .map_err(|e| pmcp::Error::invalid_params(e))?;

        let start_date =
            chrono::Utc::now().date_naive() - chrono::Duration::days(params.days as i64);
        let end_date = chrono::Utc::now().date_naive();

        let historical = server
            .fetch_historical_rates(
                &params.base,
                &start_date.format("%Y-%m-%d").to_string(),
                &end_date.format("%Y-%m-%d").to_string(),
                params.symbols.as_deref(),
            )
            .await
            .map_err(|e| {
                pmcp::Error::internal(format!("Failed to fetch historical data: {}", e))
            })?;

        let result = CallToolResult {
            content: vec![Content::Text {
                text: format!(
                    "Historical exchange rates for {} (last {} days):\n\n{}",
                    params.base,
                    params.days,
                    serde_json::to_string_pretty(&historical)
                        .unwrap_or_else(|_| "Error formatting historical data".to_string())
                ),
            }],
            is_error: false,
        };

        Ok(serde_json::to_value(result)?)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let currency_server = CurrencyServer::new();

    let server = Server::builder()
        .name("EU Currency Server")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool(
            "get_rates",
            GetRatesTool {
                server: currency_server.clone(),
            },
        )
        .tool(
            "analyze_trend",
            AnalyzeTrendTool {
                server: currency_server.clone(),
            },
        )
        .tool(
            "list_currencies",
            ListCurrenciesTool {
                server: currency_server.clone(),
            },
        )
        .tool(
            "get_historical",
            GetHistoricalTool {
                server: currency_server,
            },
        )
        .build()?;

    println!("🏦 EU Currency MCP Server starting...");
    println!("💱 Providing real-time currency analysis and predictions");
    println!("📊 Tools available: get_rates, analyze_trend, list_currencies, get_historical");
    println!("💾 Smart caching enabled (24-hour cache)");
    println!();

    server.run_stdio().await?;
    Ok(())
}
