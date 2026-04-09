fn add(a: I32, b: I32) -> I32
spec {
    example "identity": add(0, 42) == 42
    example "basic": add(20, 22) == 42
    property "commutative": |a: I32, b: I32| add(a, b) == add(b, a)
}
{ a + b }

fn negate(x: I32) -> I32
spec {
    example "zero": negate(0) == 0
    example "positive": negate(5) == 0 - 5
    property "double_negate": |x: I32| negate(negate(x)) == x
}
{ 0 - x }

struct Point {
    x: I32,
    y: I32,
}

fn distance_squared(p: Point) -> I32
spec {
    example "origin": distance_squared(Point { x: 0, y: 0 }) == 0
    example "unit": distance_squared(Point { x: 3, y: 4 }) == 25
}
{ p.x * p.x + p.y * p.y }

fn translate(p: Point, dx: I32, dy: I32) -> Point { Point { x: p.x + dx, y: p.y + dy } }

fn apply(f: (I32) -> I32, x: I32) -> I32 { f(x) }

fn double(x: I32) -> I32
spec {
    example "zero": double(0) == 0
    example "five": double(5) == 10
}
{ x * 2 }

fn compose(f: (I32) -> I32, g: (I32) -> I32) -> (I32) -> I32 { |x: I32| f(g(x)) }

type Shape {
    Circle(I32),
    Rect(I32, I32),
}

fn area(s: Shape) -> I32
spec {
    example "circle": area(Circle(5)) == 75
    example "rect": area(Rect(3, 4)) == 12
}
{ match s {
    Circle(r) => r * r * 3,
    Rect(w, h) => w * h,
} }

fn factorial(n: I32) -> I32
spec {
    example "base": factorial(0) == 1
    example "five": factorial(5) == 120
}
{ match n {
    0 => 1,
    _ => n * factorial(n - 1),
} }

fn fibonacci(n: I32) -> I32
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

fn is_even(n: I32) -> Bool
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

fn greet(name: Str) -> Str
spec {
    example "world": greet("world") == "Hello, world!"
}
{ "Hello, " + name + "!" }

fn main() -> I32 {
    let sum = add(20, 22)
    let p = Point { x: 3, y: 4 }
    let d = distance_squared(p)
    let tripled = apply(|x: I32| x * 3, 14)
    let piped = 10 |> double
    let c = Circle(5)
    let a = area(c)
    let f5 = factorial(5)
    let fib = fibonacci(10)
    let even = is_even(42)
    sum + d + tripled + piped + a
}
