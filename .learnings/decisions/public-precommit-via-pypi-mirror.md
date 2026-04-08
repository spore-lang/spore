---
title: "Public pre-commit hooks should use a thin mirror repo"
category: decisions
tags: [pre-commit, pypi, maturin, packaging, rust]
created: 2026-04-08
context: "Replacing spore's repo-local published hook scripts with a cleaner public distribution model"
---

## Problem

Publishing reusable pre-commit hooks directly from the `spore` source repo looked attractive at first, but the implementation quality was poor:

- `pre-commit`'s `language: rust` model expects the hook repo root to be installable, which does not fit Spore's virtual-workspace root
- `language: script` plus `cargo run --manifest-path ...` worked, but it forced public consumers to build from a source checkout with a Rust toolchain
- this made the source repo the wrong long-term public hook surface

## Decision

Use a two-layer public distribution model:

1. package `spore-cli` as a PyPI binary package from `crates/spore-cli` via `maturin`
2. publish public pre-commit hooks from a separate thin mirror repo that depends on the packaged CLI instead of compiling Spore from source

The `spore` repo should keep only local maintainer hook UX (`prek.toml`, `just pre-commit-install`, `just pre-commit`) and should not advertise itself as the long-term reusable hook source.

## Why

- matches the successful Ruff / `ruff-pre-commit` split:
  - main repo owns the binary/package
  - thin mirror repo owns hook installation UX
- avoids hard-coding Cargo workspace internals into the public hook contract
- allows future hook users to get prebuilt wheels from PyPI instead of paying source-build costs in hook setup
- keeps review boundaries cleaner: packaging concerns stay in `spore`, hook-distribution concerns stay in the mirror repo

## Implementation Notes

- add a root `pyproject.toml` with `maturin` `bindings = "bin"` and `manifest-path = "crates/spore-cli/Cargo.toml"`
- validate packaging in CI by building a wheel and sdist, installing the wheel, and smoke-testing `spore run/check/test`
- remove `.pre-commit-hooks.yaml` and repo-local public hook scripts from `spore`
- retain local `prek`-based hook workflow for Spore contributors

## Follow-up

- create a dedicated `spore-pre-commit` mirror repo
- make that mirror repo a thin Python package depending on the published `spore-cli` wheel
- keep Cargo workspace version and `pyproject.toml` version in sync until release tooling automates the bump
