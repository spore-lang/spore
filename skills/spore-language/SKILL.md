---
name: spore-language
description: >
  Reference guide for writing Spore code — covers syntax, types, effects,
  cost annotations, pattern matching, holes, and the CLI toolchain.
  Use when generating, reviewing, or refactoring .spore files.
metadata:
  primary-tools:
    - spore
---

# Spore Language Guide

## Use this skill when

1. You need to write, generate, or review Spore (`.spore` / `.sp`) source code.
2. You need to understand Spore function signatures (types, errors, costs, capabilities).
3. You need to fill typed holes (`?name`) in Spore programs.
4. You need to run or type-check Spore files with the CLI.

## Core philosophy

Spore is expression-only: there are no statements, no loops, no null.
Control flow uses pattern matching and recursion.
Function signatures are **gravity centers** — they carry type, error, effect,
cost, and capability information in a fixed clause order.

## CLI quick reference

```bash
spore run <file>             # compile and execute (tree-walk interpreter)
spore check <file...>        # type-check only (exit 0 = no errors)
spore check --verbose <file> # show inferred types, costs, capabilities
spore holes <file>           # JSON hole report (candidates, dependency graph)
spore build <file>           # compile without running
spore watch <file>           # re-check on file changes
spore watch --json <file>    # NDJSON events for IDE/agent consumption
```

Build the compiler:

```bash
cargo build                  # debug build
cargo test --all             # run all tests
cargo run --bin spore -- run demo.sp   # run demo (outputs 204)
```

## Syntax reference

### Functions

Fixed clause order: `-> return` → `! errors` → `where` → `cost` → `uses` → `body`.

```spore
fn add(a: Int, b: Int) -> Int { a + b }

fn fetch(url: String) -> String ! [NetError, Timeout]
    cost ≤ 1000
    uses [NetRead]
{
    ?todo
}

@unbounded
fn explore(depth: Int) -> List[String]
    uses [FileRead]
{
    ?search
}
```

### Lambdas

```spore
let double = |x: Int| x * 2;
let greet = |name: String| "Hello, " + name;
```

### Let bindings and blocks

```spore
fn example() -> Int {
    let x = 10;
    let y = x + 5;
    y * 2                   // last expression is the return value (no semicolon)
}
```

### If expressions

```spore
fn abs(x: Int) -> Int {
    if x < 0 { 0 - x } else { x }
}
```

### Pipe operator

```spore
fn process(data: List[Int]) -> List[Int] {
    data |> filter(|x: Int| x > 0)
         |> map(|x: Int| x * 2)
}
```

### Structs

```spore
struct Point { x: Int, y: Int }

fn origin() -> Point { Point { x: 0, y: 0 } }
fn get_x(p: Point) -> Int { p.x }
```

### Enums (algebraic data types)

```spore
type Shape {
    Circle(Int),
    Rect(Int, Int),
}

type Option[T] {
    Some(T),
    None,
}

type Result[T, E] {
    Ok(T),
    Err(E),
}
```

### Pattern matching

Exhaustive — the compiler verifies all variants are covered.

```spore
fn area(s: Shape) -> Int {
    match s {
        Circle(r) => r * r * 3,
        Rect(w, h) => w * h,
    }
}
```

Supported patterns: variable, wildcard (`_`), literal (int, string, bool),
constructor (`Some(x)`), struct (`Point { x, y }`), or-pattern (`A | B`),
list pattern (`[h, ..t]`).

```spore
fn describe(x: Int) -> String {
    match x {
        0 => "zero",
        1 | 2 | 3 => "small",
        _ => "big",
    }
}
```

### Capabilities and traits

```spore
capability Display[T] {
    fn show(self: T) -> String
}

impl Display for Point {
    fn show(self: Point) -> String {
        to_string(self.x) + ", " + to_string(self.y)
    }
}

capability IO = [FileRead, FileWrite]
```

### Concurrency (syntax accepted, not yet runnable)

```spore
fn fetch_all(urls: List[String]) -> List[String] ! [NetError]
    uses [NetRead, Spawn]
{
    parallel_scope {
        urls |> map(|url| spawn { fetch(url) })
             |> map(|task| await task)
    }
}
```

## Type system

### Primitive types

| Type     | Description                | Literal examples         |
|----------|----------------------------|--------------------------|
| `Int`    | 64-bit signed integer      | `42`, `-7`, `0`          |
| `Float`  | 64-bit floating point      | `3.14`, `-0.5`           |
| `Bool`   | Boolean                    | `true`, `false`          |
| `String` | UTF-8 string               | `"hello"`, `""`          |
| `Char`   | Single character           | `'a'`, `'Z'`            |
| `Unit`   | Unit type (empty tuple)    | `()`                     |
| `Never`  | Bottom type (no values)    | —                        |

### Composite types

| Syntax           | Description               |
|------------------|---------------------------|
| `List[T]`        | Homogeneous list           |
| `(A) -> B`       | Function type              |
| `Option[T]`      | `Some(T)` or `None`        |
| `Result[T, E]`   | `Ok(T)` or `Err(E)`        |
| `Task[T]`        | Async task (planned)       |

### Generics

```spore
fn identity[T](x: T) -> T { x }

fn map_option[A, B](opt: Option[A], f: (A) -> B) -> Option[B] {
    match opt {
        Some(a) => Some(f(a)),
        None => None,
    }
}
```

### Error sets

Functions declare errors they can raise with `!`:

```spore
fn parse(input: String) -> Int ! [ParseError] { ?todo }
fn load(path: String) -> String ! [IoError, ParseError]
    uses [FileRead]
{
    ?todo
}
```

Use `?` (try operator) to propagate errors:

```spore
fn load_config(path: String) -> Config ! [IoError, ParseError]
    uses [FileRead]
{
    let raw = read_file(path)?;
    parse_config(raw)?
}
```

## Effect system (capabilities)

### Atomic capabilities

| Capability   | Description                     |
|--------------|---------------------------------|
| `FileRead`   | Read files from filesystem      |
| `FileWrite`  | Write/create/delete files       |
| `NetRead`    | Inbound network requests        |
| `NetWrite`   | Outbound network requests       |
| `StateRead`  | Read mutable state              |
| `StateWrite` | Write mutable state             |
| `Spawn`      | Create concurrent tasks         |
| `Clock`      | Read system clock               |
| `Random`     | Access RNG                      |
| `Compute`    | Pure computation (always valid) |
| `Exit`       | Terminate process               |

### Rules

1. Callee capabilities must be a subset of caller capabilities.
2. Module-level `uses` sets the ceiling for all functions in that module.
3. Functions with empty `uses` (or no `uses`) are pure.

```spore
module my_lib uses [FileRead, Compute]

fn read_data(path: String) -> String
    uses [FileRead]
{
    ?todo
}

fn transform(data: String) -> String {   // pure — no uses clause needed
    data |> trim |> to_upper
}
```

## Cost analysis

### Cost clauses

```spore
fn linear_search(list: List[Int], target: Int) -> Bool
    cost ≤ O(n)
{
    ?search
}

fn constant_work(x: Int) -> Int
    cost ≤ 42
{
    x + 1
}
```

### Three tiers

| Tier     | Coverage | Mechanism                                        |
|----------|----------|--------------------------------------------------|
| Tier 1   | ~70%     | Automatic — structural recursion infers `O(n)`   |
| Tier 2   | ~20%     | Declared — `cost ≤ expr` verified by compiler    |
| Tier 3   | ~10%     | Escape — `@unbounded` (contagious to callers)    |

`@unbounded` marks a function as having no analyzable cost bound.
Any function calling an `@unbounded` function inherits that status.

## Hole system

Holes are typed placeholders that the compiler analyzes but doesn't execute.

```spore
fn sort(list: List[Int]) -> List[Int]
    cost ≤ O(n * log(n))
{
    ?sort_impl
}
```

### Hole report

`spore holes <file>` outputs JSON with:

- **expected_type**: what type the hole must produce
- **bindings**: all in-scope variables and their types
- **capabilities**: available capabilities at the hole site
- **errors_to_handle**: error types the hole must handle or propagate
- **candidates**: ranked list of functions that could fill the hole
- **dependency_graph**: DAG of holes with suggested fill order

```json
{
  "holes": [{
    "name": "sort_impl",
    "expected_type": "List[Int]",
    "bindings": {"list": "List[Int]"},
    "capabilities": [],
    "candidates": [...]
  }],
  "dependency_graph": {
    "roots": ["sort_impl"],
    "suggested_order": ["sort_impl"]
  }
}
```

### Filling holes

When filling a hole, ensure:

1. The expression has the correct type (`expected_type`).
2. Only use bindings listed in the report.
3. Stay within declared capabilities (`uses` clause).
4. Handle all errors in `errors_to_handle`.
5. Respect the cost budget if declared.

## Builtin functions

### List operations

| Function                       | Signature                                      |
|--------------------------------|------------------------------------------------|
| `len(list)`                    | `List[T] -> Int`                               |
| `head(list)`                   | `List[T] -> T`                                 |
| `tail(list)`                   | `List[T] -> List[T]`                           |
| `append(list, item)`          | `(List[T], T) -> List[T]`                      |
| `prepend(item, list)`         | `(T, List[T]) -> List[T]`                      |
| `reverse(list)`               | `List[T] -> List[T]`                           |
| `range(start, end)`           | `(Int, Int) -> List[Int]`                      |
| `contains(list, item)`        | `(List[T], T) -> Bool` or `(String, String) -> Bool` |

### Higher-order functions

| Function                       | Signature                                      |
|--------------------------------|------------------------------------------------|
| `map(list, f)`                 | `(List[A], (A) -> B) -> List[B]`              |
| `filter(list, f)`             | `(List[T], (T) -> Bool) -> List[T]`            |
| `fold(list, init, f)`         | `(List[T], U, (U, T) -> U) -> U`              |
| `each(list, f)`               | `(List[T], (T) -> Unit) -> Unit`               |

### String operations

| Function                       | Signature                                      |
|--------------------------------|------------------------------------------------|
| `string_length(s)`            | `String -> Int`                                |
| `split(s, sep)`               | `(String, String) -> List[String]`             |
| `trim(s)`                     | `String -> String`                             |
| `to_upper(s)`                 | `String -> String`                             |
| `to_lower(s)`                 | `String -> String`                             |
| `starts_with(s, prefix)`      | `(String, String) -> Bool`                     |
| `ends_with(s, suffix)`        | `(String, String) -> Bool`                     |
| `char_at(s, idx)`             | `(String, Int) -> String`                      |
| `substring(s, start, end)`    | `(String, Int, Int) -> String`                 |
| `replace(s, from, to)`        | `(String, String, String) -> String`           |

### Math and conversion

| Function                       | Signature                                      |
|--------------------------------|------------------------------------------------|
| `abs(n)`                       | `Int -> Int`                                   |
| `min(a, b)`                    | `(Int, Int) -> Int`                            |
| `max(a, b)`                    | `(Int, Int) -> Int`                            |
| `to_string(x)`                | `T -> String`                                  |

### I/O

| Function                       | Signature                                      |
|--------------------------------|------------------------------------------------|
| `print(s)`                     | `String -> Unit`                               |
| `println(s)`                   | `String -> Unit`                               |
| `read_line()`                 | `() -> String`                                 |

## Operators

### Arithmetic

`+`, `-`, `*`, `/`, `%` — standard arithmetic on `Int` and `Float`.

### Comparison

`==`, `!=`, `<`, `<=`, `>`, `>=` — return `Bool`.

### Logical

`&&`, `||`, `!` — short-circuit boolean operators.

### String

`+` — string concatenation: `"hello" + " " + "world"`.

### Special

| Operator | Description                                     |
|----------|-------------------------------------------------|
| `\|>`    | Pipe: `x \|> f` desugars to `f(x)`             |
| `?`      | Try: propagates error from `Result` or `Option` |
| `??`     | Unwrap-or-default (planned)                     |

## Common patterns

### Entry point

Every executable must have a `main` function:

```spore
fn main() -> Int {
    42
}
```

### Working with enums

```spore
type Color { Red, Green, Blue }

fn to_hex(c: Color) -> String {
    match c {
        Red => "#FF0000",
        Green => "#00FF00",
        Blue => "#0000FF",
    }
}
```

### Higher-order pipelines

```spore
fn sum_positive_doubled(numbers: List[Int]) -> Int {
    numbers |> filter(|x: Int| x > 0)
            |> map(|x: Int| x * 2)
            |> fold(0, |acc: Int, x: Int| acc + x)
}
```

### Struct with methods via capability/impl

```spore
struct Circle { radius: Int }

capability Area[T] {
    fn area(self: T) -> Int
}

impl Area for Circle {
    fn area(self: Circle) -> Int {
        self.radius * self.radius * 3
    }
}
```

## Current limitations

1. **No loops** — use recursion or higher-order functions (`map`, `fold`, `each`).
2. **Concurrency syntax-only** — `spawn`/`await`/`parallel_scope` parse but don't execute.
3. **No standard library modules** — `std.io`, `std.json`, etc. not yet available.
4. **No `foreign fn`** — platform I/O bindings not yet supported.
5. **Tree-walking interpreter** — no compiled output; suitable for prototyping.
6. **Refinement types** — parsed but not enforced at type-check time.
