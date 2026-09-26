// Standalone file example for quick experimentation
//
// This file demonstrates Spore's core features without requiring a project or Platform.
// Standalone files do not participate in a package-backed Platform contract.
// CLI return values have no default host meaning: they are not printed for you and
// they are not treated as process exit codes.
//
// For production code, prefer creating a project with
// `cargo run --bin spore -- new app-name` from a source checkout (or
// `spore new app-name` if the CLI is installed), which provides Platform-
// handled effects and uses `fn main() -> ()` as the entry point.
//
// Run this file with: cargo run --bin spore -- run examples/demo.sp
// (or `spore run examples/demo.sp` if the CLI is installed)
// Expected output: 204
fn add(a: I64, b: I64) -> I64
properties {
    identity(): add(0, 42) == 42
    basic(): add(20, 22) == 42
    left_identity(b: I64): add(0, b) == b
}
{ a + b }

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

struct Point {
    x: I64,
    y: I64,
}

fn distance_squared(p: Point) -> I64
properties {
    origin(): distance_squared(Point { x: 0, y: 0 }) == 0
    unit(): distance_squared(Point { x: 3, y: 4 }) == 25
}
{ p.x * p.x + p.y * p.y }

fn translate(p: Point, dx: I64, dy: I64) -> Point { Point { x: p.x + dx, y: p.y + dy } }

fn apply(f: (I64) -> I64, x: I64) -> I64 { f(x) }

fn double(x: I64) -> I64
properties {
    zero(): double(0) == 0
    five(): double(5) == 10
}
{ x * 2 }

fn compose(f: (I64) -> I64, g: (I64) -> I64) -> (I64) -> I64 { |x: I64| f(g(x)) }

enum Shape {
    Circle(I64),
    Rect(I64, I64),
}

fn area(s: Shape) -> I64
properties {
    circle(): area(Circle(5)) == 75
    rect(): area(Rect(3, 4)) == 12
}
{
    match s {
        Circle(r) => r * r * 3,
        Rect(w, h) => w * h,
    }
}

fn factorial(n: I64) -> I64
properties {
    base(): factorial(0) == 1
    five(): factorial(5) == 120
}
{
    match n {
        0 => 1,
        _ => n * factorial(n - 1),
    }
}

fn fibonacci(n: I64) -> I64
properties {
    base0(): fibonacci(0) == 0
    base1(): fibonacci(1) == 1
    fib10(): fibonacci(10) == 55
}
{
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

fn is_even(n: I64) -> Bool
properties {
    zero(): is_even(0) == true
    one(): is_even(1) == false
    four(): is_even(4) == true
}
{ n % 2 == 0 }

fn both(a: Bool, b: Bool) -> Bool
properties {
    tt(): both(true, true) == true
    tf(): both(true, false) == false
    ff(): both(false, false) == false
}
{ a && b }

fn greet(name: Str) -> Str
properties {
    world(): greet("world") == "Hello, world!"
}
{ "Hello, " + name + "!" }

fn main() -> () {
    let sum = add(20, 22);
    let p = Point { x: 3, y: 4 };
    let d = distance_squared(p);
    let tripled = apply(|x: I64| x * 3, 14);
    let piped = 10 |> double;
    let c = Circle(5);
    let a = area(c);
    let f5 = factorial(5);
    let fib = fibonacci(10);
    let even = is_even(42);
    sum;
    d;
    tripled;
    piped;
    a;
    f5;
    fib;
    even;
    return
}
