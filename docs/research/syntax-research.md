Agent completed. agent_id: syntax-research, agent_type: explore, status: completed, description: Research language syntax design, elapsed: 216s, total_turns: 0, duration: 135s

# Comprehensive Syntax Design Analysis Across Modern Languages

I'll provide a detailed comparative analysis of syntax design across these 10 languages, covering all requested dimensions.

---

## Individual Language Profiles

### 1. RUST

**1. Keywords & Philosophy**
- **Count**: 39 keywords (minimal-to-moderate)
- **Philosophy**: Expression-based, safety-first, explicit over implicit
- **Key keywords**: `let`, `mut`, `fn`, `struct`, `enum`, `impl`, `trait`, `match`, `if`, `loop`, `while`, `for`, `unsafe`, `async`, `await`, `pub`, `mod`, `use`, `crate`

**2. Operators**
- **Arithmetic**: `+`, `-`, `*`, `/`, `%`
- **Comparison**: `==`, `!=`, `<`, `>`, `<=`, `>=`
- **Logical**: `&&`, `||`, `!`
- **Bitwise**: `&`, `|`, `^`, `<<`, `>>`
- **Assignment**: `=`, `+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`, `^=`, `<<=`, `>>=`
- **Range**: `..`, `..=` (inclusive)
- **Type coercion**: `as` (explicit casting)
- **Custom operators**: ❌ Not allowed (fixed set only)

**3. Syntactic Sugar**
- Method call syntax: `obj.method()` desugars to `Type::method(obj)`
- Range patterns: `1..=5` in match expressions
- Closure syntax: `|x| x + 1` with automatic type inference
- Lifetime shorthand: `'_` for elided lifetimes
- `?` operator for `Result`/`Option` propagation (monadic sugar)
- Module paths: `use crate::module::Type`
- Destructuring in `let` and function parameters

**4. Comments**
- Line: `// comment`
- Block: `/* nested /* supported */ */`
- Doc comments: `/// Doc above`, `//! Module doc`

**5. String Literals**
- Regular: `"hello"`
- Raw: `r#"path\like\this"#` (escape sequences ignored)
- Byte string: `b"bytes"`
- Raw byte string: `br#"bytes"#`
- **Interpolation**: ❌ None in literals; uses `format!` macro: `format!("{}", var)`
- **Multiline**: Implicit with newline escape in literals

**6. Control Flow**
```rust
// if-else (expressions!)
let x = if cond { 5 } else { 6 };

// Pattern matching (not switch)
match value {
    1 => println!("one"),
    2..=5 => println!("two to five"),
    _ => println!("other"),
}

// Loops with labels
'outer: loop {
    'inner: loop {
        break 'outer; // labeled break
    }
}

// for-in with ranges
for i in 0..10 { }
for item in vec.iter() { }

// Early return
return value;
```

**7. Type Annotation Syntax**
```rust
let x: i32 = 5;
fn add(a: i32, b: i32) -> i32 { a + b }
struct Point { x: i32, y: i32 }
impl Point { fn distance(&self) -> f64 { } }
trait Drawable { fn draw(&self); }
```

**8. Error Handling**
- Primary: `Result<T, E>` enum
- `?` operator for error propagation
- No exceptions; `panic!` for unrecoverable errors
- `match` or `.unwrap()` for explicit handling

**9. Lambda/Closure Syntax**
```rust
|x: i32| x + 1           // With type annotation
|x| x + 1                // Type inference
||x + 1                  // No parameters
move |x| x + y           // Capture by move
```

**10. Unique Features**
- **Lifetime annotations**: `fn borrow<'a>(s: &'a str)`
- **Macro system**: `println!`, `vec!` with `!` suffix
- **Expression-based everything**: No semicolon = return value
- **Deref coercion**: `&String` automatically coerces to `&str`
- **Trait objects**: `dyn Trait` for dynamic dispatch
- **Unsafe blocks**: `unsafe { }` for bypassing borrow checker

---

### 2. KOTLIN

**1. Keywords & Philosophy**
- **Count**: ~50 keywords
- **Philosophy**: Pragmatic conciseness, null-safety by default, interop-friendly
- **Key keywords**: `val`, `var`, `fun`, `class`, `object`, `data`, `sealed`, `interface`, `when`, `if`, `for`, `while`, `return`, `fun`, `in`, `is`, `by`, `delegate`

**2. Operators**
- **Arithmetic**: `+`, `-`, `*`, `/`, `%`
- **Comparison**: `==`, `!=`, `<`, `>`, `<=`, `>=`
- **Logical**: `&&`, `||`, `!`
- **Bitwise**: `and`, `or`, `xor`, `inv`, `shl`, `shr`, `ushr` (infix functions)
- **Assignment**: `=`, `+=`, `-=`, `*=`, `/=`, `%=`
- **Null-coalescing**: `?:` (Elvis operator)
- **Smart cast**: `is` for type check + implicit cast
- **Scope functions**: `.let`, `.apply`, `.run`, `.with`, `.also`
- **Custom operators**: ✅ Limited; can overload `operator fun`

**3. Syntactic Sugar**
- `?.` safe call operator: `obj?.method()` returns null if obj is null
- `!!` not-null assertion: `obj!!.method()`
- String interpolation: `"Hello $name, age ${age + 1}"`
- Destructuring: `val (a, b) = Pair(1, 2)`
- Trailing lambda: `list.map { it + 1 }`
- Extension functions: `fun String.double() = this + this`
- Scope functions: `.let`, `.apply`, `.run`

**4. Comments**
- Line: `// comment`
- Block: `/* nested /* supported */ */`
- KDoc (Javadoc-like): `/** KDoc comment */`

**5. String Literals**
- Regular: `"hello"`
- Raw: `"""multiline\nno escape"""` (triple-quoted)
- **Interpolation**: `"value: $var or ${expr}"`
- **Multiline**: Triple-quoted strings with automatic indentation removal

**6. Control Flow**
```kotlin
// if-else (expressions!)
val x = if (cond) 5 else 6

// Pattern matching with when
when (value) {
    1 -> println("one")
    2, 3 -> println("two or three")
    in 4..5 -> println("range")
    is String -> println("string")
    else -> println("other")
}

// Loops
for (i in 1..10) { }
for ((key, value) in map) { }
while (cond) { }
do { } while (cond)

// Early return
return value
```

**7. Type Annotation Syntax**
```kotlin
val x: Int = 5
var y: String = "hello"
fun add(a: Int, b: Int): Int = a + b
class Point(val x: Int, val y: Int)
interface Drawable { fun draw() }
```

**8. Error Handling**
- **Primary**: Exceptions (like Java) `throw Exception()`
- No checked exceptions
- `try-catch-finally` blocks
- No `Result` type in stdlib (modern: Arrow library)

**9. Lambda/Closure Syntax**
```kotlin
{ x: Int -> x + 1 }           // Explicit type
{ x -> x + 1 }                // Type inference
{ it + 1 }                     // Single param implicit
{ a, b -> a + b }             // Multiple params
```

**10. Unique Features**
- **Null-safety types**: `String?` vs `String` with compiler enforcement
- **Sealed classes**: `sealed class Result { data class Success(val x: Int) : Result() }`
- **Data classes**: `data class Point(val x: Int, val y: Int)` auto-generates equals, hashCode, toString, copy
- **Object expressions**: `object : Interface { }`
- **Coroutines**: `suspend fun`, `launch`, `async`
- **Delegation**: `class By(x: Int) : Interface by delegateInstance`

---

### 3. SWIFT

**1. Keywords & Philosophy**
- **Count**: ~80+ (rich but necessary)
- **Philosophy**: Clean, safe defaults, Objective-C compatibility, expressiveness
- **Key keywords**: `let`, `var`, `func`, `class`, `struct`, `enum`, `protocol`, `extension`, `guard`, `defer`, `if`, `switch`, `for`, `while`, `repeat`, `async`, `await`

**2. Operators**
- **Arithmetic**: `+`, `-`, `*`, `/`, `%`
- **Comparison**: `==`, `!=`, `<`, `>`, `<=`, `>=`, `===` (identity)
- **Logical**: `&&`, `||`, `!`
- **Bitwise**: `&`, `|`, `^`, `~`, `<<`, `>>`
- **Assignment**: `=`, `+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`, `^=`, `<<=`, `>>=`
- **Range**: `...` (closed), `..<` (half-open)
- **Nil-coalescing**: `??`
- **Custom operators**: ✅ Full support with precedence declaration

**3. Syntactic Sugar**
- `guard let` / `guard case` for unwrapping: `guard let x = optional else { return }`
- Optional chaining: `obj?.method()?. property`
- Range patterns: `case 1..<10:`
- Trailing closure: `array.map { $0 * 2 }`
- Shorthand argument names: `{ $0 + $1 }`
- Property observers: `didSet`, `willSet`
- Computed properties: `var computed: Int { get { } set { } }`
- Result builder (DSL): `@resultBuilder` for custom DSLs

**4. Comments**
- Line: `// comment`
- Block: `/* nested /* supported */ */`
- Markdown: Full markdown support in doc comments: `/// Explanation\n/// - Parameter x: description`

**5. String Literals**
- Regular: `"hello"`
- Raw: `` #"path\like\this"# ``
- **Interpolation**: `"value: \(var) or \(expr)"`
- **Multiline**: Triple-quoted: `""" multiline string """`
- Extended delimiters: `#"string with "quotes""#`

**6. Control Flow**
```swift
// if-else (expressions in newer Swift)
let x = if cond { 5 } else { 6 }

// Pattern matching with switch
switch value {
case 1:
    print("one")
case 2...5:
    print("range")
case let x where x > 10:
    print("where clause")
default:
    break
}

// Loops
for i in 1...10 { }
for (idx, val) in array.enumerated() { }
while cond { }
repeat { } while cond

// Early exit
guard let x = optional else { return }
defer { cleanup() }
```

**7. Type Annotation Syntax**
```swift
let x: Int = 5
var y: String = "hello"
func add(a: Int, b: Int) -> Int { a + b }
struct Point { let x: Int; let y: Int }
protocol Drawable { func draw() }
```

**8. Error Handling**
- **Primary**: `throws`/`try-catch` (throws checked exceptions)
- `try?` for optional, `try!` for force unwrap
- Error protocol: `enum MyError: Error { case caseA, caseB }`
- **Async errors**: `async throws` functions

**9. Lambda/Closure Syntax**
```swift
{ (x: Int) in x + 1 }           // Explicit type
{ x in x + 1 }                  // Type inference
{ $0 + 1 }                       // Shorthand
{ x, y in x + y }               // Multiple params
@escaping { }                   // Closure escapes scope
```

**10. Unique Features**
- **Optionals with safe unwrapping**: `if let x = optional { }`
- **Protocol-oriented programming**: Heavy use of `protocol` over inheritance
- **Result builder (DSL support)**: `@resultBuilder` for SwiftUI-like DSLs
- **Property wrappers**: `@Published`, `@State` with `@` prefix
- **Async/await first-class**: `async` functions with structured concurrency
- **Type-safe variadic generics** (Swift 5.9+): `Pack` expansion
- **Actor model**: `actor` keyword for thread-safe isolation

---

### 4. ZIG

**1. Keywords & Philosophy**
- **Count**: 32 keywords (minimal)
- **Philosophy**: "Simple language, simple compiler", explicit control flow, no hidden costs
- **Key keywords**: `const`, `var`, `fn`, `pub`, `struct`, `enum`, `union`, `error`, `try`, `catch`, `if`, `else`, `switch`, `while`, `for`, `defer`, `inline`, `comptime`, `asm`, `noreturn`

**2. Operators**
- **Arithmetic**: `+`, `-`, `*`, `/`, `%`
- **Comparison**: `==`, `!=`, `<`, `>`, `<=`, `>=`
- **Logical**: `and`, `or`, `not`
- **Bitwise**: `&`, `|`, `^`, `~`, `<<`, `>>`
- **Assignment**: `=`, `+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`, `^=`, `<<=`, `>>=`
- **Pointer**: `.*` (dereference), `.&` (address-of, within struct context)
- **Custom operators**: ❌ Not allowed

**3. Syntactic Sugar**
- `defer` for cleanup: `defer allocator.free(ptr);`
- `try-catch` for error propagation: `try operation()`
- `orelse` operator: `try value orelse defaultValue`
- `catch` blocks: `catch |err| { }`
- `inline` loops/functions: `inline for`, `inline fn`
- `comptime` execution: `comptime var constant = value`

**4. Comments**
- Line: `// comment`
- No block comments (intentional design choice for simplicity)

**5. String Literals**
- Regular: `"hello"`
- Multi-line: Implicit (newlines in string literals)
- **Interpolation**: ❌ None (use string formatting functions)
- **Escape sequences**: Standard C-style

**6. Control Flow**
```zig
// if-else (expressions!)
const x = if (cond) 5 else 6;

// Pattern matching with switch
switch (value) {
    1 => print("one"),
    2, 3 => print("two or three"),
    else => print("other"),
}

// Loops with labels
outer: while (cond) {
    while (cond) {
        break :outer;
    }
}

for (array, 0..) |item, i| { }  // Multi-item iteration
for (0..10) |i| { }               // Range iteration

// Early return
return value;
```

**7. Type Annotation Syntax**
```zig
const x: i32 = 5;
var y: []const u8 = "hello";
fn add(a: i32, b: i32) i32 { return a + b; }
pub struct Point { x: i32, y: i32 }
pub const Point = struct { x: i32, y: i32 };
```

**8. Error Handling**
- **Primary**: Error union types: `!ErrorSet` or `Type!Error`
- `try` for propagation: `try operation()`
- `catch` for handling: `operation() catch |err| { }`
- `orelse` for defaults: `try operation() orelse defaultValue`
- No exceptions; no hidden control flow

**9. Lambda/Closure Syntax**
- ❌ No first-class closures/lambdas in traditional sense
- Can pass function pointers: `fn add(a: i32, b: i32) i32 { return a + b; }`
- Anonymous functions via `fn` declarations

**10. Unique Features**
- **Error union types**: `fn read() !Data { }` (never hides errors)
- **Comptime execution**: `comptime var x = computeAtCompileTime();`
- **Defer statements**: Cleanup at end of scope like RAII
- **Tagged unions**: `union(enum)` for type-safe variants
- **Builtin functions**: `@intCast()`, `@ptrCast()`, `@typeInfo()`, `@alignOf()`, etc.
- **No function overloading**: Explicit naming required
- **Optionals via `?Type`**: `?i32` for nullable types

---

### 5. GO

**1. Keywords & Philosophy**
- **Count**: 25 keywords (extremely minimal)
- **Philosophy**: "Less is exponentially more", simplicity over cleverness, fast compilation
- **Key keywords**: `package`, `import`, `const`, `var`, `func`, `struct`, `interface`, `type`, `if`, `else`, `switch`, `case`, `for`, `range`, `go`, `defer`, `return`, `break`, `continue`, `fallthrough`, `goto`, `select`, `chan`

**2. Operators**
- **Arithmetic**: `+`, `-`, `*`, `/`, `%`
- **Comparison**: `==`, `!=`, `<`, `>`, `<=`, `>=`
- **Logical**: `&&`, `||`, `!`
- **Bitwise**: `&`, `|`, `^`, `&^` (AND NOT), `<<`, `>>`
- **Assignment**: `=`, `+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`, `^=`, `<<=`, `>>=`, `&^=`
- **Channel**: `<-` (send/receive), `<-chan` (receive-only)
- **Custom operators**: ❌ Not allowed

**3. Syntactic Sugar**
- Short variable declaration: `:=` in function scope
- Multiple return: `return a, b, err`
- Blank identifier: `_` for unused values
- Deferred execution: `defer statement`
- Type assertion: `value.(Type)`
- Type switch: `switch v := x.(type) { }`
- Goroutine launch: `go function()`

**4. Comments**
- Line: `// comment`
- Block: `/* nested /* supported */ */`
- Doc comments: `// Package name describes...`

**5. String Literals**
- Regular: `"hello"`
- Raw: `` `raw\nstring` `` (backticks)
- **Interpolation**: ❌ None (use `fmt.Sprintf`)
- **Rune literal**: `'a'` for single character

**6. Control Flow**
```go
// if-else (statement only!)
if cond {
    x = 5
} else {
    x = 6
}

// Pattern matching with switch
switch value {
case 1:
    println("one")
case 2, 3:
    println("two or three")
default:
    println("other")
}

// Loops
for i := 0; i < 10; i++ { }
for range array { }               // Range without value
for i, v := range array { }       // With index and value
for {  }                            // Infinite loop

// Early return
return value
```

**7. Type Annotation Syntax**
```go
var x int = 5
y := 6                           // Type inference
const z = 7
func add(a int, b int) int { return a + b }
type Point struct { X, Y int }
type Reader interface { Read(p []byte) (n int, err error) }
```

**8. Error Handling**
- **Primary**: Multiple returns with `error` type
- Idiomatic: `if err != nil { return err }`
- No exceptions; errors as values
- `error` interface: `type error interface { Error() string }`

**9. Lambda/Closure Syntax**
```go
func(x int) int { return x + 1 }      // Anonymous function
func(x int) { println(x) }            // No return
var f func(int) int = func(x int) int { return x + 1 }
```

**10. Unique Features**
- **Multiple return values**: `return val, err` (not tuple unwrapping)
- **Named return values**: `func read(p []byte) (n int, err error) { n = len(p); return }`
- **Defer statements**: `defer cleanup()` executes at function exit
- **Goroutines**: `go function()` launches lightweight thread
- **Channels**: `chan Type` for goroutine communication
- **Interface satisfaction**: Implicit (no explicit implements keyword)
- **Embedding over inheritance**: Struct field embedding for composition

---

### 6. GLEAM

**1. Keywords & Philosophy**
- **Count**: ~20 keywords (minimal)
- **Philosophy**: ML-inspired, statically typed, no null/exceptions, compile to JavaScript or Erlang
- **Key keywords**: `fn`, `let`, `assert`, `use`, `case`, `if`, `todo`, `panic`, `import`, `pub`, `const`, `opaque type`, `type`, `try`

**2. Operators**
- **Arithmetic**: `+`, `-`, `*`, `/`, `%`
- **Comparison**: `==`, `!=`, `<`, `>`, `<=`, `>=`
- **Logical**: `&&`, `||`, `!`
- **Pipe**: `|>` (primary operator)
- **List cons**: `[head, ..tail]`
- **Custom operators**: ❌ Not allowed

**3. Syntactic Sugar**
- Pipe operator: `value |> function |> other_function`
- Pattern matching: `case pattern -> body`
- List comprehension: Not directly; use `list.map`
- String interpolation: ❌ None (concatenation via `<>`)
- Guard expressions: `case x if x > 0 -> body`

**4. Comments**
- Line: `// comment`
- No block comments

**5. String Literals**
- Regular: `"hello"`
- **Interpolation**: ❌ None; concatenate with `<>`
- **Multiline**: Implicit newlines in strings

**6. Control Flow**
```gleam
// if-else (expression)
let x = case cond {
    True -> 5
    False -> 6
}

// Pattern matching (case)
case value {
    1 -> "one"
    2 | 3 -> "two or three"
    _ -> "other"
}

// Loops via recursion (no loops!)
fn loop(n) {
    case n {
        0 -> Nil
        n -> loop(n - 1)
    }
}

// Early return via case-as-expression
case operation() {
    Ok(x) -> x
    Error(e) -> handle_error(e)
}
```

**7. Type Annotation Syntax**
```gleam
let x: Int = 5
fn add(a: Int, b: Int) -> Int { a + b }
pub type Point = Point(x: Int, y: Int)
pub opaque type Secret = Secret(value: String)
```

**8. Error Handling**
- **Primary**: `Result(a, b)` type: `Ok(value)` or `Error(error)`
- No exceptions; errors are values
- No `null`/`nil` (use `Option(a)`: `Some(value)` or `None`)
- Pattern matching for handling

**9. Lambda/Closure Syntax**
```gleam
fn(x) { x + 1 }                  // Anonymous function
fn(x: Int) -> Int { x + 1 }      // With types
```

**10. Unique Features**
- **Pipe operator**: `|>` as primary composition method
- **Pattern matching over conditionals**: No `if` for complex logic
- **No exceptions/null**: Compile-time guarantee
- **Option and Result types**: Mandatory handling
- **Immutability**: All values immutable by default
- **Functional**: No loops, only recursion
- **Multiplatform**: Compiles to JavaScript (Node, browser) or Erlang/BEAM

---

### 7. ELM

**1. Keywords & Philosophy**
- **Count**: ~20 keywords (minimal)
- **Philosophy**: "Delightful language for reliable web apps", pure functional, no side effects visible in types
- **Key keywords**: `module`, `import`, `type`, `alias`, `case`, `of`, `if`, `then`, `else`, `let`, `in`, `port`, `subscription`

**2. Operators**
- **Arithmetic**: `+`, `-`, `*`, `/`, `//` (integer division), `^` (power)
- **Comparison**: `==`, `/=`, `<`, `>`, `<=`, `>=`
- **Logical**: `&&`, `||`, `not`
- **Pipe**: `|>` (primary composition)
- **Function composition**: `<<`, `>>`
- **Custom operators**: ✅ Limited (custom infix operators)

**3. Syntactic Sugar**
- Pipe operator: `value |> function |> other`
- Function composition: `f << g` (math notation)
- Record syntax: `{ x = 5, y = 3 }`
- Record update: `{ point | x = 10 }`
- Pattern matching: `case value of pattern -> body`
- Guard expressions: `case n of x when x > 0 -> body`

**4. Comments**
- Line: `-- comment`
- Block: `{- nested {- supported -} -}`

**5. String Literals**
- Regular: `"hello"`
- Character: `'a'` (single char)
- **Interpolation**: ❌ None; concatenate with `++`
- **Multiline**: Triple-quoted: `"""multiline"""`

**6. Control Flow**
```elm
-- if-else (expression)
x = if cond then 5 else 6

-- Pattern matching (case)
case value of
    1 -> "one"
    2 -> "two"
    _ -> "other"

-- Loops via recursion (no loops!)
loop n = case n of
    0 -> 0
    n -> n + loop (n - 1)

-- let-in binding
let
    x = 5
    y = 10
in
    x + y
```

**7. Type Annotation Syntax**
```elm
x : Int
x = 5

add : Int -> Int -> Int
add a b = a + b

type Point = Point Int Int

type alias PointRecord = { x : Int, y : Int }
```

**8. Error Handling**
- **Primary**: `Maybe a` (Some/None) or custom types
- No exceptions; errors modeled as data
- Pattern matching for handling
- HTTP: `Task` type for async operations

**9. Lambda/Closure Syntax**
```elm
\x -> x + 1                      // Lambda with \
\x y -> x + y                    // Multiple params
```

**10. Unique Features**
- **Pure functional**: No side effects; HTML/events via Elm Runtime
- **No null/nil**: Explicit `Maybe`
- **Strong typing**: Compiler catches null reference errors
- **Immutability**: All values immutable
- **Time-traveling debugger**: Records all state changes
- **Pipe operator**: `|>` as standard composition
- **Union types**: `type Result a b = Ok a | Err b`

---

### 8. ROC

**1. Keywords & Philosophy**
- **Count**: ~15 keywords (minimal)
- **Philosophy**: Fast, practical functional language, "Roc moves rocks", focus on compilation speed
- **Key keywords**: `when`, `if`, `then`, `else`, `let`, `in`, `as`, `dbg`, `crash`, `inspect`, `type`, `alias`, `opaque`

**2. Operators**
- **Arithmetic**: `+`, `-`, `*`, `/`, `%`
- **Comparison**: `==`, `!=`, `<`, `>`, `<=`, `>=`
- **Logical**: `&&`, `||`, `!`
- **Pipe**: `|>` (primary)
- **List cons**: `[elem, ..list]`
- **Custom operators**: ✅ Limited custom infix operators

**3. Syntactic Sugar**
- Pipe operator: `value |> func1 |> func2`
- Field shorthand in records: `{ name, age }` from `{ name: name, age: age }`
- Record access: `.fieldname` as function
- List pattern matching: `[head, ..tail]`
- Ability parameters: `provides [Effect]` (unique to Roc)

**4. Comments**
- Line: `# comment`
- No block comments

**5. String Literals**
- Regular: `"hello"`
- Escape sequences: `\n`, `\"`, etc.
- **Interpolation**: ❌ None (concatenation with `|> Str.concat`)
- **Multiline**: Implicit

**6. Control Flow**
```roc
-- if-then-else (expression)
x = if cond then 5 else 6

-- Pattern matching (when)
result = when value
    1 -> "one"
    2 | 3 -> "two or three"
    _ -> "other"

-- Recursion-based loops
loop : U32 -> U32
loop = \n ->
    when n
        0 -> 0
        n -> n + loop (n - 1)
```

**7. Type Annotation Syntax**
```roc
x : U32
x = 5

add : U32, U32 -> U32
add = \a, b -> a + b

Point : type { x : U32, y : U32 }
```

**8. Error Handling**
- **Primary**: `Result a b` type: `Ok a` or `Err b`
- No exceptions
- Pattern matching for handling
- `dbg` for debugging (returns the value)

**9. Lambda/Closure Syntax**
```roc
\x -> x + 1                      // Backslash lambda
\a, b -> a + b                   // Multiple params
```

**10. Unique Features**
- **Ability syntax**: `provides [Effect]` for effect tracking (unique system)
- **Platform separation**: Code vs. platform layer (strict separation)
- **Field punning**: `{ name, age }` abbreviates `{ name: name, age: age }`
- **Accessibility**: `.fieldname` as direct function application
- **Fast compilation**: LLVM-based, focuses on speed
- **Type inference**: Hindley-Milner with Roc extensions
- **Immutability by default**: All data immutable

---

### 9. OCAML

**1. Keywords & Philosophy**
- **Count**: ~30 keywords (moderate)
- **Philosophy**: ML tradition, industrial-strength functional language, safety + performance
- **Key keywords**: `let`, `rec`, `and`, `in`, `fun`, `function`, `match`, `with`, `type`, `module`, `sig`, `if`, `then`, `else`, `try`, `with`, `raise`, `begin`, `end`, `for`, `while`, `do`, `done`, `mutable`

**2. Operators**
- **Arithmetic**: `+`, `-`, `*`, `/`, `mod`, `lsl` (left shift), `lsr` (right shift)
- **Comparison**: `=`, `<>`, `<`, `>`, `<=`, `>=`, `==` (physical), `!=` (physical not-equal)
- **Logical**: `&&`, `||`, `not`
- **Pipe-like**: `|>` (OCaml 4.01+), but more commonly `|` in pattern matching
- **Field access**: `.` (dot notation)
- **Custom operators**: ✅ Full support for custom infix/prefix operators

**3. Syntactic Sugar**
- Pattern matching: `match x with pattern -> body | pattern -> body`
- List pattern: `head :: tail` (cons operator)
- Parentheses required for precedence
- Record shorthand: `{ x; y }` omits type in many contexts
- Mutable references: `!ref` for dereference, `ref := value` for assign

**4. Comments**
- Line: None (only block)
- Block: `(* nested (* supported *) *)`

**5. String Literals**
- Regular: `"hello"`
- Character: `'a'` (single char)
- **Interpolation**: ❌ None in core (libraries available); `Printf.sprintf` for formatting
- **Multiline**: Implicit newlines

**6. Control Flow**
```ocaml
(* Pattern matching *)
let result = match value with
    | 1 -> "one"
    | 2 | 3 -> "two or three"
    | _ -> "other"

(* If-then-else *)
let x = if cond then 5 else 6

(* For loop *)
for i = 1 to 10 do
    printf "%d\n" i
done

(* While loop *)
while !counter < 10 do
    incr counter
done

(* Recursion *)
let rec loop n =
    match n with
    | 0 -> 0
    | n -> n + loop (n - 1)
```

**7. Type Annotation Syntax**
```ocaml
let x : int = 5
let add : int -> int -> int = fun a b -> a + b
type point = { x : int; y : int }
type shape = Circle of int | Square of int
module M : sig val x : int end = struct let x = 5 end
```

**8. Error Handling**
- **Primary**: Exceptions with `try-with` and `raise`
- Custom exception types: `exception MyError of string`
- Pattern matching on exceptions
- Modern: `Result` type via libraries

**9. Lambda/Closure Syntax**
```ocaml
fun x -> x + 1                   (* Anonymous function *)
fun x y -> x + y                 (* Multiple params *)
function                         (* Pattern matching in function *)
    | pattern -> body
    | _ -> default
```

**10. Unique Features**
- **Polymorphic variants**: `` `Tag value ``
- **Module system**: `module M = struct end`, `module type`, `functor`
- **Immutability by default**: Mutable via `ref` or `mutable` field
- **Strong static typing**: Hindley-Milner with imperative extensions
- **Pattern matching**: Exhaustiveness checking
- **Labeled arguments**: `let f ~x ~y = x + y` and `f ~x:5 ~y:10`
- **Optional arguments**: `let f ?x () = match x with None -> 0 | Some v -> v`

---

### 10. HASKELL

**1. Keywords & Philosophy**
- **Count**: ~30+ keywords (moderate, plus reserved identifiers)
- **Philosophy**: Pure functional, lazy evaluation, strong type system, research-oriented
- **Key keywords**: `let`, `in`, `where`, `case`, `of`, `if`, `then`, `else`, `do`, `return`, `import`, `module`, `type`, `data`, `class`, `instance`, `qualified`, `as`, `hiding`

**2. Operators**
- **Arithmetic**: `+`, `-`, `*`, `/`, `^`, `**`, `mod`, `div`, `rem`
- **Comparison**: `==`, `/=`, `<`, `>`, `<=`, `>=`
- **Logical**: `&&`, `||`, `not`
- **Function composition**: `.` (dot, right-to-left), `<|` (apply), `|>` (pipe, left-to-right)
- **List cons**: `:` (head cons)
- **List append**: `++`
- **Custom operators**: ✅ Full support with fixity declaration

**3. Syntactic Sugar**
- List notation: `[1, 2, 3]`, `[1..10]` (ranges), `[x | x <- list, x > 0]` (comprehension)
- Tuple: `(a, b, c)`
- Function composition: `f . g` (math notation)
- Do-notation: `do { x <- action; y <- action; return (x, y) }`
- Where clauses: `where x = 5` for local bindings
- Operator sections: `(+1)` partially applies operator
- Pattern guards: `| guard -> body`

**4. Comments**
- Line: `-- comment`
- Block: `{- nested {- supported -} -}`

**5. String Literals**
- Regular: `"hello"`
- Character: `'a'` (single char)
- List of chars: `"hello"` is `['h', 'e', 'l', 'l', 'o']` (compatibility quirk)
- Escape sequences: Standard
- **Interpolation**: None in core; libraries like `interpolate` available
- **Multiline**: Implicit

**6. Control Flow**
```haskell
-- Pattern matching (case)
result = case value of
    1 -> "one"
    2 -> "two"
    _ -> "other"

-- If-then-else
x = if cond then 5 else 6

-- Where clause (local bindings)
fn x = y + z where
    y = x + 1
    z = x + 2

-- Let-in (local bindings)
x = let
        y = 5
        z = 10
    in y + z

-- Do-notation (monadic)
result = do
    x <- getX
    y <- getY
    return (x + y)

-- Recursion
loop 0 = 0
loop n = n + loop (n - 1)
```

**7. Type Annotation Syntax**
```haskell
x :: Int
x = 5

add :: Int -> Int -> Int
add a b = a + b

data Point = Point Int Int

type PointAlias = (Int, Int)

class Show a where
    show :: a -> String

instance Show Point where
    show (Point x y) = ...
```

**8. Error Handling**
- **Primary**: `Maybe a` (Nothing | Just a) or `Either a b` (Left a | Right b)
- No exceptions (pure functions); exceptions exist but avoided in pure code
- `error :: String -> a` for runtime errors
- Monadic composition for result chaining

**9. Lambda/Closure Syntax**
```haskell
\x -> x + 1                      -- Lambda
\x y -> x + y                    -- Multiple params
\(x, y) -> x + y                 -- Pattern match in lambda
```

**10. Unique Features**
- **Lazy evaluation**: Expressions evaluated on demand
- **Monads and do-notation**: Elegant effect handling
- **Type classes**: Parametric polymorphism with constraints
- **Operator as function**: `(+) 5 3` applies operator
- **Partial application**: All functions curried by default
- **Pattern matching**: Guards, multiple clauses
- **List comprehensions**: `[f x | x <- list, p x]` similar to set notation
- **GADTs and advanced types**: Sophisticated type system

---

## Cross-Cutting Comparisons

### Table 1: Expression vs. Statement Philosophy

| Language | Philosophy | If/Else | Loops | Details |
|----------|-----------|---------|-------|---------|
| **Rust** | Expression-based | Expr (no semicolon) | Expr loop, for | Everything is an expression; semicolon suppresses return |
| **Kotlin** | Expression-based | Expr | Expr for | `when` and `if` return values; versatile |
| **Swift** | Hybrid (recent shift to expr) | Expr (5.9+) | Stmt for | Modern Swift allows `if` as expr |
| **Zig** | Expression-based | Expr | Expr | `if/switch/while` all return values |
| **Go** | Statement-based | Stmt | Stmt | Everything is a statement; no expression semantics |
| **Gleam** | Expression-based | Expr (case) | Expr (recursion) | Pure functional; all control flow returns |
| **Elm** | Expression-based | Expr | Expr (recursion) | Pure functional; `if/case` always return |
| **Roc** | Expression-based | Expr (when) | Expr (recursion) | Pure functional; designed for expressions |
| **OCaml** | Expression-based | Expr | Mix (stmt-like) | `match/if` return values; some imperative features |
| **Haskell** | Expression-based | Expr | Expr (recursion) | Pure lazy functional; all control flow returns |

**Summary**: Rust, Kotlin, Zig, and functional languages (Gleam, Elm, Roc, OCaml, Haskell) embrace expression-based design. Swift is transitioning. Go is resolutely statement-based.

---

### Table 2: Semicolon Philosophy

| Language | Status | Details |
|----------|--------|---------|
| **Rust** | Optional (with semantics) | `;` suppresses return value; `x;` vs `x` matters |
| **Kotlin** | Optional | Newline-sensitive; `;` required in some contexts |
| **Swift** | Optional | Newlines/braces implicitly separate statements |
| **Zig** | Required | Statements must end with `;` |
| **Go** | Optional (with ASI) | Automatic semicolon insertion before newline |
| **Gleam** | Not used | Newline-based separation |
| **Elm** | Not used | Newline-based separation |
| **Roc** | Not used | Newline-based separation |
| **OCaml** | Not used | Rarely needed; `;` has special meaning (sequence) |
| **Haskell** | Not used | Indentation-based; `;` separates in `{ ; }` syntax |

**Summary**: Modern languages favor optional or absent semicolons. Zig stands out as requiring them explicitly.

---

### Table 3: Braces vs. Indentation

| Language | Primary | Fallback | Details |
|----------|---------|----------|---------|
| **Rust** | Braces `{}` | None | Mandatory braces; indentation optional |
| **Kotlin** | Braces `{}` | None | Mandatory braces; some optional (trailing lambda) |
| **Swift** | Braces `{}` | None | Mandatory braces; indentation optional |
| **Zig** | Braces `{}` | None | Mandatory braces; indentation optional |
| **Go** | Braces `{}` | Forced indentation | Braces required; indentation style enforced by `gofmt` |
| **Gleam** | Indentation | None | Indentation-based (Python-like); no braces in core syntax |
| **Elm** | Indentation | None | Indentation-based (Python-like) |
| **Roc** | Indentation | None | Indentation-based (Python-like) |
| **OCaml** | Braces `begin/end` | `()` parens | Optional; commonly used for grouping |
| **Haskell** | Indentation | `{ ; }` | Indentation primary; braces optional explicit syntax |

**Summary**: C-like languages (Rust, Kotlin, Swift, Zig, Go) mandate braces. Functional languages (Gleam, Elm, Roc, Haskell) embrace indentation.

---

### Table 4: Pipe Operators & Composition

| Language | Operator | Syntax | Usage |
|----------|----------|--------|-------|
| **Rust** | None (method chaining) | `obj.method().other()` | Implicit chaining via methods |
| **Kotlin** | `.let`, `.apply`, `.run` | `value.let { it + 1 }` | Scope functions (limited piping) |
| **Swift** | None (method chaining) | `obj.method().other()` | Implicit chaining via methods |
| **Zig** | None | Manual wrapping | No built-in piping |
| **Go** | None | Manual wrapping | No built-in piping |
| **Gleam** | `\|>` | `value \|> func \|> other` | **Primary composition**; left-associative |
| **Elm** | `\|>` | `value \|> func \|> other` | **Primary composition**; left-associative |
| **Roc** | `\|>` | `value \|> func \|> other` | **Primary composition**; left-associative |
| **OCaml** | `\|>` | `value \|> func \|> other` | Available (OCaml 4.01+); not primary |
| **Haskell** | `.` (right) `\|>` (left) | `f . g` or `f <\| g` | Right-to-left (function composition); left rarely used |

**Summary**: Functional languages elevate piping as a primary composition tool. Imperative languages use method chaining. Haskell prefers function composition with `.`.

---

### Table 5: Custom Operators

| Language | Support | Details |
|----------|---------|---------|
| **Rust** | ❌ No | Fixed set only; no operator overloading in definition |
| **Kotlin** | ⚠️ Limited | Can overload `operator fun` for existing operators only |
| **Swift** | ✅ Yes | Full support with `infix`, `prefix`, `postfix` declarations and precedence |
| **Zig** | ❌ No | Fixed set only |
| **Go** | ❌ No | Fixed set only; method receivers simulate some overloading |
| **Gleam** | ❌ No | Fixed set only |
| **Elm** | ✅ Limited | Can define custom infix operators with `infixl`, `infixr` |
| **Roc** | ✅ Limited | Can define custom infix operators |
| **OCaml** | ✅ Yes | Full support for custom operators; symbolic characters allowed |
| **Haskell** | ✅ Yes | Full support with `infixl`, `infixr`, `infix` and custom precedence |

**Summary**: Functional languages (OCaml, Haskell, Elm, Roc) embrace custom operators. Systems languages (Rust, Zig) restrict to fixed sets for clarity and compiler simplicity.

---

### Table 6: Keyword Count & Philosophy

| Language | Count | Philosophy | Approach |
|----------|-------|-----------|----------|
| **Rust** | 39 | Moderate; explicit safety | Expression-based; memory-focused |
| **Kotlin** | 50+ | Pragmatic; interop-focused | Concise with escape hatches |
| **Swift** | 80+ | Rich; safety + expressiveness | Apple-curated; feature-rich |
| **Zig** | 32 | Minimal; explicit control | "Simple language, simple compiler" |
| **Go** | **25** | **Minimal; simplicity over features** | Famous for extreme minimalism |
| **Gleam** | 20 | Minimal; functional purity | ML-inspired; compilation-focused |
| **Elm** | 20 | Minimal; reliability focus | Strictly functional; limited by design |
| **Roc** | 15 | Minimal; performance focus | Newer; intentionally small |
| **OCaml** | 30 | Moderate; industrial ML | Balance of theory and practice |
| **Haskell** | 30+ | Moderate; research language | Comprehensive type system |

**Summary**: Go (25 keywords) is the minimalist champion. Functional languages are minimal (15-30). Swift is the maximalist (80+). The trend favors minimalism where possible.

---

### Table 7: String Interpolation

| Language | Native Support | Syntax | Interpolation Type |
|----------|----------------|--------|-------------------|
| **Rust** | ❌ No | `format!("{}", var)` | Macro-based |
| **Kotlin** | ✅ Yes | `"value: $var or ${expr}"` | Direct evaluation |
| **Swift** | ✅ Yes | `"value: \(var) or \(expr)"` | Direct evaluation |
| **Zig** | ❌ No | `std.fmt.bufPrint()` | Function-based |
| **Go** | ❌ No | `fmt.Sprintf("%v", var)` | Function-based |
| **Gleam** | ❌ No | `"value: " <> var <> "!"` | Concatenation |
| **Elm** | ❌ No | `"value: " ++ var ++ "!"` | Concatenation |
| **Roc** | ❌ No | `Str.concat ["value: ", var]` | Function-based |
| **OCaml** | ❌ No (core) | `Printf.sprintf "%d" x` | Printf-style |
| **Haskell** | ❌ No (core) | `"value: " ++ show x` | Concatenation + show |

**Summary**: Swift and Kotlin offer native interpolation. Most others require concatenation or formatting functions. Rust uses macros as a middle ground.

---

### Table 8: Type Annotations

| Language | Function | Variable | Return Type | Details |
|----------|----------|----------|-------------|---------|
| **Rust** | Required | Inferenced (in function scope) | Required | Explicit everywhere in public API |
| **Kotlin** | Optional (inferred) | Optional (inferred) | Inferred | `val x: Int = 5` or `val x = 5` |
| **Swift** | Optional (inferred) | Optional (inferred) | Inferred | `let x: Int = 5` or `let x = 5` |
| **Zig** | Optional (inferred) | Optional (inferred) | Inferred | `: Type` syntax throughout |
| **Go** | Required | Required | Required | No inference; explicit always |
| **Gleam** | Required | Inferred | Required | ML-style strict typing |
| **Elm** | Required | Inferred | Required | ML-style; `x : Int` annotation |
| **Roc** | Inferred | Inferred | Inferred | Hindley-Milner; full inference |
| **OCaml** | Inferred | Inferred | Inferred | Robust type inference |
| **Haskell** | Inferred | Inferred | Inferred | Powerful HM with extensions |

**Summary**: Go requires explicit annotations. ML-family languages (Gleam, Elm, OCaml, Haskell, Roc) infer everything. C-like languages vary.

---

### Table 9: Error Handling Paradigm

| Language | Primary | Secondary | Philosophy |
|----------|---------|-----------|-----------|
| **Rust** | `Result<T, E>` enum | `panic!` | Explicit; no hidden control flow |
| **Kotlin** | Exceptions (`try-catch`) | No | Java-style; pragmatic |
| **Swift** | `throws`/`try-catch` | `Result` (app-level) | Checked errors; type-safe |
| **Zig** | Error union types `!ErrorSet` | `try-catch` | Explicit; errors visible in type |
| **Go** | Multiple returns (`err`) | `panic()` | Values, not exceptions |
| **Gleam** | `Result` type | No | Functional; pure values |
| **Elm** | `Result` / `Maybe` types | No | Functional; no exceptions |
| **Roc** | `Result` type | `dbg`, `crash` | Functional; debugging first |
| **OCaml** | Exceptions (`try-with`) | `Result` (app-level) | Traditional exception model |
| **Haskell** | `Maybe`/`Either` monads | Exceptions in IO | Pure functions; effects explicit |

**Summary**: Divide between exceptions (Kotlin, Swift, OCaml) and values (Rust, Go, Gleam, Elm, Roc, Haskell). Modern trend favors explicit error values.

---

### Table 10: Lambda/Closure Syntax

| Language | Syntax | Type Inference | Closure Capture |
|----------|--------|-----------------|-----------------|
| **Rust** | `\|x\| x + 1` | ✅ Full | `move`, `&`, `&mut` explicit |
| **Kotlin** | `{ x -> x + 1 }` | ✅ Full | Implicit capture (scope aware) |
| **Swift** | `{ $0 + 1 }` | ✅ Full | Implicit capture; `[weak self]` annotations |
| **Zig** | No closures | N/A | No first-class closures |
| **Go** | `func(x int) int { return x + 1 }` | ❌ None | Implicit capture |
| **Gleam** | `fn(x) { x + 1 }` | ✅ Full | Implicit immutable capture |
| **Elm** | `\x -> x + 1` | ✅ Full | Implicit capture |
| **Roc** | `\x -> x + 1` | ✅ Full | Implicit capture |
| **OCaml** | `fun x -> x + 1` | ✅ Full | Implicit capture |
| **Haskell** | `\x -> x + 1` | ✅ Full | Implicit capture |

**Summary**: All modern languages support lambdas with type inference except Go. Rust makes capture explicit; most others capture implicitly.

---

## Syntax Design Philosophies

### Minimalism vs. Richness

**Minimalist** (15-32 keywords):
- **Zig, Gleam, Elm, Roc**: "Do one thing well"; fewer keywords = fewer ways to solve problems
- **Go**: Famous constraint; forces idiomatic code
- **Benefit**: Smaller compiler, easier to learn, predictable code
- **Tradeoff**: Less expressive; sometimes verbose

**Moderate** (30-50 keywords):
- **Rust, OCaml, Haskell**: Balance of expressiveness and simplicity
- **Benefit**: Rich feature set while remaining comprehensible
- **Tradeoff**: Slightly steeper learning curve

**Rich** (50-80+ keywords):
- **Kotlin, Swift**: Pragmatism + multiple paradigm support
- **Benefit**: Covers many use cases; less boilerplate
- **Tradeoff**: More to learn; more ways to do same thing

---

### Expression vs. Statement Semantics

**Purely Expression-Based**:
- **Rust, Gleam, Elm, Roc, OCaml, Haskell**: Everything returns a value
- `if` can be used on RHS of assignment
- `match` returns a value directly
- **Advantage**: Composable; less scaffolding

**Statement-Based**:
- **Go**: Statements don't return; everything requires explicit assignment
- `if`, `for`, `switch` are statements
- **Advantage**: Clear intent; familiar to C programmers

**Hybrid**:
- **Kotlin, Swift**: Modern versions transitioning to expression-based
- Swift 5.9+ allows `if` expressions

---

### Null Safety Strategy

| Language | Approach | Syntax |
|----------|----------|--------|
| **Rust** | `Option<T>` (enum) | `Some(x)`, `None` |
| **Kotlin** | Nullable types | `String?` vs `String` |
| **Swift** | Optionals | `String?` or `String!` (force unwrap) |
| **Zig** | `?Type` | `?i32` is optional |
| **Go** | Implicit nil | `*Type` pointers are nullable; no explicit opt |
| **Gleam** | `Option` type | No null entirely |
| **Elm** | `Maybe` type | No null entirely |
| **Roc** | Result type | No null entirely |
| **OCaml** | `Option` type | `None`, `Some x` |
| **Haskell** | `Maybe` type | `Nothing`, `Just x` |

**Trend**: Modern languages eliminate implicit null via optionals or option types. Go's implicit nil is an outlier.

---

### Pattern Matching Philosophy

| Language | Mechanism | Syntax | Power |
|----------|-----------|--------|-------|
| **Rust** | Primary control flow | `match arms { pattern => body }` | ⭐⭐⭐⭐⭐ Full destructuring, guards, ranges |
| **Kotlin** | When expressions | `when (x) { pattern -> body }` | ⭐⭐⭐⭐ Rich but less powerful than Rust |
| **Swift** | Switch cases | `switch x { case pattern: body }` | ⭐⭐⭐⭐ Patterns + where clauses |
| **Zig** | Switch expressions | `switch (x) { pattern => body }` | ⭐⭐⭐ Basic patterns |
| **Go** | Switch statements | `switch x { case value: body }` | ⭐⭐ Value matching only; no destructuring |
| **Gleam** | Case expressions | `case x { pattern -> body }` | ⭐⭐⭐⭐⭐ Full destructuring |
| **Elm** | Case expressions | `case x of pattern -> body` | ⭐⭐⭐⭐⭐ Full destructuring |
| **Roc** | When expressions | `when x is pattern -> body` | ⭐⭐⭐⭐⭐ Full destructuring |
| **OCaml** | Match expressions | `match x with pattern -> body` | ⭐⭐⭐⭐⭐ Exhaustiveness checking; variants |
| **Haskell** | Pattern guards | Function clauses with guards | ⭐⭐⭐⭐⭐ Full recursion + guards |

**Trend**: Functional languages leverage pattern matching heavily; imperative languages use it less.

---

## Key Syntax Innovations

### 1. **Rust's `?` Operator** (Error Propagation)
```rust
fn read_file(path: &str) -> Result<String, Error> {
    let data = fs::read_to_string(path)?;  // Propagate error implicitly
    Ok(data)
}
// Desugars to: if let Err(e) = fs::read_to_string(path) { return Err(e); }
```
**Impact**: Monadic error handling without explicit if-let scaffolding.

---

### 2. **Kotlin's Null-Safety Operators**
```kotlin
val length = name?.length ?: 0  // Safe call + Elvis operator
// No NPE possible in safe path
```
**Impact**: Null safety at syntax level; compiler enforces.

---

### 3. **Swift's Result Builder** (DSL Support)
```swift
@resultBuilder
struct ListBuilder {
    static func buildBlock(_ components: Item...) -> [Item] {
        components
    }
}

@ListBuilder
var items: [Item] {
    Item(name: "A")
    Item(name: "B")
}
```
**Impact**: Enables DSL-like syntax (SwiftUI, property wrappers).

---

### 4. **Zig's `defer`** (RAII without Classes)
```zig
var file = try openFile("data.txt");
defer closeFile(file);  // Guaranteed cleanup
// Process file
// closeFile called automatically at scope exit
```
**Impact**: Resource management without classes; explicit cleanup.

---

### 5. **Go's Multiple Returns** (Error Handling)
```go
data, err := readFile("data.txt")
if err != nil {
    return err
}
```
**Impact**: Idiomatic error handling; no exceptions needed.

---

### 6. **Gleam/Elm/Roc's Pipe Operator** (Composition)
```gleam
[1, 2, 3]
|> list.map(fn(x) { x + 1 })
|> list.filter(fn(x) { x > 2 })
|> list.length()
```
**Impact**: Left-to-right composition; functional readable code.

---

### 7. **Haskell's Do-Notation** (Monad Syntax)
```haskell
do
    x <- getX
    y <- getY
    return (x + y)
-- Desugars to: getX >>= \x -> getY >>= \y -> return (x + y)
```
**Impact**: Imperative-looking code for monadic operations; syntax sugar for bind.

---

### 8. **OCaml's Polymorphic Variants**
```ocaml
type shape = [`Circle of int | `Square of int]
let area = function
    | `Circle r -> 3.14 *. float_of_int r *. r
    | `Square s -> float_of_int s *. float_of_int s
```
**Impact**: More flexible than enum; subtypable variants.

---

### 9. **Roc's Ability System** (Unique to Roc)
```roc
myEffect : Str -> Task {} [Read Str, Write Str]
```
**Impact**: Effect tracking at type level; explicit capability requirements.

---

### 10. **Rust's Lifetime Annotations**
```rust
fn borrow<'a>(s: &'a str) -> &'a str {
    s
}
```
**Impact**: Explicit lifetime tracking; prevents dangling references without GC.

---

## Comparative Code Examples

### Example 1: Parsing and Error Handling

**Rust** (Explicit Result):
```rust
fn parse_int(s: &str) -> Result<i32, ParseIntError> {
    i32::from_str(s)
}

match parse_int("42") {
    Ok(n) => println!("Number: {}", n),
    Err(e) => eprintln!("Error: {}", e),
}
```

**Go** (Multiple Returns):
```go
func parseInt(s string) (int, error) {
    n, err := strconv.Atoi(s)
    if err != nil {
        return 0, err
    }
    return n, nil
}

n, err := parseInt("42")
if err != nil {
    fmt.Fprintf(os.Stderr, "Error: %v\n", err)
}
```

**Haskell** (Either Monad):
```haskell
parseInt :: String -> Either String Int
parseInt s = case reads s of
    [(n, "")] -> Right n
    _ -> Left "Invalid integer"

case parseInt "42" of
    Right n -> putStrLn $ "Number: " ++ show n
    Left e -> hPutStrLn stderr $ "Error: " ++ e
```

**Gleam** (Result Type):
```gleam
let result = parse_int("42")
case result {
    Ok(n) -> io.println("Number: " <> int.to_string(n))
    Error(e) -> io.println("Error: " <> e)
}
```

---

### Example 2: Iteration and Transformation

**Rust** (Iterator Chains):
```rust
vec![1, 2, 3, 4, 5]
    .iter()
    .filter(|x| x % 2 == 0)
    .map(|x| x * 2)
    .for_each(|x| println!("{}", x));
```

**Kotlin** (Scope Functions):
```kotlin
listOf(1, 2, 3, 4, 5)
    .filter { it % 2 == 0 }
    .map { it * 2 }
    .forEach { println(it) }
```

**Go** (Imperative Loop):
```go
for _, n := range []int{1, 2, 3, 4, 5} {
    if n%2 == 0 {
        fmt.Println(n * 2)
    }
}
```

**Gleam** (Pipe Operator):
```gleam
[1, 2, 3, 4, 5]
|> list.filter(fn(x) { x % 2 == 0 })
|> list.map(fn(x) { x * 2 })
|> list.each(fn(x) { io.print(int.to_string(x)) })
```

**Haskell** (List Comprehension):
```haskell
[ x * 2 | x <- [1, 2, 3, 4, 5], x `mod` 2 == 0 ]
```

---

### Example 3: Pattern Matching

**Rust**:
```rust
let value = Some(42);
match value {
    Some(n) if n > 0 => println!("Positive: {}", n),
    Some(0) => println!("Zero"),
    Some(_) => println!("Negative"),
    None => println!("No value"),
}
```

**Kotlin**:
```kotlin
val value: Int? = 42
when (value) {
    null -> println("No value")
    in 1..Int.MAX_VALUE -> println("Positive: $value")
    0 -> println("Zero")
    else -> println("Negative")
}
```

**Haskell**:
```haskell
let value = Just 42
in case value of
    Just n | n > 0 -> putStrLn $ "Positive: " ++ show n
    Just 0 -> putStrLn "Zero"
    Just _ -> putStrLn "Negative"
    Nothing -> putStrLn "No value"
```

**Elm**:
```elm
value = Just 42
case value of
    Just n -> if n > 0 then "Positive: " ++ String.fromInt n else "..."
    Nothing -> "No value"
```

---

### Example 4: Closures and Higher-Order Functions

**Rust**:
```rust
let multiplier = 3;
let nums = vec![1, 2, 3];
let doubled = nums.iter()
    .map(|x| x * multiplier)  // Captures multiplier
    .collect::<Vec<_>>();
```

**JavaScript-like (no language specified, but for reference)**:
```javascript
const multiplier = 3;
const nums = [1, 2, 3];
const doubled = nums.map(x => x * multiplier);
```

**Swift**:
```swift
let multiplier = 3
let nums = [1, 2, 3]
let doubled = nums.map { $0 * multiplier }  // Captures multiplier
```

**Haskell**:
```haskell
let multiplier = 3
    nums = [1, 2, 3]
    doubled = map (* multiplier) nums  -- Function partial application
in doubled
```

**Go**:
```go
multiplier := 3
nums := []int{1, 2, 3}
var doubled []int
for _, n := range nums {
    doubled = append(doubled, n * multiplier)
}
```

---

## Summary of Trends

### What's Converging:
1. **Expression-based control flow**: Rust, Kotlin, Swift, Zig, Gleam, Elm, Roc, Haskell embrace this
2. **Explicit null safety**: Optionals/Result types replacing implicit null
3. **Pattern matching**: Moving beyond switch statements
4. **Type inference**: Except Go (explicit always)
5. **Immutability by default**: Rust, Swift, Kotlin, Gleam, Elm, Roc, Haskell
6. **Pipe composition**: Functional languages standardizing on `|>`

### What's Diverging:
1. **Keyword count**: Minimalism (Zig, Gleam) vs. richness (Swift, Kotlin)
2. **Error handling**: Exceptions (Kotlin, Swift, OCaml) vs. values (Rust, Go, Gleam, Elm)
3. **Custom operators**: Allowed (OCaml, Haskell, Swift) vs. fixed (Rust, Go, Zig)
4. **Syntax sugar complexity**: Swift/Kotlin embrace it; Zig/Gleam reject it
5. **Lazy vs. strict evaluation**: Haskell lazy; all others strict by default

### Modern Language Lessons:
- **Syntax matters deeply**: Go's minimalism isn't weakness; it's deliberate simplicity
- **Null safety is universal**: Every modern language solves it, differently
- **Lambdas are standard**: Functional features in imperative languages
- **Composition beats callbacks**: Pipe operators gaining traction
- **Errors as values**: Rust/Go approach increasingly popular
- **Readability over cleverness**: Zig's "no hidden control flow" philosophy resonates

---

This comprehensive comparison demonstrates that syntax design reflects **language philosophy** more than technical necessity. Minimalist languages (Go, Zig, Gleam) enforce discipline. Rich languages (Swift, Kotlin) offer convenience. Functional languages (Haskell, OCaml, Elm) optimize for composition. The modern trend favors **explicit**, **composable**, and **type-safe** syntax.
