# Dependent Type Systems in Programming Languages

> A comprehensive survey of dependent types across seven languages, covering type system mechanics,
> practical trade-offs, and the spectrum from refinement types to full dependent types.

---

## Table of Contents

1. [What Are Dependent Types?](#what-are-dependent-types)
2. [The Spectrum of Dependent Typing](#the-spectrum-of-dependent-typing)
3. [Language Surveys](#language-surveys)
   - [Idris 2](#1-idris-2)
   - [Agda](#2-agda)
   - [Lean 4](#3-lean-4)
   - [Coq / Rocq](#4-coq--rocq)
   - [F\* (F-star)](#5-f-f-star)
   - [Dafny](#6-dafny)
   - [Liquid Haskell](#7-liquid-haskell)
4. [The "Dependent Types Are Too Hard" Argument](#the-dependent-types-are-too-hard-argument)
5. [Practical Compromises](#practical-compromises)
6. [The "Sweet Spot" Question](#the-sweet-spot-question)
7. [Cross-Language Comparison Table](#cross-language-comparison-table)

---

## What Are Dependent Types?

### Types that depend on values

In a conventional type system, types and values occupy separate universes. You can have `List<Int>` or `Map<String, Bool>`, but the *type* never refers to a *runtime value*. A dependent type system collapses this wall: types may be parameterized by values, not just other types.

The simplest example:

```
-- A vector (list) whose type includes its length
Vec : Type → Nat → Type

xs : Vec Int 3      -- a list of exactly 3 integers
ys : Vec Int 0      -- an empty list of integers
```

Here `3` and `0` are ordinary natural numbers — *values* — that appear inside the type. The type checker can now reject `head ys` at compile time because `ys` has length `0`.

### Simple examples

| Concept | Type | What it prevents |
|---------|------|-----------------|
| Length-indexed vector | `Vec T n` | Out-of-bounds access, length mismatch in `zip` |
| Bounded integer | `{x : Int \| 0 ≤ x < n}` | Array index overflow |
| Non-empty list | `Vec T (S n)` | Calling `head` on empty list |
| Matrix dimensions | `Matrix m n` | Incompatible multiplication shapes |

### Complex examples

- **Well-typed `printf`**: The format string `"%d is %s"` determines the *type* of the remaining arguments: `(Int, String)`. In Idris or Agda, a `PrintfFormat` GADT encodes the argument sequence, and the `printf` function is only well-typed when the provided arguments match.

- **Database query types**: A table schema `{id: Int, name: String}` encoded at the type level ensures that `SELECT age FROM users` fails at compile time (no `age` column). Projects like Prisma's TypedSQL approximate this in TypeScript.

- **Protocol state machines**: A network protocol's state (e.g., `Closed → Connected → Authenticated`) is tracked in the type. Calling `query` when the connection is `Closed` is a *type error*, not a runtime exception.

---

## The Spectrum of Dependent Typing

```
No dependent types    Refinement types    Indexed types / GADTs    Full dependent types
──────────────────────────────────────────────────────────────────────────────────────►
   Java, Go             Liquid Haskell      Haskell GADTs,           Idris, Agda,
   Python               F*, Dafny           Rust const generics,     Lean, Coq
                                            TS template literals
```

| Level | What's allowed at type level | Automation | Languages |
|-------|------------------------------|-----------|-----------|
| **None** | Only other types | Full inference | Java, Go, Python |
| **Refinement** | Predicates over values, checked by SMT | High (SMT solver) | Liquid Haskell, F*, Dafny |
| **Indexed / GADT** | Discrete value parameters, constructor-scoped | Moderate | Haskell + GADTs, Rust `const N`, OCaml |
| **Full dependent** | Arbitrary computation in types | Low (manual proofs) | Idris 2, Agda, Lean 4, Coq |

---

## Language Surveys

---

### 1. Idris 2

**Foundation**: Full dependent types · Quantitative Type Theory (QTT) · Compiles via Chez Scheme / RefC / Node.js

#### Type System Summary

Idris 2 is the most deliberate attempt to make dependent types a *practical programming language feature* rather than a proof-assistant afterthought. Built on Edwin Brady's research, it treats types as first-class values: functions can compute types, and types can pattern-match on values. The canonical example is the length-indexed vector:

```idris
data Vect : Nat -> Type -> Type where
  Nil  : Vect 0 a
  (::) : a -> Vect n a -> Vect (S n) a

-- Safe head — only compiles for non-empty vectors
head : Vect (S n) a -> a
head (x :: _) = x
```

Idris 2's key innovation is **Quantitative Type Theory (QTT)**, which annotates every variable binding with a *quantity*:

| Annotation | Meaning | Runtime presence | Example |
|-----------|---------|-----------------|---------|
| `(0 n : Nat)` | Erased | No — compile-time only | `head : (0 n : Nat) -> Vect (S n) a -> a` |
| `(1 x : a)` | Linear — used exactly once | Yes | `consume : (1 x : FileHandle) -> IO ()` |
| *(no annotation)* | Unrestricted (ω) | Yes | `add : Nat -> Nat -> Nat` |

This solves a perennial problem in dependently typed languages: type indices like `n` in `Vect n a` are needed for type checking but should not exist at runtime. In Idris 2, marking them `0` guarantees erasure — the compiler enforces that erased arguments are never inspected at runtime.

Idris 2 also supports **interactive, hole-driven development**. You write a type signature, leave the body as `?hole`, and the compiler tells you the type of the hole and the variables in scope with their quantities. This makes dependent types feel like a conversation with the compiler rather than a battle.

The language compiles through multiple backends (Chez Scheme is the default) and aims for practical performance. Unlike proof assistants that produce extracted code, Idris 2 produces directly executable programs.

#### Protocol State Machine Example

```idris
data DoorState = Closed | Open

data DoorCmd : DoorState -> DoorState -> Type where
  OpenDoor  : DoorCmd Closed Open
  CloseDoor : DoorCmd Open Closed
  Knock     : DoorCmd Closed Closed

data DoorProg : DoorState -> DoorState -> Type where
  Done : DoorProg s s
  Then : DoorCmd s1 s2 -> DoorProg s2 s3 -> DoorProg s1 s3

-- Valid: knock, open, close
polite : DoorProg Closed Closed
polite = Then Knock (Then OpenDoor (Then CloseDoor Done))

-- INVALID — won't typecheck:
-- bad = Then OpenDoor (Then OpenDoor Done)  -- can't open an already-open door
```

#### What Problems Dependent Types Solve

- **Eliminates entire classes of runtime errors**: Vector bounds, null pointer (via `Maybe` forced by types), protocol violations, format-string mismatches.
- **Proofs as programs**: You can prove that your sorting function returns a sorted permutation of the input. The proof is checked at compile time and erased at runtime via QTT.
- **Resource safety**: Linear types (quantity `1`) ensure file handles, database connections, and memory are used exactly once — preventing double-free and resource leaks.

#### The Cost

- **Learning curve**: QTT adds a new dimension to learn on top of already-complex dependent types. Most programmers have no prior exposure to linearity or erasure annotations.
- **Ecosystem maturity**: Small standard library compared to mainstream languages. Limited third-party packages.
- **Compilation time**: Type checking requires normalizing terms, which can be slow for large programs.
- **Error messages**: When types contain computations, error messages expose normalized terms that may not resemble the original source code.

#### Key Innovations

- **Quantitative Type Theory in practice**: First language to ship QTT as a usable programming feature, unifying linearity and erasure in one framework.
- **Type-driven development workflow**: Holes, case-splitting, and proof-search integrated into the editor make dependent types interactive.
- **Erasure annotations**: Principled compile-time-only arguments that solve the "type indices at runtime" performance problem without ad-hoc optimization passes.

---

### 2. Agda

**Foundation**: Full dependent types · Martin-Löf Type Theory · Proof assistant · Universe polymorphism

#### Type System Summary

Agda is a dependently typed functional programming language *and* proof assistant based on intensional Martin-Löf type theory. Unlike Idris, which prioritizes practical programming, Agda prioritizes *correctness and expressiveness* — it is the language of choice for formalizing type theory itself and for programming-language metatheory.

Agda's type system features a stratified universe hierarchy (`Set₀`, `Set₁`, `Set₂`, …) to maintain logical consistency. This avoids Girard's paradox (which arises from `Set : Set`) by ensuring that each universe lives in the next higher universe:

```agda
-- Universe polymorphic identity
id : ∀ {ℓ : Level} {A : Set ℓ} → A → A
id x = x
```

The classic example — length-indexed vectors with a safe `head`:

```agda
data Vec (A : Set) : ℕ → Set where
  []  : Vec A zero
  _∷_ : ∀ {n} → A → Vec A n → Vec A (suc n)

-- Only accepts non-empty vectors — [] case is impossible
head : ∀ {A : Set} {n : ℕ} → Vec A (suc n) → A
head (x ∷ _) = x
```

Pattern matching in Agda is *dependent*: when you pattern-match on a constructor, Agda learns about the index. In `head`, matching `(x ∷ _)` tells the checker that `n = suc m` for some `m`, so the `[]` case is statically impossible and need not be written.

Agda supports **universe polymorphism** for writing truly generic definitions:

```agda
data _×_ {ℓ₁ ℓ₂ : Level} (A : Set ℓ₁) (B : Set ℓ₂) : Set (ℓ₁ ⊔ ℓ₂) where
  _,_ : A → B → A × B
```

This lets the product type work at any combination of universe levels — `Nat × Bool` lives in `Set`, while `Set × Set` lives in `Set₁`.

Agda programs must be *total*: all functions must terminate and cover all cases. This is essential for its role as a proof assistant (a non-terminating proof would prove anything), but it also means you cannot write arbitrary recursive programs. The termination checker uses structural recursion and sized types to verify totality.

#### What Problems Dependent Types Solve

- **Metatheory formalization**: Agda is used to formalize programming-language semantics, type soundness proofs, and category theory. The Agda Universal Algebra Library formalizes Birkhoff's HSP Theorem.
- **Certified data structures**: Red-black trees with balance invariants in the type, sorted lists with ordering proofs, well-scoped lambda terms.
- **Constructive mathematics**: Agda's type theory is constructive by default — a proof of `∃x. P(x)` always yields a witness `x`.

#### The Cost

- **No built-in tactic system**: Unlike Coq or Lean, Agda proofs are written as terms (programs). This makes proofs transparent but verbose — you write every step explicitly.
- **Totality requirement**: No general recursion. You must convince the termination checker, which sometimes requires restructuring algorithms in unnatural ways.
- **Performance**: Agda is primarily a proof assistant, not a compiler for production code. The type checker can be slow on large developments, and there is no mature extraction or compilation pipeline for efficient executables.
- **Steep learning curve**: Universe levels, implicit arguments, instance arguments, and dependent pattern matching create significant cognitive overhead.

#### Key Innovations

- **Dependent pattern matching**: Agda pioneered the *with*-abstraction for dependent pattern matching, allowing you to pattern-match on intermediate computations while maintaining type correctness.
- **Universe polymorphism**: The Level-based universe hierarchy is more flexible than Coq's original universe system, enabling truly generic definitions across all universe levels.
- **Mixfix operators**: Agda allows defining operators with arbitrary fixity and placement of arguments (e.g., `if_then_else_`), enabling highly readable DSLs.
- **Cubical Agda**: An experimental mode supporting *Homotopy Type Theory (HoTT)* and *univalence*, making it possible to formally reason about equivalences as equalities — a frontier in type theory research.

---

### 3. Lean 4

**Foundation**: Dependent types · Calculus of Inductive Constructions · Designed for both theorem proving and practical programming

#### Type System Summary

Lean 4 is a dependently typed language that uniquely straddles two worlds: it is both a serious theorem prover (hosting Mathlib, the largest unified mathematical library) and a practical functional programming language with a sophisticated compiler generating native code via C. This dual identity is its defining characteristic.

Lean 4's type system is based on the Calculus of Inductive Constructions (CIC), similar to Coq's foundation. Types and terms share a single syntactic category — `Nat` is a type, but `Nat → Type` is also a valid type (a family of types indexed by natural numbers):

```lean
-- Length-indexed vector
inductive Vec (α : Type) : Nat → Type where
  | nil  : Vec α 0
  | cons : α → Vec α n → Vec α (n + 1)

-- Safe indexing with Fin n (naturals less than n)
def Vec.get : Vec α n → Fin n → α
  | .cons x _,  ⟨0, _⟩      => x
  | .cons _ xs, ⟨n + 1, h⟩  => xs.get ⟨n, Nat.lt_of_succ_lt_succ h⟩
```

Lean 4 provides **two proof styles**. Term-mode proofs construct proof objects directly, while tactic-mode proofs use `by` blocks with interactive tactics:

```lean
-- Term-mode
theorem and_comm_term (p q : Prop) (h : p ∧ q) : q ∧ p :=
  ⟨h.2, h.1⟩

-- Tactic-mode
theorem and_comm_tactic (p q : Prop) (h : p ∧ q) : q ∧ p := by
  obtain ⟨hp, hq⟩ := h
  exact ⟨hq, hp⟩
```

A breakthrough feature is **metaprogramming in Lean itself**. Tactics, syntax extensions, and code transformations are written in Lean 4's own language, not in a separate metalanguage. This means users can create custom tactics and DSLs:

```lean
-- Custom tactic macro
syntax "mytrivial" : tactic
macro_rules
  | `(tactic| mytrivial) => `(tactic| exact trivial)

-- Usage
theorem demo : True := by mytrivial
```

Lean 4 compiles via C code generation with reference counting instead of garbage collection, producing efficient native executables. The compiler supports `@[inline]`, `@[specialize]`, and other pragmas for performance control.

#### What Problems Dependent Types Solve

- **Formalized mathematics at scale**: Mathlib exceeds 1.9 million lines, formalizing algebra, analysis, topology, number theory, and category theory. It has been used to verify major mathematical results including the Liquid Tensor Experiment and the Polynomial Freiman–Ruzsa conjecture.
- **Verified algorithms**: Sorting, searching, and data structure operations can be implemented with proofs of correctness that are checked by the type system.
- **Safe systems programming**: Lean 4's compiler is bootstrapped (Lean compiles itself), demonstrating that dependent types are viable for non-trivial software engineering.

#### The Cost

- **Compilation speed**: Large Lean developments (especially Mathlib) have significant build times. Incremental compilation helps, but initial builds can take hours.
- **Proof engineering burden**: Writing non-trivial proofs requires familiarity with Mathlib conventions, tactic libraries, and the underlying type theory.
- **Limited industrial ecosystem**: While growing, Lean's package ecosystem is young. Most libraries are mathematics-oriented.
- **Error messages**: Dependent type errors in Lean can be opaque, especially when unification fails deep in a term.

#### Key Innovations

- **Unified programming and proving**: Unlike Coq (which extracts to OCaml) or Agda (which lacks mature compilation), Lean 4 compiles dependently typed programs directly to efficient native code.
- **Metaprogramming in the host language**: Tactics are Lean programs, not a separate DSL. This enables unprecedented extensibility — users create new tactics as easily as new functions.
- **Mathlib's type class hierarchies**: Lean's elaborate type class resolution supports Mathlib's deep algebraic hierarchies (groups → rings → fields → …), enabling extreme code reuse.
- **Reference-counted memory management**: Instead of GC, Lean uses reference counting with optimizations (destructive updates when refcount = 1), making performance predictable.

---

### 4. Coq / Rocq

**Foundation**: Calculus of Inductive Constructions (CIC) · Proof assistant · Code extraction to OCaml/Haskell

#### Type System Summary

Coq (recently being rebranded as **Rocq**) is the elder statesman of dependently typed languages, with roots stretching back to 1984 and the Calculus of Constructions. Its type theory — the Calculus of Inductive Constructions — is the foundation that Lean's CIC derives from. Coq distinguishes between `Prop` (the universe of propositions, whose inhabitants are proofs) and `Set`/`Type` (the universe of computational data), enabling *proof irrelevance* and efficient extraction.

The core workflow in Coq: define inductive types, write functions over them, state theorems as types, and construct proofs interactively using tactics. Then *extract* the computational content to OCaml or Haskell, discarding proofs:

```coq
(* Inductive definition of sorted lists *)
Inductive sorted : list nat -> Prop :=
  | sorted_nil  : sorted []
  | sorted_one  : forall x, sorted [x]
  | sorted_cons : forall x y l,
      x <= y -> sorted (y :: l) -> sorted (x :: y :: l).

(* Insertion sort *)
Fixpoint insert (x : nat) (l : list nat) : list nat :=
  match l with
  | []     => [x]
  | h :: t => if x <=? h then x :: l else h :: (insert x t)
  end.

Fixpoint insertion_sort (l : list nat) : list nat :=
  match l with
  | []     => []
  | h :: t => insert h (insertion_sort t)
  end.

(* Theorem: insertion_sort produces sorted output *)
Theorem insertion_sort_sorted : forall l, sorted (insertion_sort l).
Proof.
  (* ... proof by induction and lemmas ... *)
Admitted.

(* Extract to OCaml *)
Require Extraction.
Extraction Language OCaml.
Extraction "sort.ml" insertion_sort.
```

The extraction mechanism is Coq's bridge to practical software. It translates Gallina (Coq's programming language) to OCaml or Haskell, erasing `Prop`-valued terms (proofs) while preserving `Set`/`Type`-valued terms (data and computation). This is how **CompCert** — a formally verified optimizing C compiler — was built: the compiler is proven correct in Coq and extracted to OCaml for execution.

Coq's tactic language (Ltac, now Ltac2) is powerful but forms a separate DSL. Tactics like `auto`, `omega`, `ring`, and `crush` provide significant automation, but the tactic language's semantics are complex and debugging failed tactics can be difficult.

Coq maintains logical consistency through a stratified universe hierarchy and strict positivity checks for inductive types. The termination checker ensures all recursive functions terminate, which is necessary for logical soundness.

#### What Problems Dependent Types Solve

- **Verified compilers**: CompCert is the gold standard — a C compiler proven correct in Coq, used in safety-critical avionics and automotive software.
- **Certified cryptography**: FSCQ (a verified file system) and various cryptographic protocol verifications.
- **Mathematical formalization**: The Four Color Theorem and the Feit-Thompson Odd Order Theorem have been formally verified in Coq.
- **Software Foundations curriculum**: The standard pedagogical path for learning formal verification, based entirely on Coq.

#### The Cost

- **Extraction semantic gap**: Extraction from Coq to OCaml/Haskell is not itself verified (though MetaCoq is working on this). Semantic mismatches can arise: Coq's `nat` has no size limit, but OCaml's `int` does; Coq guarantees termination but OCaml doesn't.
- **Two-language problem**: You write code in Gallina, prove it in Ltac, and run it in OCaml. Context switching between these layers is cognitively expensive.
- **Slow feedback loops**: Large Coq developments (like CompCert or Mathlib's predecessor MathComp) can take tens of minutes to typecheck.
- **Tactic opacity**: Ltac proofs are scripts that transform proof states. They're fragile — small changes to definitions can break long tactic scripts in non-obvious ways.

#### Key Innovations

- **The Prop/Set/Type distinction**: Separating propositions from data enables proof erasure during extraction — proofs contribute zero runtime overhead.
- **Extraction to industrial languages**: The ability to extract verified code to OCaml/Haskell makes Coq uniquely practical for producing *deployable* verified software.
- **Mature ecosystem**: 40+ years of development. CompCert, Software Foundations, MathComp, Iris (concurrent separation logic) — Coq has the richest ecosystem of any proof assistant.
- **Calculus of Inductive Constructions**: CIC itself is a key contribution to type theory, combining dependent types, inductive types, and universe polymorphism in a coherent framework that influenced Lean, Agda, and others.

---

### 5. F\* (F-star)

**Foundation**: Dependent types + effects · Refinement types · SMT-based proof automation · Extraction to OCaml/C/WASM

#### Type System Summary

F\* occupies a unique position: it combines *full dependent types* with *refinement types* and an *effect system*, then automates most proof obligations via an SMT solver (Z3). The result is a language where you write specifications as rich as Coq's but discharge most proofs automatically.

Refinement types in F\* constrain base types with logical predicates:

```fstar
(* A natural number *)
type nat = x:int{x >= 0}

(* Division that requires a non-zero divisor — checked at compile time *)
val safe_div : x:nat -> y:int{y <> 0} -> nat
let safe_div x y = x / y

(* A function whose postcondition states the result equals x + y *)
val add_positive : x:int{x > 0} -> y:int{y > 0} -> z:int{z = x + y}
let add_positive x y = x + y
```

F\*'s effect system tracks computational effects in types. Every expression has an effect: `Tot` (total/terminating), `Dv` (potentially divergent), `ST` (stateful), `ML` (full ML-like effects). Effects form a lattice, and the type system prevents total code from depending on divergent code:

```fstar
(* Total function — guaranteed to terminate *)
val factorial : x:nat -> Tot (r:nat{r >= 1})
let rec factorial x =
  if x = 0 then 1
  else x * factorial (x - 1)
```

The crown jewel of F\*'s practical application is **Project Everest** — a Microsoft Research initiative to build a verified, drop-in replacement for the HTTPS ecosystem. The project uses F\* to verify:

- **HACL\***: A high-assurance cryptographic library used in Firefox, the Windows kernel, and other production systems.
- **miTLS-F\***: A verified implementation of TLS 1.3.
- **EverParse**: Verified binary parsers for security-critical format parsing.

The key enabler is **Low\***, a subset of F\* that resembles C. Low\* code is verified in F\* and then extracted to C via the **KreMLin** compiler, producing efficient C code competitive with handwritten implementations. Proofs and specifications are erased during extraction.

```fstar
(* Low* code — verified in F*, extracted to C *)
module Buffer
open FStar.HyperStack.ST

val copy : #a:Type -> src:buffer a -> dst:buffer a ->
  len:UInt32.t{UInt32.v len <= length src /\ UInt32.v len <= length dst} ->
  Stack unit
    (requires fun h -> live h src /\ live h dst /\ disjoint src dst)
    (ensures  fun h0 _ h1 -> modifies (loc dst) h0 h1 /\
                             as_seq h1 dst == Seq.slice (as_seq h0 src) 0 (UInt32.v len))
```

#### What Problems Dependent Types Solve

- **Cryptographic correctness**: HACL\* implements Ed25519, Curve25519, AES-GCM, SHA-2, and more with formal proofs of functional correctness and side-channel resistance.
- **Protocol security**: miTLS-F\* verifies that TLS 1.3 handshakes follow the specification and that keys are handled correctly.
- **Memory safety in C**: Low\* code is proven memory-safe in F\* and extracted to C, catching buffer overflows, use-after-free, and null dereferences statically.
- **Parser security**: EverParse verifies that binary format parsers are correct and cannot be exploited with malformed input.

#### The Cost

- **SMT unpredictability**: Z3 is powerful but non-deterministic. Proof obligations that verified yesterday may fail today after a Z3 update or a seemingly unrelated code change. This makes builds fragile.
- **Effect system complexity**: The lattice of effects (`Tot`, `GTot`, `Lemma`, `Pure`, `Dv`, `ST`, `ML`, …) is powerful but creates a steep learning curve.
- **Small community**: F\* is primarily used within Microsoft Research and its collaborators. Community resources, tutorials, and libraries are limited compared to Coq or Lean.
- **Compilation pipeline**: The path from F\* → Low\* → KreMLin → C involves multiple translations, each with potential for semantic mismatch.

#### Key Innovations

- **SMT-based proof automation for dependent types**: F\* was the first language to successfully marry full dependent types with automatic SMT solving, dramatically reducing the proof burden for many common specifications.
- **Verified extraction to C**: Low\*/KreMLin enables writing verified C code in a high-level dependently typed language — a unique capability not offered by Coq (which extracts to OCaml) or Lean.
- **Effect-indexed types**: The combination of dependent types with a user-extensible effect system enables precise specification of computational behavior (termination, state, exceptions, I/O).
- **Project Everest**: The first verified, production-grade HTTPS stack demonstrates that dependent types can produce software that is simultaneously *correct* and *performant*.

---

### 6. Dafny

**Foundation**: Verification-oriented · Pre/post conditions · Ghost code · SMT-backed (Z3) · Compiles to C#/Java/Go/Python/JavaScript

#### Type System Summary

Dafny is not a dependently typed language in the traditional sense — it is a **verification-aware programming language** that uses automated reasoning to check programmer-written specifications. Where Idris or Agda encode invariants in types, Dafny encodes them in **contracts**: preconditions (`requires`), postconditions (`ensures`), loop invariants (`invariant`), and frame conditions (`modifies`/`reads`).

```dafny
method BinarySearch(a: array<int>, key: int) returns (index: int)
  requires forall i, j :: 0 <= i < j < a.Length ==> a[i] <= a[j]  // sorted
  ensures index >= 0 ==> index < a.Length && a[index] == key       // found
  ensures index < 0  ==> forall i :: 0 <= i < a.Length ==> a[i] != key  // not found
{
  var lo, hi := 0, a.Length;
  while lo < hi
    invariant 0 <= lo <= hi <= a.Length
    invariant forall i :: 0 <= i < lo ==> a[i] < key
    invariant forall i :: hi <= i < a.Length ==> a[i] > key
  {
    var mid := (lo + hi) / 2;
    if a[mid] < key      { lo := mid + 1; }
    else if a[mid] > key { hi := mid; }
    else                  { return mid; }
  }
  return -1;
}
```

Dafny's **subset types** approximate refinement types, allowing you to constrain existing types with predicates:

```dafny
type Nat = n: int | n >= 0
type NonEmptySeq<T> = s: seq<T> | |s| > 0

method SafeHead(s: NonEmptySeq<int>) returns (x: int)
  ensures x == s[0]
{
  x := s[0];
}
```

**Ghost code** is Dafny's mechanism for specification-only data. Ghost variables, ghost methods, and ghost functions exist only during verification and are erased from the compiled output:

```dafny
class Stack<T> {
  var elems: array<T>
  var size: int
  ghost var model: seq<T>  // specification-only: erased at compilation

  method Push(x: T)
    modifies this
    requires size < elems.Length
    ensures model == old(model) + [x]
    ensures size == old(size) + 1
  {
    elems[size] := x;
    size := size + 1;
    model := old(model) + [x];  // ghost update — verification only
  }
}
```

Dafny compiles to C#, Java, Go, Python, and JavaScript, making it accessible to mainstream developers. The verification runs at compile time via Z3, and the generated code contains no verification overhead.

#### What Problems Dependent Types Solve

- **Algorithm correctness**: Dafny is used in teaching and research to verify sorting algorithms, graph algorithms, and data structures with full pre/post conditions.
- **AWS infrastructure**: Amazon Web Services uses Dafny to verify critical infrastructure code, including cryptographic libraries and distributed systems components.
- **Protocol verification**: Network protocols and state machines can be specified and verified with ghost state tracking the abstract protocol.

#### The Cost

- **Annotation burden**: Loop invariants are the primary pain point. Every loop requires a hand-written invariant that the SMT solver can verify, and finding the right invariant is often harder than writing the algorithm.
- **SMT solver limitations**: Z3 can time out on complex verification conditions. Quantified formulas (∀, ∃) are particularly challenging.
- **Limited expressiveness**: Dafny is not Turing-complete at the specification level in the way Coq or Lean are. You cannot write arbitrary proofs as programs.
- **Ghost code overhead**: Maintaining ghost state in parallel with real state is tedious and error-prone — any mismatch between the ghost model and the real implementation is a verification failure.

#### Key Innovations

- **Verification for mainstream programmers**: Dafny's imperative syntax (while loops, mutable variables, classes) makes formal verification accessible to developers who don't know type theory or proof assistants.
- **Multi-language compilation**: Extracting verified code to C#, Java, Go, Python, and JavaScript is unique — no other verification tool targets this many mainstream languages.
- **Ghost code as first-class concept**: The `ghost` modifier cleanly separates specification from implementation, with compiler-enforced erasure.
- **Automated verification**: Z3 handles most proof obligations automatically, reducing the manual proof burden compared to tactic-based systems.

---

### 7. Liquid Haskell

**Foundation**: Refinement types layered on GHC Haskell · SMT-backed (Z3) · GHC plugin

#### Type System Summary

Liquid Haskell adds **refinement types** to standard Haskell via special annotations in comments, checked by an SMT solver at compile time. The key insight: you keep writing normal Haskell, but annotate types with logical predicates that constrain values. The SMT solver verifies these predicates automatically, catching bugs that Haskell's type system alone cannot.

```haskell
-- Define a refinement type: natural numbers
{-@ type Nat = {v:Int | v >= 0} @-}

-- Safe division: divisor must be non-zero
{-@ safeDiv :: x:Nat -> {y:Nat | y /= 0} -> Nat @-}
safeDiv :: Int -> Int -> Int
safeDiv x y = x `div` y

-- Calling safeDiv 10 0 is a COMPILE-TIME error
```

Refinement types compose with Haskell's existing type system. You can refine any type, including algebraic data types:

```haskell
-- A non-empty list
{-@ type NonEmpty a = {v:[a] | len v > 0} @-}

-- Safe head — only works on non-empty lists
{-@ safeHead :: NonEmpty a -> a @-}
safeHead :: [a] -> a
safeHead (x:_) = x

-- Sum of a list of naturals is also a natural
{-@ sumList :: [Nat] -> Nat @-}
sumList :: [Int] -> Int
sumList []     = 0
sumList (x:xs) = x + sumList xs
```

Liquid Haskell uses **measures** — functions lifted into the refinement logic — to reason about data structures:

```haskell
-- Measure: length of a list (available in refinements)
{-@ measure len :: [a] -> Int
    len []     = 0
    len (_:xs) = 1 + len xs
  @-}

-- Vector append preserves length
{-@ append :: xs:[a] -> ys:[a] -> {v:[a] | len v = len xs + len ys} @-}
append :: [a] -> [a] -> [a]
append []     ys = ys
append (x:xs) ys = x : append xs ys
```

**Reflection** allows lifting arbitrary Haskell function definitions into the refinement logic for theorem proving:

```haskell
{-@ reflect fib @-}
{-@ fib :: Nat -> Nat @-}
fib :: Int -> Int
fib 0 = 0
fib 1 = 1
fib n = fib (n-1) + fib (n-2)

-- Now you can prove properties about fib in refinement types
{-@ fibPositive :: n:Nat -> {fib n >= 0} @-}
fibPositive :: Int -> ()
fibPositive 0 = ()
fibPositive 1 = ()
fibPositive n = fibPositive (n-1) `seq` fibPositive (n-2)
```

#### What Problems Dependent Types Solve

- **Heartbleed-class bugs**: Liquid Haskell can verify bounds on buffer accesses, preventing the class of bug that caused Heartbleed (reading past buffer end).
- **Correctness of pure functions**: Properties like "sorting preserves length", "map preserves structure", "compiler binders are well-scoped" are checked automatically.
- **Gradual adoption**: Because refinements are in comments, you can add them incrementally to existing Haskell projects without changing the code itself.
- **Data structure invariants**: Red-black tree balance, BST ordering, and heap properties can be encoded as refinement types and checked at compile time.

#### The Cost

- **SMT solver limitations**: The refinement logic is restricted to what Z3 can handle. Non-linear arithmetic, higher-order reasoning, and inductively-defined properties push the solver's limits.
- **Slow checking**: SMT queries can be slow, especially for large modules. Liquid Haskell adds significant overhead to GHC's compilation time.
- **Fragile proofs**: Like F\*, proofs depend on Z3's heuristics. Upgrading Z3 or Liquid Haskell can break previously-passing code.
- **Limited reflection**: While `reflect` enables theorem proving, it's less expressive than Coq or Lean's proof languages. Complex proofs require awkward workarounds.
- **Annotation format**: Refinements live in special comments (`{-@ ... @-}`), which can feel ad-hoc and are not first-class syntax.

#### Key Innovations

- **Refinement types on an existing language**: Liquid Haskell is the most mature system for adding dependent-type-like guarantees to an *existing*, widely-used programming language without requiring a new language.
- **Liquid type inference**: The "liquid" in the name refers to *Liquid Types* — a technique for automatically inferring refinement types using abstract interpretation and predicate abstraction. Many refinements are inferred without annotation.
- **Measures as bridge**: Measures provide a principled way to lift inductive data structure properties into the refinement logic, enabling reasoning about recursive structures.
- **Gradual verification**: You can add refinements file-by-file, function-by-function. No all-or-nothing commitment. This is the key practical advantage over switching to Idris or Agda.

---

## The "Dependent Types Are Too Hard" Argument

### Why most mainstream languages avoid full dependent types

The gap between dependent types in research and dependent types in industry is not primarily a matter of engineering effort — it reflects fundamental theoretical trade-offs.

### 1. The Decidability Problem

In a dependently typed language, type equality may require deciding whether two *programs* produce the same result. If the language is Turing-complete, this is undecidable (by reduction from the halting problem).

**Mitigation strategies:**

| Language | Strategy | Consequence |
|----------|----------|-------------|
| Coq, Agda, Lean | Require termination | Not Turing-complete; must convince termination checker |
| Idris 2 | Default totality checking, opt-out with `partial` | Turing-complete programs possible but may not typecheck |
| F\*, Liquid Haskell | Offload to SMT solver | May time out; non-deterministic |
| Mainstream languages | Don't allow value-level types | No problem, but less expressiveness |

### 2. The Annotation Burden

Dependent types require the programmer to provide *proofs*, not just types. For every invariant you encode, you must provide evidence that it holds.

**Example**: A simple `append` for vectors in Agda requires a proof that `(n + m)` equals the result length. In Haskell, you just write `(++)` and trust it.

In practice, this means:
- **3-5× more code** for proofs compared to the implementation itself (observed in CompCert, Mathlib, and similar projects).
- **Proof maintenance**: When you change a function, all proofs that depend on it may break. This is the "proof rot" problem.

### 3. Type Inference Limitations

In Hindley-Milner type systems (Haskell, OCaml, Rust), type inference is decidable and complete — the compiler can always infer the most general type. With dependent types, inference is undecidable in general. Languages cope by:
- **Bidirectional type checking** (Idris, Lean): Infer where possible, require annotations where not.
- **Elaboration with holes** (Agda, Lean): Leave blanks (`_`) for the system to fill in.
- **SMT-backed inference** (Liquid Haskell): Automatically infer refinement predicates.

### 4. Error Message Quality

When types contain computations, type errors become *hard to understand*. Instead of "expected `Int`, got `String`", you get:

```
Expected: Vec Nat (plus (S n) m)
Got:      Vec Nat (S (plus n m))
```

The user must now understand that `plus (S n) m` reduces to `S (plus n m)` — i.e., they must debug a computation inside a type. In practice, dependent type errors routinely expose:
- Normalized terms that don't resemble source code
- Implicit arguments the user never wrote
- Unification variables the user doesn't know about

### 5. The Cultural Gap

Most software engineers have never encountered Martin-Löf type theory, the Curry-Howard correspondence, or constructive logic. Teaching dependent types requires teaching:
- Inductive data types and structural recursion
- Propositions as types, proofs as programs
- Universe hierarchies and logical consistency
- Totality and termination checking

This is a semester of graduate coursework, not a weekend tutorial.

---

## Practical Compromises

### Refinement Types (Liquid Haskell, F\*)

Types annotated with predicates, automatically checked by SMT:

```haskell
-- Liquid Haskell
{-@ type Pos = {v:Int | v > 0} @-}
{-@ abs :: Int -> Pos @-}
```

**Trade-off**: Limited expressiveness (predicates must be in a decidable fragment), but near-zero proof burden. Best for: arithmetic bounds, non-nullity, simple ordering properties.

### Indexed Types (Rust const generics, TypeScript template literals)

Limited value-level parameters in types:

```rust
// Rust: const generic parameter
fn dot_product<const N: usize>(a: [f64; N], b: [f64; N]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}
```

```typescript
// TypeScript: template literal types
type EventName<T extends string> = `on${Capitalize<T>}`;
type ClickEvent = EventName<"click">;  // "onClick"
```

**Trade-off**: Very limited computation at the type level, but integrates smoothly with existing type inference and error reporting. Best for: fixed-size arrays, string pattern types, numeric dimensions.

### GADTs (Haskell, OCaml)

Constructors with refined return types:

```haskell
-- Haskell GADT: well-typed expression language
data Expr a where
  LitInt  :: Int  -> Expr Int
  LitBool :: Bool -> Expr Bool
  Add     :: Expr Int -> Expr Int -> Expr Int
  If      :: Expr Bool -> Expr a -> Expr a -> Expr a

-- Type-safe evaluator — cannot go wrong
eval :: Expr a -> a
eval (LitInt n)    = n
eval (LitBool b)   = b
eval (Add x y)     = eval x + eval y
eval (If c t e)    = if eval c then eval t else eval e
```

**Trade-off**: More expressive than plain ADTs, but requires explicit type annotations on pattern matches. Cannot express arbitrary value-level dependencies. Best for: type-safe interpreters, well-typed ASTs, tagless final embeddings.

### Singleton Types (Haskell `singletons` library)

Encode values at the type level without full dependent types:

```haskell
-- Using singletons library
data Nat = Z | S Nat

data SNat :: Nat -> Type where
  SZ :: SNat Z
  SS :: SNat n -> SNat (S n)

-- Replicate: create a vector of length n
replicate :: SNat n -> a -> Vec n a
replicate SZ     _ = VNil
replicate (SS n) x = VCons x (replicate n x)
```

**Trade-off**: Significant boilerplate (every type needs a singleton mirror), limited to discrete data. But works in standard Haskell with GHC extensions. Best for: length-indexed vectors, size-parameterized data structures, when you can't switch to Idris/Agda.

---

## The "Sweet Spot" Question

### Where do practical languages land?

The right level of dependent typing depends on what you're trying to prevent:

| Problem Domain | Minimum Power Needed | Recommended Approach |
|---------------|---------------------|---------------------|
| **Bounded collections** (max length, non-empty) | Refinement types | Liquid Haskell, F\*, or Dafny subset types |
| **Protocol state machines** (connect-before-query) | Indexed types / GADTs | Idris 2 indexed families, Haskell GADTs, or session types |
| **Database schema types** (column existence, type matching) | Refinement + code generation | TypeScript + Prisma, or F\* refinement types |
| **Capability verification** (authorization, access control) | Full dependent types or indexed monads | Idris 2 resource protocols, or F\* with effects |
| **Cryptographic correctness** | Full dependent types + SMT | F\* (Project Everest), or Coq + extraction |
| **Mathematical proofs** | Full dependent types + tactics | Lean 4 + Mathlib, or Coq |
| **Compiler correctness** | Full dependent types + extraction | Coq (CompCert model) |

### The 80/20 Rule for Dependent Types

For most production software, **refinement types** capture 80% of the value of dependent types with 20% of the complexity:

- Non-null guarantees ✓
- Bounds checking ✓
- Simple pre/post conditions ✓
- Monotonic counters, positive integers ✓

The remaining 20% of value (protocol correctness, full functional specifications, mathematical proofs) requires the full 80% of complexity (proof engineering, termination checking, universe management).

### Emerging Middle Ground

Several trends are pushing mainstream languages toward limited dependent typing:

- **Rust's const generics**: `[T; N]` where `N` is a compile-time constant. Limited but growing.
- **TypeScript's template literal types**: Compute string types from string values.
- **Swift's `some` and opaque types**: Not dependent, but moving toward richer type-level abstraction.
- **Kotlin's contracts**: Declarative pre/post conditions, not yet checked by SMT.
- **Python's `typing` module + mypy**: Runtime values influencing types via `Literal`, `TypeGuard`.

The trend is clear: mainstream languages are incrementally absorbing ideas from dependent type theory, one restricted feature at a time, avoiding the "all or nothing" leap to full dependent types.

---

## Cross-Language Comparison Table

| Feature | Idris 2 | Agda | Lean 4 | Coq/Rocq | F\* | Dafny | Liquid Haskell |
|---------|---------|------|--------|----------|-----|-------|---------------|
| **Dependent type depth** | Full | Full | Full | Full | Full + refinement | Refinement / contracts | Refinement |
| **Primary purpose** | Programming | Proof assistant | Proving + programming | Proof assistant | Verified systems code | Verified algorithms | Haskell hardening |
| **Proof style** | Term + holes | Term | Term + tactics | Tactics (Ltac) | SMT + tactics | SMT (automatic) | SMT (automatic) |
| **Automation level** | Low | Low | Medium (Mathlib tactics) | Medium (Ltac2, `auto`) | High (Z3) | High (Z3) | High (Z3) |
| **Totality** | Default, opt-out | Required | Required | Required | Tracked via effects | Not required | Not required |
| **Extraction / compilation** | Chez Scheme, C, JS | Haskell (experimental) | Native via C | OCaml, Haskell | OCaml, C (via Low\*) | C#, Java, Go, Python, JS | GHC (standard Haskell) |
| **Linearity / effects** | QTT (linear + erased) | No | No | No | Effect system | No | No |
| **IDE support** | VS Code, Emacs | Emacs (agda-mode) | VS Code (excellent) | VS Code, Emacs | VS Code, Emacs | VS Code | GHC plugin |
| **Community size** | Small | Small | Growing fast | Large | Small | Medium | Small |
| **Learning curve** | High | Very high | High | High | High | Medium | Medium |
| **Key project** | Type-Driven Dev book | Universal Algebra Lib | Mathlib (1.9M LOC) | CompCert, Software Foundations | Project Everest (HACL\*, miTLS) | AWS verification | UCSD research |
| **Unique innovation** | QTT / erasure | Cubical types, mixfix | Meta in host language | Prop/Set extraction | SMT + dependent types | Multi-target verification | Refinements on existing language |

---

## Further Reading

### Books
- *Type-Driven Development with Idris* — Edwin Brady (Manning, 2017). The definitive introduction to practical dependent types.
- *Software Foundations* — Benjamin Pierce et al. (free online). The standard Coq curriculum.
- *Theorem Proving in Lean 4* — Jeremy Avigad et al. (free online). Official Lean 4 textbook.
- *Programming with Refinement Types* — Ranjit Jhala et al. (free online). Liquid Haskell tutorial.
- *Proof-Oriented Programming in F\** — (free online at fstar-lang.org). Official F\* textbook.

### Papers
- *Idris 2: Quantitative Type Theory in Practice* — Brady, 2021. The QTT paper.
- *Verified Low-Level Programming Embedded in F\** — Protzenko et al., 2017. The Low\*/KreMLin paper.
- *Integrating Refinement and Dependent Types* — Tweag fellowship report, 2021. Connecting the two worlds.
- *Dependently Typed Programming with Singletons* — Eisenberg & Weirich, 2012. GADTs as poor man's dependent types.
- *All Your Base are Belong to Us: Sort Polymorphism for Proof Assistants* — 2023. Universe polymorphism across Agda/Coq/Lean.

### Communities
- **Lean**: leanprover.zulipchat.com (very active)
- **Agda**: agda/agda on GitHub, agda mailing list
- **Coq/Rocq**: coq.discourse.group, rocq-prover.org
- **Idris**: idris-lang.org, Discord server
- **F\***: fstar-lang.org, GitHub discussions
- **Dafny**: github.com/dafny-lang/dafny, Gitter
- **Liquid Haskell**: github.com/ucsd-progsys/liquidhaskell
