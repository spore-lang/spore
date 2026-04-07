fn add(a: Int, b: Int) -> Int
spec {
    example "identity": add(0, 42) == 42
    example "basic": add(20, 22) == 42
    property "commutative": |a: Int, b: Int| add(a, b) == add(b, a)
}
{ a + b }

fn negate(x: Int) -> Int
spec {
    example "zero": negate(0) == 0
    example "positive": negate(5) == 0 - 5
    property "double_negate": |x: Int| negate(negate(x)) == x
}
{ 0 - x }

struct Point {
    x: Int,
    y: Int,
}

fn distance_squared(p: Point) -> Int
spec {
    example "origin": distance_squared(Point { x: 0, y: 0 }) == 0
    example "unit": distance_squared(Point { x: 3, y: 4 }) == 25
}
{ p.x * p.x + p.y * p.y }

fn translate(p: Point, dx: Int, dy: Int) -> Point { Point { x: p.x + dx, y: p.y + dy } }

fn apply(f: (Int) -> Int, x: Int) -> Int { f(x) }

fn double(x: Int) -> Int
spec {
    example "zero": double(0) == 0
    example "five": double(5) == 10
}
{ x * 2 }

fn compose(f: (Int) -> Int, g: (Int) -> Int) -> (Int) -> Int { |x: Int| f(g(x)) }

type Shape {
    Circle(Int),
    Rect(Int, Int),
}

fn area(s: Shape) -> Int
spec {
    example "circle": area(Circle(5)) == 75
    example "rect": area(Rect(3, 4)) == 12
}
{ match s {
    Circle(r) => r * r * 3,
    Rect(w, h) => w * h,
} }

fn factorial(n: Int) -> Int
spec {
    example "base": factorial(0) == 1
    example "five": factorial(5) == 120
}
{ match n {
    0 => 1,
    _ => n * factorial(n - 1),
} }

fn fibonacci(n: Int) -> Int
spec {
    example "base0": fibonacci(0) == 0
    example "base1": fibonacci(1) == 1
    example "fib10": fibonacci(10) == 55
}
{ match n {
    0 => 0,
    1 => 1,
    _ => fibonacci(n - 1) + fibonacci(n - 2),
} }

fn is_even(n: Int) -> Bool
spec {
    example "zero": is_even(0) == true
    example "one": is_even(1) == false
    example "four": is_even(4) == true
}
{ n % 2 == 0 }

fn both(a: Bool, b: Bool) -> Bool
spec {
    example "tt": both(true, true) == true
    example "tf": both(true, false) == false
    example "ff": both(false, false) == false
}
{ a && b }

fn greet(name: String) -> String
spec {
    example "world": greet("world") == "Hello, world!"
}
{ "Hello, " + name + "!" }

fn main() -> Int {
    let sum = add(20, 22)
    let p = Point { x: 3, y: 4 }
    let d = distance_squared(p)
    let tripled = apply(|x: Int| x * 3, 14)
    let piped = 10 |> double
    let c = Circle(5)
    let a = area(c)
    let f5 = factorial(5)
    let fib = fibonacci(10)
    let even = is_even(42)
    sum + d + tripled + piped + a
}
