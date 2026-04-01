// Spore language demo — exercises core features that the interpreter can run.

// 1. Basic arithmetic
fn add(a: Int, b: Int) -> Int { a + b }

// 2. Struct definition + field access
struct Point { x: Int, y: Int }

fn distance_squared(p: Point) -> Int {
    p.x * p.x + p.y * p.y
}

// 3. Lambda + pipe
fn apply(f: (Int) -> Int, x: Int) -> Int { f(x) }

fn double(x: Int) -> Int { x * 2 }

// 4. Hole (typed placeholder — resolved at type-check time)
fn todo_example() -> Int { ?todo }

// Entry point — runs in the tree-walk interpreter
fn main() -> Int {
    let sum = add(20, 22);
    let p = Point { x: 3, y: 4 };
    let d = distance_squared(p);
    let tripled = apply(|x: Int| x * 3, 14);
    let piped = 10 |> double;

    // sum=42, d=25, tripled=42, piped=20
    sum + d + tripled + piped
}
