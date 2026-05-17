// Spore standard library — math module
fn abs(x: I64) -> I64 cost [2, 0, 0, 0]
spec {
    example "positive": abs(5) == 5
    example "negative": abs(0 - 5) == 5
    example "zero": abs(0) == 0
    property "non_negative_identity": |x: I64 when self >= 0| x
}
{
    if x < 0 { 0 - x } else { x }
}

fn negate(x: I64) -> I64 cost [1, 0, 0, 0]
spec {
    example "zero": negate(0) == 0
    example "positive": negate(5) == 0 - 5
}
{ 0 - x }

fn sign(x: I64) -> I64 cost [3, 0, 0, 0]
spec {
    example "positive": sign(5) == 1
    example "negative": sign(0 - 3) == 0 - 1
    example "zero": sign(0) == 0
}
{
    if x > 0 { 1 } else {
        if x < 0 { 0 - 1 } else { 0 }
    }
}

fn min(a: I64, b: I64) -> I64 cost [2, 0, 0, 0]
spec {
    example "first_smaller": min(3, 7) == 3
    example "second_smaller": min(7, 3) == 3
    example "equal": min(5, 5) == 5
}
{
    if a < b { a } else { b }
}

fn max(a: I64, b: I64) -> I64 cost [2, 0, 0, 0]
spec {
    example "first_larger": max(7, 3) == 7
    example "second_larger": max(3, 7) == 7
    example "equal": max(5, 5) == 5
}
{
    if a > b { a } else { b }
}

fn clamp(x: I64, lo: I64, hi: I64) -> I64 cost [8, 0, 0, 0]
spec {
    example "in_range": clamp(5, 1, 10) == 5
    example "below": clamp(0, 1, 10) == 1
    example "above": clamp(15, 1, 10) == 10
}
{ min(max(x, lo), hi) }

fn is_even(n: I64) -> Bool cost [3, 0, 0, 0]
spec {
    example "even": is_even(4) == true
    example "odd": is_even(3) == false
    example "zero": is_even(0) == true
}
{ n % 2 == 0 }

fn is_odd(n: I64) -> Bool cost [3, 0, 0, 0]
spec {
    example "odd": is_odd(3) == true
    example "even": is_odd(4) == false
}
{ n % 2 != 0 }

fn is_positive(n: I64) -> Bool cost [1, 0, 0, 0]
spec {
    example "positive": is_positive(5) == true
    example "zero": is_positive(0) == false
    example "negative": is_positive(0 - 1) == false
}
{ n > 0 }

fn is_negative(n: I64) -> Bool cost [1, 0, 0, 0]
spec {
    example "negative": is_negative(0 - 1) == true
    example "zero": is_negative(0) == false
    example "positive": is_negative(5) == false
}
{ n < 0 }

fn is_zero(n: I64) -> Bool cost [1, 0, 0, 0]
spec {
    example "zero": is_zero(0) == true
    example "nonzero": is_zero(5) == false
}
{ n == 0 }

fn pow(base: I64, exp: I64) -> I64 cost [64, 0, 0, 0]
spec {
    example "zero_exp": pow(2, 0) == 1
    example "basic": pow(2, 10) == 1024
    example "cubed": pow(3, 3) == 27
}
{
    if exp <= 0 { 1 } else { base * pow(base, exp - 1) }
}

fn gcd(a: I64, b: I64) -> I64 cost [128, 0, 0, 0]
spec {
    example "basic": gcd(12, 8) == 4
    example "coprime": gcd(7, 13) == 1
    example "same": gcd(6, 6) == 6
    example "zero": gcd(5, 0) == 5
}
{
    let x = abs(a);
    let y = abs(b);
    if y == 0 { x } else { gcd(y, x % y) }
}

fn lcm(a: I64, b: I64) -> I64 cost [130, 0, 0, 0]
spec {
    example "basic": lcm(4, 6) == 12
    example "coprime": lcm(3, 5) == 15
    example "same": lcm(7, 7) == 7
}
{
    let d = gcd(a, b);
    if d == 0 { 0 } else { abs(a * b) / d }
}

// NOTE: only correct for non-negative a and positive b.
// For negative dividends, Spore's truncation-toward-zero means a/b+1
// would over-count. A future version may add a signed variant.
fn div_ceil(a: I64, b: I64) -> I64 cost [6, 0, 0, 0]
spec {
    example "exact": div_ceil(10, 5) == 2
    example "remainder": div_ceil(7, 3) == 3
    example "one": div_ceil(1, 3) == 1
}
{
    if a % b == 0 { a / b } else { a / b + 1 }
}

fn sum_list(xs: List[I64]) -> I64 cost [1000, 2, 0, 0]
spec {
    example "basic": sum_list([1, 2, 3]) == 6
    example "empty": sum_list([]) == 0
    example "single": sum_list([42]) == 42
}
{ fold(xs, 0, |acc: I64, x: I64| acc + x) }

fn product_list(xs: List[I64]) -> I64 cost [1000, 2, 0, 0]
spec {
    example "basic": product_list([2, 3, 4]) == 24
    example "empty": product_list([]) == 1
    example "single": product_list([7]) == 7
}
{ fold(xs, 1, |acc: I64, x: I64| acc * x) }

fn checked_div(a: I64, b: I64) -> Option[I64] cost [6, 0, 0, 0] {
    if b == 0 { None } else { Some(a / b) }
}
