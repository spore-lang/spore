// Spore standard library — math module
fn abs(x: I64) -> I64
properties {
    positive(): abs(5) == 5
    negative(): abs(0 - 5) == 5
    zero(): abs(0) == 0
    non_negative_identity(x: I64 when self >= 0): abs(x) == x
}
{
    if x < 0 { 0 - x } else { x }
}

fn negate(x: I64) -> I64
properties {
    zero(): negate(0) == 0
    positive(): negate(5) == 0 - 5
}
{ 0 - x }

fn sign(x: I64) -> I64
properties {
    positive(): sign(5) == 1
    negative(): sign(0 - 3) == 0 - 1
    zero(): sign(0) == 0
}
{
    if x > 0 { 1 } else {
        if x < 0 { 0 - 1 } else { 0 }
    }
}

fn min(a: I64, b: I64) -> I64
properties {
    first_smaller(): min(3, 7) == 3
    second_smaller(): min(7, 3) == 3
    equal(): min(5, 5) == 5
}
{
    if a < b { a } else { b }
}

fn max(a: I64, b: I64) -> I64
properties {
    first_larger(): max(7, 3) == 7
    second_larger(): max(3, 7) == 7
    equal(): max(5, 5) == 5
}
{
    if a > b { a } else { b }
}

fn clamp(x: I64, lo: I64, hi: I64) -> I64
properties {
    in_range(): clamp(5, 1, 10) == 5
    below(): clamp(0, 1, 10) == 1
    above(): clamp(15, 1, 10) == 10
}
{ min(max(x, lo), hi) }

fn is_even(n: I64) -> Bool
properties {
    even(): is_even(4) == true
    odd(): is_even(3) == false
    zero(): is_even(0) == true
}
{ n % 2 == 0 }

fn is_odd(n: I64) -> Bool
properties {
    odd(): is_odd(3) == true
    even(): is_odd(4) == false
}
{ n % 2 != 0 }

fn is_positive(n: I64) -> Bool
properties {
    positive(): is_positive(5) == true
    zero(): is_positive(0) == false
    negative(): is_positive(0 - 1) == false
}
{ n > 0 }

fn is_negative(n: I64) -> Bool
properties {
    negative(): is_negative(0 - 1) == true
    zero(): is_negative(0) == false
    positive(): is_negative(5) == false
}
{ n < 0 }

fn is_zero(n: I64) -> Bool
properties {
    zero(): is_zero(0) == true
    nonzero(): is_zero(5) == false
}
{ n == 0 }

fn pow(base: I64, exp: I64) -> I64
properties {
    zero_exp(): pow(2, 0) == 1
    basic(): pow(2, 10) == 1024
    cubed(): pow(3, 3) == 27
}
{
    if exp <= 0 { 1 } else { base * pow(base, exp - 1) }
}

fn gcd(a: I64, b: I64) -> I64
properties {
    basic(): gcd(12, 8) == 4
    coprime(): gcd(7, 13) == 1
    same(): gcd(6, 6) == 6
    zero(): gcd(5, 0) == 5
}
{
    let x = abs(a);
    let y = abs(b);
    if y == 0 { x } else { gcd(y, x % y) }
}

fn lcm(a: I64, b: I64) -> I64
properties {
    basic(): lcm(4, 6) == 12
    coprime(): lcm(3, 5) == 15
    same(): lcm(7, 7) == 7
}
{
    let d = gcd(a, b);
    if d == 0 { 0 } else { abs(a * b) / d }
}

// NOTE: only correct for non-negative a and positive b.
// For negative dividends, Spore's truncation-toward-zero means a/b+1
// would over-count. A future version may add a signed variant.
fn div_ceil(a: I64, b: I64) -> I64
properties {
    exact(): div_ceil(10, 5) == 2
    remainder(): div_ceil(7, 3) == 3
    one(): div_ceil(1, 3) == 1
}
{
    if a % b == 0 { a / b } else { a / b + 1 }
}

fn sum_list(xs: List[I64]) -> I64
properties {
    basic(): sum_list([1, 2, 3]) == 6
    empty(): sum_list([]) == 0
    single(): sum_list([42]) == 42
}
{ fold(xs, 0, |acc: I64, x: I64| acc + x) }

fn product_list(xs: List[I64]) -> I64
properties {
    basic(): product_list([2, 3, 4]) == 24
    empty(): product_list([]) == 1
    single(): product_list([7]) == 7
}
{ fold(xs, 1, |acc: I64, x: I64| acc * x) }

fn checked_div(a: I64, b: I64) -> Option[I64] {
    if b == 0 { None } else { Some(a / b) }
}
