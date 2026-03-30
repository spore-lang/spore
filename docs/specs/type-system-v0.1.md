# Spore Type System — Design Document v0.1

> **Status**: Draft
> **Scope**: `sporec` compiler, type checking, type inference, generics, traits, capabilities
> **Depends on**: Signature syntax v0.2, Hole system v0.2, Cost system, Capability system

---

## 1. Overview

### Position on the Type Spectrum

Spore's type system is **"Rust++ with targeted refinements"** — Level 2.5–3.5 on the
type system spectrum. More expressive than Rust (refinement predicates, row-typed effects),
more practical than Haskell (no HKT, nominal safety), and uniquely positioned for
Human–Agent collaboration via typed holes and structured compiler output.

```
Generics + Traits + Associated Types + GATs + Const Generics + Lightweight Refinements
```

### Philosophy

| Principle | Implication |
|---|---|
| **Signatures are gravity centers** | Signatures must be richly typed and explicit; bodies are inferred |
| **Nominal-primary, structural escape** | Named types are nominal; anonymous records are structural; capabilities are always nominal |
| **Capabilities = Traits** | Unified abstraction — `capability` is syntactic sugar for a trait on execution context |
| **Decidable checking** | No SMT solver; refinements are limited to decidable predicates and abstract interpretation |
| **Agent-friendly** | Richer types help Agents more than they hurt humans; Agents absorb annotation complexity |
| **Predictable** | No implicit conversions, no type-level computation, no SFINAE-style surprises |

### What Is In

- Parametric generics with trait bounds
- Nominal traits with associated types and GATs
- Const/value generics with arithmetic
- Sealed enums with exhaustive pattern matching
- Row-typed error sets
- Capability-as-trait unification
- Bidirectional type inference
- Lightweight refinement types (L0 + L1)
- `@allows` constraints on holes

### What Is Out

- Higher-kinded types (capabilities replace most monad use cases)
- Full dependent types (no proof tactics)
- SMT-backed refinement (inscrutable errors)
- Implicit conversions (source of bugs)
- Structural trait satisfaction (undermines capability safety)
- Runtime type reflection (conflicts with cost model)

---

## 2. Primitive Types

Spore provides a small, fixed set of primitive types. All are nominal.

| Type | Description | Default Literal |
|---|---|---|
| `Int` | Arbitrary-precision integer | `42` |
| `Float` | 64-bit IEEE 754 floating point | `3.14` |
| `Bool` | Boolean | `true`, `false` |
| `String` | UTF-8 string | `"hello"` |
| `Char` | Unicode scalar value | `'a'` |
| `Unit` | Zero-information type (like Rust `()`) | `()` |
| `Never` | The bottom type — uninhabited, no values exist | (no literal) |

### Numeric Sub-types (via Refinement)

Rather than proliferating primitive numeric types (i8, u16, f32, …), Spore uses refinement
types on `Int` and `Float` for bounded numerics:

```spore
type U8  = Int if 0 <= self <= 255
type I32 = Int if -2147483648 <= self <= 2147483647
type F32 = Float if self.precision == 32
```

Platform capabilities determine the runtime representation. The type system reasons about
the logical constraints; the codegen layer maps to machine types.

### The Never Type

`Never` is the return type of functions that do not return (diverging functions) and
the type of expressions like `panic` or non-terminating recursion. It is a subtype of every type,
enabling:

```spore
fn abort(msg: String) -> Never {
    panic(msg)
}

fn safe_divide(a: Int, b: Int) -> Int {
    if b == 0 {
        abort("division by zero")   -- Never coerces to Int
    } else {
        a / b
    }
}
```

`Never` appears in exhaustive pattern matching: a branch that returns `Never` is
compatible with any arm type. This is critical for error-handling patterns.

### Unit

`Unit` signals "this function completes but produces no meaningful value." It is distinct
from `Never` (which signals non-termination) and from the absence of a return type.

```spore
fn log_event(event: Event) -> Unit
uses [FileWrite, Clock]
{
    write_log(event.to_string())
}
```

---

## 3. Composite Types

### 3.1 Structs (Named Fields)

Structs are nominal product types with named fields. All fields must be named — no
positional fields, consistent with Spore's no-positional philosophy.

```spore
type Point = {
    x: Float,
    y: Float,
}

type Customer = {
    id: CustomerId,
    name: String,
    email: Email,
    tier: CustomerTier,
}
```

Struct construction uses named fields:

```spore
let p = Point { x: 1.0, y: 2.0 }
let c = Customer { id: cid, name: "Alice", email: alice_email, tier: Gold }
```

Field access uses dot notation:

```spore
let dist = sqrt(p.x * p.x + p.y * p.y)
```

### 3.2 Enums (Sealed)

All enums in Spore are **sealed** — variants cannot be added outside the defining module.
This is non-negotiable for exhaustive pattern matching, closed error sets, and Agent
reasoning.

```spore
type Shape =
    | Circle { radius: Float }
    | Rectangle { width: Float, height: Float }
    | Triangle { a: Float, b: Float, c: Float }

type HttpError =
    | NotFound { url: Url }
    | Timeout { after: Duration }
    | Unauthorized
    | ServerError { code: Int, message: String }
```

Enum variants may carry named fields (as above), or no data:

```spore
type Direction =
    | North
    | South
    | East
    | West
```

**Why sealed**: If extensibility is needed, use traits instead of enums. Enums represent
closed sets of known alternatives; traits represent open sets of conforming types.

```spore
-- Closed: all cases known at compile time
type Color = | Red | Green | Blue

-- Open: new implementations can be added
trait Drawable {
    fn draw(self, canvas: Canvas) -> Unit
}
```

### 3.3 Anonymous Records (Structural)

Anonymous records are the structural escape hatch. They have no declared name and are
compatible by field shape, not by declaration site.

```spore
fn greet(person: { name: String, age: Int }) -> String
{
    "Hello, " ++ person.name ++ " (age " ++ show(person.age) ++ ")"
}

-- Any value with matching fields satisfies this:
let alice = { name: "Alice", age: 30, role: "Engineer" }
greet(alice)   -- OK: alice has name: String and age: Int (extra fields ignored)
```

**Rules for anonymous records**:

1. Matching is width-subtyped: a record with extra fields satisfies a type expecting fewer fields.
2. Field names and types must match exactly (no implicit conversion).
3. Anonymous records cannot implement traits or capabilities — nominal types are required for that.
4. Anonymous records are useful for intermediate computation, FFI boundaries, and Agent hole-filling.

### 3.4 Function Types

Function types use `Fn` with named parameters:

```spore
type Predicate<T> = Fn(value: T) -> Bool

type Transformer<A, B> = Fn(input: A) -> B ! [TransformError]

-- Higher-order function accepting a function argument:
fn apply_twice<T>(f: Fn(x: T) -> T, value: T) -> T
where T: Clone
{
    f(x: f(x: value))
}
```

Closures capture their environment:

```spore
fn make_adder(n: Int) -> Fn(x: Int) -> Int
{
    |x: Int| -> Int { x + n }
}
```

---

## 4. Traits & Capabilities

### 4.1 Trait Definition

Traits define named interfaces that types can implement. Spore traits are nominal — a
type satisfies a trait only through an explicit `impl` declaration.

```spore
trait Eq {
    fn eq(self, other: Self) -> Bool
}

trait Ord: Eq {
    fn compare(self, other: Self) -> Ordering
}

trait Display {
    fn display(self) -> String
}

trait Serialize {
    fn serialize(self) -> Bytes ! [SerializeError]
    cost serialize <= 100
}
```

Trait inheritance uses `:` — `Ord: Eq` means every type implementing `Ord` must also
implement `Eq`.

### 4.2 Trait Implementation

```spore
impl Eq for Point {
    fn eq(self, other: Point) -> Bool {
        self.x == other.x && self.y == other.y
    }
}

impl Ord for Point {
    fn compare(self, other: Point) -> Ordering {
        let dx = self.x.compare(other.x)
        if dx != Equal { dx } else { self.y.compare(other.y) }
    }
}

impl Display for Shape {
    fn display(self) -> String {
        match self {
            Circle { radius }          => "Circle(r=" ++ show(radius) ++ ")",
            Rectangle { width, height } => "Rect(" ++ show(width) ++ "x" ++ show(height) ++ ")",
            Triangle { a, b, c }       => "Tri(" ++ show(a) ++ "," ++ show(b) ++ "," ++ show(c) ++ ")",
        }
    }
}
```

### 4.3 Trait Bounds

Trait bounds constrain generic type parameters in `where` clauses:

```spore
fn sort<T>(list: List<T>) -> List<T>
where T: Ord
cost <= 500
{
    -- implementation
}

fn serialize_all<T>(items: Vec<T>) -> Vec<Bytes> ! [SerializeError]
where T: Serialize + Display
{
    items.map(|item| item.serialize())
}
```

### 4.4 Capability = Trait (Unified)

This is one of Spore's central design decisions. A **capability** is syntactic sugar for
a trait on the execution context. `uses [X]` is equivalent to a trait bound on the
implicit execution context. The Platform implements capabilities.

```spore
-- A capability is a trait on the execution context
capability FileRead {
    fn read_file(path: Path) -> Bytes ! [IoError]
    cost read_file <= 100
}

capability FileWrite {
    fn write_file(path: Path, data: Bytes) -> Unit ! [IoError]
    cost write_file <= 200
}

capability Clock {
    fn now() -> Timestamp
    cost now <= 1
}
```

**How it works**: `uses [FileRead]` in a function signature means "this function
requires an execution context that provides the `FileRead` capability." The Platform
(OS, runtime, sandbox) implements capabilities. This is equivalent to:

```spore
-- These two are conceptually identical:
fn read_config(path: Path) -> Config ! [IoError]

fn read_config(path: Path) -> Config ! [IoError]
```

**Composite capabilities**:

```spore
capability DatabaseAccess = [NetRead, NetWrite, StateRead, StateWrite]
capability Analytics = [Compute, StateRead]

fn generate_report(org_id: OrgId, period: DateRange) -> Report ! [ConnectionLost]
uses [DatabaseAccess, Analytics]
cost <= 12000
{
    -- can use any function requiring subsets of DatabaseAccess or Analytics
}
```

### 4.5 Associated Types

Traits may declare associated types — types determined by the implementing type:

```spore
trait Iterator {
    type Item
    fn next(self) -> Option<Self.Item>
    cost next <= 10
}

impl Iterator for LineReader {
    type Item = String
    fn next(self) -> Option<String> {
        self.read_line()
    }
}

trait Collection {
    type Item
    type Iter: Iterator where Iter.Item == Self.Item

    fn iter(self) -> Self.Iter
    fn len(self) -> Int
    fn is_empty(self) -> Bool { self.len() == 0 }   -- default method
}
```

Associated types eliminate the need for extra type parameters on trait users:

```spore
-- With associated types (clean):
fn sum_all<C>(collection: C) -> Int
where
    C: Collection
    C.Item: Add<Output = Int>
{
    let mut total = 0
    for item in collection.iter() {
        total = total + item
    }
    total
}

-- Without associated types (verbose, requires extra params):
-- fn sum_all<C, T, I>(collection: C) -> Int where C: Collection<T, I>, ...
```

### 4.6 Generic Associated Types (GATs)

GATs allow associated types to have their own generic parameters. This enables patterns
like lending iterators and self-referential collections.

```spore
trait LendingIterator {
    type Item<'a>
    fn next<'a>(self: &'a mut Self) -> Option<Self.Item<'a>>
}

trait Container {
    type Elem<T>
    fn wrap<T>(value: T) -> Self.Elem<T>
    fn unwrap<T>(wrapped: Self.Elem<T>) -> T
}

impl Container for OptionContainer {
    type Elem<T> = Option<T>
    fn wrap<T>(value: T) -> Option<T> { Some(value) }
    fn unwrap<T>(wrapped: Option<T>) -> T {
        match wrapped {
            Some(v) => v,
            None    => panic("unwrap of None"),
        }
    }
}
```

GATs cover the most common use cases that HKTs would serve (abstracting over container
kinds) without introducing the full complexity of higher-kinded polymorphism.

### 4.7 Coherence & Orphan Rules

Spore adopts Rust's coherence model: you can only implement a trait for a type if you
own either the trait or the type. This prevents conflicting implementations across
modules.

**Base rule (Rust-like)**:

```spore
-- OK: you own Point, so you can implement any trait for it
impl Display for Point { ... }

-- OK: you own MyTrait, so you can implement it for any type
impl MyTrait for String { ... }

-- ERROR: you own neither Display nor String
impl Display for String { ... }   -- orphan rule violation
```

**Extension — Adapters**: For cross-crate interop, Spore provides an explicit `adapter`
mechanism that is visible in function signatures:

```spore
adapter ExternalType as Printable {
    fn display(self) -> String { ... }
}

fn use_external(x: ExternalType) -> String
where adapter: ExternalType as Printable
{
    x.display()
}
```

The adapter is scoped and explicit — it cannot cause global coherence violations.

### 4.8 Built-in Traits

Spore provides compiler-known traits for common operations:

| Trait | Purpose | Derivable |
|---|---|---|
| `Eq` | Equality comparison | Yes |
| `Ord` | Ordering | Yes |
| `Clone` | Value duplication | Yes |
| `Display` | Human-readable formatting | No |
| `Debug` | Debug formatting | Yes |
| `Hash` | Hash computation | Yes |
| `Default` | Default value construction | Yes |
| `Serialize` | Serialization to bytes | Yes |
| `Deserialize` | Deserialization from bytes | Yes |
| `Add`, `Sub`, `Mul`, `Div` | Arithmetic operators | No |

Derivable traits can be auto-implemented by the compiler for structs and enums whose
fields all implement the trait:

```spore
type Point = {
    x: Float,
    y: Float,
} deriving [Eq, Clone, Debug, Hash]
```

---

## 5. Generics

### 5.1 Type Parameters

Type parameters are declared in angle brackets and constrained in `where` clauses:

```spore
fn identity<T>(value: T) -> T
{
    value
}

fn map<A, B>(list: List<A>, f: Fn(item: A) -> B) -> List<B>
{
    -- implementation
}

fn merge<T, U, V>(left: List<T>, right: List<U>, resolver: Fn(a: T, b: U) -> V) -> List<V>
where
    T: Eq + Hash
    U: Eq + Hash
    V: Serialize
cost <= 800
{
    -- implementation
}
```

### 5.2 Const Generics

Const generics allow value-level parameters in type position. This is central to Spore's
bounded collections and cost-aware types.

```spore
type Vec<T, max: Int> = {
    data: Array<T>,
    len: Int,
}

fn take<T, N: Int>(list: List<T>, count: N) -> Vec<T, max: N>
where N <= list.max
{
    -- implementation
}

type Matrix<T, rows: Int, cols: Int> = {
    data: Array<Array<T>>,
}
```

### 5.3 Arithmetic in Type Position

Const generic parameters support arithmetic in type-level expressions, enabling
compile-time dimensional checking:

```spore
fn concat<T, M: Int, N: Int>(
    a: Vec<T, max: M>,
    b: Vec<T, max: N>,
) -> Vec<T, max: M + N>
{
    -- implementation
}

fn transpose<T, R: Int, C: Int>(
    matrix: Matrix<T, rows: R, cols: C>,
) -> Matrix<T, rows: C, cols: R>
{
    -- implementation
}

fn flatten<T, N: Int, M: Int>(
    nested: Vec<Vec<T, max: M>, max: N>,
) -> Vec<T, max: N * M>
{
    -- implementation
}
```

Supported arithmetic operations in type position: `+`, `-`, `*`, `/`, `%`, `min`, `max`.
All arithmetic is evaluated at compile time. Division by zero and overflow are
compile-time errors.

### 5.4 Interaction with Cost

Const generics interact naturally with the cost system. A function's cost may depend
on its const generic parameters:

```spore
fn linear_search<T, N: Int>(items: Vec<T, max: N>, target: T) -> Option<Int>
where T: Eq
cost <= N * 5
{
    -- O(N) search, cost scales linearly
}

fn sort_bounded<T, N: Int>(items: Vec<T, max: N>) -> Vec<T, max: N>
where T: Ord
cost <= N * N * 2   -- O(N²) worst case
{
    -- implementation
}
```

This allows the compiler to verify cost bounds parametrically: calling
`sort_bounded` on a `Vec<T, max: 100>` implies cost ≤ 20000.

---

## 6. Refinement Types

### Overview

Spore supports **lightweight refinement types** — value-level predicates attached to
types that are verified at compile time without an SMT solver. This is a targeted subset
of what Liquid Haskell offers, designed for decidable and predictable checking.

Refinements are organized into two levels:

| Level | Mechanism | Examples |
|---|---|---|
| **L0**: Decidable Predicates | Direct compile-time evaluation | Numeric bounds, const equality |
| **L1**: Abstract Interpretation | Flow-sensitive propagation | Range narrowing, null tracking |

### 6.1 L0 — Decidable Predicates

L0 refinements are predicates that the compiler can fully evaluate at compile time.
They use the `if` clause on type aliases with `self` referring to the value.

```spore
type Port = Int if 1 <= self <= 65535

type Percentage = Float if 0.0 <= self <= 100.0

type NonEmptyString = String if self.len() > 0

type PositiveInt = Int if self > 0

type HttpStatusCode = Int if 100 <= self <= 599
```

**Usage in signatures**:

```spore
fn connect(host: String, port: Port) -> Connection ! [ConnectionError]
uses [NetRead, NetWrite]
{
    -- `port` is guaranteed to be in [1, 65535]
}

fn compute_discount(rate: Percentage, price: Float) -> Float
{
    price * (rate / 100.0)
}
```

**Compile-time checking**:

```spore
connect(host: "example.com", port: 8080)    -- OK: 1 <= 8080 <= 65535
connect(host: "example.com", port: 0)       -- ERROR: 0 violates 1 <= self
connect(host: "example.com", port: 70000)   -- ERROR: 70000 violates self <= 65535
```

**L0 decidable predicates include**:

- Numeric comparisons: `<`, `<=`, `==`, `!=`, `>=`, `>`
- Arithmetic on constants: `self + 1 <= 100`
- String length: `self.len() > 0`, `self.len() <= 255`
- Collection size: `self.len() <= N` (with const generics)
- Boolean connectives: `&&`, `||`, `!`
- Const equality: `self == "production" || self == "staging"`

### 6.2 L1 — Abstract Interpretation Propagation

L1 goes beyond individual checks. The compiler uses **abstract interpretation** to
propagate refinement information through control flow, narrowing types as values flow
through conditionals and assertions.

```spore
fn process_port(raw: Int) -> Port ! [InvalidPort] {
    if raw < 1 || raw > 65535 {
        raise InvalidPort { value: raw }
    }
    -- Here the compiler knows: 1 <= raw <= 65535
    -- `raw` is automatically narrowed to `Port`
    raw   -- OK: refinement satisfied by control flow
}
```

**Range narrowing through branches**:

```spore
fn categorize_age(age: Int) -> String
{
    if age < 0 {
        panic("negative age")
    }
    -- compiler knows: age >= 0

    if age < 18 {
        -- compiler knows: 0 <= age < 18
        "minor"
    } else if age < 65 {
        -- compiler knows: 18 <= age < 65
        "adult"
    } else {
        -- compiler knows: age >= 65
        "senior"
    }
}
```

**Propagation through let bindings**:

```spore
fn clamp_to_port(raw: Int) -> Port {
    let clamped = max(1, min(raw, 65535))
    -- compiler infers: 1 <= clamped <= 65535
    clamped   -- OK: satisfies Port refinement
}
```

**L1 does NOT include**:

- Arbitrary logical formulas (no quantifiers)
- Aliasing analysis
- Heap reasoning
- SMT-level theorem proving

This keeps type checking **decidable** and error messages **predictable**.

### 6.3 Error Messages for Refinement Violations

Refinement errors follow Spore's dual-channel error design:

```
-- v0: Machine-readable (for Agent)
{"error": "refinement_violation", "type": "Port",
 "predicate": "1 <= self <= 65535", "actual_value": "0",
 "location": "server.spore:15:38"}

-- v2: Contextual (Elm-style, for human)
Error: Refinement violation

15 |    connect(host: "example.com", port: 0)
                                           ^
   The function `connect` expects `port` to be a Port,
   which requires: 1 <= value <= 65535

   But the literal `0` does not satisfy: 1 <= 0  (false)

   Hint: Port values must be between 1 and 65535.
```

When the compiler cannot statically determine if a refinement holds (e.g., value
comes from runtime input), it requires an explicit check:

```
Error: Cannot verify refinement

20 |    connect(host: "example.com", port: user_input)
                                           ^^^^^^^^^^
   `user_input` is Int, but `port` requires Port (1 <= self <= 65535).
   The compiler cannot prove this statically.

   Hint: Add a runtime check:
     if user_input < 1 || user_input > 65535 {
         raise InvalidPort { value: user_input }
     }
     connect(host: "example.com", port: user_input)
```

---

## 7. Pattern Matching

### Overview

Spore requires **full, exhaustive pattern matching** on all enums. The compiler enforces
that every possible variant is handled. This is non-negotiable for:

- Closed error handling (`! [Err1, Err2]`)
- Agent-generated code correctness
- Preventing "forgot a case" bugs at compile time

### 7.1 Exhaustiveness

```spore
fn describe(shape: Shape) -> String
{
    match shape {
        Circle { radius }           => "circle with radius " ++ show(radius),
        Rectangle { width, height } => show(width) ++ "x" ++ show(height) ++ " rectangle",
        Triangle { a, b, c }        => "triangle with sides " ++ show(a) ++ ", " ++ show(b) ++ ", " ++ show(c),
    }
}
```

Missing a variant is a compile-time error:

```
Error: Non-exhaustive match

12 |    match shape {
   |    ^^^^^
   Missing variant: Triangle { a, b, c }

   All Shape variants must be handled because Shape is a sealed enum.
```

### 7.2 Nested Patterns

Patterns can be nested to match deeply into data structures:

```spore
type Expr =
    | Literal { value: Int }
    | BinOp { op: Op, left: Expr, right: Expr }
    | UnaryOp { op: Op, operand: Expr }

fn simplify(expr: Expr) -> Expr
{
    match expr {
        BinOp { op: Add, left: Literal { value: 0 }, right: e } => simplify(e),
        BinOp { op: Add, left: e, right: Literal { value: 0 } } => simplify(e),
        BinOp { op: Mul, left: Literal { value: 1 }, right: e } => simplify(e),
        BinOp { op: Mul, left: e, right: Literal { value: 1 } } => simplify(e),
        BinOp { op: Mul, left: Literal { value: 0 }, right: _ } => Literal { value: 0 },
        BinOp { op, left, right } => BinOp {
            op: op,
            left: simplify(left),
            right: simplify(right),
        },
        UnaryOp { op, operand } => UnaryOp { op: op, operand: simplify(operand) },
        other => other,
    }
}
```

### 7.3 Guard Clauses

Guards add boolean conditions to pattern branches:

```spore
fn classify_temperature(temp: Float) -> String
{
    match temp {
        t if t < -40.0  => "extreme cold",
        t if t < 0.0    => "freezing",
        t if t < 20.0   => "cool",
        t if t < 35.0   => "warm",
        t if t < 50.0   => "hot",
        _               => "extreme heat",
    }
}
```

**Important**: Guards weaken exhaustiveness guarantees. The compiler requires a
catch-all (`_` or variable) arm when guards are used, because it cannot generally
prove that guards cover all cases.

### 7.4 Or-Patterns

Or-patterns match multiple alternatives with the same arm:

```spore
fn is_weekend(day: Day) -> Bool
{
    match day {
        Saturday | Sunday => true,
        _                 => false,
    }
}

fn area(shape: Shape) -> Float
{
    match shape {
        Circle { radius }           => pi * radius * radius,
        Rectangle { width, height } => width * height,
        Triangle { a, b, c } => {
            let s = (a + b + c) / 2.0
            sqrt(s * (s - a) * (s - b) * (s - c))
        },
    }
}
```

Or-patterns can combine with nested patterns:

```spore
fn is_origin(point: Point) -> Bool
{
    match point {
        Point { x: 0.0, y: 0.0 } => true,
        _                         => false,
    }
}

fn is_simple_shape(shape: Shape) -> Bool
{
    match shape {
        Circle { .. } | Rectangle { .. } => true,
        _                                 => false,
    }
}
```

### 7.5 Destructuring in Let Bindings

Pattern matching is not limited to `match` — destructuring works in `let` bindings:

```spore
fn distance(a: Point, b: Point) -> Float
{
    let Point { x: x1, y: y1 } = a
    let Point { x: x2, y: y2 } = b
    sqrt((x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1))
}
```

### 7.6 Pattern Matching on Error Types

Pattern matching integrates with Spore's row-typed error sets for exhaustive error
handling:

```spore
fn handle_result(result: Invoice ! [TaxError, ValidationError]) -> String
{
    match result {
        Ok(invoice) => "Invoice #" ++ show(invoice.id),
        Err(TaxError { region, reason }) => "Tax error in " ++ show(region) ++ ": " ++ reason,
        Err(ValidationError { field, message }) => "Validation failed on " ++ field ++ ": " ++ message,
    }
}
```

---

## 8. Type Inference

### Principle: Signatures Explicit, Bodies Inferred

Spore uses **bidirectional type checking**: function signatures provide top-down
type expectations, and function bodies synthesize types bottom-up. The two meet at
call boundaries and return expressions.

### 8.1 What MUST Be Annotated

| Element | Why |
|---|---|
| Function parameter types | Gravity center — the signature IS the API |
| Function return type | Agent reads signatures for synthesis; human reads for understanding |
| Error sets (`! [...]`) | Error contract — must be visible |
| Effect/capability sets | Security boundary — must be visible |
| Cost bounds | Performance contract — must be visible |
| Struct/type field types | Data definition — must be explicit |
| Trait method signatures | Interface contract |
| Public constants | API surface |

### 8.2 What CAN Be Inferred

| Element | Inference Strategy |
|---|---|
| Local variable types | `let x = compute(...)` — inferred from RHS |
| Generic type params at call sites | `sort(list: my_list)` — T inferred from `my_list` |
| Closure parameter types (in context) | `.map(\|x\| x + 1)` — x inferred from Iterator.Item |
| Intermediate expression types | Standard local inference |
| Effect propagation within bodies | If body calls `read_file(...)`, FileRead is inferred and checked against declared `uses` |

### 8.3 Bidirectional Checking in Practice

```spore
fn process_items<T>(items: Vec<T>) -> Vec<String> ! [FormatError]
where T: Display
cost <= 1000
{
    -- Check mode (top-down): return type Vec<String> ! [FormatError] pushes down
    -- Synth mode (bottom-up): expression types bubble up

    let results = items.map(|item| {
        -- item type inferred from Vec<T>.map signature → T
        -- T: Display, so .display() is available
        let formatted = item.display()   -- inferred: String
        let trimmed = formatted.trim()   -- inferred: String
        trimmed                          -- synthesized: String, checked against Vec<String>
    })

    results   -- synthesized: Vec<String>, checked against return type ✓
}
```

### 8.4 Inference and Holes

Holes participate in bidirectional checking: the expected type flows into the hole
(check mode), and the hole's inferred type from context flows out (synth mode).

```spore
fn example(x: Int, y: String) -> Bool ! []
{
    let a: Int = ?h1          -- check mode: ?h1 must produce Int
    let b = if a > 0 {
        ?h2                   -- check mode from return type: ?h2 must produce Bool
    } else {
        false
    }
    b
}
```

The compiler reports:
- `?h1` has expected type `Int` (from let binding annotation)
- `?h2` has expected type `Bool` (from return type, since this branch determines the return)

### 8.5 Compiler Diagnostic Output

When a function omits declarations the compiler infers, it reports what was inferred:

```spore
fn add(a: Int, b: Int) -> Int {
    a + b
}
```

Compiler output:

```
[ok] add : (a: Int, b: Int) -> Int
  inferred:
    effects: pure, deterministic, total
    cost = 1
    uses: []
```

When a function uses capabilities without declaring them:

```
ERROR [incomplete-function] fetch_data is incomplete:
  Detected capability dependency without declared `uses`.
  Inferred capabilities: [NetRead]

  Suggest adding:
    uses [NetRead]

  Current state: can simulate, cannot execute
```

---

## 9. Nominal vs Structural

### When Each Applies

| Context | Typing Discipline | Rationale |
|---|---|---|
| Named types (`type X = ...`) | **Nominal** | `UserId ≠ String` even if same shape |
| Enums | **Nominal** | Sealed, exhaustiveness-checked |
| Traits | **Nominal** | Explicit `impl` required |
| Capabilities | **Nominal** (always) | Security boundary — no structural coincidence |
| Anonymous records `{ ... }` | **Structural** | Flexibility for intermediate values |
| Hole-filling search (internal) | **Structural** | Agent searches by shape, compiler enforces nominal at boundaries |
| Function call boundaries | **Nominal** | Callee specifies named types, caller must provide them |

### Nominal Type Examples

```spore
type UserId = String       -- nominal wrapper: UserId ≠ String
type Celsius = Float       -- nominal: Celsius ≠ Fahrenheit
type Fahrenheit = Float    -- nominal: Fahrenheit ≠ Celsius

fn format_temp(temp: Celsius) -> String
{
    show(temp) ++ "°C"
}

let c: Celsius = 100.0
let f: Fahrenheit = 212.0

format_temp(temp: c)    -- OK
format_temp(temp: f)    -- ERROR: expected Celsius, got Fahrenheit
format_temp(temp: 98.6) -- ERROR: expected Celsius, got Float
```

### Anonymous Record Rules

```spore
-- Anonymous record type in signature:
fn summarize(data: { count: Int, total: Float }) -> String
{
    "Count: " ++ show(data.count) ++ ", Avg: " ++ show(data.total / data.count)
}

-- Any matching record works (width subtyping):
let stats = { count: 10, total: 95.5, median: 9.2 }
summarize(data: stats)   -- OK: stats has count: Int and total: Float

-- Named types do NOT satisfy anonymous record types:
type Stats = { count: Int, total: Float }
let named_stats = Stats { count: 10, total: 95.5 }
summarize(data: named_stats)   -- ERROR: Stats is nominal, not { count: Int, total: Float }
```

**Key insight**: Structural typing is an implementation detail of the Agent's search
strategy, not a user-facing type compatibility rule at API boundaries. Users think
nominally; Agents search structurally; the compiler enforces nominally at boundaries.

To use a named type where an anonymous record is expected, explicit conversion is required:

```spore
summarize(data: { count: named_stats.count, total: named_stats.total })   -- OK
```

---

## 10. System Interactions

### 10.1 Capabilities

The type system and capability system are unified through the capability=trait design.
Every capability is a trait, and `uses [Cap]` is a trait bound on the execution context.

**Capability checking flow**:

1. Function signature declares `uses [FileRead, NetWrite]`
2. Compiler checks that every function called in the body has required capabilities
   that are a subset of the declared set
3. Missing capability → compile error with clear message
4. Excess declared capability (declared but unused) → warning

```spore
fn fetch_and_save(url: Url, path: Path) -> Unit ! [NetworkError, IoError]
uses [NetRead, FileWrite]
cost <= 5000
{
    let data = http_get(url)       -- requires NetRead ✓
    write_file(path, data)         -- requires FileWrite ✓
    -- send_email(...)             -- would require EmailService, not in uses → ERROR
}
```

### 10.2 Cost

Cost bounds interact with the type system through const generics:

```spore
fn batch_process<T, N: Int>(items: Vec<T, max: N>) -> Vec<T, max: N>
where T: Processable
cost <= N * 50
{
    items.map(|item| item.process())   -- each process() costs <= 50
}
```

The compiler verifies: if `process()` has `cost <= 50` and we call it `N` times,
the total cost is bounded by `N * 50`, matching the declared bound.

### 10.3 Holes and @allows

`@allows` is a hole-level constraint that limits which functions an Agent may use
to fill a specific hole. This provides fine-grained control over Agent behavior
without affecting the type system's soundness.

```spore
fn process_payment(amount: Money, card: Card) -> Receipt ! [PaymentFailed]
uses [PaymentGateway, AuditLog]
cost <= 2000
{
    let validated = validate_card(card)
    @allows[charge, charge_with_retry]
    ?payment_logic
}
```

**Semantics of @allows**:

- `@allows[f1, f2, ...]` annotates the immediately following hole
- The Agent, when filling this hole, may only call the listed functions (plus pure
  helper functions that require no capabilities)
- The compiler verifies that the filling respects the `@allows` constraint
- `@allows` is **not** a type — it is a **synthesis constraint** that restricts
  the search space for hole-filling

**Why @allows exists**: In a capability-rich environment, an Agent might have access
to many functions that type-check. `@allows` lets the human narrow the search space
to a trusted subset, expressing architectural intent.

```spore
fn build_dashboard(org: Org) -> Dashboard ! [DataError]
uses [Database, Cache, Analytics]
cost <= 20000
{
    @allows[fetch_cached_metrics, compute_summary]
    let metrics = ?gather_metrics

    @allows[render_chart, render_table]
    let charts = ?render_visualizations

    Dashboard.new(org, metrics, charts)
}
```

If an Agent proposes a filling for `?gather_metrics` that calls `raw_sql_query`,
the compiler rejects it:

```
Error: @allows violation

  Hole ?gather_metrics is constrained by @allows[fetch_cached_metrics, compute_summary]
  but the proposed filling calls `raw_sql_query`, which is not in the allowed set.
```

### 10.4 Modules and Imports

Spore uses a module and import system (not `FuncCall` or `Module` capabilities —
those are removed). Modules provide namespacing and visibility control:

```spore
-- src/billing/tax.spore
module billing.tax

import core.math { round, ceil }
import billing.types { Money, TaxRegion, TaxRate }

fn compute_tax(amount: Money, region: TaxRegion) -> Money ! [TaxError]
cost <= 200
{
    let rate = lookup_rate(region)
    round(amount * rate.value)
}
```

Import visibility is orthogonal to the type system: what you can import is determined
by module exports, not by types. The type system then checks that imported functions
are used correctly (right types, right capabilities, right cost bounds).

### 10.5 Snapshots

The snapshot system computes a hash over each function's signature. The type system
defines what constitutes the signature:

| Component | In Signature Hash |
|---|---|
| Function name | ✓ |
| Parameter names, types, order | ✓ |
| Return type | ✓ |
| Error set | ✓ |
| Effect annotations | ✓ |
| Cost bound | ✓ |
| Capability set (`uses`) | ✓ |
| Generic constraints | ✓ |
| Body (including holes) | ✗ |
| `@allows` annotations | ✗ |

Changing any ✓ component produces a new hash and requires `--permit` for downstream
dependents. Changing body or `@allows` does not.

### 10.6 Error Types

Spore's `! [Err1, Err2]` is a **closed, row-typed error union**. Error types are
themselves enum variants (ADTs), and the `!` syntax computes unions automatically
across call chains:

```spore
fn parse(input: String) -> Ast ! [SyntaxError, EncodingError]
fn validate(ast: Ast) -> ValidAst ! [TypeError, RangeError]

-- Error sets union automatically via `?` propagation:
fn compile(input: String) -> ValidAst ! [SyntaxError, EncodingError, TypeError, RangeError]
{
    let ast = parse(input)?
    validate(ast)?
}
```

Each error type carries structured data:

```spore
type SyntaxError = {
    line: Int,
    column: Int,
    message: String,
    snippet: String,
}

type TypeError = {
    expected: String,
    found: String,
    location: SourceLocation,
}
```

---

## 11. Edge Cases

### 11.1 The Never Type

`Never` is the bottom type — it has no values and is a subtype of every type.

```spore
fn unreachable_branch() -> Never {
    panic("should not reach here")
}

-- Never coerces to any type:
fn safe_unwrap<T>(opt: Option<T>) -> T {
    match opt {
        Some(v) => v,
        None    => panic("unwrap of None"),   -- panic returns Never, coerces to T
    }
}
```

`Never` is the natural type for:
- `panic(...)` and `abort(...)` calls
- Diverging expressions (non-terminating recursion)
- Exhaustive match arms that provably never execute
- Error-only functions that always raise

### 11.2 Newtypes

Newtypes are zero-cost nominal wrappers around existing types. They enforce type
distinctions without runtime overhead:

```spore
type UserId = String
type Email = String
type Meters = Float
type Seconds = Float

-- These are all distinct types:
fn send_email(to: Email, subject: String) -> Unit ! [DeliveryError]

let uid: UserId = "user-123"
let email: Email = "alice@example.com"

send_email(to: email, subject: "Hello")    -- OK
send_email(to: uid, subject: "Hello")      -- ERROR: expected Email, got UserId
```

Newtypes may have refinements:

```spore
type Port = Int if 1 <= self <= 65535
type NonEmptyString = String if self.len() > 0
type Latitude = Float if -90.0 <= self <= 90.0
type Longitude = Float if -180.0 <= self <= 180.0
```

### 11.3 Recursive Types

Enums and structs may be recursive. The compiler detects and supports this:

```spore
type Expr =
    | Literal { value: Int }
    | BinOp { op: Op, left: Expr, right: Expr }
    | UnaryOp { op: Op, operand: Expr }
    | IfExpr { cond: Expr, then_branch: Expr, else_branch: Expr }

type JsonValue =
    | JsonNull
    | JsonBool { value: Bool }
    | JsonNumber { value: Float }
    | JsonString { value: String }
    | JsonArray { elements: List<JsonValue> }
    | JsonObject { fields: List<{ key: String, value: JsonValue }> }
```

**Infinite-size check**: The compiler rejects types that would require infinite
memory without indirection:

```spore
-- ERROR: infinite-size type (struct directly contains itself)
type Bad = {
    next: Bad,   -- Bad contains Bad with no indirection
}

-- OK: indirection via List breaks the cycle
type Tree<T> = {
    value: T,
    children: List<Tree<T>>,   -- List provides indirection
}
```

### 11.4 Type Aliases

Type aliases provide alternative names without creating new types. Unlike newtypes,
aliases are transparent — they are interchangeable with their definition:

```spore
alias StringList = List<String>
alias Callback<T> = Fn(event: T) -> Unit
alias Result<T> = T ! [GenericError]

fn process(items: StringList) -> StringList   -- same as List<String>
fn on_click(handler: Callback<ClickEvent>) -> Unit
```

The `alias` keyword distinguishes aliases from newtypes (`type`):

```spore
type UserId = String       -- newtype: UserId ≠ String (nominal)
alias UserName = String    -- alias: UserName == String (transparent)
```

### 11.5 Orphan Rules

The orphan rule prevents conflicting trait implementations:

```spore
-- In module `shapes`:
type Circle = { radius: Float }

-- In module `rendering`:
trait Drawable { fn draw(self, canvas: Canvas) -> Unit }

-- In module `app`:
-- Can implement Drawable for Circle ONLY if `app` owns Circle OR Drawable
impl Drawable for Circle {    -- OK if shapes or rendering is the same crate
    fn draw(self, canvas: Canvas) -> Unit { ... }
}
```

**The adapter escape hatch**:

```spore
-- When you own neither the trait nor the type:
adapter Circle as Drawable {
    fn draw(self, canvas: Canvas) -> Unit { ... }
}

-- Adapter must be declared in the function signature:
fn render(shape: Circle) -> Unit
where adapter: Circle as Drawable
{
    shape.draw(canvas)
}
```

Adapters are always scoped and explicit. They cannot cause global coherence violations.

---

## 12. Design Rationale

### Why No HKT

Higher-kinded types (`Functor`, `Monad`, etc.) were explicitly excluded because:

1. **Capabilities replace monads.** Spore's `uses [FileRead, NetWrite]` handles effect
   sequencing — the primary use case for monads in Haskell. An IO monad is unnecessary
   when effects are tracked by the capability system.

2. **GATs + associated types cover 80% of HKT use cases.** The remaining 20% (abstracting
   over container kinds generically) is rare in practice and can be handled via code
   generation.

3. **HKT error messages are terrible.** Even experienced Haskell developers struggle with
   kind-mismatch errors. Spore targets Elm-level error quality; HKT makes this impossible.

4. **Agents don't benefit.** Type-directed synthesis via holes works with trait bounds
   and associated types. HKT adds complexity to the type context without proportionally
   improving synthesis quality.

5. **Forward-compatible deferral.** If HKT proves necessary, it can be added as an
   opt-in language extension without breaking existing code.

### Why No SMT Solver

Full refinement types (Liquid Haskell-style) require an SMT solver for verification.
Spore rejects this because:

1. **SMT is unpredictable.** Slight changes to code can cause the solver to time out
   or produce different results, violating Spore's predictability principle.

2. **SMT errors are inscrutable.** When a refinement fails to verify, the error message
   is often "could not prove P" with no actionable guidance.

3. **L0 + L1 cover practical needs.** Decidable predicates (numeric bounds, size limits)
   plus abstract interpretation (range narrowing through control flow) handle the vast
   majority of real-world refinement needs.

4. **Implementation burden.** Integrating Z3 or similar adds significant compiler
   complexity and build-time dependency.

### Why Nominal-Primary

Structural typing (TypeScript, Go interfaces) was rejected as the default because:

1. **Capability safety requires nominal types.** If `FileRead` and `NetRead` were
   structural, code could accidentally gain capabilities by structural coincidence.
   This is a security boundary.

2. **Error messages are clearer.** "Expected `Temperature`, got `Pressure`" is better
   than "Expected `{ value: Float, unit: String }`, got `{ value: Float, unit: String }`"
   when two structurally identical types are semantically different.

3. **Refactoring is safer.** Renaming a nominal type is caught by the compiler everywhere.
   Structural matches silently survive renames that change semantics.

4. **Anonymous records provide the escape hatch.** For cases where structural typing
   genuinely helps (FFI, intermediate values, Agent hole-filling), anonymous records
   offer structural matching without compromising the nominal default.

### Why Sealed Enums

All enums are sealed (closed) because:

1. **Exhaustiveness checking requires closure.** If a new variant could be added from
   another module, exhaustive matching is impossible.

2. **Error sets depend on closure.** `! [Err1, Err2]` means exactly these two errors —
   not "these two plus whatever someone adds later."

3. **Agent reasoning requires completeness.** When an Agent generates match arms, it
   needs to know ALL possible cases.

4. **Open extensibility uses traits.** When open extension is genuinely needed, traits
   (not enums) are the right abstraction.

### Why Capability = Trait

Unifying capabilities and traits provides:

1. **Conceptual economy.** One mechanism (traits) handles both "what operations does
   this type support" and "what operations does this execution context provide."

2. **Signature-clause separation.** Trait bounds use `where`, effects and capabilities use `with` and
   `uses` — all in the function signature with consistent syntax.

3. **Composability.** Composite capabilities (`capability DB = [Read, Write]`) work exactly
   like trait supertypes.

4. **Implementation reuse.** The compiler's trait resolver, coherence checker, and
   constraint solver work for both data traits and capability traits.

### Why @allows

`@allows` exists because:

1. **Capabilities are necessary but not sufficient for trust.** A function may have
   `uses [Database]`, but the human might only trust specific database functions for
   a given hole.

2. **Architectural intent needs expression.** "Use the caching layer, not raw queries"
   is a design constraint that type signatures cannot express.

3. **Agent search space reduction.** With many capability-matching functions in scope,
   `@allows` narrows the candidates to a manageable, trusted set.

4. **Not a type — a synthesis constraint.** `@allows` doesn't change type checking. It
   changes what the Agent is permitted to propose. The compiler verifies compliance,
   but the type system remains unaffected.

### Why FuncCall/Module Removed

The original `FuncCall<f>` and `Module<m>` capabilities were removed because:

1. **Too granular.** Tracking every function call as a capability makes signatures
   unreadable and snapshot hashes unstable.

2. **Replaced by better mechanisms.**
   - **Imports** control which modules and functions are accessible.
   - **@allows** controls which functions an Agent may use for a specific hole.
   - **Call-graph query tool** (`sporec --call-graph`) provides the dependency
     analysis that `FuncCall` was trying to capture.

3. **Capability sets should be semantic.** `uses [FileRead, NetWrite]` describes *what
   effects occur*, not *which functions are called*. The latter is an implementation
   detail.

---

## Appendix A: Type System Interaction Matrix

How each type system feature interacts with Spore's other systems:

|  | Capabilities | Cost Model | Holes | Error Sets | Pattern Matching |
|---|---|---|---|---|---|
| **Generics** | Type bounds in `where`, capabilities via `with`/`uses` | Cost of generic calls | Type params flow into holes | Error sets are generic | Generic type destructuring |
| **Traits** | Capabilities ARE traits | Trait methods have costs | Trait bounds constrain holes | Error traits compose | Trait-based dispatch |
| **Enums** | ✗ (nominal only) | Construction has cost | Variant fields fill holes | Error enums in `! [...]` | Exhaustive matching |
| **Refinements** | ✗ | `cost ≤ N` IS refinement | Refinement narrows hole type | Error set IS refinement | Guard clause integration |
| **Const Generics** | ✗ | `cost ≤ N * M` | Size bounds in holes | ✗ | ✗ |
| **Inference** | Effects inferred in bodies | Costs inferred, checked | Hole types inferred | Error unions inferred | Exhaustiveness inferred |
| **@allows** | Orthogonal | Orthogonal | Constrains hole filling | Orthogonal | Orthogonal |

## Appendix B: Complete Worked Example

A full example showing multiple type system features working together:

```spore
-- types.spore
module billing.types

type Money = Float if self >= 0.0

type TaxRegion =
    | US { state: String }
    | EU { country: String }
    | Other { code: String }

type LineItem = {
    name: String,
    quantity: Int if self > 0,
    unit_price: Money,
}

type Invoice = {
    customer: Customer,
    items: Vec<LineItem>,
    subtotal: Money,
    tax: Money,
    total: Money,
}

type TaxError =
    | UnknownRegion { region: TaxRegion }
    | RateUnavailable { reason: String }

type ValidationError =
    | EmptyItems
    | InvalidQuantity { item: String, quantity: Int }
```

```spore
-- tax.spore
module billing.tax

import billing.types { Money, TaxRegion, TaxError }

capability TaxTable {
    fn lookup_rate(region: TaxRegion) -> Float ! [TaxError]
    cost lookup_rate <= 50
}

fn compute_tax(amount: Money, region: TaxRegion) -> Money ! [TaxError]
uses [TaxTable]
cost <= 200
{
    let rate = lookup_rate(region)?
    let tax = amount * rate
    tax
}
```

```spore
-- invoice.spore
module billing.invoice

import billing.types { Money, LineItem, Invoice, Customer, TaxRegion,
                       TaxError, ValidationError }
import billing.tax { compute_tax }

fn validate_items(items: Vec<LineItem>) -> Vec<LineItem> ! [ValidationError]
cost <= 500
{
    if items.is_empty() {
        raise EmptyItems
    }
    for item in items {
        if item.quantity <= 0 {
            raise InvalidQuantity { item: item.name, quantity: item.quantity }
        }
    }
    items
}

fn generate_invoice(
    customer: Customer,
    items: Vec<LineItem>,
    tax_region: TaxRegion,
) -> Invoice ! [TaxError, ValidationError]
uses [TaxTable]
cost <= 5000
{
    let validated = validate_items(items)?
    let subtotal = validated
        .map(|item| item.unit_price * item.quantity)
        .sum()
    let tax = compute_tax(amount: subtotal, region: tax_region)?
    let total = subtotal + tax

    Invoice {
        customer: customer,
        items: validated,
        subtotal: subtotal,
        tax: tax,
        total: total,
    }
}
```

```spore
-- invoice_display.spore
module billing.display

import billing.types { Invoice, LineItem, TaxRegion }

impl Display for LineItem {
    fn display(self) -> String {
        self.name ++ " x" ++ show(self.quantity) ++ " @ " ++ show(self.unit_price)
    }
}

impl Display for TaxRegion {
    fn display(self) -> String {
        match self {
            US { state }     => "US-" ++ state,
            EU { country }   => "EU-" ++ country,
            Other { code }   => code,
        }
    }
}

impl Display for Invoice {
    fn display(self) -> String {
        let header = "Invoice for " ++ self.customer.name ++ "\n"
        let items = self.items
            .map(|item| "  " ++ item.display())
            .join("\n")
        let footer = "\nSubtotal: " ++ show(self.subtotal)
            ++ "\nTax: " ++ show(self.tax)
            ++ "\nTotal: " ++ show(self.total)
        header ++ items ++ footer
    }
}
```

## Appendix C: Summary of Confirmed Decisions

| # | Decision | Status |
|---|---|---|
| 1 | Nominal-primary + anonymous structural records | **Confirmed** |
| 2 | Capability = Trait (unified) | **Confirmed** |
| 3 | Associated types + GATs | **Confirmed** |
| 4 | No HKT | **Confirmed** |
| 5 | Refinement types L0 + L1 (no SMT) | **Confirmed** |
| 6 | Sealed enums | **Confirmed** |
| 7 | Signatures explicit, bodies inferred | **Confirmed** |
| 8 | Const generics with arithmetic | **Confirmed** |
| 9 | Full pattern matching (exhaustive + nested + guard + or-pattern) | **Confirmed** |
| 10 | @allows (hole-level Agent constraint) | **Confirmed** |
| 11 | FuncCall/Module removed (replaced by imports + @allows + call-graph query) | **Confirmed** |
