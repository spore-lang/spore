// Spore standard library — math module

fn abs(x: Int) -> Int { if x < 0 { 0 - x } else { x } }
fn min(a: Int, b: Int) -> Int { if a < b { a } else { b } }
fn max(a: Int, b: Int) -> Int { if a > b { a } else { b } }
fn clamp(x: Int, lo: Int, hi: Int) -> Int { min(max(x, lo), hi) }
