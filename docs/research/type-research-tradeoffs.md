# Spore Type System: Design Trade-offs & Recommendations

> 📦 **Frozen design artifact. Key decisions incorporated into type-system-v0.1.md.**

> **Status**: Research deliverable · June 2025
> **Scope**: Comprehensive analysis of type-system design choices for Spore, a general-purpose language optimized for Human–Agent collaboration.

---

## 0. Spore's Existing Commitments (Constraints on This Analysis)

Before analyzing options, note what is already decided—these form hard constraints:

| Decision | Implication for type system |
|---|---|
| Function signatures as "gravity centers" | Signatures must be richly typed and explicit; bodies may be inferred |
| Capability-based effects `uses [FileRead, NetWrite]` | Already an effect type system; must integrate with generics/traits |
| Abstract cost model `cost ≤ N` | Value-level information in types (proto-refinement) |
| Typed holes `?name` | Type-directed synthesis is a first-class concern |
| No positional parameters | Structural matching of argument records by name |
| Closed enum errors `! [Err1, Err2]` | Row-typed error sets; must compose across call boundaries |
| `where`/`with`/`cost`/`uses` clauses | Structured signature clauses needed |
| Bounded types `List<T, max: 500>` | Value-level type parameters already exist |

**Key insight**: Spore has *already* committed to several features that push it beyond "basic generics + traits." The question isn't *whether* to have rich types, but *how far to go* and *how to keep them ergonomic*.

---

## 1. The Type System Spectrum: Where Spore Should Land

### The Landscape

| Level | Examples | Annotation Effort | Safety Ceiling | Agent Leverage |
|---|---|---|---|---|
| **L1** Basic generics | Go, early Java | Low | Low | Low |
| **L2** Generics + traits/interfaces | Rust, Kotlin, Swift | Medium | Medium | Medium |
| **L3** Generics + HKT + type classes | Haskell, Scala 3 | High | High | High |
| **L4** Refinement types | Liquid Haskell, F* | Very High | Very High | Very High |
| **L5** Full dependent types | Idris, Agda, Lean 4 | Extreme | Maximum | Maximum |

### Analysis

**L1 is ruled out.** Spore already needs value-level type parameters (`max: 500`), capability tracking, and cost annotations. Go-level generics cannot express these.

**L2 is the floor.** Rust-style generics + traits are the minimum viable system to express Spore's existing features. But `cost ≤ N` and `max: 500` push beyond what pure L2 can encode natively—these require const generics at minimum.

**L3 (full HKT) is a trap for Spore.** HKTs solve the "abstract over container kinds" problem (`Functor`, `Monad`, etc.). But:
- Spore's capability system already replaces where monads are most used (effect sequencing).
- HKTs dramatically increase type error complexity, conflicting with Elm-like error messages.
- The primary beneficiaries of HKT are library authors building deep abstraction towers—Spore's Agent can handle complexity, but HKT error messages are hard even for Agents to parse.
- **Verdict**: Defer HKTs. Provide associated types (Swift-style) as the 80/20 solution. Reserve HKT as a future extension if ecosystem demand proves it necessary.

**L4 (refinement types) is Spore's sweet spot—but only a *targeted subset*.** Spore already has:
- `cost ≤ N` — this IS a refinement predicate on function types
- `max: 500` — this IS a refinement on collection types
- `! [Err1, Err2]` — closed error sets are row-typed refinements

Rather than bolting on full Liquid Haskell-style refinement (which requires SMT solvers and creates inscrutible errors), Spore should support **lightweight, enumerated refinement predicates**:
- Numeric bounds: `Int if 0 < self ≤ 100`
- Size bounds: `List<T, max: N>`
- Cost bounds: `cost ≤ N`
- Effect sets: `uses [Cap1, Cap2]`
- These predicates are decidable, compile-time checkable, and produce clear errors.

**L5 (full dependent types) is out.** The implementation burden is extreme. Lean 4 is the current best attempt at "practical dependent types" and still requires proof tactics. Spore does not need theorem proving.

### ⭐ Recommendation: Level 2.5–3.5

**"Rust++ with targeted refinements."**

```
Generics + Traits + Associated Types + Const Generics + Lightweight Refinements
```

Specifically:
- Generics with trait bounds (Rust-like)
- Associated types in traits (no full HKT)
- Const/value generics (`List<T, max: 500>`, `cost ≤ N`)
- Lightweight refinement predicates (numeric bounds, decidable constraints)
- Row-typed effects and error sets

This gives Spore ~90% of L3–L4 expressiveness with ~60% of the complexity.

---

## 2. Nominal vs Structural: The Hybrid Answer

### The Core Tension

| Dimension | Nominal wins | Structural wins |
|---|---|---|
| **Human readability** | ✅ "This is a `UserId`, not just a `String`" | |
| **Capability safety** | ✅ `FileRead` ≠ `NetRead` even if same shape | |
| **Agent hole-filling** | | ✅ Machine matches by shape, not name |
| **Cross-crate interop** | | ✅ No need to agree on names a priori |
| **Error message clarity** | ✅ "Expected `Temperature`, got `Pressure`" | |
| **Refactoring safety** | ✅ Renames don't silently break things | |

### Spore-Specific Analysis

**Capabilities MUST be nominal.** If `FileRead` and `NetRead` were structural (both "a capability with a `path: String` field"), code could accidentally gain capabilities by structural coincidence. This is a security boundary — nominal types are non-negotiable here.

**Data types should be *primarily nominal with structural escape hatches*.** Specifically:

1. **Named types are nominal by default:**
   ```spore
   type UserId = String    // nominal wrapper — UserId ≠ String
   type Celsius = Float64  // nominal — Celsius ≠ Fahrenheit
   ```

2. **Anonymous records/tuples are structural:**
   ```spore
   fn process(input: { name: String, age: Int }) -> Result
   // Any record with matching fields satisfies this
   ```

3. **Trait satisfaction is nominal (explicit `impl`):**
   ```spore
   trait Printable { fn display(self) -> String }
   impl Printable for UserId { ... }  // explicit declaration
   ```

4. **Hole-filling uses structural matching internally, nominal matching at boundaries:**
   - Agent sees a hole `?x : { name: String, age: Int }` → structural search for candidates
   - But at function call boundaries, nominal types are enforced
   - This gives Agents maximum flexibility while maintaining safety

### ⭐ Recommendation: Nominal-primary with structural records

**Model**: Similar to OCaml's approach (nominal types + structural polymorphic variants) or TypeScript's hybrid (nominal classes + structural interfaces), but with the Spore twist that **capabilities are always strictly nominal**.

The key insight: Structural typing is an *implementation detail of the Agent's search strategy*, not a user-facing type compatibility rule. Users think nominally; Agents search structurally; the compiler enforces nominally at API boundaries.

---

## 3. Trait/Typeclass/Protocol: Spore "Capabilities as Traits"

### Comparison Matrix

| Feature | Rust traits | Haskell TC | TS interfaces | Swift protocols | Scala 3 given | Go interfaces |
|---|---|---|---|---|---|---|
| Dispatch | Static + dyn | Static (dict) | Structural | Static + existential | Static (ctx) | Dynamic |
| Orphan safety | ✅ Strict | ⚠️ Possible | N/A | ⚠️ Possible | ⚠️ Scoped | N/A |
| Associated types | ✅ | ✅ (type families) | ❌ | ✅ | ✅ | ❌ |
| Default methods | ✅ | ✅ | ❌ | ✅ | ✅ | ❌ |
| Coherence | Global | Global | N/A | Global | Scoped | N/A |
| Const in traits | ⚠️ (nightly) | ❌ | ❌ | ❌ | ❌ | ❌ |
| Effect integration | ❌ | ❌ (monads) | ❌ | ❌ | ❌ | ❌ |

### The Spore-Specific Insight: Capabilities ARE Traits

Spore's `uses [FileRead, NetWrite]` is fundamentally the same mechanism as trait bounds:

```spore
// Capabilities can be declared with either syntax:
fn read_file(path: Path) -> Bytes  uses [FileRead]
fn read_file(path: Path) -> Bytes  with [FileRead]  // capability as context
```

This means Spore's trait system and capability system should be **unified**:

```spore
// A capability is a trait on the execution context
capability FileRead {
    fn read(path: Path) -> Bytes ! [IoError]
    cost read ≤ 100  // capability methods have costs
}

// A regular trait
trait Printable {
    fn display(self) -> String
    cost display ≤ 10
}

// Constraints split across where, with, uses, and cost clauses
fn process<T>(item: T) -> String ! [IoError]
where T: Printable
with [FileRead]
cost ≤ 200
```

### Orphan Rule Design

**Spore should adopt Rust's coherence model with one extension:**

- **Base rule** (Rust-like): You can only implement a trait for a type if you own either the trait or the type.
- **Extension for Agents**: An `adapter` mechanism allows local, scoped trait implementations that are explicit at use-sites:

```spore
// In a third-party integration module:
adapter ExternalType as Printable {
    fn display(self) -> String { ... }
}

// Must be explicitly imported and activated:
fn use_it(x: ExternalType) -> String
where adapter: ExternalType as Printable
{
    x.display()
}
```

This avoids the orphan instance problem while preserving extensibility. The `adapter` is visible in the function signature (gravity center principle), making it transparent.

### ⭐ Recommendation: Rust-style nominal traits unified with capabilities

- Nominal trait implementation (explicit `impl`)
- Associated types (no HKT)
- Default method implementations
- Cost annotations on trait methods
- Capability = trait on execution context
- Rust-like coherence + explicit `adapter` for cross-crate extension
- Traits compose via `where` (type constraints), `with` (effects/capabilities), `cost`, and `uses` clauses

---

## 4. Algebraic Data Types: Spore's Design Choices

### 4.1 Sum Types: Rust-style Enums with Named Fields

```spore
type Shape =
    | Circle { radius: Float64 }
    | Rectangle { width: Float64, height: Float64 }
    | Triangle { a: Float64, b: Float64, c: Float64 }
```

**Why Rust-style, not Haskell-style:**
- Spore already requires named parameters → enum variant fields should also be named
- Named fields make Agent hole-filling easier: `?shape : Shape.Circle` has discoverable fields
- Consistent with Spore's "no positional" philosophy

### 4.2 Product Types: Structs with Named Fields (Already Decided)

```spore
type Point = { x: Float64, y: Float64 }
```

### 4.3 Pattern Matching: Comprehensive but Pragmatic

```spore
fn area(shape: Shape) -> Float64 =
    match shape {
        Circle { radius } => pi * radius * radius,
        Rectangle { width, height } => width * height,
        Triangle { a, b, c } => {
            let s = (a + b + c) / 2.0
            sqrt(s * (s - a) * (s - b) * (s - c))
        }
    }
```

**Features to include:**
- ✅ **Exhaustiveness checking** — mandatory, no escape hatch. This is critical for Agent-generated code correctness and for closed error handling.
- ✅ **Nested patterns** — `Circle { radius: r } if r > 0.0 =>`
- ✅ **Guard clauses** — `| x if x > 0 =>`
- ✅ **Or-patterns** — `Circle { .. } | Rectangle { .. } =>`
- ✅ **Named field destructuring** — consistent with no-positional philosophy
- ❌ **View patterns** (Haskell) — too complex, defer
- ❌ **Active patterns** (F#) — defer

### 4.4 Sealed Types: Yes, All Spore Enums Are Sealed

Spore enums are closed by definition. You cannot add variants outside the defining module. This is essential for:
- Exhaustiveness checking
- Closed error sets (`! [Err1, Err2]`)
- Agent reasoning (Agent needs to know ALL possible cases)

**If extensibility is needed**, use traits instead of enums:

```spore
// Closed (enum) — use when all cases are known
type HttpError =
    | NotFound { url: Url }
    | Timeout { after: Duration }
    | Unauthorized

// Open (trait) — use when new implementations should be addable
trait Serializable {
    fn serialize(self) -> Bytes
}
```

### 4.5 Error Type Interaction with ADTs

Spore's `! [Err1, Err2]` is a **closed, row-typed error union**:

```spore
fn parse(input: String) -> Ast ! [SyntaxError, EncodingError]
fn validate(ast: Ast) -> ValidAst ! [TypeError, RangeError]

// Composition: error sets union automatically
fn compile(input: String) -> ValidAst ! [SyntaxError, EncodingError, TypeError, RangeError] = {
    let ast = parse(input: input)?
    validate(ast: ast)?
}
```

**Key design decision**: Error types in `! [...]` are themselves enum variants (ADTs), but the `! [...]` syntax is row-typed — it automatically computes the union of possible errors across call chains. This is similar to:
- Koka's effect rows (for the row-typing mechanism)
- Rust's `Result<T, E>` (for the explicitness)
- Zig's error sets (for the union semantics)

But unlike Zig, each error type carries structured data (it's a full ADT variant, not just a name).

---

## 5. Generics: The Feature Budget

### What to Include (Priority Order)

#### P0: Must Have for Day 1

**Basic parametric generics with trait bounds:**
```spore
fn sort<T>(list: List<T>) -> List<T>
where T: Ord
```

**Const/value generics (already committed):**
```spore
type FixedList<T, max: Int> = ...
fn take<T, N: Int>(list: List<T>, count: N) -> List<T, max: N>
where N ≤ list.max
```

**Associated types in traits:**
```spore
trait Iterator {
    type Item
    fn next(self) -> Option<Self.Item>
    cost next ≤ 10
}
```

This covers 90%+ of real-world generic code needs. Rust operated without GATs for years and shipped enormous production systems.

#### P1: Include in v1.0, Can Defer from Bootstrap

**Bounded quantification / variance annotations:**
```spore
fn upcast<T, U>(list: List<T>) -> List<U>
where T: U  // T is a subtype of U
```

**Generic constraints in `where` blocks (already the plan):**
```spore
fn process<T>(items: List<T>) -> Summary
where T: Serializable + Printable
with [pure]
cost ≤ items.len * 10
```

#### P2: Design Now, Implement Later

**Variadic generics:**
```spore
fn zip<...Ts>(lists: ...List<Ts>) -> List<(...Ts)>
```

This is genuinely useful (especially for typed tuple operations and function composition) but extremely hard to implement well. TypeScript's approach (mapped types + conditional types) is powerful but creates incomprehensible error messages. Rust doesn't have them. Only C++ (parameter packs) and Zig (comptime) have practical variadic generics.

**Recommendation**: Design the syntax now (so the grammar is forward-compatible) but defer implementation. Use code generation / macros for the bootstrap period.

#### P3: Explicitly Defer (Maybe Never)

**Higher-kinded types:**
```spore
// NOT in Spore v1:
fn map<F: Functor, A, B>(f: A -> B, fa: F<A>) -> F<B>
```

**Reasons to defer:**
1. Spore's capability system replaces most monad use cases
2. Associated types handle 80% of HKT use cases
3. HKT error messages are terrible, conflicting with Spore's Elm-like error goal
4. Agents don't need HKT to do type-directed synthesis—associated types + trait bounds are sufficient

**If HKT proves necessary**, Spore can add it as a language extension (like Haskell's `{-# LANGUAGE TypeFamilies #-}`) without breaking existing code.

---

## 6. Inference vs Annotation: The Gravity Center Rule

### Spore's Principle: "Explicit at boundaries, inferred in bodies"

This is already the right approach. Here's the precise specification:

#### MUST Be Annotated (Explicit)

| Element | Why |
|---|---|
| Function parameter types | Gravity center — the function signature IS the API |
| Function return type | Agent reads signatures for synthesis; human reads for understanding |
| Effect/capability sets | Security boundary — must be visible |
| Cost bounds | Performance contract — must be visible |
| Error sets | Error contract — must be visible |
| Struct/type field types | Data definition — must be explicit |
| Trait method signatures | Interface contract |
| Public constants | API surface |

#### CAN Be Inferred (Optional Annotation)

| Element | Inference strategy |
|---|---|
| Local variable types | `let x = compute(...)` — inferred from RHS |
| Generic type parameters at call sites | `sort(list: my_list)` — T inferred from `my_list` |
| Closure parameter types (when context known) | `.map(fn(x) => x + 1)` — x inferred from Iterator.Item |
| Intermediate expression types | Standard HM-like local inference |
| Effect propagation within function bodies | If body calls `read_file(...)`, FileRead capability inferred |

#### SPECIAL: Effects and Cost Inference Within Bodies

The capability system creates an interesting inference question: should effects be inferred or declared?

**Recommended approach**: **Declared at boundaries, inferred within bodies with verification.**

```spore
// The programmer declares the effect boundary:
fn process(path: Path) -> Data ! [IoError]
uses [FileRead]
cost ≤ 500
{
    // Within the body, the compiler INFERS which capabilities are used
    // and VERIFIES they're within the declared set.
    // If the body uses NetWrite, compiler error: "NetWrite not in declared uses"
    let bytes = read_file(path: path)  // uses FileRead — OK, declared
    parse(data: bytes)                  // pure — OK
}
```

This is analogous to Elm's approach: signatures are required and checked, but the compiler verifies the body against them rather than requiring effect annotations on every subexpression.

### Bidirectional Type Checking

Spore should use bidirectional type checking:
- **Check mode** (top-down): When a type is expected (from annotation or context), check that the expression matches.
- **Synth mode** (bottom-up): When no type is expected, synthesize from the expression.

This naturally implements the "explicit at boundaries, inferred in bodies" rule:
- Function signatures provide the checking context (top-down)
- Function bodies synthesize types (bottom-up)
- The two meet at `return` and at function calls

**Error message quality**: Bidirectional checking produces *localized* errors because mismatches are caught at the boundary between check and synth modes, pointing to exactly where expectation meets reality.

---

## 7. The Agent Factor: How AI Changes the Calculus

### 7.1 Typed Holes as the Primary Agent Interface

Spore's `?name` holes are the killer feature for Agent collaboration. Research from Hazel (OOPSLA 2024, "Statically Contextualizing Large Language Models with Typed Holes") shows that providing LLMs with the static type context of a hole dramatically improves code generation quality.

**What Agents need from the type system:**

| Agent Need | Type System Feature | Spore Status |
|---|---|---|
| "What type goes here?" | Typed hole with inferred expected type | ✅ `?name : ExpectedType` |
| "What effects can I use?" | Capability set from enclosing function | ✅ `uses [FileRead, ...]` |
| "What errors can I throw?" | Error set from enclosing function | ✅ `! [Err1, Err2]` |
| "What's in scope?" | Binding context with types | Standard |
| "What cost budget remains?" | Cost bound minus already-spent cost | ✅ `cost ≤ remaining` |
| "What satisfies this trait?" | Searchable impl database | Nominal traits ✅ |

### 7.2 HoleReport: Machine-Readable Type Context

The compiler should emit structured hole reports:

```json
{
  "hole": "?parser",
  "expected_type": "(input: String) -> Ast ! [SyntaxError]",
  "available_capabilities": ["FileRead"],
  "cost_budget_remaining": 200,
  "bindings_in_scope": [
    { "name": "config", "type": "ParserConfig" },
    { "name": "cache", "type": "Cache<String, Ast>" }
  ],
  "trait_constraints": ["T: Parseable"],
  "candidate_functions": [
    { "name": "parse_v2", "signature": "...", "cost": 150 },
    { "name": "parse_cached", "signature": "...", "cost": 50 }
  ]
}
```

This is Spore's unfair advantage: the type system doesn't just check correctness — it generates a **specification for program synthesis**.

### 7.3 Richer Types Help Agents More Than They Hurt Humans

Traditional language design assumes human-only users and optimizes for annotation burden. Spore's Agent-collaboration model changes this:

| Concern | Human-only | Human+Agent |
|---|---|---|
| Complex type annotations | High burden | Agent writes them |
| Reading complex signatures | Hard | Agent summarizes; human reads v1-v4 format |
| Type error messages | Must be simple | Dual-format: JSON for Agent, English for human |
| Exhaustive error handling | Tedious | Agent generates match arms; human reviews |
| Cost/effect annotation | Extra work | Agent infers; human approves |

**Implication**: Spore can afford a *somewhat* richer type system than a human-only language because the Agent absorbs the annotation complexity. But the type system must remain **predictable** — Agents fail catastrophically with unpredictable type-level computation (like Scala 2 implicit resolution or C++ SFINAE).

### 7.4 Type Error Messages: Dual-Channel Design

```
// v0: Machine-readable (for Agent)
{"error": "type_mismatch", "expected": "Int", "got": "String",
 "location": "main.spore:42:15", "context": "argument 'count' of fn process"}

// v1: Terse human-readable
Error: Type mismatch at main.spore:42:15
  expected: Int
  got: String

// v2: Contextual (Elm-style)
Error: Type mismatch

42 |    process(count: user_input)
                       ^^^^^^^^^^
  The function `process` expects `count` to be an Int,
  but `user_input` is a String.

  Hint: Did you mean to use `parse_int(input: user_input)`?

// v3-v4: Progressive detail (show constraint chain, effect context, etc.)
```

---

## 8. Final Recommendation: Spore's Type System Blueprint

### Tier 1: Core (Must ship in bootstrap)

| Feature | Design | Rationale |
|---|---|---|
| **Parametric generics** | `fn f<T>(x: T)` with trait bounds | Standard, essential |
| **Nominal traits** | Explicit `impl Trait for Type` | Safety, coherence, capability unification |
| **Associated types** | `trait Iter { type Item }` | 80% of HKT benefits without complexity |
| **Const/value generics** | `List<T, max: 500>` | Already committed; enables bounded types |
| **Sealed enums (ADTs)** | Rust-style with named fields | Exhaustiveness, error types, Agent reasoning |
| **Exhaustive pattern matching** | Nested, guards, or-patterns | Core language feature, non-negotiable |
| **Row-typed error sets** | `! [Err1, Err2]` with auto-union | Already committed |
| **Capability-as-trait** | `capability FileRead { ... }` | Unified abstraction |
| **Bidirectional type inference** | Explicit signatures, inferred bodies | Gravity center principle |
| **Lightweight refinements** | `cost ≤ N`, `max: N`, numeric bounds | Already committed via cost/bounded types |

### Tier 2: v1.0 (Design now, implement after bootstrap)

| Feature | Design | Rationale |
|---|---|---|
| **Adapter mechanism** | Scoped, explicit cross-crate trait impl | Solves orphan problem without unsafety |
| **Structural anonymous records** | `{ name: String, age: Int }` | Agent flexibility, FFI convenience |
| **Signature clauses (`where`/`with`/`cost`/`uses`)** | Type constraints in `where`, effects in `with`, cost/uses standalone | Already committed; needs full impl |
| **Dual-channel error messages** | JSON (Agent) + Elm-style (human) | Core to Human–Agent collaboration |

### Tier 3: Post-v1.0 (Design direction, explicit deferral)

| Feature | Design | Rationale |
|---|---|---|
| **Variadic generics** | `fn zip<...Ts>(...)` | Useful but implementation-heavy; use macros for now |
| **Higher-kinded types** | Opt-in language extension | Only if ecosystem proves need; capabilities replace most monad use |
| **Richer refinement predicates** | `Int if self > 0`, sortedness, etc. | Extend the lightweight system gradually |
| **Effect polymorphism** | `fn map<E>(f: A -> B uses E)` | Useful for generic combinators; design carefully |
| **Subtyping / variance** | Covariant/contravariant annotations | Needed for collections but adds complexity |

### Tier 4: Explicitly Never (Unless Proven Wrong)

| Feature | Why not |
|---|---|
| **Full dependent types** | Requires proof tactics; Lean-level complexity |
| **Implicit conversions** | Source of bugs in Scala 2; Spore prefers explicit |
| **Runtime type reflection** | Conflicts with cost model and static analysis |
| **Structural trait satisfaction** (Go-style) | Undermines capability safety |
| **Type-level computation** (TS conditional types) | Unpredictable, terrible errors |

### Implementation Priority Order

```
Phase 1 (Bootstrap):  Generics + Traits + Enums + Pattern Matching + Basic Inference
Phase 2 (Alpha):      Const Generics + Error Rows + Capability-as-Trait + Hole Reports
Phase 3 (Beta):       Refinement Bounds + Adapters + Structural Records + Dual Errors
Phase 4 (v1.0):       Signature Clause Unification + Effect Verification + Full Cost Checking
Phase 5 (Post-v1.0):  Variadics + HKT Extension + Richer Refinements
```

### The One-Line Summary

> **Spore's type system is "Rust traits + Koka effects + Liquid Haskell-lite refinements, designed for Agent-assisted program synthesis via typed holes."**

It is more expressive than Rust (refinements, row-typed effects), more practical than Haskell (no HKT requirement, nominal safety), and uniquely positioned for Agent collaboration (structured hole reports, dual-channel errors, capability-scoped synthesis).

---

## Appendix A: Key References

- **Hazel / Typed Holes + LLMs**: "Statically Contextualizing Large Language Models with Typed Holes" (OOPSLA 2024) — foundational for Spore's hole system
- **Koka effect system**: Row-typed algebraic effects — model for Spore's capability integration
- **Liquid Haskell**: Refinement types with SMT — inspiration for Spore's lightweight refinements (without the SMT)
- **Rust trait coherence**: Orphan rules — model for Spore's trait implementation rules
- **Elm error messages**: Gold standard for human-readable type errors — target for Spore's v2+ error format
- **Scala 3 contextual abstractions**: `given`/`using` — reference for capability-as-context design
- **Bidirectional type checking**: Standard technique for "explicit at boundaries, inferred in bodies"

## Appendix B: Interaction Matrix

How each type system feature interacts with Spore's existing systems:

|  | Capabilities | Cost Model | Holes | Error Sets | Named Params |
|---|---|---|---|---|---|
| **Generics** | Capability bounds in where | Cost of generic calls | Type params flow into holes | Error sets are generic | N/A |
| **Traits** | Capabilities ARE traits | Trait methods have costs | Trait bounds constrain holes | Error traits compose | Trait methods use named params |
| **ADTs** | Capability enums? No—too dangerous | Enum construction has cost | Variant fields fill holes | Error enums in ! [...] | Variant fields are named |
| **Pattern matching** | N/A | Match has cost | Match on holes = partial program | Exhaustive error handling | Named field patterns |
| **Refinements** | N/A | cost ≤ N IS refinement | Refinement narrows hole type | Error set IS refinement | N/A |
| **Inference** | Effects inferred in bodies | Costs inferred, checked against bound | Hole types inferred from context | Error unions inferred | Param names not inferred (explicit) |
