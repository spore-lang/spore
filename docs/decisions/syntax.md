# Syntax decisions

This note records the **current implementation surface** for Spore syntax in the
main repository. The sibling [`spore-evolution`](../../../spore-evolution)
repository remains the normative design source; this file exists so parser,
formatter, driver, LSP, and tests can share one short, local summary.

## Intent signatures

The current function surface is:

```spore
fn name[T: Bound + Other](params: List[T]) -> Ret ! ErrorA | ErrorB
uses [Console, NetConnect]
budget {
    calls: 2
    holes: 0
}
properties {
    preserves_shape(params: List[T]): len(name(params)) >= 0
}
{
    ?body
}
```

### Clause order

The formatter canonicalizes function clauses in this order:

1. `fn name[...](...)`
2. `-> ReturnType`
3. `! ErrorA | ErrorB`
4. `uses [Effect, ...]`
5. `budget { field: limit }`
6. `properties { name(params): predicate }`
7. function body

## Generic bounds

Generic bounds live **inline** on type parameters:

```spore
fn member[T: Eq + Hash](xs: List[T], value: T) -> Bool { ?body }
```

Spore does **not** use a trailing `where` clause for generic bounds.

## Effects

Effect requirements live in `uses <surface-expr>` and name source-level effect
requirements visible to the checker, hole reports, CLI, and LSP. Atomic
protocols use `effect`; reusable finite sets use `surface`.

```spore
effect Console {
    fn println(message: Str) -> ();
}

surface CliIO = [Console, FileRead, FileWrite]

fn read_line_trimmed() -> Str uses [Console] { trim(read_line()) }
fn run() -> () uses CliIO { ?body }
```

## Budgets

Source-level implementation-shape constraints use `budget { ... }`.

Currently supported built-in fields are:

- `branches`
- `nesting`
- `recursion`
- `parallelism`
- `calls`
- `effects`
- `holes`

Budgets are checked as errors and also projected into hole reports as
`budget_context`.

## Properties

Source-level behavior assertions use `properties { ... }`.

```spore
fn abs(x: I64) -> I64
properties {
    non_negative(x: I64): abs(x) >= 0
}
{
    if x < 0 { 0 - x } else { x }
}
```

Each property declaration has the form:

```spore
name(param: Type, ...): predicate
```

Property predicates must type-check as `Bool`. Property names are projected into
hole reports as `property_context`.

## Holes

The current hole surface is:

- anonymous hole: `?`
- named hole: `?todo`
- typed hole: `?todo: ExpectedType`

Typed-hole annotations constrain the expected realization type and are included
in HoleReport output. The compiler also reports visible bindings, required
effects, budget context, and property context for each hole.

## Intentionally rejected source forms

The parser rejects these forms with teaching diagnostics:

- trailing generic `where` clauses
- `cost [...]` clauses
- `spec { ... }` blocks
- effect aliases such as `effect IO = A | B`
- function annotations such as `@...`
- hole metadata annotations such as `?todo @...`

Use the current surface instead:

- inline bounds: `fn f[T: Trait](...)`
- effect clauses: `uses [Effect]`
- reusable effect surfaces: `surface IO = [EffectA, EffectB]`
- budgets: `budget { field: limit }`
- properties: `properties { name(...): expr }`

## Related design docs

- [SEP-0001 core syntax](../../../spore-evolution/seps/SEP-0001-core-syntax.md)
- [SEP-0003 effect system](../../../spore-evolution/seps/SEP-0003-effect-system.md)
- [SEP-0004 realization-shape budgets](../../../spore-evolution/seps/SEP-0004-cost-analysis.md)
- [SEP-0005 hole system](../../../spore-evolution/seps/SEP-0005-hole-system.md)
