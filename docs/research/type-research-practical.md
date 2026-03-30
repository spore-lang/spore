# Practical Type Systems in Mainstream Programming Languages

> Comprehensive research covering type system architecture, key features, strengths,
> pain points, and unique innovations across seven languages. Includes cross-cutting
> analysis on nominal/structural typing, type inference, error messages, and algebraic
> data types.

---

## Table of Contents

1. [Rust](#1-rust)
2. [TypeScript](#2-typescript)
3. [Python Typing](#3-python-typing)
4. [Kotlin](#4-kotlin)
5. [Swift](#5-swift)
6. [Scala 3](#6-scala-3)
7. [Haskell](#7-haskell)
8. [Cross-Cutting Analysis](#8-cross-cutting-analysis)
   - [Nominal vs Structural Typing](#81-nominal-vs-structural-typing)
   - [Type Inference](#82-type-inference)
   - [Error Messages](#83-error-messages)
   - [Algebraic Data Types](#84-algebraic-data-types)

---

## 1. Rust

### Type System Architecture

| Dimension | Description |
|-----------|-------------|
| **Foundation** | Hindley-Milner-inspired local inference + affine type system (ownership) |
| **Nominal vs Structural** | **Nominal**. Two structs with identical fields are distinct types. |
| **Subtyping** | Lifetime subtyping only (`'a: 'b`). No structural subtyping. Trait-based dispatch replaces OOP subtyping. |
| **Parametric Polymorphism** | Monomorphized generics — zero-cost abstractions, each instantiation compiled separately. |

### Key Type Features

#### 1.1 Ownership & Lifetimes

The defining innovation. Each value has exactly one owner; borrowing is tracked statically.

```rust
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}

fn main() {
    let s1 = String::from("long");
    let result;
    {
        let s2 = String::from("hi");
        result = longest(&s1, &s2);
    } // ERROR: s2 doesn't live long enough
}
```

**Rules enforced at compile time:**
- One mutable reference XOR any number of immutable references.
- References must never outlive the data they point to.
- No garbage collector — RAII-based deterministic destruction.

#### 1.2 Traits & Associated Types

Traits are Rust's interface abstraction. Associated types reduce generic noise:

```rust
trait Iterator {
    type Item;                              // Associated type
    fn next(&mut self) -> Option<Self::Item>;
}

impl Iterator for Counter {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        self.count += 1;
        if self.count <= 5 { Some(self.count) } else { None }
    }
}
```

#### 1.3 Const Generics

Types parameterized over compile-time constant values (stabilized):

```rust
struct Matrix<T, const ROWS: usize, const COLS: usize> {
    data: [[T; COLS]; ROWS],
}

impl<T: Default + Copy, const R: usize, const C: usize> Matrix<T, R, C> {
    fn new() -> Self {
        Matrix { data: [[T::default(); C]; R] }
    }
}

let m: Matrix<f64, 3, 3> = Matrix::new(); // Statically sized 3×3
```

#### 1.4 Generic Associated Types (GATs)

Stabilized in Rust 1.65. Associated types that are themselves generic — critical for lending iterators, async traits, and streaming APIs:

```rust
trait StreamingIterator {
    type Item<'a> where Self: 'a;
    fn next(&mut self) -> Option<Self::Item<'_>>;
}

// Now the yielded item can borrow from the iterator itself
struct WindowsMut<'s, T> { data: &'s mut [T], pos: usize }

impl<'s, T> StreamingIterator for WindowsMut<'s, T> {
    type Item<'a> = &'a mut [T] where Self: 'a;
    fn next(&mut self) -> Option<&mut [T]> {
        if self.pos + 2 <= self.data.len() {
            let window = &mut self.data[self.pos..self.pos + 2];
            self.pos += 1;
            Some(window)
        } else { None }
    }
}
```

#### 1.5 Enums as Algebraic Data Types

```rust
enum Shape {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
    Triangle { base: f64, height: f64 },
}

fn area(s: &Shape) -> f64 {
    match s {
        Shape::Circle { radius }           => std::f64::consts::PI * radius * radius,
        Shape::Rectangle { width, height }  => width * height,
        Shape::Triangle { base, height }    => 0.5 * base * height,
        // Exhaustive — compiler enforces all variants handled
    }
}
```

#### 1.6 HKT Workarounds

Rust has no native higher-kinded types. Common emulation via "type family" traits:

```rust
// Emulate HKT with a "type constructor" trait
trait HKT {
    type Applied<T>;
}

struct OptionHKT;
impl HKT for OptionHKT {
    type Applied<T> = Option<T>;
}

struct VecHKT;
impl HKT for VecHKT {
    type Applied<T> = Vec<T>;
}

// Now write code generic over the "container kind"
fn wrap_value<H: HKT>(val: i32) -> H::Applied<i32>
where H::Applied<i32>: From<i32> {
    H::Applied::<i32>::from(val)
}
```

**Limitation:** This is verbose and doesn't compose well for multi-parameter abstractions like `Monad`.

### What Works Well

- **Zero-cost memory safety** — no GC, no runtime overhead, bugs caught at compile time.
- **Fearless concurrency** — `Send`/`Sync` traits prevent data races statically.
- **Enum + `match`** — best-in-class algebraic data types with exhaustive checking.
- **Error messages** — extremely helpful, with `help:`, `note:`, and suggested fixes.
- **Trait system** — powerful, coherent (orphan rules prevent conflicting impls).

### What Causes Pain

- **Borrow checker friction** — self-referential structs, complex graph structures require `Pin`, `Rc<RefCell<>>`, or `unsafe`.
- **Lifetime annotation verbosity** — especially in async code with `Pin<Box<dyn Future<Output = ...> + Send + 'a>>`.
- **No HKT** — limits direct expression of `Functor`/`Monad` abstractions.
- **Compile times** — monomorphization leads to slow builds on large codebases.
- **Orphan rules** — can't implement external trait for external type; workaround = newtype wrapper.

### Unique Innovations

| Innovation | Description |
|-----------|-------------|
| **Ownership/Borrowing** | Compile-time memory management without GC — adopted by no other mainstream language at this depth |
| **`Send`/`Sync` auto-traits** | Data-race freedom as a type system property |
| **Const generics** | Type-level integers for statically-sized arrays/matrices |
| **`?` operator + `Result`** | Ergonomic, type-safe error handling without exceptions |

---

## 2. TypeScript

### Type System Architecture

| Dimension | Description |
|-----------|-------------|
| **Foundation** | Structural type system layered onto JavaScript |
| **Nominal vs Structural** | **Structural**. Compatibility based on shape, not declared names. |
| **Subtyping** | Structural subtyping — `{a: string, b: number}` is a subtype of `{a: string}` |
| **Erasure** | Types are completely erased at runtime — zero runtime representation |
| **Unsoundness** | Intentionally unsound in specific spots (e.g., bivariant function parameter checking for methods) for pragmatic JS interop |

### Key Type Features

#### 2.1 Union & Intersection Types

```typescript
// Union: value is ONE of these types
type Result = Success | Failure;
type StringOrNumber = string | number;

// Intersection: value has ALL properties of both
type Named = { name: string };
type Aged = { age: number };
type Person = Named & Aged; // { name: string; age: number }
```

#### 2.2 Type Narrowing & Control Flow Analysis

TypeScript tracks type refinements through control flow:

```typescript
function process(value: string | number | null) {
    if (value === null) return;         // value: string | number
    if (typeof value === "string") {
        console.log(value.toUpperCase()); // value: string
    } else {
        console.log(value.toFixed(2));    // value: number
    }
}
```

**Discriminated unions** enable exhaustive checking:

```typescript
type Shape =
    | { kind: "circle"; radius: number }
    | { kind: "rect"; w: number; h: number };

function area(s: Shape): number {
    switch (s.kind) {
        case "circle": return Math.PI * s.radius ** 2;
        case "rect":   return s.w * s.h;
        // If a new variant is added, TS errors here:
        default: const _exhaustive: never = s; return _exhaustive;
    }
}
```

#### 2.3 Mapped Types

Transform every property of an existing type:

```typescript
type Readonly<T> = { readonly [P in keyof T]: T[P] };
type Optional<T> = { [P in keyof T]?: T[P] };

// Key remapping (TS 4.1+)
type Getters<T> = {
    [K in keyof T as `get${Capitalize<string & K>}`]: () => T[K]
};
// { getName: () => string; getAge: () => number }
```

#### 2.4 Conditional Types

Type-level `if/else` with `infer` for extraction:

```typescript
type IsString<T> = T extends string ? true : false;

// Extract return type of a function
type ReturnType<T> = T extends (...args: any[]) => infer R ? R : never;

// Distributive over unions
type ToArray<T> = T extends any ? T[] : never;
type Result = ToArray<string | number>; // string[] | number[]
```

#### 2.5 Template Literal Types

String manipulation at the type level:

```typescript
type EventName<T extends string> = `on${Capitalize<T>}`;
type Events = EventName<"click" | "hover">; // "onClick" | "onHover"

// Typed CSS property builder
type CSSProperty = `${string}-${string}`;
const prop: CSSProperty = "background-color"; // ✓
```

#### 2.6 `satisfies` Operator (TS 4.9+)

Validate a type without widening:

```typescript
type Color = "red" | "green" | "blue";
type Theme = Record<string, Color | Color[]>;

const theme = {
    primary: "red",
    secondary: ["green", "blue"],
} satisfies Theme;

// theme.primary is still "red" (literal), NOT Color
// theme.secondary is still ["green", "blue"] (tuple), NOT Color[]
theme.primary.toUpperCase(); // ✓ — knows it's string literal "red"
```

### What Works Well

- **Structural typing** — perfectly matches JavaScript's duck-typed nature.
- **Type-level computation** — conditional types + template literals = Turing-complete type system.
- **Gradual adoption** — `any` as an escape hatch; strict mode for full safety.
- **IDE integration** — best-in-class IntelliSense; types drive autocomplete, refactoring, documentation.
- **`satisfies`** — validates without losing narrowed types; solves a major ergonomic gap.

### What Causes Pain

- **Unsoundness by design** — `any` leaks, array covariance, method bivariance.
- **Complex error messages** — deeply nested mapped/conditional types produce multi-line type diffs that are hard to read.
- **Runtime erasure** — no way to check types at runtime; must use separate validation (Zod, etc.).
- **`enum` issues** — numeric enums are unsound; prefer `as const` + union types.
- **Distributive conditional gotchas** — `T extends U ? X : Y` distributes over unions unexpectedly unless wrapped in `[T]`.

### Unique Innovations

| Innovation | Description |
|-----------|-------------|
| **Template literal types** | String-level type computation — unique to TypeScript |
| **Mapped types with key remapping** | Programmatic transformation of object shapes |
| **`satisfies`** | Type validation without type widening |
| **Discriminated union narrowing** | Automatic type refinement via literal discriminant fields |

---

## 3. Python Typing

### Type System Architecture

| Dimension | Description |
|-----------|-------------|
| **Foundation** | **Gradual typing** — optional annotations, no runtime enforcement by default |
| **Nominal vs Structural** | Primarily **nominal** (class hierarchy); **structural** via `Protocol` (PEP 544) |
| **Enforcement** | External type checkers (mypy, pyright, pyrefly, ty) — not the interpreter |
| **Evolution** | PEP-driven incremental improvement; new features added each Python release |

### PEP-by-PEP Feature Guide

#### PEP 484 — Type Hints (Python 3.5)

The foundation. Introduced `typing` module, `Optional`, `Union`, `List[int]`, function annotations:

```python
from typing import Optional, List

def greet(name: str, times: int = 1) -> str:
    return f"Hello, {name}! " * times

def find_user(user_id: int) -> Optional[dict]:
    ...  # Returns dict or None
```

#### PEP 526 — Variable Annotations (Python 3.6)

```python
name: str = "Alice"
count: int                # Declared but not assigned
items: list[str] = []
```

#### PEP 544 — Protocols / Structural Subtyping (Python 3.8)

Duck typing made static. No need to inherit — just match the shape:

```python
from typing import Protocol, runtime_checkable

@runtime_checkable
class Drawable(Protocol):
    def draw(self) -> None: ...

class Circle:
    def draw(self) -> None:
        print("○")

def render(obj: Drawable) -> None:  # Circle matches without inheriting Drawable
    obj.draw()

render(Circle())  # ✓ — structural match
```

#### PEP 586 — Literal Types (Python 3.8)

```python
from typing import Literal

def set_mode(mode: Literal["read", "write", "append"]) -> None: ...

set_mode("read")   # ✓
set_mode("delete")  # ✗ — type error
```

#### PEP 589 — TypedDict (Python 3.8)

Typed dictionaries with fixed string keys:

```python
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int
    score: float

m: Movie = {"name": "Inception", "year": 2010, "score": 8.8}
```

#### PEP 591 — `Final` (Python 3.8)

```python
from typing import Final

MAX_SIZE: Final = 100
MAX_SIZE = 200  # ✗ — type error: cannot reassign Final variable

class Base:
    def critical(self) -> None: ...  # @final prevents override in subclass
```

#### PEP 604 — Union with `|` (Python 3.10)

```python
# Before PEP 604
from typing import Union
def f(x: Union[int, str]) -> None: ...

# After PEP 604
def f(x: int | str) -> None: ...
def g(x: int | None) -> None: ...  # Replaces Optional[int]
```

#### PEP 612 — ParamSpec (Python 3.10)

Capture and forward function parameter types — essential for decorator typing:

```python
from typing import ParamSpec, TypeVar, Callable

P = ParamSpec("P")
R = TypeVar("R")

def logged(func: Callable[P, R]) -> Callable[P, R]:
    def wrapper(*args: P.args, **kwargs: P.kwargs) -> R:
        print(f"Calling {func.__name__}")
        return func(*args, **kwargs)
    return wrapper

@logged
def add(a: int, b: int) -> int:
    return a + b
# Type checker knows: add(a: int, b: int) -> int
```

#### PEP 646 — Variadic Generics (Python 3.11)

`TypeVarTuple` — generic over an arbitrary number of types:

```python
from typing import TypeVarTuple, Unpack

Ts = TypeVarTuple("Ts")

def head(first: int, *rest: Unpack[Ts]) -> int:
    return first

# Enables typing for NumPy-style shape-typed arrays:
# Array[Batch, Height, Width, Channels]
```

#### PEP 673 — `Self` Type (Python 3.11)

```python
from typing import Self

class Builder:
    def set_name(self, name: str) -> Self:
        self.name = name
        return self  # Correctly typed as the subclass in subclass usage

class SpecialBuilder(Builder):
    def set_extra(self, v: int) -> Self:
        self.extra = v
        return self
# SpecialBuilder().set_name("x").set_extra(1)  ✓ — returns SpecialBuilder
```

#### PEP 675 — `LiteralString` (Python 3.11)

Prevents SQL injection and similar attacks at the type level:

```python
from typing import LiteralString

def execute_sql(query: LiteralString) -> None: ...

execute_sql("SELECT * FROM users")  # ✓ — literal string
user_input = input()
execute_sql(user_input)  # ✗ — not a literal string
```

#### PEP 681 — Data Class Transforms (Python 3.11)

Tells type checkers that a decorator produces dataclass-like behavior:

```python
from typing import dataclass_transform

@dataclass_transform()
class ModelMeta(type): ...

class User(metaclass=ModelMeta):
    name: str
    age: int

# Type checker treats User like a dataclass:
u = User(name="Alice", age=30)  # ✓ constructor inferred
```

#### PEP 695 — Type Parameter Syntax (Python 3.12)

Cleaner, more readable generic syntax:

```python
# Before PEP 695
from typing import TypeVar
T = TypeVar("T")
def first(items: list[T]) -> T: ...

# After PEP 695
def first[T](items: list[T]) -> T: ...

type Point[T] = tuple[T, T]    # Type alias with parameter
type Matrix[T, N: int] = list[list[T]]
```

#### PEP 696 — Defaults for Type Parameters (Python 3.13)

```python
# T defaults to str if not specified
def process[T = str](items: list[T]) -> T: ...

process(["a", "b"])  # T inferred as str
process([1, 2])      # T inferred as int
```

#### PEP 702 — Deprecation Markers (Python 3.13)

```python
import warnings

@warnings.deprecated("Use new_func() instead")
def old_func() -> None: ...

old_func()  # Type checker shows deprecation warning
```

#### PEP 742 — Narrowing with `TypeIs` (Python 3.13)

Improved version of `TypeGuard` — narrows in **both** branches:

```python
from typing import TypeIs

def is_str_list(val: list[object]) -> TypeIs[list[str]]:
    return all(isinstance(x, str) for x in val)

def process(data: list[object]) -> None:
    if is_str_list(data):
        # data: list[str] — narrowed in True branch
        print(data[0].upper())
    else:
        # data: list[object] — correctly NOT list[str] in False branch
        pass
```

#### Newer PEPs (2024–2025)

| PEP | Title | Status | Python |
|-----|-------|--------|--------|
| 705 | `ReadOnly` TypedDict keys | Final | 3.13+ |
| 728 | TypedDict with Typed Extra Items | Proposed | 3.15+ |
| 746 | Type checking `Annotated` metadata | In progress | — |
| 764 | Inline typed dictionaries | Proposed | 3.15+ |
| 781 | `TYPE_CHECKING` as built-in constant | Proposed | 3.14+ |
| 800 | Disjoint bases in the type system | Draft | — |
| 821 | Unpacking TypedDicts in Callable | Draft | — |

### What Works Well

- **Gradual adoption** — add types incrementally; no all-or-nothing commitment.
- **Protocol** — structural subtyping matches Python's duck-typing culture perfectly.
- **Ecosystem maturity** — mypy, pyright, ty, pyrefly; excellent IDE support.
- **PEP evolution** — community-driven, steady improvement track.
- **`ParamSpec`** — finally makes decorator typing correct and practical.

### What Causes Pain

- **No runtime enforcement** — types are advisory; runtime `isinstance` doesn't check generic params.
- **Type checker divergence** — mypy, pyright, pyrefly don't always agree on edge cases.
- **Generic ergonomics** — pre-PEP 695 syntax is noisy (`TypeVar` boilerplate).
- **Circular import headaches** — `TYPE_CHECKING` + string annotations = fragile.
- **Performance of type checkers** — mypy can be slow on large codebases (pyright/ty are faster).

### Unique Innovations

| Innovation | Description |
|-----------|-------------|
| **PEP 612 ParamSpec** | Forwarding exact parameter signatures through decorators |
| **PEP 675 LiteralString** | Security-oriented type: prevents injection attacks at type level |
| **PEP 681 dataclass_transform** | Teaches type checkers about ORM/framework metaclass magic |
| **Gradual typing model** | Optional types that interoperate with untyped code seamlessly |

---

## 4. Kotlin

### Type System Architecture

| Dimension | Description |
|-----------|-------------|
| **Foundation** | Nominal type system with null safety built into the type hierarchy |
| **Nominal vs Structural** | **Nominal**. JVM-based; types identified by declaration. |
| **Subtyping** | `Any` at top (non-null), `Any?` for nullable, `Nothing` at bottom. Uses declaration-site variance (`out`/`in`). |
| **Null Safety** | Nullable types (`T?`) are a distinct supertype of `T`. Compiler enforces null checks. |

### Key Type Features

#### 4.1 Nullable Types & Null Safety

The type system distinguishes nullable from non-nullable at every level:

```kotlin
var name: String = "Alice"     // Cannot be null
var maybe: String? = null       // Can be null

// Safe call, Elvis, assertion
val len = maybe?.length         // Int? — null if maybe is null
val lenOrDefault = maybe?.length ?: 0  // Int — 0 if null
val lenForced = maybe!!.length  // Throws if null (escape hatch)
```

#### 4.2 Smart Casts

The compiler automatically narrows types after checks:

```kotlin
fun describe(obj: Any): String = when {
    obj is String -> "String of length ${obj.length}"  // Smart cast to String
    obj is Int && obj > 0 -> "Positive int: $obj"      // Smart cast to Int
    obj is List<*> -> "List of size ${obj.size}"        // Smart cast to List
    else -> "Unknown"
}

// Also works with null checks:
fun process(s: String?) {
    if (s != null) {
        println(s.length)  // s smart-cast to String (non-null)
    }
}
```

#### 4.3 Sealed Classes & Interfaces

Restricted hierarchies with exhaustive `when`:

```kotlin
sealed interface Result<out T> {
    data class Ok<T>(val value: T) : Result<T>
    data class Err(val message: String) : Result<Nothing>
    data object Loading : Result<Nothing>
}

fun <T> handle(r: Result<T>): String = when (r) {
    is Result.Ok      -> "Got: ${r.value}"
    is Result.Err     -> "Error: ${r.message}"
    is Result.Loading -> "Loading..."
    // Exhaustive — no else needed
}
```

#### 4.4 Inline (Value) Classes

Zero-overhead type wrappers:

```kotlin
@JvmInline
value class UserId(val id: String)

@JvmInline
value class Email(val value: String) {
    init { require("@" in value) { "Invalid email" } }
}

fun sendEmail(to: Email, from: UserId) { ... }
// sendEmail(UserId("123"), Email("a@b.com")) — compile error: argument order matters!
```

At runtime, `UserId` is just a `String` — no heap allocation for the wrapper.

#### 4.5 Context Parameters (replacing Context Receivers)

Implicit contextual dependencies (Beta in Kotlin 2.2+, replacing experimental context receivers):

```kotlin
interface Logger { fun log(msg: String) }
interface UserRepository { fun findUser(id: Int): User? }

context(logger: Logger, repo: UserRepository)
fun processUser(id: Int) {
    val user = repo.findUser(id)
    logger.log("Processing user: $user")
}

// At call site, contexts must be in scope
with(myLogger) {
    with(myRepo) {
        processUser(42)  // logger and repo resolved from context
    }
}
```

### What Works Well

- **Null safety** — eliminates NPEs by design; the #1 reason teams adopt Kotlin.
- **Smart casts** — automatic type narrowing reduces boilerplate significantly.
- **Sealed classes** — excellent ADT + exhaustive checking.
- **Coroutines** — structured concurrency with typed `Flow<T>` streams.
- **Java interop** — pragmatic `@Nullable` annotation bridging.

### What Causes Pain

- **Reified generics** — only in `inline` functions; JVM erasure leaks through.
- **Variance complexity** — declaration-site `out`/`in` + use-site `*` projection can confuse.
- **Context parameters** — still evolving (Beta); migration from context receivers.
- **Build times** — Kotlin compiler slower than Java's javac.
- **No union types** — must use sealed classes or `Any` + smart casts.

### Unique Innovations

| Innovation | Description |
|-----------|-------------|
| **Null safety in type system** | `T` vs `T?` — the gold standard for null safety adopted by many later languages |
| **Smart casts** | Automatic type narrowing after control-flow checks |
| **Inline/value classes** | Zero-cost newtype wrappers on JVM |
| **Context parameters** | Implicit, type-safe dependency provision |

---

## 5. Swift

### Type System Architecture

| Dimension | Description |
|-----------|-------------|
| **Foundation** | Protocol-oriented nominal type system with value semantics emphasis |
| **Nominal vs Structural** | **Nominal**. Protocols declare interfaces; conformance is explicit. |
| **Subtyping** | Class inheritance + protocol conformance. Structs/enums don't subtype. |
| **Generics** | Constrained generics with protocol requirements. Specialization for performance. |

### Key Type Features

#### 5.1 Protocol-Oriented Programming

Protocols with default implementations via extensions:

```swift
protocol Shape {
    func area() -> Double
}

extension Shape {
    func describe() -> String {
        "Shape with area \(area())"
    }
}

struct Circle: Shape {
    let radius: Double
    func area() -> Double { .pi * radius * radius }
}

struct Square: Shape {
    let side: Double
    func area() -> Double { side * side }
}
```

#### 5.2 Associated Types

Protocols with abstract type members:

```swift
protocol Container {
    associatedtype Element
    mutating func append(_ item: Element)
    var count: Int { get }
    subscript(i: Int) -> Element { get }
}

struct Stack<T>: Container {
    typealias Element = T     // Concrete associated type
    private var items: [T] = []
    mutating func append(_ item: T) { items.append(item) }
    var count: Int { items.count }
    subscript(i: Int) -> T { items[i] }
}
```

#### 5.3 Opaque Types (`some`)

Hide concrete types while preserving type identity (SE-0244):

```swift
func makeShape() -> some Shape {
    Circle(radius: 5.0)
    // Caller sees "some Shape" — can't know it's Circle
    // But compiler knows — enables static dispatch + optimization
}

// Opaque parameter types (SE-0341)
func render(_ shape: some Shape) {
    print(shape.area())
}
```

**Contrast with existential types:**
```swift
// Existential (type-erased) — any Shape can flow through
func renderAny(_ shape: any Shape) { ... }

// Opaque — compiler knows concrete type, enables optimizations
func renderOpaque(_ shape: some Shape) { ... }
```

#### 5.4 Enums with Associated Values

Swift's ADTs:

```swift
enum NetworkResult {
    case success(Data)
    case failure(Error)
    case loading(progress: Double)
}

func handle(_ result: NetworkResult) {
    switch result {
    case .success(let data):
        print("Got \(data.count) bytes")
    case .failure(let error):
        print("Failed: \(error)")
    case .loading(let progress):
        print("Loading: \(Int(progress * 100))%")
    // Exhaustive — compiler enforces all cases
    }
}
```

#### 5.5 Result Builders (SE-0289)

Type-safe DSL construction — powers SwiftUI:

```swift
@resultBuilder
struct HTMLBuilder {
    static func buildBlock(_ components: String...) -> String {
        components.joined(separator: "\n")
    }
    static func buildOptional(_ component: String?) -> String {
        component ?? ""
    }
}

func html(@HTMLBuilder _ content: () -> String) -> String {
    "<html>\n\(content())\n</html>"
}

let page = html {
    "<head><title>Hello</title></head>"
    "<body>World</body>"
}
```

### What Works Well

- **Protocol-oriented design** — composition over inheritance; well-suited for value types.
- **`some`/`any` distinction** — clear separation of opaque vs existential; drives performance.
- **Value types by default** — structs are copied; safer concurrency model.
- **SwiftUI + result builders** — type-safe declarative UI is a major success story.
- **Exhaustive switch** — compiler enforces handling all enum cases.

### What Causes Pain

- **Protocol with associated types (PAT)** — can't use as existential directly; requires `some`/`any` or generics.
- **ABI stability constraints** — limits type system evolution on Apple platforms.
- **Generics complexity** — `where` clauses can get very verbose.
- **No union types** — must use enums or protocol existentials.
- **Error messages** — improving but still opaque for complex generic constraints.

### Unique Innovations

| Innovation | Description |
|-----------|-------------|
| **`some`/`any` keyword pair** | Explicit distinction between opaque and existential types |
| **Result builders** | Type-safe DSL construction; enables SwiftUI |
| **Protocol extensions with defaults** | Composition-oriented alternative to abstract classes |
| **Value semantics emphasis** | Structs + copy-on-write as the default design pattern |

---

## 6. Scala 3

### Type System Architecture

| Dimension | Description |
|-----------|-------------|
| **Foundation** | DOT calculus (Dependent Object Types); unified type system |
| **Nominal vs Structural** | Primarily **nominal** with structural type support. Union/intersection types blur the line. |
| **Subtyping** | Full subtyping lattice. `Any` at top, `Nothing` at bottom. Union `A \| B` and intersection `A & B`. |
| **Paradigm** | Seamlessly mixes OOP and FP with implicits (now `given`/`using`). |

### Key Type Features

#### 6.1 Union Types

```scala
def handle(input: Int | String): String = input match
  case i: Int    => s"Number: $i"
  case s: String => s"Text: $s"

// No wrapper type needed — direct type union
val x: Int | String = if true then 42 else "hello"
```

#### 6.2 Intersection Types

```scala
trait Drawable:
  def draw(): Unit

trait Serializable:
  def serialize(): Array[Byte]

// Value must satisfy BOTH
def process(obj: Drawable & Serializable): Unit =
  obj.draw()
  val bytes = obj.serialize()
```

#### 6.3 Match Types

Type-level pattern matching — compute types from types:

```scala
type Elem[X] = X match
  case String       => Char
  case Array[t]     => t
  case List[t]      => t
  case Option[t]    => t

val c: Elem[String] = 'a'         // Char
val i: Elem[List[Int]] = 42       // Int
val s: Elem[Array[String]] = "hi" // String
```

#### 6.4 Opaque Types

Zero-cost type wrappers:

```scala
object Quantity:
  opaque type Kilograms = Double

  def apply(value: Double): Kilograms = value
  extension (kg: Kilograms)
    def value: Double = kg
    def +(other: Kilograms): Kilograms = kg + other

import Quantity.*
val weight: Kilograms = Quantity(75.5)
// Outside this scope, Kilograms ≠ Double — type safety preserved
// Inside, it IS Double — zero runtime overhead
```

#### 6.5 Type Lambdas

Anonymous type constructors for higher-kinded programming:

```scala
// Type lambda: a type constructor that takes one parameter
type StringMap = [V] =>> Map[String, V]

val m: StringMap[Int] = Map("a" -> 1, "b" -> 2) // Map[String, Int]

// Useful for partially applying type constructors
trait Functor[F[_]]:
  extension [A](fa: F[A])
    def map[B](f: A => B): F[B]

// Can now use: Functor[[A] =>> Either[String, A]]
```

#### 6.6 Dependent Function Types

Return type depends on argument value:

```scala
trait Entry:
  type Value
  val value: Value

def extractValue(entry: Entry): entry.Value = entry.value

object IntEntry extends Entry:
  type Value = Int
  val value = 42

val result: Int = extractValue(IntEntry) // Return type is Int, computed from argument
```

#### 6.7 Context Functions (`given`/`using`)

```scala
trait ExecutionContext
trait Database

// Context function type: takes an implicit parameter
type Transactional[T] = Database ?=> T

def withTransaction[T](body: Transactional[T])(using db: Database): T =
  body(using db)
```

### What Works Well

- **Union/intersection types** — natural, lightweight alternatives to wrapper types.
- **Match types** — type-level computation without boilerplate type classes.
- **Opaque types** — zero-cost domain types; replaces `AnyVal` wrappers.
- **`given`/`using`** — cleaner than Scala 2 implicits; more principled.
- **Full HKT support** — type lambdas + higher-kinded type parameters natively.

### What Causes Pain

- **Complexity** — the type system is very powerful but has a steep learning curve.
- **Slow compilation** — especially with heavy implicit resolution and match types.
- **Migration from Scala 2** — `implicit` → `given`/`using` migration is non-trivial.
- **JVM erasure** — runtime type information lost for generics.
- **Library ecosystem fragmentation** — Scala 2 vs 3 library split.

### Unique Innovations

| Innovation | Description |
|-----------|-------------|
| **Match types** | Type-level pattern matching — reduces type class boilerplate |
| **Union + intersection types** | First-class; no wrapper overhead |
| **Type lambdas** | `[A] =>> F[A]` — clean partial application at the type level |
| **Dependent function types** | Return type depends on argument value (limited) |
| **Opaque type aliases** | Zero-cost newtype pattern built into the language |

---

## 7. Haskell

### Type System Architecture

| Dimension | Description |
|-----------|-------------|
| **Foundation** | System F derivative (System FC in GHC) with Hindley-Milner inference |
| **Nominal vs Structural** | **Nominal** for data types, **structural** for type class constraints |
| **Subtyping** | No subtyping. Polymorphism through type classes and parametric polymorphism. |
| **Purity** | Types encode effects — `IO a` for impure computation, pure by default |
| **Extensions** | GHC extends the core with ~100+ language extensions |

### Key Type Features

#### 7.1 Type Classes

Ad-hoc polymorphism with coherent global instances:

```haskell
class Eq a where
  (==) :: a -> a -> Bool
  (/=) :: a -> a -> Bool
  x /= y = not (x == y)  -- Default implementation

instance Eq Color where
  Red   == Red   = True
  Green == Green = True
  Blue  == Blue  = True
  _     == _     = False

-- Multi-parameter type class
class Convertible a b where
  convert :: a -> b

instance Convertible String Int where
  convert = read
```

#### 7.2 Higher-Kinded Types (HKT)

Abstract over type constructors — Haskell's defining feature for generic programming:

```haskell
class Functor f where
  fmap :: (a -> b) -> f a -> f b

-- f has kind * -> * (a type constructor)
instance Functor [] where
  fmap = map

instance Functor Maybe where
  fmap _ Nothing  = Nothing
  fmap f (Just x) = Just (f x)

-- Monad: higher-kinded abstraction
class Functor m => Monad m where
  return :: a -> m a
  (>>=)  :: m a -> (a -> m b) -> m b
```

#### 7.3 GADTs (Generalized Algebraic Data Types)

Constructors with refined return types — encode invariants:

```haskell
{-# LANGUAGE GADTs #-}

data Expr a where
  IntLit  :: Int  -> Expr Int
  BoolLit :: Bool -> Expr Bool
  Add     :: Expr Int  -> Expr Int  -> Expr Int
  If      :: Expr Bool -> Expr a    -> Expr a -> Expr a
  Eq      :: Eq a => Expr a -> Expr a -> Expr Bool

-- Type-safe evaluation — impossible to add a Bool to an Int
eval :: Expr a -> a
eval (IntLit n)    = n
eval (BoolLit b)   = b
eval (Add x y)     = eval x + eval y
eval (If c t e)    = if eval c then eval t else eval e
eval (Eq x y)      = eval x == eval y
```

#### 7.4 Type Families

Type-level functions — compute types from types:

```haskell
{-# LANGUAGE TypeFamilies #-}

-- Closed type family (all equations defined together)
type family Element a where
  Element [a]        = a
  Element (Maybe a)  = a
  Element String     = Char

-- Associated type family (bound to a type class)
class Container c where
  type Elem c
  empty :: c
  insert :: Elem c -> c -> c

instance Container [a] where
  type Elem [a] = a
  empty = []
  insert = (:)
```

#### 7.5 DataKinds

Promote data constructors to the type level:

```haskell
{-# LANGUAGE DataKinds, KindSignatures, GADTs #-}

data Nat = Zero | Succ Nat  -- Value level

-- Type-level natural numbers via DataKinds
data Vec (n :: Nat) a where
  Nil  :: Vec 'Zero a
  Cons :: a -> Vec n a -> Vec ('Succ n) a

-- Type-safe head — can't call on empty vector
head :: Vec ('Succ n) a -> a
head (Cons x _) = x

-- This is a compile error:
-- head Nil  -- Type error: Vec 'Zero a ≠ Vec ('Succ n) a
```

#### 7.6 Linear Types (GHC 9.0+)

Track resource usage: values must be consumed exactly once:

```haskell
{-# LANGUAGE LinearTypes #-}

-- The %1 means the argument must be used exactly once
duplicate :: a %1 -> (a, a)  -- ERROR: can't use 'a' twice

-- Correct usage
consume :: a %1 -> ()
consume _ = ()

-- Practical: safe file handles
withFile :: FilePath -> (Handle %1 -> IO a) -> IO a
-- The handle MUST be consumed (closed) exactly once
```

#### 7.7 TypeInType / PolyKinds

Types and kinds unified — kinds can be as expressive as types:

```haskell
{-# LANGUAGE PolyKinds, DataKinds, TypeFamilies #-}

type family Map (f :: a -> b) (xs :: [a]) :: [b] where
  Map f '[]       = '[]
  Map f (x ': xs) = f x ': Map f xs
```

### What Works Well

- **Type inference** — Hindley-Milner infers almost everything; minimal annotation.
- **HKT + type classes** — unmatched for generic programming (`Functor`, `Monad`, etc.).
- **GADTs** — encode complex invariants in types; used for DSLs, typed ASTs.
- **Purity** — `IO` monad makes effects explicit and composable.
- **Equational reasoning** — types enable refactoring with mathematical confidence.

### What Causes Pain

- **Extension soup** — GHC extensions (`{-# LANGUAGE ... #-}`) fragment the language.
- **Type error messages** — can be cryptic for complex type-level code.
- **Orphan instances** — non-local type class instances cause coherence issues.
- **String types** — `String` is `[Char]` (slow list); `Text` vs `ByteString` confusion.
- **Lazy evaluation** — space leaks are hard to diagnose; strict-by-default emerging.
- **Learning curve** — monad transformers, existential types, rank-N polymorphism are steep.

### Unique Innovations

| Innovation | Description |
|-----------|-------------|
| **Type classes** | Coherent ad-hoc polymorphism — copied by Rust (traits), Scala (given/using), Swift (protocols) |
| **Higher-kinded types** | Abstract over type constructors — no other mainstream language has this fully |
| **GADTs** | Refined constructors encoding invariants — adopted in OCaml, Scala |
| **DataKinds** | Promote values to types — enables type-level natural numbers, etc. |
| **Linear types** | Resource tracking at the type level — inspired by linear logic |
| **`IO` monad** | Effects tracked in the type system — purity enforced by types |

---

## 8. Cross-Cutting Analysis

### 8.1 Nominal vs Structural Typing

#### Comparison Matrix

| Language | Primary Model | Structural Elements | Nominal Elements |
|----------|--------------|-------------------|-----------------|
| **Rust** | Nominal | — | Structs, enums, traits (explicit `impl`) |
| **TypeScript** | Structural | Objects, functions, interfaces | Classes (partially); branded types (workaround) |
| **Python** | Hybrid | `Protocol` (PEP 544) | Classes, ABC |
| **Kotlin** | Nominal | — | Classes, interfaces, sealed classes |
| **Swift** | Nominal | — | Protocols (explicit conformance), structs |
| **Scala 3** | Nominal (hybrid) | Structural types, union/intersection | Classes, traits, opaque types |
| **Haskell** | Nominal (types) / Structural (constraints) | Type class constraints match structurally | `data`, `newtype` are nominal |

#### Why Structural?

TypeScript chose structural typing because JavaScript is inherently duck-typed. Objects are bags of properties; requiring explicit interface implementation would break interop with existing JS code.

```typescript
// This "just works" in TypeScript — no implements declaration needed
interface HasName { name: string }
const obj = { name: "Alice", age: 30 };
const named: HasName = obj; // ✓ — structural match
```

#### Why Nominal?

Rust chose nominal typing because distinct types with identical structures should not be interchangeable:

```rust
struct Meters(f64);
struct Seconds(f64);

// These are DIFFERENT types even though both wrap f64
fn speed(distance: Meters, time: Seconds) -> f64 {
    distance.0 / time.0
}
// speed(Seconds(5.0), Meters(100.0)) — compile error!
```

#### Hybrid Approaches

Python's `Protocol` lets you opt into structural subtyping per-type:

```python
from typing import Protocol

class Quackable(Protocol):          # Structural — any class with quack() matches
    def quack(self) -> str: ...

class Duck:                          # No inheritance from Quackable
    def quack(self) -> str:
        return "Quack!"

def make_noise(animal: Quackable) -> None:
    print(animal.quack())

make_noise(Duck())  # ✓ — structural match via Protocol
```

### 8.2 Type Inference

#### Inference Power Spectrum

```
More inference ←──────────────────────────────→ More annotations
   Haskell     Scala 3    Rust     Kotlin    Swift    TypeScript    Python
   (HM)        (local+    (local   (local    (local   (local        (gradual;
                bidi)      bidi)    flow)     bidi)    flow)         optional)
```

#### Language-by-Language

| Language | Function Signatures | Local Variables | Generic Args | Return Types |
|----------|-------------------|----------------|-------------|-------------|
| **Haskell** | Inferred (recommended to annotate) | Fully inferred | Fully inferred | Fully inferred |
| **Rust** | Parameters: explicit. Return: explicit. | `let x = 5;` inferred | Often inferred (turbofish `::` for ambiguity) | Must annotate in `fn` signatures |
| **TypeScript** | Parameters: explicit (usually). Return: inferred. | Fully inferred | Usually inferred from arguments | Inferred but explicit recommended in public APIs |
| **Kotlin** | Parameters: explicit. Return: usually inferred. | `val x = 5` inferred | Usually inferred | Inferred for `=`-expression functions |
| **Swift** | Parameters: explicit. Return: explicit in functions. | `let x = 5` inferred | Usually inferred | Required in `func` declarations |
| **Scala 3** | Parameters: explicit. Return: recommended. | Fully inferred | Often inferred | Inferred for non-recursive functions |
| **Python** | Optional (gradual) | Optional | Optional | Optional |

#### The "Annotation Budget" Concept

Different languages strike different balances between how much the programmer must write and how much the compiler infers:

- **Haskell** — Near-zero mandatory annotations. Convention is to annotate top-level functions for documentation.
- **Rust** — Function signatures are fully explicit (parameters + return); locals are inferred. This is deliberate: function boundaries are the documentation boundary.
- **TypeScript** — Parameters usually annotated, returns inferred. The `--strict` flag raises the bar.
- **Kotlin/Swift** — Parameters explicit, local variables inferred. A practical middle ground.

#### Bidirectional Type Checking

Used by Rust, Swift, Scala 3, and parts of TypeScript. The key idea: type information flows in two directions:

1. **Synthesis mode** (bottom-up): "What type does this expression have?"
2. **Checking mode** (top-down): "Does this expression match the expected type?"

```rust
// Bidirectional checking in action:
let items: Vec<String> = vec!["a", "b", "c"]
    .into_iter()
    .map(|s| s.to_string())  // Closure param type inferred from context
    .collect();                // Return type inferred from let binding
```

### 8.3 Error Messages

#### Rankings (Community Consensus)

| Rank | Language | Quality | Approach |
|------|----------|---------|----------|
| 🥇 | **Elm** | Best-in-class | Long, conversational explanations; "Did you mean…?" suggestions |
| 🥈 | **Rust** | Excellent | Detailed spans, `help:` suggestions, links to docs, colored output |
| 🥉 | **Kotlin** | Very Good | Clear, concise; smart cast suggestions; IDE-integrated |
| 4 | **Swift** | Good (improving) | Improved in recent versions; still struggles with complex generics |
| 5 | **TypeScript** | Moderate | Terse but powerful in IDE; cryptic for complex mapped/conditional types |
| 6 | **Scala 3** | Improving | Better than Scala 2; match type errors still opaque |
| 7 | **Haskell** | Variable | Simple cases: excellent. Complex type-level code: bewildering |

#### Elm's Approach — The Gold Standard

Elm pioneered "compiler as assistant" philosophy:

```
-- TYPE MISMATCH ----------------------------------------- src/Main.elm

The 2nd argument to `map` is not what I expect:

   8| List.map String.toUpper [1, 2, 3]
                               ^^^^^^^
This argument is a list of type:

    List number

But `String.toUpper` needs the 1st argument to be:

    String

Hint: I always figure out the type of arguments from left to right.
If an argument is acceptable when I check it, I assume it is "correct"
and move on. So the problem may actually be in how previous arguments
interact with this one.
```

**Key principles:** explain both what was expected and what was found; suggest fixes; avoid jargon.

#### Rust's Approach — Explain the "Why"

```
error[E0502]: cannot borrow `v` as mutable because it is also borrowed as immutable
 --> src/main.rs:4:5
  |
3 |     let first = &v[0];
  |                  - immutable borrow occurs here
4 |     v.push(4);
  |     ^^^^^^^^^ mutable borrow occurs here
5 |     println!("{first}");
  |               ------- immutable borrow later used here
  |
  = help: consider cloning the value if it is owned
```

**Key principles:** highlight exact spans; explain the borrow checker's reasoning; provide actionable `help:` suggestions.

#### TypeScript's Structural Mismatch Errors

```
Type '{ name: string; age: number; }' is not assignable to
  type '{ name: string; age: string; }'.
  Types of property 'age' are incompatible.
    Type 'number' is not assignable to type 'string'.
```

**Key principles:** hierarchical diff of structural types; pinpoint exact property mismatch. Works well for shallow objects; becomes hard to read for deeply nested conditional/mapped types.

### 8.4 Algebraic Data Types

#### Sum Types Across Languages

| Language | Syntax | Exhaustiveness | Data Attached? |
|----------|--------|----------------|---------------|
| **Rust** | `enum Foo { A(i32), B { x: f64 } }` | ✅ Enforced | Yes — tuple or struct variants |
| **TypeScript** | `type Foo = A \| B` (discriminated union) | ⚠️ Manual (`never` trick) | Separate types with discriminant field |
| **Python** | No native sum types; `Union`, `Literal`, enum | ❌ Not enforced | Via `@dataclass` + `Union` |
| **Kotlin** | `sealed class/interface` | ✅ Enforced in `when` | Yes — data classes as subclasses |
| **Swift** | `enum Foo { case a(Int), b(String) }` | ✅ Enforced in `switch` | Yes — associated values |
| **Scala 3** | `enum` or `sealed trait` + `case class` | ✅ Enforced in `match` | Yes — case class parameters |
| **Haskell** | `data Foo = A Int \| B String` | ✅ Enforced (warning) | Yes — constructor parameters |

#### Pattern Matching Comparison

**Rust** — most complete: guards, bindings, nested patterns, `@` bindings:

```rust
match value {
    Shape::Circle { radius } if radius > 10.0 => println!("big circle"),
    Shape::Circle { radius: r @ 0.0..=10.0 }  => println!("small circle r={r}"),
    Shape::Rectangle { width, height } if width == height => println!("square!"),
    other => println!("other: {:?}", other),
}
```

**Haskell** — concise, with view patterns and guards:

```haskell
describe :: Shape -> String
describe (Circle r) | r > 10    = "big circle"
                    | otherwise = "small circle"
describe (Rectangle w h) | w == h = "square"
describe _ = "other shape"
```

**TypeScript** — manual, but functional with discriminated unions:

```typescript
type Shape =
    | { kind: "circle"; radius: number }
    | { kind: "rect"; width: number; height: number };

function describe(s: Shape): string {
    switch (s.kind) {
        case "circle":
            return s.radius > 10 ? "big circle" : "small circle";
        case "rect":
            return s.width === s.height ? "square" : "rectangle";
    }
}
```

#### Sealed Hierarchies

The middle ground between enums and open class hierarchies. Used in Kotlin, Scala, Swift for modelling finite state:

```kotlin
// Kotlin sealed interface — the sweet spot
sealed interface UIState {
    data object Loading : UIState
    data class Success(val data: List<Item>) : UIState
    data class Error(val message: String, val retryable: Boolean) : UIState
}

// Exhaustive, with data attached, extensible within the file
fun render(state: UIState) = when (state) {
    is UIState.Loading -> showSpinner()
    is UIState.Success -> showItems(state.data)
    is UIState.Error   -> showError(state.message, state.retryable)
}
```

---

## Summary: Feature Presence Matrix

| Feature | Rust | TS | Python | Kotlin | Swift | Scala 3 | Haskell |
|---------|------|----|--------|--------|-------|---------|---------|
| **Null safety (type-level)** | `Option<T>` | `\| null` | `Optional` | `T?` ✅ | `Optional` | `Option` | `Maybe` |
| **Sum types / ADTs** | `enum` ✅ | Union types | `Union` ⚠️ | `sealed` ✅ | `enum` ✅ | `enum`/`sealed` ✅ | `data` ✅ |
| **Pattern matching** | `match` ✅ | `switch` ⚠️ | `match` (3.10) | `when` ✅ | `switch` ✅ | `match` ✅ | Case exprs ✅ |
| **Exhaustiveness** | ✅ | ⚠️ Manual | ❌ | ✅ | ✅ | ✅ | ✅ (warning) |
| **Higher-kinded types** | ❌ (workaround) | ❌ | ❌ | ❌ | ❌ | ✅ | ✅ |
| **Type classes / traits** | Traits ✅ | — | Protocol | Interface | Protocol | given/using | Type classes ✅ |
| **Const generics** | ✅ | ❌ | ❌ | ❌ | ❌ | Literal types | DataKinds |
| **GADTs** | ⚠️ Partial | ❌ | ❌ | ❌ | ❌ | ⚠️ Match types | ✅ |
| **Dependent types** | ❌ | ❌ | ❌ | ❌ | ❌ | ⚠️ Limited | ⚠️ Singletons |
| **Linear/affine types** | Affine ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ (GHC 9.0+) |
| **Type inference** | Local | Local | Optional | Local | Local | Local+HM | Global HM |
| **Structural typing** | ❌ | ✅ | Protocol ✅ | ❌ | ❌ | ⚠️ Partial | ⚠️ Constraints |
| **Union types** | ❌ (enum) | ✅ | `\|` syntax | ❌ | ❌ | ✅ | ❌ |
| **Intersection types** | ❌ (trait bounds) | `&` ✅ | ❌ | ❌ | ❌ | `&` ✅ | ❌ |
| **Opaque types** | — | — | — | — | `some` ✅ | `opaque` ✅ | — |
| **Gradual typing** | ❌ | `any` ⚠️ | ✅ | ❌ | ❌ | ❌ | ❌ |

---

## Key Takeaways for Language Design

1. **Null safety should be type-level.** Kotlin proved `T?` eliminates NPEs. Every new language should adopt this.

2. **Sum types + exhaustive matching are table stakes.** Rust, Kotlin, Swift, Scala all do this well. TypeScript's discriminated unions are a creative workaround.

3. **Structural typing suits dynamic ecosystems.** TypeScript's structural approach is perfect for JavaScript interop. For systems languages, nominal typing prevents accidental type confusion.

4. **Type inference should stop at function boundaries.** Rust and Kotlin's approach (explicit signatures, inferred locals) hits the sweet spot of documentation vs. ergonomics.

5. **Error messages are a feature.** Elm and Rust showed that investing in error message quality pays massive dividends in developer adoption and productivity.

6. **Gradual typing enables adoption.** Python and TypeScript proved that optional types can be adopted incrementally. This is critical for large existing codebases.

7. **Higher-kinded types are powerful but niche.** Only Haskell and Scala 3 support them fully. Most codebases don't need them; those that do really need them.

8. **Zero-cost abstractions matter.** Rust's const generics, Kotlin's inline classes, Scala's opaque types — the best type features add no runtime cost.
