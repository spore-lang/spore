# Spec relation declarations

The current Spore Spec source contract, reviewed on 2026-11-15, introduces `impl` for independent relations between existing types. This is a language target, not a claim that the bootstrap compiler accepts it. The compiler's existing `impl Trait for Type` form belongs to its earlier trait model and remains unchanged.

The authoritative local sources are `spore-spec/spec/syntax.md` (G-relations) and `spore-spec/spec/semantics.md` (S-relation-completeness, S-orphan-relations and S-path-coherence).

## Forms and obligations

```text
impl A <: B;
impl A <: B { method render() -> Str { self.text } };
impl@<u>[T: Type@<u>] F[T] <: G[T];
impl B :> A;
```

An impl without a body checks existing members, defaults and evidence. A body completes missing implementations or proofs; it cannot conflict with existing members or add required storage to existing values. Neither form is an axiom. Both directions normalize to the same implementation-to-abstraction relation.

Every impl must satisfy these requirements:

- The current package owns at least one endpoint's outer nominal type constructor. Transparent aliases, imports, re-exports and local arguments inside foreign generic constructors do not transfer ownership.
- Required members are complete and inductive branches are covered. A matching field layout alone does not register a relation.
- Overlapping implementations and multiple paths have a coherent observable interpretation. Import order cannot select a preferred implementation.

Parameter bounds, relation propositions and case family indices retain `<:` / `:>` without impl. Structural membership completion `F: C { ... };` also keeps its existing form. Old independent declarations such as `A <: B;` are rejected by the current Spec recognition grammar.

## Compiler boundary

This update changes documentation only. It does not translate the target into `impl Trait for Type`, alter the parser or lower the relation to an arbitrary conversion function. Spec recognition and highlighting do not establish type checking, ownership or path coherence. A compiler migration must implement and validate those obligations against Spec conformance cases before claiming support.
