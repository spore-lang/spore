# Spore Roadmap

This is a living document in the **Spore implementation repository**. It
describes what Spore intends to build, organized by system area.

Design rationale and normative semantics live in the sibling
[`spore-evolution`](../spore-evolution) repository. The vision states design
principles, SEP-0000 owns review questions and process mechanics, and each
section below links to the concrete SEPs that define _how_ and _why_.

## Compiler core (`sporec`)

Design basis: [SEP-0006](../spore-evolution/seps/SEP-0006-compiler-architecture.md)

- Cranelift code generation behind a stable TypedHIR boundary
- Incremental compilation keyed by signature, intent, property, realization,
  evidence, and dependency provenance
- End-to-end pipeline: Source -> Lex -> Parse -> Resolve -> TypeCheck -> Verify
  -> Codegen -> Cranelift IR -> native

## Signature, intent, and properties

Design basis: [SEP-0001](../spore-evolution/seps/SEP-0001-core-syntax.md),
[SEP-0006](../spore-evolution/seps/SEP-0006-compiler-architecture.md)

- Base Signature parsing and canonicalization
- Intent Signature parsing for `uses`, `budget`, and `properties`
- Internal Claim construction from source properties
- EvidenceRecord generation and storage

## Hole system

Design basis: [SEP-0005](../spore-evolution/seps/SEP-0005-hole-system.md)

- Stabilize HoleReport protocol around typed absence
- Project capability, budget, and property context into every report
- Multi-hole atomic fill with cross-hole consistency checking
- Agent fill-and-verify loop: propose -> validate -> review if flagged

## Type system

Design basis: [SEP-0002](../spore-evolution/seps/SEP-0002-type-system.md)

- Inline generic bounds in type parameter lists
- L0 refinement type enforcement for decidable predicates
- L1 abstract interpretation propagation for value flow analysis
- Type evidence emitted through the compiler evidence layer

## Effect and capability system

Design basis: [SEP-0003](../spore-evolution/seps/SEP-0003-effect-system.md)

- Runtime effect handler dispatch
- Capability-surface filtering for hole reports
- Reference CLI platform: console, filesystem, network connect/listen,
  environment, process spawning, clock, random, and exit
- Reference web platform: HTTP server, request/response

## Budget model

Design basis: [SEP-0004 realization-shape budgets](../spore-evolution/seps/SEP-0004-cost-analysis.md)

- Quantitative realization-shape constraints
- Built-in budget fields for branches, nesting, recursion, parallelism, calls,
  effects, and holes
- Budget evidence emitted per checked realization

## Concurrency

Design basis: [SEP-0007](../spore-evolution/seps/SEP-0007-concurrency-model.md)

- Runtime execution of structured concurrency primitives (`spawn`, `await`,
  `parallel_scope`)
- Compiler-enforced task lifetimes
- Cancellation propagation
- Budget-aware fan-out and nesting checks
- Channel boundary checks

## Module and package system

Design basis: [SEP-0008](../spore-evolution/seps/SEP-0008-module-package-system.md)

- Content-addressed package store
- `spore.toml` plus generated lock data
- Package discovery, search, and documentation generation
- Provenance hashes for signatures, intents, properties, realizations, evidence,
  and dependencies

## Diagnostics and developer experience

Design basis: [SEP-0006](../spore-evolution/seps/SEP-0006-compiler-architecture.md)

- LSP server with completions, diagnostics, go-to-definition, hole integration
- `spore watch` with real-time incremental diagnostics
- `spore watch --json` NDJSON events for IDE and Agent consumption

## Standard library

Design basis: [SEP-0009](../spore-evolution/seps/SEP-0009-standard-library.md)

- `spore.list`, `spore.map`, `spore.set`, `spore.str`, `spore.math`, `spore.ref`
- Standard library properties for core operations
- All further libraries as third-party packages

## Self-hosting

Design basis: [SEP-0006](../spore-evolution/seps/SEP-0006-compiler-architecture.md)

- Partial self-hosting: parser, type checker, budget checker, and evidence
  builder rewritten in Spore
- Performance target: compiled output within 2x of equivalent Rust for compute-bound code
- Formal language specification beyond design docs
- Stability policy for signatures, holes, and evidence protocols

## Long-term explorations

These are not committed but may influence future design:

- Distributed effect delegation with cryptographic attestation
- Optional formal verification mode for safety-critical paths
- Visual hole explorer for interactive dependency graph navigation
- Cross-platform compilation: WASM, embedded, GPU via effect-gated backends
