# Module System Design: Cross-Language Research

> Comparative analysis of module systems across 10 programming languages, focusing on **why** each language made its design choices and what trade-offs they imply.

---

## Table of Contents

1. [Rust](#1-rust)
2. [OCaml / SML](#2-ocaml--sml)
3. [Roc](#3-roc)
4. [Unison](#4-unison)
5. [Elm](#5-elm)
6. [Koka](#6-koka)
7. [Go](#7-go)
8. [Haskell](#8-haskell)
9. [Zig](#9-zig)
10. [Idris 2](#10-idris-2)
11. [Comparison Matrix](#comparison-matrix)
12. [Cross-Cutting Observations](#cross-cutting-observations)

---

## 1. Rust

### Design Summary

Rust organizes code into a three-tier hierarchy: **packages** (described by `Cargo.toml`), **crates** (compilation units — a binary or library), and **modules** (namespaces within a crate). Every item is **private by default**; visibility must be explicitly opted into with graduated keywords: `pub` (visible to any downstream code), `pub(crate)` (visible within the current crate only), `pub(super)` (visible to the parent module), and `pub(in path)` (visible to a specific ancestor module).

Files map to modules through a compiler-enforced convention: `mod foo;` in a parent module tells the compiler to look for `foo.rs` or `foo/mod.rs`. This creates an explicit module tree that mirrors the filesystem. The `use` keyword brings paths into scope; `mod` declares that a module exists.

Rust enforces the **orphan rule** for trait coherence: you may only implement `Trait for Type` if either the trait or the type is local to your crate. This prevents conflicting trait implementations across the ecosystem and is key to Rust's ability to reason about generic code at scale.

```rust
// src/lib.rs
mod network;          // looks for src/network.rs or src/network/mod.rs
pub mod api;          // public module

// src/network.rs
pub(crate) struct Connection { /* ... */ }  // visible within crate only
pub fn connect() -> Connection { /* ... */ }

// src/api.rs
use crate::network::Connection;  // absolute path from crate root
```

### Key Design Rationale

- **Explicitness over convention**: Every visibility boundary is stated in source code, making API surfaces auditable and refactoring safe.
- **Crate = compilation unit = coherence boundary**: The crate is the atom of compilation, dependency, and trait coherence. This triple duty is what makes the orphan rule possible and necessary.
- **File = module is enforced by the compiler**, not just convention. This removes ambiguity but also means reorganizing files requires updating `mod` declarations.

### Strengths

- Fine-grained visibility spectrum (`pub`, `pub(crate)`, `pub(super)`, `pub(in path)`) is unmatched in expressiveness
- Orphan rule guarantees global trait coherence — no "diamond problem" for trait impls
- Module tree is explicit and compiler-verified
- Scales well from single-file scripts to multi-crate workspaces
- `pub(crate)` elegantly separates internal APIs from public APIs within a library

### Weaknesses / Known Pain Points

- **Steep learning curve**: The module system is the #2 most-cited pain point after ownership. "The Rust module system is too confusing" (without.boats, 2017) remains a well-known blog post
- **File-system coupling**: Renaming or moving files requires updating `mod` declarations; this is more friction than file-is-module languages
- **Crate vs. module vs. package terminology** is confusing — all three are distinct concepts but often conflated in documentation
- **No circular crate dependencies**: Module-level cycles within a crate are fine, but crate-level cycles are forbidden. This can force awkward monolithic crate designs or excessive `pub(crate)` usage
- **Orphan rule frustration**: When you need a trait impl for two foreign types, the orphan rule forces newtype wrappers, which adds boilerplate

### Unique Insights for Spore

- The `pub(in path)` visibility is a novel middle ground — not just "public" or "private" but "visible to this subtree." Few other languages offer this.
- Separating `mod` (declaration) from `use` (import) is powerful but adds cognitive load. A system that unifies these could reduce confusion.
- The orphan rule is a coherence mechanism that only matters when you have open type-class-like dispatch. If Spore uses capabilities or effects instead, it may not need orphan rules.

---

## 2. OCaml / SML

### Design Summary

The ML module system is the most theoretically powerful module system in any mainstream-adjacent language. It has three layers: **Signatures** (module types that describe the interface a module must provide), **Structures** (concrete module implementations), and **Functors** (functions from modules to modules, i.e., parameterized modules).

In OCaml, a source file `foo.ml` automatically defines a structure `Foo`, and an optional `foo.mli` file defines its signature. SML requires explicit `structure` and `signature` declarations. Both languages use signature ascription to hide implementation details — you can seal a structure against a signature, making its internal types abstract.

OCaml adds **first-class modules**: modules can be packed into values, stored in data structures, and unpacked at runtime. This bridges the historically strict separation between the "module language" and the "core language." SML does not support first-class modules, keeping its module system entirely static.

```ocaml
(* Signature *)
module type STACK = sig
  type 'a t
  val empty : 'a t
  val push : 'a -> 'a t -> 'a t
  val pop : 'a t -> ('a * 'a t) option
end

(* Structure implementing the signature *)
module ListStack : STACK = struct
  type 'a t = 'a list
  let empty = []
  let push x s = x :: s
  let pop = function [] -> None | x :: xs -> Some (x, xs)
end

(* Functor: parameterized module *)
module MakeSet (Ord : Set.OrderedType) = Set.Make(Ord)

(* First-class module (OCaml only) *)
let pick_stack (use_list : bool) : (module STACK) =
  if use_list then (module ListStack) else (module ArrayStack)
```

### Key Design Rationale

- **Signatures as contracts**: ML's type theory treats module interfaces as types over modules. This is not just syntactic sugar — signatures enable separate compilation, abstract data types, and modular type-checking.
- **Functors solve the expression problem for modules**: You can write a `Set` functor once and instantiate it with any `OrderedType`, getting a type-safe Set implementation without runtime dispatch.
- **First-class modules (OCaml)** were added to close the gap between static modular abstraction and runtime flexibility (plugins, dynamic dispatch, configuration).

### Strengths

- Most expressive module system in practical use — functors enable code reuse that generics alone cannot
- Signature ascription provides true information hiding at the module level (abstract types)
- Separate `.mli` files serve as both documentation and API contract
- First-class modules (OCaml) enable runtime module selection — useful for plugin architectures
- Strong theoretical foundations (System F-omega modules)

### Weaknesses / Known Pain Points

- **Complexity**: Functors, applicative vs. generative semantics, and module type constraints are hard to learn
- **Boilerplate**: Signatures must be written separately from implementations. Keeping `.mli` and `.ml` in sync is a maintenance burden
- **No recursive modules by default** (OCaml has limited support with `module rec`, but it's restricted)
- **Functors are static** in SML — no runtime module selection without OCaml's first-class modules
- **Module and core languages are separate type systems** — you can't naturally pass a module to a function or pattern-match on module membership without first-class modules

### Unique Insights for Spore

- Functors are the gold standard for parameterized modules. If Spore wants to support generic "plug-in" implementations (e.g., different effect handlers), functors or something functor-like may be valuable.
- The `.mli` pattern (separate interface file) has proven effective for API documentation and separate compilation, but the sync cost is real. An alternative: derive the interface from the implementation with explicit `expose` annotations.
- First-class modules show the value of treating modules as values — especially for effect handler selection at runtime.

---

## 3. Roc

### Design Summary

Roc has a distinctive three-level structure: **Platforms**, **Packages**, and **Modules**. An **application** is always built on top of a **platform**, which owns all IO primitives. The standard library contains only pure data structures — no IO, no file access, no networking. All effectful operations are provided exclusively by the platform.

**Modules** expose items explicitly via a header: `module [functionA, functionB]`. Only what is listed in this header is accessible to importers. **Packages** are collections of modules distributed as tarballs referenced by URL and content hash. **Platforms** are a special kind of package that provide the runtime and all IO capabilities.

Importing uses `import Module.Name exposing [item]` syntax, with support for aliasing. The platform is declared in the app's header, making the IO boundary visible at the top of every application file.

```roc
# Dir/Hello.roc — a module
module [hello]

hello : Str -> Str
hello = |name| "Hello ${name}!"

# main.roc — an application
app [main!] {
    cli: platform "https://github.com/roc-lang/basic-cli/releases/download/0.20.0/hash.tar.br",
}

import cli.Stdout
import Dir.Hello exposing [hello]

main! = |_args|
    Stdout.line!(hello("World"))
```

### Key Design Rationale

- **Capability-based security**: Platforms own all IO. Packages cannot perform IO unless the platform explicitly provides it. This is a direct application of the principle of least privilege — downloaded third-party code cannot "smuggle" in filesystem access, network calls, or environment variable reads.
- **Supply-chain hardening**: Because packages are pure by default, auditing a Roc dependency is dramatically simpler than in languages where any import can do arbitrary IO.
- **Domain-specific IO surfaces**: A CLI platform exposes stdout/stdin; a web-server platform exposes request/response. You never accidentally use filesystem APIs in a browser context because the browser platform doesn't provide them.

### Strengths

- Strongest supply-chain security model of any language in this survey
- Packages are pure by default — dramatically reduces attack surface for dependencies
- Platform abstraction enables domain-specific IO surfaces without global ambient authority
- Content-hashed package URLs enable reproducible builds
- Module export lists are explicit and concise

### Weaknesses / Known Pain Points

- **Ecosystem is young** — few platforms exist, and creating a new platform requires writing host code in Rust or C
- **Platform lock-in**: Switching platforms can require rewriting IO-interacting code
- **No circular module dependencies** allowed
- **Platform authoring is complex** — the boundary between Roc and the host language is non-trivial
- **Effect abstraction is at the platform level, not the type level** — you can't abstractly reason about "this function does IO" in the type signature the way you can in Haskell or Koka

### Unique Insights for Spore

- The platform/package separation is the most practical implementation of capability-based security in a real language. Spore could adopt a similar model: effects-as-capabilities provided by a "runtime platform" rather than globally available.
- Content-hashed package URLs solve supply-chain integrity at the import level. This is complementary to Unison's content-addressing of individual definitions.
- The "no IO in the standard library" decision is radical but eliminates an entire class of supply-chain attacks.

---

## 4. Unison

### Design Summary

Unison is the most radical departure from file-based code organization. Every definition (function, type, value) is identified by a **cryptographic hash (SHA3-512)** of its fully-resolved abstract syntax tree. Human-readable names are metadata stored separately — they are "pointers" to hashes, analogous to Git branch names pointing to commits. Code is stored in a database (SQLite by default), not in text files on the filesystem.

**Namespaces** provide hierarchical organization (like `base.List.map`), but these are purely metadata — renaming a namespace doesn't change any code. Modules and libraries are collections of definitions referenced within namespaces. The Unison Codebase Manager (UCM) is the interface for editing, type-checking, and publishing code.

There are no build steps: since every definition is type-checked once and stored by hash, the system achieves perfect incremental compilation. Dependency management is trivial — different versions of the same function have different hashes and can coexist without conflict.

```
-- In the Unison scratch file (not a persistent file):
myList.map : (a -> b) -> [a] -> [b]
myList.map f = cases
  [] -> []
  h +: t -> f h +: myList.map f t

-- After running `add` in UCM, this definition is stored by its hash.
-- The name `myList.map` is just metadata pointing to hash #a8f3k2...
-- Renaming to `List.transform` is instant and non-breaking.
```

### Key Design Rationale

- **Eliminate merge conflicts**: Since identity is content-hash, not text position, formatting changes, import reordering, and variable renaming don't create conflicts. Only genuine semantic conflicts matter.
- **Solve dependency hell permanently**: Two versions of a function can coexist because they have different hashes. No version resolution, no diamond dependencies.
- **Enable distributed computation**: Because code is immutable and portable by hash, a computation can reference a function by hash and send it across the network with full confidence it's the same code.
- **Perfect incremental compilation**: Changed one function? Only that function's hash changes and gets re-typechecked. Everything else is cached forever.

### Strengths

- No merge conflicts from formatting, whitespace, or import changes
- Multiple library versions can coexist trivially — no "dependency hell"
- Renaming is instant and guaranteed non-breaking (only metadata changes)
- Perfect incremental compilation — never recompile unchanged code
- Distributed programming is built into the design, not bolted on
- Content-addressing makes code integrity trivially verifiable

### Weaknesses / Known Pain Points

- **Abandons all existing tooling**: No `grep` on files, no `git diff` on source, no editor integration without custom Unison-aware tools
- **Steep learning curve**: Developers must learn an entirely new workflow (UCM commands instead of file editing)
- **Namespace management is unfamiliar**: Without files as anchors, organizing large codebases requires new mental models
- **Ecosystem is tiny** — the language is still niche
- **Debugging and code review** require Unison-specific tools — standard `diff` and `blame` workflows don't apply
- **Loss of "files as units of change"** means pull requests, code review, and CI/CD need rethinking

### Unique Insights for Spore

- Content-addressing of definitions is the most compelling solution to dependency versioning ever proposed. Even if Spore doesn't go full Unison, **content-hashing module interfaces** (not just packages) could provide similar benefits.
- The separation of names from identity is powerful. If Spore's module system treats names as metadata over some underlying identity (even just within a crate), it enables fearless renaming.
- The trade-off is tooling: Unison sacrificed the entire file-based ecosystem. A middle ground — content-addressed modules that still live in files — could capture 80% of the benefit.

---

## 5. Elm

### Design Summary

Elm enforces a strict **one file = one module** rule. Each file begins with a `module` declaration that must match the file path, and an `exposing` clause that explicitly lists every function, type, and constructor that is public. Everything not in the `exposing` list is private. Circular dependencies between modules are forbidden at the compiler level.

Imports are explicit: you can import a whole module (qualified), import specific items, or use `exposing (..)` to import everything (discouraged in practice). There is no wildcard re-export mechanism and no way to hide imports from downstream modules — the system is deliberately simple.

```elm
-- src/Utils/Math.elm
module Utils.Math exposing (add, multiply)

add : Int -> Int -> Int
add x y = x + y

multiply : Int -> Int -> Int
multiply x y = x * y

-- Private helper, not in exposing list
halve : Int -> Int
halve x = x // 2

-- src/Main.elm
import Utils.Math exposing (add)
import Utils.Math as Math      -- qualified access: Math.multiply
```

### Key Design Rationale

- **Simplicity above all**: Elm targets front-end developers who may not have systems programming backgrounds. The module system should be learnable in minutes.
- **No circular dependencies forces clean architecture**: If two modules depend on each other, you must extract shared code into a third module. This naturally produces well-layered codebases.
- **Explicit exposing prevents API creep**: You must consciously decide to export each item. This encourages thoughtful API design and reduces breaking changes.
- **File = module = discoverability**: Given a qualified name, you can always find the source file. No indirection, no configuration.

### Strengths

- Extremely simple to learn and reason about
- Explicit `exposing` makes module interfaces self-documenting
- No circular dependencies guarantees a clean DAG — predictable compilation order
- Strong community norms around module organization
- Error messages are excellent — the compiler explains module issues clearly

### Weaknesses / Known Pain Points

- **No parameterized modules or functors** — code reuse across module boundaries is limited to functions and types
- **Rigid one-file-one-module** can lead to very large files when a module has many related functions
- **No access control beyond public/private** — no equivalent of `pub(crate)` or `internal`
- **Can't re-export from submodules** — every public item must originate in its own module's `exposing` list
- **The "extract to a third module" solution for circular deps** can create many small, semantically thin modules

### Unique Insights for Spore

- Elm proves that enforcing no circular dependencies is livable and even beneficial, despite the occasional friction of extracting shared types.
- The explicit `exposing` list at the module level is the simplest visibility mechanism that works well in practice.
- Elm's commitment to "one obvious way to do things" in the module system reduces cognitive load dramatically. Spore should consider whether graduated visibility (like Rust) is worth the complexity.

---

## 6. Koka

### Design Summary

Koka's module system is minimal and designed to work in concert with its **row-polymorphic effect system**. Each source file is a module. Items are **private by default** and made visible with the `pub` keyword. Imports use `import module.path` syntax and can selectively import specific items with `import module.{item1, item2}`.

The distinctive feature is that **effects are first-class exportable items**. When you define an effect, it becomes part of the module's public interface (if marked `pub`), and any function that uses that effect carries it in its type signature. This means the module system and the effect system are tightly coupled: importing a module with public effects means those effects become available in the importing module's type context.

```koka
// file: io/console.kk
pub effect console
  fun println(msg: string): ()
  fun readln(): string

pub fun greet(): console ()
  val name = readln()
  println("Hello, " ++ name)

// file: main.kk
import io/console

fun main(): <console, div> ()
  greet()
  // The `console` effect appears in main's type because greet uses it
```

### Key Design Rationale

- **Effects as module-level contracts**: By making effects exportable and tracking them in types, Koka ensures that a module's side-effect surface is part of its API contract. You can see at a glance what effects a module introduces.
- **Row polymorphism avoids effect subtyping**: Functions can be polymorphic over effects without needing a complex subtyping hierarchy. This keeps the module interface clean.
- **Private by default** follows the principle of least authority — you must explicitly choose to export.
- **Simplicity in the module system, complexity in the effect system**: Koka keeps modules minimal because the effect system carries the heavy semantic load.

### Strengths

- Effects are part of the module interface — unmatched transparency about side effects
- Row-polymorphic effects compose naturally across module boundaries
- Private-by-default with `pub` opt-in is simple and secure
- Minimal module syntax — little boilerplate
- Effect handlers can be provided by different modules, enabling a form of effect-level dependency injection

### Weaknesses / Known Pain Points

- **Small ecosystem** — Koka is a research language with limited production use
- **Effect types can become verbose** in function signatures when many effects are in play
- **Limited tooling** compared to mainstream languages
- **Module system is underdocumented** — most Koka documentation focuses on effects, not module organization
- **No parameterized modules** — you can't functor-ize over effects at the module level (though row polymorphism partially compensates)

### Unique Insights for Spore

- Koka demonstrates that effects and modules should be designed together, not independently. If Spore has an effect system, module exports should include effect declarations as first-class citizens.
- Row-polymorphic effects crossing module boundaries is the cleanest solution to the "what effects does this dependency bring?" question.
- The pattern of "simple modules + rich effect types" is a viable alternative to "rich modules + simple types" (the OCaml approach).

---

## 7. Go

### Design Summary

Go's package system is designed for maximal simplicity. A **package** consists of all `.go` files in a single directory, sharing the same `package` declaration. Visibility is controlled by a single rule: **identifiers starting with an uppercase letter are exported; lowercase are unexported.** There are no `public`, `private`, or `protected` keywords.

Import paths are full, URL-like paths (e.g., `github.com/user/repo/pkg`). Since Go modules (introduced in Go 1.11), the `go.mod` file defines the module root and all dependencies. The `internal/` directory convention provides an additional layer of encapsulation: packages under `internal/` can only be imported by code in the parent directory tree.

Circular dependencies between packages are forbidden. Relative imports are disallowed. The build system processes packages in topological order.

```go
// pkg/math/math.go
package math

// Exported — starts with uppercase
func Add(x, y int) int { return x + y }

// Unexported — starts with lowercase
func helper(x int) int { return x * 2 }

// main.go
package main

import (
    "fmt"
    "github.com/myuser/myproject/pkg/math"
)

func main() {
    fmt.Println(math.Add(2, 3))
    // math.helper(5) — compile error: unexported
}
```

### Key Design Rationale

- **Capitalization as visibility** eliminates an entire keyword category and makes exported items visually distinct at a glance. The design was chosen to reduce syntactic noise and make code scannable.
- **URL-based import paths** make dependency provenance explicit — you know exactly where code comes from.
- **No circular dependencies** enables fast, parallel compilation in topological order.
- **One directory = one package** prevents the ambiguity of multiple packages competing for the same namespace.
- **"internal" directory convention** was added later to provide sub-package encapsulation without a new language feature.

### Strengths

- Extremely simple — the entire module system can be learned in 30 minutes
- Capitalization rule makes export status immediately visible without looking at declarations
- Fast compilation due to acyclic dependency graph
- URL import paths make dependencies self-documenting
- `internal/` provides pragmatic encapsulation without language complexity
- Excellent tooling: `go mod`, `go vet`, `goimports` handle most module tasks automatically

### Weaknesses / Known Pain Points

- **Only two visibility levels**: exported or unexported. No equivalent to `pub(crate)`, `internal` (C#), or friend classes
- **Circular dependency ban causes artificial splitting**: Two tightly-coupled concepts must be split into a shared third package, often creating thin "model" or "types" packages
- **Capitalization rule interacts poorly with acronyms** (e.g., `ID` vs `Id`, `URL` vs `Url` — Go chose `ID` and `URL` but this is inconsistent with the "first letter" rule)
- **No parameterized modules or generics at the module level** (generics were added in Go 1.18, but only at the function/type level)
- **Package naming pressure**: Short, lowercase, no-underscore names lead to naming conflicts in large projects
- **No module-level export lists** — you can't see a package's API without scanning all files

### Unique Insights for Spore

- Go proves that extreme simplicity in the module system is viable for large-scale software (Google-scale). The trade-off is coarseness.
- Capitalization-as-visibility is clever but language-specific. The insight is: **make visibility instantly visible at the use-site, not just the definition site.**
- The `internal/` directory is an admission that two visibility levels aren't enough. Spore should plan for at least three levels from the start.

---

## 8. Haskell

### Design Summary

Haskell modules use explicit **export lists** to control visibility. A module declaration like `module Foo (bar, Baz(..)) where` exports only `bar` and type `Baz` with all its constructors. If the export list is omitted, everything is exported. Imports can be **qualified** (`import qualified Data.Map as Map`), **selective** (`import Data.Map (lookup, insert)`), or **hiding** (`import Prelude hiding (map)`).

The most distinctive (and controversial) aspect is **typeclass instances**: they are **always exported** and cannot be hidden or selectively imported. If module A defines `instance Show MyType`, any module that transitively imports A will see that instance. This creates the **orphan instance problem**: an instance defined in a module that owns neither the class nor the type can cause coherence issues.

```haskell
module MyLib
  ( Config(..)       -- export type with all constructors
  , defaultConfig    -- export specific function
  , runApp           -- export specific function
  -- parseInternal is NOT exported
  ) where

import qualified Data.Map as Map
import Data.Text (Text, pack, unpack)

data Config = Config { port :: Int, host :: Text }

defaultConfig :: Config
defaultConfig = Config 8080 (pack "localhost")

runApp :: Config -> IO ()
runApp cfg = putStrLn $ "Running on " ++ unpack (host cfg)

parseInternal :: Text -> Maybe Config
parseInternal = undefined  -- private helper
```

### Key Design Rationale

- **Export lists as API surface**: Explicit export lists are the primary encapsulation mechanism. The philosophy is: "if you didn't list it, it doesn't exist to the outside world."
- **Typeclass instances must be global** because Haskell's type inference depends on there being exactly one instance of `Class Type` in the entire program. If instances could be hidden, type inference would be incoherent — the same expression could have different types depending on which instances are in scope.
- **Qualified imports** solve name collision elegantly. `Map.lookup` and `Set.lookup` can coexist without ambiguity.
- **No circular module dependencies** in standard Haskell (GHC has limited support via `.hs-boot` files, but it's fragile and discouraged).

### Strengths

- Export lists are clear and well-understood
- Qualified imports are the best name-disambiguation mechanism of any language in this survey
- Module system is simple enough to learn quickly, powerful enough for large codebases
- Haddock documentation integrates naturally with export lists
- Re-export mechanism (`module Data.Map`) enables clean public API facades

### Weaknesses / Known Pain Points

- **Orphan instances are the #1 module-system pain point**: GHC warns about them, the community discourages them, but they're sometimes unavoidable when integrating libraries. The "newtype wrapper" workaround adds significant boilerplate
- **No instance import/export control**: You cannot say "import this module but not its instances." This is a fundamental limitation
- **No fine-grained visibility**: No equivalent of `pub(crate)` — items are either exported or not. There's no "visible to this package but not to downstream"
- **Module = file is rigid**: One module per file, one file per module. Large modules become unwieldy
- **Circular modules require `.hs-boot` files**: These are fragile, poorly documented, and a maintenance burden
- **No parameterized modules**: Haskell relies on typeclasses and type families instead of functors. This works but is less structured

### Unique Insights for Spore

- The orphan instance problem is Haskell's strongest cautionary tale. If Spore has typeclass-like dispatch, it must either (a) enforce orphan rules like Rust, (b) make instances local/explicit, or (c) use a different dispatch mechanism (effects, explicit dictionaries).
- Qualified imports are worth stealing. `import X as Y` with `Y.function` is more readable than `use X::function`.
- The inability to control instance scope is a design flaw Spore should avoid. If instances (or effect handlers) exist, they should be explicitly importable.

---

## 9. Zig

### Design Summary

Zig takes the "file = module" concept to its logical extreme: **every `.zig` file is implicitly a struct**, and `@import("file.zig")` returns that struct as a value. There is no separate module declaration syntax — the file's top-level declarations (functions, types, constants) are the struct's fields. The `pub` keyword controls which items are accessible from outside.

Everything is **private by default**. The build system (`build.zig`) explicitly declares which files are modules and how they relate. Imports are done with `@import`, which is a **comptime (compile-time) function** — it's evaluated during compilation, and the result is a compile-time-known struct value. There are no hidden control flows: no macros, no implicit constructors, no automatic imports.

```zig
// math_utils.zig
pub fn add(a: i32, b: i32) i32 {
    return a + b;
}

fn helper(x: i32) i32 {  // private — no pub
    return x * 2;
}

pub const PI: f64 = 3.14159265;

// main.zig
const math = @import("math_utils.zig");
const std = @import("std");

pub fn main() void {
    const result = math.add(2, 3);
    std.debug.print("Result: {}\n", .{result});
    // math.helper(5) — compile error: not pub
}

// Comptime import + generics
fn Range(comptime T: type) type {
    return struct {
        from: T,
        to: T,
    };
}
```

### Key Design Rationale

- **File = struct = module** unifies three concepts into one. There's no separate "module system" to learn — if you understand structs, you understand modules.
- **`@import` is a comptime function**, not a special syntax. This means imports follow the same rules as all other compile-time code — no magic, no special cases.
- **No hidden control flow** is Zig's core philosophy. You should be able to look at any line of code and understand what it does without consulting macros, implicit conversions, or automatic behaviors.
- **Private by default** follows the principle of least privilege.

### Strengths

- Conceptually minimal — file, struct, and module are the same thing
- `@import` as a comptime function is elegant and consistent with Zig's comptime philosophy
- No hidden control flow makes code maximally readable and auditable
- Comptime generics eliminate the need for template or macro systems
- Build system is written in Zig itself (`build.zig`), so module relationships are expressed in the same language
- Private-by-default with `pub` opt-in is simple and sufficient

### Weaknesses / Known Pain Points

- **Only two visibility levels** (`pub` and private) — no equivalent of `pub(crate)` or package-private
- **No module hierarchy built into the language** — directory structure is managed by `build.zig`, not the compiler
- **File-as-struct** can be confusing when files have complex initialization or when the "struct" metaphor breaks down
- **No module-level export lists** — you must scan the file for `pub` items to understand the API
- **Package management is still maturing** — Zig's package ecosystem is less developed than Rust's or Go's
- **No parameterized modules** — comptime generics partially compensate but aren't as structured as functors

### Unique Insights for Spore

- File = struct is the simplest possible module system and worth considering. It means there's no "module language" — just the core language applied to files.
- `@import` as a comptime function shows that imports don't need special syntax if you have powerful enough compile-time evaluation.
- The "no hidden control flow" principle is valuable and often overlooked. Spore should consider: can imports trigger side effects? Can module initialization have hidden costs?

---

## 10. Idris 2

### Design Summary

Idris 2 provides a three-level visibility system designed to work with **dependent types**. Each source file is a module with a hierarchical name matching its path. The visibility modifiers are:

- **`private`** (default): Not exported — internal helpers only.
- **`export`**: The type signature is exported, but not the definition. Other modules can use the function/type in their types but cannot see the implementation.
- **`public export`**: Both the type and definition are exported. External code can pattern-match on constructors, reduce functions, and use the definition in type-level computation.

The distinction between `export` and `public export` is critical in dependent types: if a function appears in a type, its definition must be `public export` for the type-checker in importing modules to reduce it. This creates a unique tension between encapsulation and type-level computation.

```idris
module BTree

public export
data BTree : Type -> Type where
  Leaf : BTree a
  Node : BTree a -> a -> BTree a -> BTree a

export
insert : Ord a => a -> BTree a -> BTree a
insert x Leaf = Node Leaf x Leaf
insert x (Node l v r) =
  if x < v then Node (insert x l) v r
            else Node l v (insert x r)

export
toList : BTree a -> List a
toList Leaf = []
toList (Node l v r) = toList l ++ (v :: toList r)

-- BTree and its constructors are fully visible (public export)
-- insert and toList are usable but their definitions are opaque (export)
-- Any private helpers would be invisible entirely
```

### Key Design Rationale

- **`export` vs `public export` exists because of dependent types**: In a dependently typed language, types can contain arbitrary computations. If a function `f` appears in a type `Vec (f n)`, the type-checker must be able to evaluate `f` — which requires its definition to be visible. The `export` modifier lets you export a function's type without its definition, preventing the type-checker from reducing it in other modules.
- **This creates deliberate abstraction barriers at the type level**: You can export a type like `ValidatedInput` where the validation function's definition is hidden. Importers can use values of type `ValidatedInput` but cannot forge them by exploiting the validation logic.
- **Namespaces** (introduced within modules) provide sub-module organization without separate files, enabling name disambiguation within a single module.

### Strengths

- Three-level visibility is perfectly suited for dependent types — the `export` / `public export` distinction doesn't exist in other languages because they don't need it
- Enables principled abstraction over type-level computation
- `public export` for data types means importers can pattern-match; `export` means they can't — giving precise control over data abstraction
- Namespace blocks within modules provide flexible sub-organization
- Module system integrates naturally with the elaborator and totality checker

### Weaknesses / Known Pain Points

- **Confusing for newcomers**: The `export` / `public export` distinction is subtle and error-prone. Getting it wrong produces confusing type errors
- **Compilation can be slow**: `public export` means the compiler must inline definitions across module boundaries for type-checking, which can be expensive
- **Small ecosystem** — Idris 2 is primarily a research language
- **No parameterized modules** — Idris relies on dependent types themselves (pi types over modules) rather than functors
- **Module = file is rigid** — no multi-file modules
- **Limited tooling** for module navigation and API exploration

### Unique Insights for Spore

- The `export` vs `public export` distinction is the most nuanced visibility system in any language. If Spore uses dependent types or type-level computation, it will need something similar.
- The insight that "exporting a type is different from exporting a definition" generalizes beyond dependent types. Even in simpler type systems, you might want to export a type alias without exposing what it aliases.
- Idris 2 shows that visibility and the type-checker are deeply intertwined. Spore's module system should be designed alongside its type system, not independently.

---

## Comparison Matrix

| Feature | Rust | OCaml/SML | Roc | Unison | Elm | Koka | Go | Haskell | Zig | Idris 2 |
|---|---|---|---|---|---|---|---|---|---|---|
| **File = Module?** | No (file + `mod` decl) | Yes (`.ml` = struct) | Yes | No (no files) | Yes | Yes | Yes (dir = pkg) | Yes | Yes (file = struct) | Yes |
| **Default visibility** | Private | Public (OCaml), Explicit (SML) | Private (header-based) | Public (by name) | Private (exposing) | Private | Lowercase = private | Public (no export list) | Private | Private |
| **Circular deps allowed?** | Within crate only | Limited (`module rec`) | No | N/A (hash-based) | No | No | No | Limited (`.hs-boot`) | No | No |
| **Content addressing?** | No (crates.io uses semver) | No | Pkg URLs are hashed | Yes (core concept) | No | No | go.sum checksums | No | No | No |
| **Parameterized modules?** | No (generics instead) | Yes (functors) | No | No | No | No | No | No (typeclasses instead) | No (comptime instead) | No (dep. types instead) |
| **Effects interact with modules?** | No | No | Yes (platform = IO) | No | No | Yes (effects in types) | No | Yes (IO monad) | No | No |
| **Capability/IO restriction?** | No | No | Yes (platform owns IO) | No | No (but pure) | Yes (effect tracking) | No | Partial (IO monad) | No | No |
| **Visibility levels** | 4+ (pub, pub(crate), pub(super), pub(in path)) | 2 (public/abstract via sig) | 2 (exported/private) | 1 (names are public) | 2 (exposed/private) | 2 (pub/private) | 2 (uppercase/lowercase) | 2 (exported/not) | 2 (pub/private) | 3 (private/export/public export) |
| **Orphan rules?** | Yes (trait coherence) | N/A | N/A | N/A | N/A | N/A | N/A | Warned, not enforced | N/A | N/A |
| **Module-level export list?** | No (per-item `pub`) | Yes (`.mli` sig files) | Yes (module header) | N/A | Yes (`exposing`) | No (per-item `pub`) | No (per-item case) | Yes (module header) | No (per-item `pub`) | No (per-item modifier) |
| **Qualified imports?** | Yes (`use X as Y`) | Yes (`Module.name`) | Yes (`import X as Y`) | Yes (namespace paths) | Yes (`import X as Y`) | Yes | Yes (pkg prefix) | Yes (best-in-class) | Yes (`const x = @import`) | Yes |

---

## Cross-Cutting Observations

### 1. The Visibility Spectrum

Languages fall on a spectrum from **coarse** to **fine-grained** visibility:

| Coarseness | Languages | Approach |
|---|---|---|
| **Coarsest** | Go, Elm, Koka | Two levels only (public/private) |
| **Medium** | Haskell, Zig, Roc, Unison | Two levels + conventions or module-level lists |
| **Fine** | Rust, Idris 2 | 3-4+ explicit visibility levels |
| **Structural** | OCaml/SML | Visibility via signature ascription (abstract types) |

**Observation**: Most languages start with two levels and add more over time (Go added `internal/`; Rust added `pub(crate)`). Starting with three levels (private, package-internal, public) seems to be the sweet spot.

### 2. The Circular Dependency Question

Nearly every language in this survey forbids circular dependencies between modules or packages. The only exceptions are limited: Rust allows cycles within a crate (but not between crates), and OCaml and Haskell have restricted `module rec` / `.hs-boot` support. Elm, Go, Roc, Koka, Zig, and Idris 2 all forbid cycles entirely.

**Consensus**: Forbidding circular dependencies is the clear majority position. The pain of extracting shared modules is considered acceptable in exchange for predictable compilation, clean architecture, and simpler tooling.

### 3. Effects and Modules Are Converging

Three languages explicitly tie effects to modules:
- **Roc**: Platform provides effects; packages can't do IO.
- **Koka**: Effects appear in function type signatures and are exportable module items.
- **Haskell**: `IO` monad is the effect boundary, but it's coarse (all IO or no IO).

**Trend**: Modern languages increasingly treat "what effects does this module bring?" as a first-class question. Koka's approach (effects in types) is the most principled; Roc's approach (platform owns effects) is the most practical for security.

### 4. File-Based vs. Content-Addressed

Every language except Unison uses files as the primary unit of code organization. Unison's content-addressing solves real problems (dependency hell, merge conflicts, incremental compilation) but at the cost of abandoning the entire file-based tooling ecosystem.

**Middle ground**: Content-hash package/module identifiers (like Roc's URL hashes or Go's `go.sum`) within a file-based system captures many benefits without the radical tooling disruption.

### 5. Module Parameterization

Only OCaml/SML provides true parameterized modules (functors). Other languages compensate with:
- **Rust, Go**: Generics at the function/type level
- **Haskell**: Typeclasses
- **Zig**: Comptime evaluation
- **Idris 2**: Dependent types

**Observation**: Functors are powerful but rarely adopted because they add a separate "module language." Languages that integrate parameterization into the core type system (generics, typeclasses, dependent types, comptime) seem to achieve similar results with less conceptual overhead.

### 6. The Export List Question

Languages split on whether exports are declared **per-item** or **per-module**:

| Approach | Languages |
|---|---|
| **Per-item** (`pub` on each item) | Rust, Koka, Go, Zig, Idris 2 |
| **Module-level list** | Elm, Haskell, Roc, OCaml (`.mli`) |

Both approaches work. Module-level lists provide a single place to see the entire API surface. Per-item markers keep the export decision co-located with the definition. A hybrid (per-item `pub` with an optional module-level summary) could capture both benefits.
