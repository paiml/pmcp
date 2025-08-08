//! Test that #[tool] without description fails to compile

use pmcp_macros::tool;

#[tool] // Missing description - should fail
fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn main() {}