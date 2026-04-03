// Spore standard library — math module

fn abs(x: Int) -> Int cost <= 2 { if x < 0 { 0 - x } else { x } }
fn negate(x: Int) -> Int cost <= 1 { 0 - x }
fn sign(x: Int) -> Int cost <= 2 { if x > 0 { 1 } else { if x < 0 { 0 - 1 } else { 0 } } }

fn min(a: Int, b: Int) -> Int cost <= 2 { if a < b { a } else { b } }
fn max(a: Int, b: Int) -> Int cost <= 2 { if a > b { a } else { b } }
fn clamp(x: Int, lo: Int, hi: Int) -> Int cost <= 5 { min(max(x, lo), hi) }

fn is_even(n: Int) -> Bool cost <= 2 { n % 2 == 0 }
fn is_odd(n: Int) -> Bool cost <= 2 { n % 2 != 0 }
fn is_positive(n: Int) -> Bool cost <= 1 { n > 0 }
fn is_negative(n: Int) -> Bool cost <= 1 { n < 0 }
fn is_zero(n: Int) -> Bool cost <= 1 { n == 0 }

fn pow(base: Int, exp: Int) -> Int cost <= 64 {
    if exp <= 0 { 1 }
    else { base * pow(base, exp - 1) }
}

fn gcd(a: Int, b: Int) -> Int cost <= 128 {
    let x = abs(a);
    let y = abs(b);
    if y == 0 { x }
    else { gcd(y, x % y) }
}

fn lcm(a: Int, b: Int) -> Int cost <= 130 {
    let d = gcd(a, b);
    if d == 0 { 0 }
    else { abs(a * b) / d }
}

fn div_ceil(a: Int, b: Int) -> Int cost <= 3 {
    if a % b == 0 { a / b }
    else { a / b + 1 }
}

fn sum_list(xs: List[Int]) -> Int cost <= 1000 {
    fold(xs, 0, |acc: Int, x: Int| acc + x)
}

fn product_list(xs: List[Int]) -> Int cost <= 1000 {
    fold(xs, 1, |acc: Int, x: Int| acc * x)
}

fn checked_div(a: Int, b: Int) -> Option[Int] cost <= 2 {
    if b == 0 { None } else { Some(a / b) }
}
