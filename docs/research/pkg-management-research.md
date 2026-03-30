Agent completed. agent_id: pkg-research, agent_type: explore, status: completed, description: Research package management systems, elapsed: 401s, total_turns: 0, duration: 190s

I'll provide you with a comprehensive research document on these 10 package management systems. This is based on my training knowledge of these systems.

---

# Comprehensive Package Management Systems Research

## Table of Contents
1. Individual Package Manager Profiles
2. Cross-Cutting Comparison Tables
3. Dependency Conflict Resolution
4. Reproducibility & Security

---

## 1. RUST: Cargo

### 1.1 Manifest File Format & Key Fields
**File:** `Cargo.toml` (TOML format)

```toml
[package]
name = "my-crate"
version = "0.1.0"
edition = "2021"
authors = ["Author <email@example.com>"]
description = "A brief description"
license = "MIT OR Apache-2.0"
repository = "https://github.com/user/repo"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.35", optional = true }
custom-crate = { path = "../custom-crate" }
git-crate = { git = "https://github.com/user/repo", rev = "abc123" }

[dev-dependencies]
criterion = "0.5"

[build-dependencies]
cc = "1.0"

[features]
default = ["async"]
async = ["tokio"]
```

**Key Fields:**
- `version`: Semantic versioning (required)
- `dependencies`: Production dependencies with version specs, optional flags, features
- `dev-dependencies`: Testing dependencies
- `build-dependencies`: Build script dependencies
- `features`: Conditional compilation flags; enable subsets of functionality
- `edition`: Language version (2015, 2018, 2021)
- `workspace`: Monorepo root declaration

### 1.2 Dependency Resolution Algorithm

**Algorithm:** **SAT solver-based** (PubGrub algorithm since Cargo 1.51+)

- **Historical:** Backtracking (pre-1.51)
- **Current:** PubGrub SAT solver (faster, more predictable failures)
- **Process:**
  1. Fetch all candidate versions satisfying constraints
  2. Build version graph with compatibility relationships
  3. Solve for the **maximal compatible set** (prefers newest versions)
  4. Fails fast with clear explanations of why versions don't work together
  5. No backtracking to older versions once a choice is made

**Characteristics:**
- Prefers newest versions that satisfy all constraints
- Deterministic outcomes (same lockfile across runs)
- Can detect unsolvable dependencies upfront

### 1.3 Version Constraint Syntax & Strategy

**Semver-focused:**

```toml
serde = "1.0"           # ^1.0.0 (caret, allow 1.x.y)
serde = "^1.0.0"        # Allow 1.x.y (same major)
serde = "~1.2.3"        # Allow 1.2.z (patch only)
serde = "=1.2.3"        # Exact version
serde = ">=1.0,<2.0"    # Range
serde = "1.0,2.0"       # Multiple constraints
serde = "*"             # Any version
```

**Semantic Versioning Guarantee:**
- Breaking changes require **major version bump** (1.0 → 2.0)
- Crates.io enforces 0.x.0 is NOT backward compatible by default
- 0.0.x versions are treated as completely unstable

**Strategy:**
- Conservative by default (caret ranges)
- Relies on semver trust and community discipline
- Ecosystem culture: breaking changes = major version bump

### 1.4 Registry/Distribution Model

**Centralized (primarily):**
- **crates.io**: Official Rust package registry (GitHub-hosted, Git-based storage)
- **Mirrors**: Aliyun, Tsinghua, etc. for performance in regions
- **Alternative registries**: Can be configured for private dependencies
- **Source access**: All crates must include source code (open source requirement)

**Distribution:**
- crates.io publishes `.crate` files (tarball with source)
- Cargo.lock coordinates downloads from registry
- Can use git dependencies (auto-cloned, slower)
- Path dependencies for local development

### 1.5 Lockfile Format & Purpose

**File:** `Cargo.lock` (TOML-like format)

```toml
[[package]]
name = "serde"
version = "1.0.197"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "3fb1c873e..."

[[package]]
name = "tokio"
version = "1.35.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = ["bytes", "libc", "mio"]
checksum = "d4f8..."

[metadata]
rust-version = "1.70"
```

**Purpose:**
- **Reproducible builds**: Pins exact versions + checksums
- **Binary projects**: Committed to VCS (for reproducibility)
- **Libraries**: May not be committed (consumers resolve independently)
- **Checksum verification**: Ensures no tampering
- **Git resolution tracking**: Records exact commits resolved

**Format:** Human-readable TOML; includes package graph, checksums, source locations

### 1.6 Workspace/Monorepo Support

**Built-in first-class support:**

```toml
# workspace root Cargo.toml
[workspace]
members = ["crate1", "crate2", "crate3"]
resolver = "2"

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }

# Individual crates can inherit:
[package]
name = "crate1"
version.workspace = true
dependencies.serde.workspace = true
```

**Features:**
- Unified `target/` build directory (shared artifacts)
- Shared dependency versions across crates
- Single Cargo.lock per workspace
- Workspaces resolve all dependencies together
- Virtual manifests (workspace without a package root)
- Dependency inheritance for DRY configuration

### 1.7 Notable Innovations & Unique Features

1. **Features System**: Compile-time feature flags built into package management
   - Enables conditional dependencies and code paths
   - Solves the "configuration" problem elegantly
   - Prevents splitting packages for minor variations

2. **Semantic Versioning Enforcement**: Community-wide commitment to semver (not mandatory but strongly encouraged)

3. **PubGrub SAT Solver**: Clear error messages when dependency resolution fails
   - Shows exactly why versions don't work together
   - Faster than backtracking algorithms

4. **Path & Git Dependencies**: First-class support for unreleased code
   - Excellent for monorepos and development workflows
   - Simplifies integration testing

5. **Platform-Specific Dependencies**:
   ```toml
   [target.'cfg(windows)'.dependencies]
   winapi = "0.3"
   ```

6. **Yanked Versions**: Crates can be yanked (hidden) if broken, but Cargo.lock still works
   - Doesn't break existing builds
   - Prevents new builds from using bad versions

### 1.8 Known Pain Points & Criticisms

1. **Feature Hell**: Combinatorial explosion of feature combinations can cause subtle bugs
   - No "recommended" feature sets by default
   - Testing all combinations is expensive

2. **Cargo Build Times**: Incremental builds often slow, especially on Windows
   - Monorepo can amplify this
   - Dependency graph can cause cascading rebuilds

3. **Semver Enforcement Reliance**: Relies on humans not breaking semver
   - No automated enforcement mechanism
   - Some ecosystem members still make breaking changes in patch releases

4. **Large Dependency Trees**: Encourages small, focused crates
   - Results in deep dependency graphs
   - Supply chain risk for large projects

5. **Binary Size**: Monomorphization of generics increases binary size
   - No tree-shaking for unused code paths

6. **Platform-Specific Complexity**: Handling multiple targets requires special syntax

---

## 2. GO: Go Modules

### 2.1 Manifest File Format & Key Fields

**File:** `go.mod` (custom format, human-readable)

```go
module github.com/user/project

go 1.21

require (
    github.com/user/dep1 v1.2.3
    github.com/google/uuid v1.5.0
)

require (
    github.com/internal/dep2 v0.1.0 // indirect
)

exclude github.com/badlib v1.0.0
retract [v1.0.0, v1.1.0]
```

**Key Fields:**
- `module`: Module path (import path)
- `go`: Go version requirement
- `require`: Direct dependencies with exact versions
- `indirect`: Transitive dependencies (automatically tracked)
- `exclude`: Versions to ignore
- `retract`: Versions to mark as invalid (security, bugs)
- `replace`: Override dependencies (useful for patches, local testing)

**Format:** Each line is semantically meaningful; tools modify it directly

### 2.2 Dependency Resolution Algorithm

**Algorithm:** **Minimal Version Selection (MVS)** — unique approach

- **Philosophy:** Select the **minimal (lowest) version** that satisfies all constraints
- **Process:**
  1. Start with direct dependencies at versions specified
  2. For each, add its dependencies (recursively)
  3. If a module appears multiple times, use the **maximum** required version
  4. This is deterministic but differs from "latest compatible" approach

**Characteristics:**
- **Predictable**: No backtracking; results determined by simple rules
- **Conservative**: Minimizes version jumps, reducing surprise changes
- **Fast**: O(n) graph traversal (single pass)
- **Transparency**: Easy to understand what versions you're getting
- **Downside**: May not use latest patch fixes if not explicitly required

**Example:**
```
A requires B v1.2, C v1.0
B v1.2 requires D v1.1
C v1.0 requires D v1.0

Result: uses D v1.1 (maximum of 1.1 and 1.0)
```

### 2.3 Version Constraint Syntax & Strategy

**Precise versions, no ranges:**

```go
require github.com/user/dep v1.2.3     // Exact version
require github.com/user/dep v1.2       // Not allowed (error)
require github.com/user/dep latest     // Not allowed (error)
```

**Pre-release & Pseudo-versions:**

```go
require github.com/user/dep v2.0.0-beta.1
require github.com/user/dep v0.0.0-20231225123456-abcdef123456  // Pseudo-version
```

**Strategy:**
- **No ranges**: Forces explicit version pinning
- **Semantic versioning**: Expected but not enforced
- **Pseudo-versions**: For unreleased commits (v0.0.0-YYYYMMDDhhmmss-abcdef)
  - Generated from git tags
  - Sortable and reproducible
- **Pre-release versions**: Explicitly required to use

**Update Strategy:**
- `go get -u`: Upgrade to latest compatible
- `go get -u=patch`: Only patch versions
- `go get -u=minor`: Only minor+ versions (rare usage)

### 2.4 Registry/Distribution Model

**Decentralized (default):**
- **No central registry**: Modules can be hosted anywhere (GitHub, GitLab, Gitea, etc.)
- **Module path = import path**: `github.com/user/repo` is fetched from that URL
- **Module Proxy (optional)**: 
  - `proxy.golang.org`: Official proxy (caches modules, provides security)
  - Private proxies available (Artifactory, Nexus, etc.)
- **VCS Integration**: Go directly queries git, hg, svn repositories

**Distribution:**
- Source code only (no compiled artifacts)
- Git tags determine versions
- Checksum database (`sum.golang.org`) verifies integrity
- Proxy acts as cache + security layer

### 2.5 Lockfile Format & Purpose

**File:** `go.sum` (hash verification file)

```
github.com/user/dep v1.2.3 h1:abc...xyz...
github.com/user/dep v1.2.3/go.mod h1:def...xyz...
github.com/google/uuid v1.5.0 h1:uvw...
```

**Purpose:**
- **Security**: Verifies module integrity (SHA-256 hashes)
- **Immutability**: Prevents tag rewriting attacks
- **No version pinning**: Does NOT pin versions (that's go.mod's job)
- **Verification**: Checked against Google's Checksum Database

**Format:**
- Each line: `module@version hash`
- `.mod` suffix: Hash of go.mod file specifically
- Default: Hash of full module

### 2.6 Workspace/Monorepo Support

**Go Workspaces (Go 1.18+):**

```go
// go.work
go 1.21

use (
    ./services/api
    ./services/auth
    ./lib/common
)

replace github.com/user/external => ./patches/external
```

**Features:**
- Unified resolution across multiple modules
- Replaces modules locally during development
- Each sub-module keeps its own go.mod
- Single go.sum for all modules combined
- Not committed to VCS (local development only)

**Characteristics:**
- **Late addition**: Workspaces were added in Go 1.18 (late adoption)
- **Local-first**: Designed for development, not distribution
- **Optional**: Libraries don't require workspace support

### 2.7 Notable Innovations & Unique Features

1. **Minimal Version Selection (MVS)**:
   - Philosophically opposite of "latest compatible"
   - Reduces surprise version updates
   - Built on simple, verifiable rules

2. **Decentralized Registry**:
   - No central authority required
   - Works with any VCS hosting
   - Proxy is optional (for performance/security)

3. **Checksum Database Integration**:
   - Public immutable hash registry
   - Prevents supply chain attacks
   - Verifies consistency across proxy/direct access

4. **Semantic Versioning as Default**:
   - Expected practice but not enforced
   - v0.x.x = unstable (by convention)
   - v1.0.0+ = stable

5. **Pseudo-versions for Unreleased Code**:
   - Automatically generated from commits
   - Sortable and reproducible
   - Bridge between releases and git commits

6. **Tidy & Vendor Modes**:
   - `go mod tidy`: Removes unused dependencies
   - `go mod vendor`: Vendoring for offline/reproducible builds
   - Both built-in, first-class

### 2.8 Known Pain Points & Criticisms

1. **No Version Ranges**: Forces repetitive version updates
   - `go get -u` updates all dependencies
   - No middle ground between "latest" and "exact"

2. **Minimal Version Selection Philosophy**: Counterintuitive for many
   - Developers expect "latest compatible"
   - Can miss critical patch fixes
   - Requires discipline in semantic versioning

3. **Decentralized = Verification Burden**: Must trust module hosts
   - Early implementations vulnerable to domain hijacking
   - Requires explicit proxy for security

4. **Breaking Changes in v0.x**: Common practice but not enforced
   - v0.x.y treated as any version can break
   - Can surprise users

5. **Monorepo Support Gap**: Workspaces added late (Go 1.18)
   - Initially no built-in support
   - Still local-only (can't publish workspaces)

6. **Dependency Size**: No native size limits
   - Large transitive trees possible
   - No tree-shaking

---

## 3. DENO: Package Management

### 3.1 Manifest File Format & Key Fields

**Files:** `deno.json` (or `deno.jsonc`) + `deno.lock`

```json
{
  "name": "@user/my-app",
  "version": "0.1.0",
  "description": "My Deno app",
  "exports": "./src/mod.ts",
  
  "imports": {
    "std/": "https://deno.land/std@0.208.0/",
    "oak": "https://deno.land/x/oak@v12.6.1/mod.ts",
    "jsr:@std/assert": "jsr:@std/assert@^0.208.0",
    "./lib/": "./src/lib/"
  },
  
  "tasks": {
    "dev": "deno run --allow-net src/server.ts"
  }
}
```

**JSR (JavaScript Registry) format:**
```json
{
  "name": "@user/my-package",
  "version": "0.1.0",
  "exports": "./mod.ts",
  "imports": {
    "jsr:@std/assert": "^1.0.0"
  }
}
```

**Key Fields:**
- `imports`: URL-based dependency mapping (supports JSR, Deno, npm)
- `exports`: Public API entry point
- `tasks`: Custom scripts (like npm scripts)
- `name` / `version`: Package metadata (for JSR registry)

### 3.2 Dependency Resolution Algorithm

**Algorithm:** **Graph resolution with URL canonicalization**

- **No central resolver**: Deno doesn't "resolve" in traditional sense
- **Process:**
  1. Import map redirects (import URLs → actual URLs)
  2. Direct URL fetches (HTTP request to module URL)
  3. Transitive dependencies tracked through recursive imports
  4. Version constraints evaluated per-import

- **JSR Registry (new)**: Uses semver resolution like npm
  - Provides artifact hosting + metadata
  - Can specify version ranges in JSR imports

**Characteristics:**
- **URL-based**: Dependencies are explicit URLs
- **No global resolver**: Each import path resolved independently
- **Transparency**: You see exactly which URL is fetched
- **Import map flexibility**: Sophisticated redirection possible

### 3.3 Version Constraint Syntax & Strategy

**Import Maps (URL-based):**

```json
"imports": {
  "oak": "https://deno.land/x/oak@v12.6.1/mod.ts",
  "oak/": "https://deno.land/x/oak@v12.6.1/",
  "jsr:@std/assert": "jsr:@std/assert@^1.0.0"
}
```

**JSR Ranges:**
```json
"jsr:@std/assert": "^1.0.0"     // Caret (npm-compatible)
"jsr:@std/assert": "~1.2.3"     // Tilde (npm-compatible)
"jsr:@std/assert": "1.0.0"      // Exact
"jsr:@std/assert": "*"          // Latest
```

**Strategy:**
- **URL pinning**: Most common (explicit version in URL)
  - No range evaluation needed
  - Maximally reproducible
  - Straightforward but requires manual updates
- **JSR ranges**: Semver-based (npm-like)
  - Introduced with JSR registry
  - Automatic resolution by JSR
  - Hybrid approach

### 3.4 Registry/Distribution Model

**Hybrid (evolved over time):**

1. **Deno.land/x** (third-party module registry):
   - Curated, no central package ownership
   - Git-based (fetches from GitHub)
   - Version = git tag
   - No security checks

2. **JSR** (JavaScript Registry, new official):
   - Centralized registry (like npm, PyPI)
   - Official TypeScript/JSR packages
   - Artifact publishing
   - Security + quality checks
   - GitHub integration

3. **npm Registry** (supported):
   - Via `npm:` prefix in imports
   - Requires specifying exact version
   - Slower (requires compatibility layer)

4. **Decentralized** (HTTP URLs):
   - Direct GitHub URLs
   - Self-hosted modules
   - Raw.githubusercontent.com, etc.

**Distribution:**
- Source-based (no pre-compiled artifacts for Deno)
- Caching: Deno caches modules locally (~/.deno/)
- CDN: deno.land acts as CDN for x/ packages

### 3.5 Lockfile Format & Purpose

**File:** `deno.lock` (JSON format)

```json
{
  "version": "3",
  "specifiers": {
    "jsr:@std/assert@^1.0.0": "jsr:@std/assert@1.0.2",
    "oak": "https://deno.land/x/oak@v12.6.1/mod.ts"
  },
  "imports": {
    "jsr:@std/assert": {
      "specifier": "jsr:@std/assert@^1.0.0",
      "resolved": "jsr:@std/assert@1.0.2",
      "dependencies": {
        "jsr:@std/internals": "jsr:@std/internals@1.0.0"
      }
    }
  },
  "modules": {
    "jsr:@std/assert@1.0.2": {
      "type": "jsr",
      "jsr": {
        "name": "@std/assert",
        "version": "1.0.2"
      }
    },
    "https://deno.land/x/oak@v12.6.1/mod.ts": {
      "type": "javascript",
      "size": 2456,
      "hash": "sha256:abc...",
      "headers": {
        "content-type": "text/typescript"
      }
    }
  }
}
```

**Purpose:**
- **Reproducibility**: Locks exact resolved versions
- **Offline access**: Contains all module metadata
- **Integrity**: Checksums for verification
- **Transitive tracking**: Full dependency graph

### 3.6 Workspace/Monorepo Support

**Workspaces (Deno 1.40+):**

```json
{
  "workspace": ["./packages/api", "./packages/ui", "./packages/cli"],
  "imports": {
    "@myapp/api": "jsr:@myapp/api@workspace",
    "@myapp/ui": "jsr:@myapp/ui@workspace"
  }
}
```

**Features:**
- Unified dependency resolution
- Internal workspace references
- Shared node_modules (if using npm deps)
- Each package can have own deno.json

**Status**: Recently added (1.40+), still evolving

### 3.7 Notable Innovations & Unique Features

1. **URL-Based Imports**:
   - No package name registry needed
   - Explicit dependency URLs
   - Maximally transparent
   - Can directly depend on GitHub URLs

2. **Import Maps**:
   - Sophisticated URL redirection
   - Enables version pinning at runtime
   - Browser-compatible standard

3. **JSR Registry** (newest innovation):
   - Deno's official package registry
   - JavaScript-ecosystem wide (works with npm, Node)
   - TypeScript-first
   - Package publishing (unlike deno.land/x)

4. **Deno.lock Format**:
   - Human-readable (JSON)
   - Contains full transitive graph
   - Integrity hashes per module

5. **npm Compatibility Layer**:
   - Can use npm packages via `npm:` prefix
   - Node.js standard library shims
   - Gradual migration path

6. **No Manifest Algorithm Complexity**:
   - Simpler mental model
   - Transparent version resolution
   - No SAT solving needed (URLs are explicit)

### 3.8 Known Pain Points & Criticisms

1. **URL-Based Bloat**: Repetitive version specifications
   - Every import lists full URL with version
   - Harder to upgrade multiple uses of same dep
   - Import maps mitigate but still verbose

2. **No Central Namespace**: Early confusion about package organization
   - deno.land/x had no ownership model
   - Quality/security issues with third-party modules
   - JSR addresses this (relatively new)

3. **JSR Adoption Gap**: Migration from deno.land/x incomplete
   - Ecosystem split between old and new
   - Tooling still catching up
   - npm compatibility adds complexity

4. **Module Caching Complexity**: ~/.deno/ directory management
   - Can be difficult to clear/troubleshoot
   - Different from npm node_modules
   - Trust-on-first-use model (security implications)

5. **Versioning Strategy Unclear**: Multiple approaches
   - URL pinning (most explicit)
   - Import map ranges (requires JSR)
   - npm: prefix (requires npm semantics)
   - Confusing for newcomers

6. **Limited Monorepo Tooling**: Workspaces very recent
   - Ecosystem hasn't fully adopted
   - Still evolving

---

## 4. ROC: Package Management

### 4.1 Manifest File Format & Key Fields

**File:** `platform/main.roc` (platform definitions)

```roc
platform "https://github.com/roc-lang/basic-cli/releases/download/0.8.0/basic-cli.tar.br"
    requires {}
    exposes [main]
    packages {}
    imports [pf.Task, pf.Stdout]
    provides [Task] to pf

main : Task {} I32
main = Stdout.line! "Hello, world!"
```

**Note:** Roc has NO traditional package manager or manifest file like other languages.

### 4.2 Dependency Resolution Algorithm

**Not applicable**: Roc doesn't have a package manager.

- **Platform-based design**: Dependencies embedded in platform code
- **No registry**: No central or decentralized package registry
- **Static linking**: All dependencies compile into single binary
- **No dependency resolution**: Platforms define available modules

### 4.3 Version Constraint Syntax & Strategy

**Not applicable**: Versioning handled at platform level, not package level.

- Platforms have versions (in URLs)
- No semantic versioning in Roc ecosystem
- Versions part of platform distribution URLs

### 4.4 Registry/Distribution Model

**None**: Roc is not package-distribution oriented.

- Platforms distributed via URLs (GitHub releases, etc.)
- Direct download URLs in platform declarations
- No central registry
- Open-source platform code (reference implementations available)

### 4.5 Lockfile Format & Purpose

**Not used**: Roc doesn't require lockfiles (everything statically defined).

### 4.6 Workspace/Monorepo Support

**Not applicable**: Roc's model doesn't include traditional workspaces.

- Single platform per project
- Monorepo concerns not relevant in current design

### 4.7 Notable Innovations & Unique Features

1. **Platform-Based Package Model**:
   - Packages are entire platforms (runtime + stdlib + FFI)
   - Radically different from "library packages"
   - Enables capability-based security

2. **No Runtime Overhead**:
   - All dependencies statically linked
   - No dynamic loading
   - Predictable performance

3. **Lack of Traditional Package Manager**:
   - Simplicity (no version conflicts)
   - Not a limitation but intentional design
   - All code visible, composable

4. **Content-Addressed Packages**:
   - Planned feature (not yet implemented)
   - Would enable content-based deduplication
   - Hash-based package references

### 4.8 Known Pain Points & Criticisms

1. **No Package Ecosystem Yet**: Limits third-party code reuse
   - Community libraries must be platform extensions
   - Slows adoption

2. **Hard to Extend**: Creating new platforms is complex
   - Platform development requires systems-level knowledge
   - FFI bindings difficult to write

3. **Limited Interop**: Can't use existing C libraries easily
   - FFI must be manually written
   - Contrast with languages like Zig/Rust (easy FFI)

4. **Unclear Future Direction**: Package management model still evolving
   - Not stabilized post-1.0
   - May change significantly

---

## 5. ELM: Package Management

### 5.1 Manifest File Format & Key Fields

**File:** `elm.json` (JSON format)

```json
{
  "type": "application",
  "source-directories": ["src"],
  
  "elm-version": "0.19.1",
  "dependencies": {
    "direct": {
      "elm/core": "1.0.0",
      "elm/html": "1.0.0",
      "elm/json": "1.1.0",
      "rtfeldman/elm-css": "18.0.0"
    },
    "indirect": {
      "elm/virtual-dom": "1.0.0",
      "elm/json": "1.1.0"
    }
  },
  "test-dependencies": {
    "direct": {
      "elm-explorations/test": "2.0.0"
    }
  }
}
```

**For packages:**

```json
{
  "type": "package",
  "name": "author/package",
  "summary": "A brief description",
  "license": "MIT",
  "version": "1.0.0",
  "exposed-modules": ["Module"],
  "elm-version": "0.19.1",
  "dependencies": {
    "elm/core": "1.0.0"
  }
}
```

**Key Fields:**
- `type`: "application" or "package"
- `elm-version`: Exact Elm version required
- `dependencies.direct`: Explicitly specified packages
- `dependencies.indirect`: Transitive dependencies (auto-managed)
- `exposed-modules`: Public API for packages
- `source-directories`: Where source code lives

### 5.2 Dependency Resolution Algorithm

**Algorithm:** **Minimal version selection with enforced semver**

- **Similar to Go's MVS** but stricter
- **Process:**
  1. Read elm.json constraints
  2. Fetch all candidate versions from registry
  3. Check semantic versioning compliance (automated)
  4. Select minimal version satisfying constraints
  5. Build transitive dependencies

- **Semver Checking**: Registry validates that versions follow semver strictly
  - No v0.x special cases
  - Breaking change = major version bump (always)
  - Caught at publish time, not runtime

### 5.3 Version Constraint Syntax & Strategy

**Exact versions ONLY (no ranges):**

```json
"elm/core": "1.0.0"
```

**Strategy:**
- **No version ranges**: Forces explicit, reproducible pinning
- **Semantic versioning enforcement**: 
  - All versions must be valid semver (MAJOR.MINOR.PATCH)
  - All breaking changes require major version bump (enforced by tooling)
  - Prevents range explosion problem
- **Update strategy**:
  - `elm install author/package`: Specific version (prompts for version)
  - No auto-upgrade (must explicitly request)

### 5.4 Registry/Distribution Model

**Centralized:**

- **package.elm-lang.org**: Official Elm package registry
- **GitHub-based**: Registry fetches from GitHub releases
- **Single registry**: No alternatives or mirrors
- **Curated**: Published packages reviewed for quality

**Distribution:**
- Source files (Elm code only, no binaries)
- Documentation auto-generated from code
- Hosted at package.elm-lang.org

### 5.5 Lockfile Format & Purpose

**File:** `elm.json` (serves as lock)

The `elm.json` itself is the lockfile because:
- Exact versions always specified
- No range resolution
- Transitive dependencies listed (indirect)

```json
"indirect": {
  "elm/virtual-dom": "1.0.0"
}
```

**Format:**
- Direct vs. indirect clearly separated
- All dependencies listed explicitly

### 5.6 Workspace/Monorepo Support

**Limited/None:**

- No built-in workspace support
- Elm packages can depend on other local packages via `../` paths
- Not designed for monorepos
- Each package is independent

### 5.7 Notable Innovations & Unique Features

1. **Enforced Semantic Versioning**:
   - Registry validates that all versions follow semver
   - Prevents accidental breaking changes
   - Breaking change = major version bump (guaranteed)

2. **Exact Version Pinning**:
   - No ranges, no resolution ambiguity
   - Elm.json is self-contained (no need for separate lock)
   - Maximally reproducible

3. **Automatic Documentation Generation**:
   - Package registry auto-generates docs from code
   - All packages have consistent documentation
   - No missing/stale docs

4. **Minimal Dependency Graphs**:
   - Culture discourages large dependency trees
   - Elm standard library covers many use cases
   - Registry shows dependency count (peer pressure)

5. **No Prerelease Versions**:
   - All packages must be stable (no v1.0.0-beta)
   - Simpler mental model
   - Encourages thorough testing before release

### 5.8 Known Pain Points & Criticisms

1. **Exact Versions = Manual Updates**:
   - No ranges, so must manually bump all versions
   - `elm install` workflow not smooth
   - Tedious for large dependency trees

2. **Inflexible Versioning**:
   - Can't release prerelease for testing
   - Can't do v0.x development (all 0.x treated as 0.0.0)
   - Unusual for pre-1.0 development

3. **Registry Gatekeeping**: Single official registry
   - Can't publish to alternatives
   - Community packages must be approved
   - Can feel restrictive

4. **No Monorepo Support**: Difficult for large projects
   - Path dependencies don't scale well
   - Each package completely separate

5. **Ecosystem Size**: Smaller than mainstream languages
   - Fewer packages available
   - May need to write your own libraries

---

## 6. ZIG: Package Management

### 6.1 Manifest File Format & Key Fields

**File:** `build.zig.zon` (Zig Object Notation - custom format)

```zig
.{
    .name = "my-project",
    .version = "0.1.0",
    .minimum_zig_version = "0.12.0",
    
    .dependencies = .{
        .zstd = .{
            .url = "https://github.com/ziglang/zig-zstd/archive/refs/tags/v0.1.0.tar.gz",
            .hash = "1220abc1234567890abcdef1234567890abcdef1234567890abc",
        },
        .curl = .{
            .url = "file:///path/to/curl",
            .hash = "1220def...",
        },
        .local_lib = .{
            .path = "../local-lib",
        },
    },
    
    .paths = .{
        .@"local-lib" = "local_lib",
    },
}
```

**Key Fields:**
- `name`: Package name
- `version`: Semantic version
- `dependencies`: URL-based or local packages with hashes
- `paths`: Import path mappings
- `minimum_zig_version`: Zig compiler version requirement

### 6.2 Dependency Resolution Algorithm

**Algorithm:** **Hash-based verification (no resolution)**

- **No dependency resolution**: Dependencies are explicit URLs
- **Process:**
  1. Parse build.zig.zon
  2. Fetch URL (tarball, git, local path)
  3. Verify hash matches (content-addressed)
  4. Extract/link into build system
  5. No transitive dependency resolution

- **Characteristics:**
  - Zero-install (exact URLs specified)
  - No "diamond dependency" issues (explicit includes)
  - Build system handles integration

### 6.3 Version Constraint Syntax & Strategy

**URL-based with Hash Pinning:**

```zig
.{
    .url = "https://github.com/user/lib/archive/refs/tags/v1.2.3.tar.gz",
    .hash = "1220abcdef...",
}

// Or latest main:
.{
    .url = "https://github.com/user/lib/archive/refs/heads/main.tar.gz",
    .hash = "1220xyz789...",
}

// Local path:
.{
    .path = "../lib",
}
```

**Strategy:**
- **No version ranges**: Versions are URLs
- **Hash-based**: Content-addressed (hash = version)
- **Exact pinning**: Every dependency pinned by hash
- **Reproducible**: Same hash = same code (cryptographically guaranteed)

### 6.4 Registry/Distribution Model

**Decentralized (git/tarball-based):**

- **No central registry**: Dependencies fetched from arbitrary URLs
- **GitHub-dominant**: Most packages on GitHub
- **Tarballs/git**: Direct download from source
- **Zig package index** (community, not official):
  - Community-maintained index
  - Not required (direct URLs work)

**Distribution:**
- Source code (Zig code)
- Tarballs (.tar.gz) or git archives
- No package artifacts/binaries

### 6.5 Lockfile Format & Purpose

**File:** `build.zig.zon` (serves as lock)

The `build.zig.zon` itself IS the lock because:
- Exact URLs specified
- Hash pinning is mandatory
- Content-addressed (hash = identity)
- No separate lock needed

**Purpose of Hashes:**
- **Integrity verification**: Detect tampering
- **Immutability**: Same hash guarantees same content
- **Supply chain security**: Can't silently update packages
- **Offline verification**: Don't need registry

### 6.6 Workspace/Monorepo Support

**Path Dependencies:**

```zig
.dependencies = .{
    .lib1 = .{ .path = "../lib1" },
    .lib2 = .{ .path = "../lib2" },
}
```

**Features:**
- Path-based dependencies (similar to Rust/npm workspaces)
- Each sub-package has own build.zig.zon
- Shared dependency hashing

**Status**: Basic support, not as developed as other languages

### 6.7 Notable Innovations & Unique Features

1. **Hash-Based Package Identity**:
   - Packages identified by content hash (not name+version)
   - Prevents version confusion attacks
   - Guarantees reproducibility

2. **Decentralized by Default**:
   - No central authority required
   - URLs are primary package identifier
   - Git tags supported directly

3. **Zero-Installation Model**:
   - Packages fetched on-demand
   - No package manager daemon
   - Minimal tooling complexity

4. **Build Integration**:
   - Package fetching integrated into build system (build.zig)
   - Simpler mental model
   - Single source of truth (build.zig.zon)

5. **Language in Manifest**:
   - build.zig.zon is Zig code (with restrictions)
   - Enables sophisticated configuration
   - Type-checked configurations

### 6.8 Known Pain Points & Criticisms

1. **Manual Hash Updates**:
   - Must update hash when version changes
   - No tooling to auto-generate (sometimes)
   - Zig automatically discovers new hash on mismatch, then requires manual confirmation

2. **No Central Discovery**:
   - Hard to find packages (no central search)
   - Community index exists but not official
   - Ecosystem fragmentation

3. **Versioning Strategy Unclear**:
   - No standard versioning convention
   - Semantic versioning recommended but not enforced
   - Pre-release handling inconsistent

4. **Early-Stage Ecosystem**:
   - Relatively few packages
   - Tool stability still evolving
   - Best practices not yet settled

5. **Transitive Dependencies**: Not built-in
   - If dep A depends on dep B, must explicitly include B
   - No automatic transitive resolution
   - Can lead to version conflicts if not careful

---

## 7. PYTHON: uv / pip / Poetry

### 7.1 Manifest File Format & Key Fields

**Modern: `pyproject.toml` (TOML, PEP 621)**

```toml
[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[project]
name = "my-package"
version = "0.1.0"
description = "My package"
authors = [{name = "Author", email = "author@example.com"}]
license = {text = "MIT"}
dependencies = [
    "requests>=2.28.0",
    "numpy>=1.21.0,<2.0.0",
    "pydantic[extra]>=2.0.0",
]

[project.optional-dependencies]
dev = ["pytest>=7.0", "ruff>=0.1.0"]
docs = ["sphinx>=5.0"]

[tool.uv]
dev-dependencies = ["pytest-cov"]

[tool.uv.sources]
custom = { url = "https://example.com/custom.whl" }
```

**Legacy: `setup.py` (Python code, older)**

```python
from setuptools import setup
setup(
    name="my-package",
    version="0.1.0",
    install_requires=[
        "requests>=2.28.0",
    ],
)
```

**Legacy: `requirements.txt` (simple, flat)**

```
requests==2.28.0
numpy>=1.21.0,<2.0.0
```

**Key Fields:**
- `dependencies`: Production dependencies with version specs
- `optional-dependencies`: Feature groups (extras)
- `requires-python`: Python version requirement
- `build-backend`: Build system (hatchling, poetry, etc.)

### 7.2 Dependency Resolution Algorithm

**uv (modern, Rust-based):**
- **SAT solver-based** (similar to PubGrub)
- Fast, parallel resolution
- Clear error messages
- Backtracking when needed

**pip (legacy):**
- **Greedy with backtracking**
- Installs latest matching version
- Backtracks if conflict discovered
- Slower than uv

**Poetry:**
- **Similar to pip** but with more metadata
- Pre-computed resolution graph
- Can be slow on large dependency trees

**Process (general):**
1. Parse version constraints
2. Fetch candidate versions from PyPI
3. Build compatibility graph
4. Resolve conflicts (backtrack if needed)
5. Pin exact versions to lockfile

### 7.3 Version Constraint Syntax & Strategy

**PEP 440 Version Specifiers:**

```
requests==2.28.0          # Exact version
requests>=2.28.0          # Minimum version
requests<3.0.0            # Maximum version
requests>=2.28.0,<3.0.0   # Range
requests>=2.28.0,!=2.29.0 # Exclude specific version
requests~=2.28.0          # Compatible release (~= 2.28.x, >= 2.28.0)
requests~=2.28            # ~= 2.x, >= 2.28
requests===2.28.0         # Arbitrary equality (rare)
requests[extra]>=2.0      # With optional dependency
```

**Pre-releases:**
```
requests>=2.28.0a1        # Allow alpha
requests>=2.28.0          # Exclude pre-releases (default)
```

**Strategy:**
- **Ranges common**: Most packages specify ranges
- **Pip upgrades automatically**: `pip install -U` gets latest
- **Minimal semver enforcement**: Ecosystem relies on convention
- **Dev versions**: v1.0.dev0, v1.0a1 common in pre-release

### 7.4 Registry/Distribution Model

**Centralized:**

- **PyPI (Python Package Index)**: Official repository
- **Mirrors**: Warehouse (official CDN), Aliyun, etc.
- **Private indexes**: Can be configured (Artifactory, Nexus, etc.)
- **Package distribution**: Wheels (.whl) and source distributions (.tar.gz)

**Distribution:**
- Binary wheels (pre-compiled, fast install)
- Source distributions (tarball, requires compilation)
- Version metadata and release history
- Hash verification (SHA256)

### 7.5 Lockfile Format & Purpose

**uv: `uv.lock` (custom format, new)**

```
version = 5
requires-python = ">=3.8"

[package]
name = "requests"
version = "2.31.0"
source = { registry = "https://pypi.org/simple" }
requires = []

[[package.sdist]]
url = "https://files.pythonhosted.org/packages/.../requests-2.31.0.tar.gz"
hash = "sha256:abc..."

[[package.wheels]]
url = "..."
hash = "sha256:def..."
```

**pip: `requirements.lock` or `Pipfile.lock` (Poetry)**

Poetry's `Pipfile.lock`:
```json
{
  "_meta": {...},
  "default": {
    "requests": {
      "version": "==2.31.0",
      "hashes": ["sha256:abc..."]
    }
  }
}
```

**Purpose:**
- **Reproducibility**: Exact versions + hashes
- **Integrity**: Prevents tampering
- **Speed**: No resolution needed on install
- **Offline builds**: All metadata included

### 7.6 Workspace/Monorepo Support

**Minimal/Lacking:**

- **No native workspace support** (unlike Rust/Node)
- **Workarounds:**
  - Editable installs: `pip install -e ./packages/lib1`
  - Each package has own pyproject.toml
  - Shared requirements
- **Poetry workspaces** (limited):
  ```toml
  [tool.poetry.group.main.dependencies]
  lib1 = { path = "./packages/lib1", develop = true }
  ```

**Issues:**
- No unified resolution across workspace
- Each package managed independently
- No shared lock file (until uv added some support)

### 7.7 Notable Innovations & Unique Features

1. **uv (Rust-based, modern)**:
   - Fast parallel resolution (100x faster than pip)
   - Clear error messages
   - First-class lock file support
   - Project management (like Poetry)

2. **Optional Dependencies ("extras")**:
   - Packages can define feature groups
   - Install subsets: `pip install requests[security,socks]`
   - Elegant dependency configuration

3. **Wheels as Standard**:
   - Pre-compiled binaries (unlike many languages)
   - Eliminates compilation at install time
   - Platform-specific (cp310-win_amd64, etc.)

4. **Multiple Tools Standardization**:
   - pip (official, minimal)
   - Poetry (all-in-one, popular)
   - uv (modern, fast)
   - No single standard (ecosystem fragmentation)

5. **Virtual Environments**:
   - Isolated Python environments per project
   - `python -m venv` built-in
   - Essential for reproducibility

### 7.8 Known Pain Points & Criticisms

1. **Dependency Resolution Complexity**:
   - pip's greedy approach can miss valid solutions
   - Version conflicts common
   - No clear error messages (until uv)
   - Backtracking slow on large trees

2. **Multiple Tool Fragmentation**:
   - pip vs Poetry vs uv vs pipenv
   - No consensus on best practice
   - Learning curve
   - Migration between tools difficult

3. **Pre-release Handling**: Inconsistent
   - pip excludes pre-releases by default
   - Causes surprises when dependency specifies prerelease
   - Version specifiers can be ambiguous

4. **C Extension Compilation**: Binary distribution issues
   - Packages with C extensions need compilation
   - No wheel available = slow install
   - Platform-specific issues (missing headers, etc.)

5. **Supply Chain Vulnerabilities**:
   - PyPI initially had no access controls
   - Typosquatting attacks common
   - No official verification of package origins
   - Recent improvements (MFA, publishing tokens)

6. **Monorepo Limitations**: Weak support
   - editable installs not perfect
   - No unified lock file strategy
   - Path dependencies don't work well across workspaces

---

## 8. NIX: Package Management (Nixpkgs + Flakes)

### 8.1 Manifest File Format & Key Fields

**File: `flake.nix` (Nix language, lazy evaluation)**

```nix
{
  description = "My project flake";
  
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-23.11";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  
  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlays.default ];
      };
    in {
      devShells.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          rust-bin.stable.latest.default
          cargo
          rustfmt
        ];
      };
      
      packages.default = pkgs.rustPlatform.buildRustPackage {
        name = "my-app";
        src = ./.;
        cargoHash = "sha256:...";
      };
    });
}
```

**Key Fields:**
- `inputs`: Declarative dependencies (flakes)
- `outputs`: Function computing build artifacts
- `description`: Human description
- Build configuration (mkShell, buildRustPackage, etc.)

### 8.2 Dependency Resolution Algorithm

**Algorithm:** **Lazy evaluation with fixed-point semantics**

- **Not traditional resolution**: Uses Nix's functional evaluation
- **Process:**
  1. Evaluate flake inputs (fetch specified commits)
  2. Import nixpkgs (package set at that revision)
  3. Override with overlays
  4. Lazy evaluation (packages computed on-demand)
  5. Fixed-point: recursive package definitions resolved

- **Characteristics:**
  - No version solving (explicit commits/pins)
  - Deterministic (same inputs = same outputs)
  - Powerful but complex (Nix language knowledge required)

### 8.3 Version Constraint Syntax & Strategy

**Git-based Pinning:**

```nix
inputs = {
  nixpkgs.url = "github:nixos/nixpkgs/nixos-23.11";
  nixpkgs-unstable.url = "github:nixos/nixpkgs/master";
  my-package.url = "github:user/package/v1.2.3";
  my-package.inputs.nixpkgs.follows = "nixpkgs"; # Input inheritance
};
```

**Attribute Resolution (no version ranges):**
- Packages selected by attribute path: `pkgs.python310` vs `pkgs.python39`
- No ranges; exact derivation selection
- Breaking changes = different attribute name or API

**Strategy:**
- **Content-addressed**: Outputs identified by hash of inputs
- **No version ranges**: Specific revisions, tags, or branches
- **Input inheritance**: Dependencies of dependencies controlled via `follows`
- **Overrides possible**: Modify packages globally

### 8.4 Registry/Distribution Model

**Mostly Decentralized (Nixpkgs + Flakes):**

- **Nixpkgs**: Central monorepo of 80,000+ packages
  - GitHub-hosted
  - Curated by community
  - One package tree per Nixpkgs revision
  
- **Flakes**: Can reference any Git repository
  - Each project can be a flake (with flake.nix)
  - Can depend on any GitHub repo
  - Namespace control

- **Binary Cache**: Nix caches build outputs
  - Official: cache.nixos.org
  - Can add custom caches
  - Substitution for downloaded binaries

**Distribution:**
- Source-based (Nix builds from source)
- Binary caches for pre-built artifacts
- Content-addressed (same inputs = same artifacts)

### 8.5 Lockfile Format & Purpose

**File: `flake.lock` (JSON)**

```json
{
  "nodes": {
    "root": {
      "inputs": {
        "nixpkgs": "nixpkgs",
        "flake-utils": "flake-utils"
      }
    },
    "nixpkgs": {
      "locked": {
        "owner": "nixos",
        "repo": "nixpkgs",
        "rev": "d0e6b8f...",
        "narHash": "sha256-abc..."
      }
    },
    "flake-utils": {
      "locked": {
        "lastModified": 1680386169,
        "narHash": "sha256-def..."
      }
    }
  },
  "root": "root",
  "version": 7
}
```

**Purpose:**
- **Exact commit pins**: Records exact Git revisions
- **Reproducibility**: Same flake.lock = identical build
- **Integrity**: narHash verifies content
- **Auto-generated**: `nix flake update` updates lockfile
- **Development**: Developers can lock versions without changing flake.nix

### 8.6 Workspace/Monorepo Support

**Flake Modules (newer feature):**

```nix
{
  outputs = { self }: {
    flakeModules.default = {
      perSystem = { pkgs, ... }: {
        packages.lib1 = pkgs.callPackage ./lib1 { };
        packages.lib2 = pkgs.callPackage ./lib2 { };
      };
    };
  };
}
```

**Features:**
- Multiple packages per flake
- Shared dependencies
- Modular organization
- Import flakes from other projects

**Status**: Monorepo support developing but less mature than other languages

### 8.7 Notable Innovations & Unique Features

1. **Content-Addressed Package Management**:
   - Packages identified by hash of inputs
   - Prevents "version confusion" attacks
   - Same inputs guarantees identical outputs

2. **Reproducible Builds by Design**:
   - Entire system describable in Nix
   - No hidden dependencies
   - Can reproduce exact environment later

3. **Flakes (standardized packaging)**:
   - Explicit input/output contracts
   - Version pinning through lock file
   - Can reference any Git repo (no registry needed)

4. **Declarative System Configuration**:
   - Packages, build tools, dev environments all in one file
   - Infrastructure as code
   - NixOS builds entire system from flake

5. **Binary Caches**:
   - Pre-built artifacts by hash
   - Avoids recompilation across machines
   - Substitution model (optional)

6. **Overlays & Overrides**:
   - Package customization without modifying nixpkgs
   - Powerful composition system
   - Multiple versions coexist naturally

### 8.8 Known Pain Points & Criticisms

1. **Steep Learning Curve**:
   - Nix language is functional, lazy, unusual
   - Documentation sparse
   - Requires systems thinking

2. **Build Times**: Often slower than direct compilation
   - Nix can't always optimize
   - Reproducibility comes at performance cost
   - Binary cache helps but not always available

3. **Complexity for Simple Tasks**:
   - Setup overhead for small projects
   - Many options and configurations
   - "Simple" package declaration requires learning Nix

4. **Ecosystem Maturity**:
   - Smaller than mainstream package managers
   - Some packages missing from nixpkgs
   - Maintenance quality varies

5. **Error Messages**: Can be cryptic
   - Lazy evaluation delays error reporting
   - Complex evaluation paths hard to debug
   - Nix language errors not always clear

6. **Disk Space**: Multiple package versions
   - Content-addressed storage can use significant space
   - Garbage collection not always intuitive
   - /nix/store can grow large

---

## 9. UNISON: Package Management

### 9.1 Manifest File Format & Key Fields

**No traditional manifest file**

- Unison uses **Unison Codebase Manager (UCM)**
- Code stored in **content-addressed namespace**
- No package.json, Cargo.toml, etc.
- Metadata stored in codebase itself

**Releases/Publishing:**

```unison
-- In UCM REPL:
.> release 1.0.0
-- Creates immutable snapshot of codebase
```

**Dependencies expressed as:**
```unison
-- Import by content hash
lib.data: type List a = ...
-- Or by stable name
remote.github.org.user.lib.data.List
```

### 9.2 Dependency Resolution Algorithm

**Not applicable in traditional sense**

- **No "resolution" algorithm**: Hashes are direct content addresses
- **Process:**
  1. Code identified by hash (SHA3, content-addressed)
  2. Dependencies expressed as direct hashes or named references
  3. No version solving needed (hashes are immutable)
  4. Namespace provides stable names for hashes

- **Characteristics:**
  - **Zero ambiguity**: Same hash = identical code (guaranteed)
  - **No conflicts**: Dependencies explicitly reference content
  - **Transparent**: All dependencies visible in code

### 9.3 Version Constraint Syntax & Strategy

**No versions in traditional sense**

- **Content-addressed identifiers**:
  ```unison
  -- Dependency by hash
  #abc123def456...
  
  -- Dependency by stable name
  @github/user/lib@1.0.0  -- stable release
  @github/user/lib@latest -- current development
  ```

- **No semver**: Versions are snapshots of code
- **Breaking changes**: New namespace/hash required (not major version bump)
- **No ranges**: All dependencies explicit (by hash or stable name)

**Strategy:**
- **By content**: Every definition has unique hash
- **By release**: Stable releases get named references
- **By reference**: Can depend on latest branch
- **Zero conflicts**: Incompatible versions coexist (different hashes)

### 9.4 Registry/Distribution Model

**Decentralized + Optional Central Registry**

- **Local-first**: Code stored locally in .unison codebase
- **Code sharing**: Published to registry (self-hosted or hosted service)
- **Unison Share** (hosted service, community):
  - Central repository of published code
  - Hash-based identification
  - Anyone can publish

- **Git-based distribution**: Code can be versioned in Git
- **No binary artifacts**: Code is code

### 9.5 Lockfile Format & Purpose

**Codebase itself is immutable**

- No separate lockfile needed
- Code stored in content-addressed store
- Namespaces provide stable pointers

```unison
-- .unison/codebase structure (content-addressed)
abc123def.hash -> code
def456ghi.hash -> code
main.released.1.0.0 -> hash pointer
```

**Purpose:**
- **Immutability**: Content never changes
- **Reproducibility**: Same code base = identical behavior
- **No "version drift"**: Code identified by hash, not mutable versions

### 9.6 Workspace/Monorepo Support

**Built-in to model**

- **Namespace organization**: Hierarchical namespaces
  ```unison
  my-project.lib.data
  my-project.lib.io
  my-project.app.main
  ```

- **Single codebase**: Everything in one content-addressed store
- **No separate packages**: Just namespaces
- **Shared definitions**: Cross-namespace references by hash

### 9.7 Notable Innovations & Unique Features

1. **Content-Addressed Package Model**:
   - Every definition identified by SHA3 hash
   - Prevents "version confusion" and supply chain attacks
   - Same hash guarantees identical code

2. **No Versioning (in traditional sense)**:
   - Versions are snapshots (immutable)
   - Breaking changes create new content, new hash
   - No need for semantic versioning

3. **Codebase as Database**:
   - All code in content-addressed database
   - Rich querying and navigation
   - Refactoring tracked automatically

4. **Namespace-Based Organization**:
   - Hierarchical organization (like packages)
   - Flexible, no global registry required
   - Multiple versions coexist naturally

5. **UCM (Unison Codebase Manager)**:
   - Interactive REPL for code management
   - Add, remove, refactor code with feedback
   - Type-aware operations

### 9.8 Known Pain Points & Criticisms

1. **Early-Stage Project**:
   - Unison language not widely used
   - Limited ecosystem
   - Rapid changes to design

2. **Learning Curve**:
   - Entirely different model from traditional languages
   - Content-addressed thinking unfamiliar
   - UCM REPL requires adjustment

3. **Tooling Immaturity**:
   - IDE support limited
   - Build tools still developing
   - Documentation sparse

4. **Lack of Standard Library**:
   - Smaller standard library than mainstream languages
   - Fewer packages in ecosystem
   - May require writing primitives

5. **Performance Unknown**:
   - Content-addressed lookup overhead
   - Large codebases may be slow
   - Optimization focus still developing

6. **Publication Model Unclear**:
   - How sharing code across projects works still evolving
   - Best practices not established
   - Ecosystem conventions not settled

---

## 10. SWIFT: Swift Package Manager (SPM)

### 10.1 Manifest File Format & Key Fields

**File: `Package.swift` (Swift code as manifest)**

```swift
// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "MyPackage",
    platforms: [
        .macOS(.v10_15),
        .iOS(.v13),
    ],
    products: [
        .library(name: "MyPackage", targets: ["MyPackage"]),
        .executable(name: "my-tool", targets: ["MyTool"]),
    ],
    dependencies: [
        .package(url: "https://github.com/user/lib1.git", from: "1.0.0"),
        .package(url: "https://github.com/user/lib2.git", .upToNextMajor(from: "2.0.0")),
        .package(url: "https://github.com/user/lib3.git", revision: "abc123def"),
        .package(path: "../LocalPackage"),
    ],
    targets: [
        .target(
            name: "MyPackage",
            dependencies: [
                .product(name: "Lib1", package: "lib1"),
                "LocalPackage",
            ]
        ),
        .testTarget(
            name: "MyPackageTests",
            dependencies: ["MyPackage"]
        ),
    ]
)
```

**Key Fields:**
- `name`: Package name
- `products`: Exportable artifacts (libraries, executables)
- `dependencies`: Git-based packages with version constraints
- `targets`: Build targets (code modules)
- `platforms`: OS/version support
- `swift-tools-version`: Minimum SPM version

### 10.2 Dependency Resolution Algorithm

**Algorithm:** **Greedy with version range matching**

- **Similar to npm's older algorithm**
- **Process:**
  1. Parse version constraints
  2. Fetch candidate versions from git tags
  3. Select highest version matching constraint
  4. Recursively resolve transitive dependencies
  5. Detect conflicts (error if incompatible)

- **Characteristics:**
  - **Greedy**: Always picks highest matching version
  - **Range-based**: Version constraints supported
  - **Git-native**: Fetches from git repos directly
  - **No backtracking**: If conflict, error (no retry with older versions)

### 10.3 Version Constraint Syntax & Strategy

**Semantic Versioning with Ranges:**

```swift
.package(url: "...", from: "1.0.0"),           // >= 1.0.0, < 2.0.0
.package(url: "...", .upToNextMajor(from: "1.0.0")), // same as from:
.package(url: "...", .upToNextMinor(from: "1.2.0")), // >= 1.2.0, < 1.3.0
.package(url: "...", "1.0.0"..<"2.0.0"),       // >= 1.0.0, < 2.0.0
.package(url: "...", "1.0.0"..."2.0.0"),       // >= 1.0.0, <= 2.0.0 (rare)
.package(url: "...", exact: "1.2.3"),          // Exact version
.package(url: "...", revision: "abc123"),      // Specific commit
.package(url: "...", branch: "main"),          // Branch (not recommended)
```

**Strategy:**
- **Semantic versioning default**: `from:` assumes semver (caret semantics)
- **Git tags as versions**: Tags must be valid semver
- **Conservative ranges**: `from:` is conservative (up to next major)
- **Exact pinning available**: But not common

### 10.4 Registry/Distribution Model

**Decentralized (Git-based)**

- **No central registry**: Like Go, dependencies are Git URLs
- **Swift Package Index** (community catalog):
  - Searchable index of Swift packages
  - Not required (direct Git URLs work)
  - Metadata aggregation

- **Git repositories**: Primary distribution mechanism
  - GitHub, GitLab, etc. all work
  - Version = Git tags
  - Source code distribution

**Characteristics:**
- **URL-based**: Package path = Git URL
- **No artifact hosting**: Source only
- **Clone-based installation**: Fetches full git repo

### 10.5 Lockfile Format & Purpose

**File: `Package.resolved` (XML or JSON format, newer)**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<object version="1">
  <array key="pins">
    <dict>
      <key>identity</key>
      <string>lib1</string>
      <key>kind</key>
      <string>remoteSourceControl</string>
      <key>location</key>
      <string>https://github.com/user/lib1.git</string>
      <key>state</key>
      <dict>
        <key>branch</key>
        <null/>
        <key>revision</key>
        <string>abc123def456...</string>
        <key>version</key>
        <string>1.2.3</string>
      </dict>
    </dict>
  </array>
</object>
```

**Purpose:**
- **Reproducibility**: Exact git revisions pinned
- **Offline builds**: Can work without network
- **Version resolution tracking**: Records resolved versions
- **Auto-generated**: SPM updates on `swift package update`

### 10.6 Workspace/Monorepo Support

**Limited/None**

- No official multi-package support (unlike Rust workspaces)
- Workarounds:
  - Local path dependencies: `.package(path: "../LocalPackage")`
  - Each package separate manifest
  - No unified lock file

**Best practice**: Monorepos use path dependencies, each sub-package self-contained

### 10.7 Notable Innovations & Unique Features

1. **Swift as Manifest Language**:
   - Package.swift is compilable Swift
   - Type-safe package declarations
   - Enables complex configuration

2. **Git-Based Distribution**:
   - Decentralized by default
   - No central registry required
   - Natural for open-source development

3. **Semantic Versioning by Convention**:
   - Not enforced but strongly encouraged
   - Version ranges leverage semver
   - Whole ecosystem expects semver

4. **Platform Support**:
   - Built-in multi-platform support
   - macOS, iOS, tvOS, watchOS, Linux
   - Platform-specific dependencies

5. **Binary Targets**:
   ```swift
   .binaryTarget(
       name: "MyBinary",
       url: "https://example.com/archive.zip",
       checksum: "abc..."
   )
   ```
   - Can host pre-compiled binaries
   - Avoids compilation overhead

### 10.8 Known Pain Points & Criticisms

1. **Git-Clone Overhead**: All dependencies require git clone
   - Slower than downloading pre-built artifacts
   - Large repositories slow to clone
   - Network-dependent

2. **Limited Version Constraint Options**:
   - No wildcard/pre-release filters
   - Limited expressiveness compared to npm/Cargo
   - Can't easily skip pre-releases

3. **No Central Discovery**:
   - Finding packages requires web search
   - Swift Package Index helps but not official
   - Ecosystem less discoverable than npm/PyPI

4. **Monorepo Support Missing**:
   - Late addition (workspaces, still developing)
   - Path dependencies work but limited
   - Large projects struggle

5. **Fragmentation Across Apple Platforms**:
   - iOS, macOS, tvOS, watchOS all supported
   - Platform-specific code common
   - Testing across platforms difficult

6. **Build Performance**:
   - Swift compilation slow
   - Incremental builds help but rebuilds still expensive
   - Linking large binaries can be slow

---

# Cross-Cutting Comparisons

## 2. Dependency Conflict Resolution

| Language | Conflict Detection | Resolution Strategy | Backtracking | Error Messages |
|----------|-------------------|-------------------|--------------|-----------------|
| **Rust (Cargo)** | PubGrub SAT solver | Max version satisfying all constraints | Yes (SAT) | Excellent - detailed explanations |
| **Go (Modules)** | Minimal Version Selection | Uses maximum required version, single pass | No | Good - shows version graph |
| **Deno** | Import map redirection | No conflicts (explicit URLs) | N/A | N/A - transparent |
| **Roc** | N/A - no package manager | N/A | N/A | N/A |
| **Elm** | Enforced semver validation | Minimal version (exact pins) | N/A | Clear - version mismatch errors |
| **Zig** | N/A - explicit URLs | N/A - no conflicts possible | N/A | N/A - declarative |
| **Python (uv)** | SAT solver (PubGrub) | Max compatible version | Yes (modern) | Excellent (uv) vs poor (pip) |
| **Python (pip)** | Greedy approach | Latest compatible | Limited | Poor - often cryptic |
| **Nix** | Lazy evaluation | Fixed-point resolution | N/A | Varies - can be unclear |
| **Unison** | N/A - content-addressed | N/A - versions coexist | N/A | N/A - hash-based |
| **Swift** | Greedy matching | Highest matching version | No | Moderate - version conflict errors |

**Key Insights:**
- **Modern approaches (Cargo, uv)**: SAT solvers provide clarity
- **Conservative approaches (Go, Elm)**: Minimal version selection avoids surprises
- **Transparent approaches (Deno, Zig, Unison)**: No conflicts by design (explicit URLs/hashes)
- **Legacy (pip)**: Greedy + limited backtracking = frequent failures

---

## 3. Reproducible Builds

| Language | Mechanism | Lockfile Required | Hash-Based | Offline Capable |
|----------|-----------|-------------------|-----------|-----------------|
| **Rust** | Exact versions + checksums in Cargo.lock | Yes (for binaries) | SHA256 checksums | Yes (with vendor/) |
| **Go** | go.mod + go.sum hash verification | go.sum (implicit) | SHA256 (go.sum) | Yes (with vendor/) |
| **Deno** | deno.lock exact URLs + hashes | Yes (deno.lock) | SHA256 hashes | Yes |
| **Roc** | Platform URLs pinned | Implicit | Implicit (URLs) | URL-dependent |
| **Elm** | Exact versions in elm.json | elm.json serves as lock | Package hashes | Yes |
| **Zig** | Hash-based package identity | build.zig.zon serves as lock | SHA256 hashes mandatory | URL-dependent |
| **Python (uv)** | uv.lock with exact versions + hashes | Yes (uv.lock) | SHA256 wheels/sdists | Yes |
| **Python (pip)** | requirements.lock (manual) | Optional | Hash presence varies | Depends on setup |
| **Nix** | flake.lock commits + narHash | Yes (flake.lock) | SHA256 narHash | Yes (with cache) |
| **Unison** | Content-addressed codebase | N/A | SHA3 content hash | Yes (local store) |
| **Swift** | Package.resolved exact revisions | Yes (Package.resolved) | Git SHAs | Limited (requires git) |

**Key Insights:**
- **Hash-based integrity**: Rust, Go, Deno, Zig, Nix all verify content hashes
- **Offline capability**: Most require separate vendoring step
- **Best reproducibility**: Zig (hash mandatory), Unison (content-addressed), Nix (flake.lock + narHash)

---

## 4. Platform/OS-Specific Dependencies

| Language | Mechanism | Expressiveness | Common Patterns |
|----------|-----------|-----------------|-----------------|
| **Rust** | `[target.'cfg(...)'.dependencies]` | Very flexible (cfg expressions) | Windows/Unix, architecture-specific, feature gates |
| **Go** | `//go:build` build tags | Simple compile-time flags | OS, architecture, version tags |
| **Deno** | Conditional imports in code | Manual (no manifest syntax) | Runtime checks, dynamic imports |
| **Roc** | Platform-level (by design) | Handled entirely in platform | All platform specifics in platform code |
| **Elm** | Limited (discouraged) | Not idiomatic | Rare, usually app-level not package |
| **Zig** | Build system integration | In build.zig logic | Conditional compilation in build script |
| **Python** | `markers` in dependencies, `sys.platform` checks | Good (PEP 508 environment markers) | OS, Python version, architecture |
| **Nix** | System-specific package selection | Very flexible (Nix evaluation) | Full system configuration, overlays |
| **Unison** | Code organization only | Namespace separation | Different code modules per platform |
| **Swift** | `platforms[]` and conditional compilation | Good (Swift #if, platform tags) | macOS, iOS, tvOS, watchOS, Linux |

**Key Insights:**
- **Configuration-driven**: Rust (cfg), Python (markers), Swift (platforms)
- **Build-system driven**: Zig (build.zig), Go (build tags)
- **By convention**: Elm (discouraged), Unison (namespace)
- **By design**: Roc (platform abstracts it)

---

## 5. Typical Cold-Start Install Time

| Language | Initial Fetch | Resolution | Download | Extract | Compile | Total | Notes |
|----------|---------------|-----------|----------|---------|---------|-------|-------|
| **Rust** | 5s | 2-10s | 10-30s | 5s | 30-120s+ | 1-3min | Large, compile-heavy; incremental fast |
| **Go** | 5s | 1-3s | 5-15s | 2s | 0s | 15-30s | Very fast; no compilation needed |
| **Deno** | 5s | 0s | 5-20s | 2s | 0s | 10-30s | URL fetching, no build needed |
| **Roc** | 5s | N/A | 5-15s | 2s | Varies | 15s+ | Minimal; depends on platform |
| **Elm** | 5s | 1-3s | 5-15s | 2s | ~1s | 15-30s | Fast; small packages |
| **Zig** | 3s | 0s | 5-15s | 2s | 0s | 10-25s | Very fast; hash verification minimal |
| **Python (pip)** | 2s | 2-5s | 10-30s | 2s | 0-30s* | 20-60s | Wheels = no compile; source = slow |
| **Python (uv)** | 2s | 1-3s | 10-30s | 2s | 0-30s* | 15-50s | Much faster than pip |
| **Nix** | 5s | 1-2s | Vary | 0s | 0-60s* | 10s-3min | Substitutes (pre-built) = fast; compile = slow |
| **Unison** | 1s | 0s | 5-15s | 0s | 0s | 5-20s | Very fast; local content-addressed store |
| **Swift** | 5s | 2-5s | 5-30s* | 2s | 0-120s+ | 20s-2min | Git clone overhead; compilation slow |

* = Conditional (wheels/binaries available, pre-built artifacts)

**Key Insights:**
- **Fastest cold start**: Unison (content-addressed local), Zig (URL hash), Deno (URL fetch)
- **Compilation overhead**: Rust (yes), Swift (yes), Python/C extensions (maybe)
- **Binary wheels matter**: Python with wheels = 20s, from source = 60s+
- **Average modern**: 30-60 seconds for most languages

---

## 6. Security & Supply Chain Concerns

| Language | Central Registry | Verification | Access Control | Notable Incidents |
|----------|-----------------|--------------|-----------------|-------------------|
| **Rust** | crates.io (yes) | Checksum (SHA256) | Maintainer account | Account takeover (2023) - limited impact |
| **Go** | No (module proxy) | go.sum + checksum DB | Source host controls | Dependency confusion (academic) |
| **Deno** | JSR (new) | Content hash | JSR controls | Early; limited track record |
| **Roc** | None | N/A | N/A | New language, not applicable yet |
| **Elm** | package.elm-lang.org | GitHub tag verification | Reviewer approval | Very curated, minimal incidents |
| **Zig** | None | Hash mandatory | Source host controls | Hash-based verification mitigates risk |
| **Python** | PyPI (yes) | Hash (sometimes) | MFA (recent), tokens | Typosquatting, abandoned packages, W1LL incidents |
| **Nix** | nixpkgs (monorepo) | Checksum verification | Community review | Eval attacks possible, usually mitigated |
| **Unison** | Share (optional) | Content-addressed | Unison Share controls | New, limited track record |
| **Swift** | Package Index (registry) | Checksum, but optional | GitHub auth | Limited incidents; source-based model safer |

**Security Measures Across All:**

| Concern | Rust | Go | Deno | Roc | Elm | Zig | Python | Nix | Unison | Swift |
|---------|------|-----|------|-----|-----|-----|--------|-----|--------|-------|
| **Checksum Verification** | ✓ (SHA256) | ✓ (go.sum) | ✓ (hash) | ✗ | ✓ | ✓ (mandatory) | ~ (optional) | ✓ (narHash) | ✓ (SHA3) | ~ (weak) |
| **Typosquatting Protection** | Namespace control | DNS-based | Registry | N/A | Manual review | DNS-based | Limited | Namespace | N/A | GitHub |
| **Version Immutability** | Yank possible | Pseudo-ver | URL versioning | N/A | Git tag | Git tag | Limited | Immutable | Content hash | Git tag |
| **MFA/2FA** | Optional | Optional | JSR TBD | N/A | Manual | Optional | Required (new) | Optional | N/A | GitHub |
| **Vulnerability Database** | advisory.rs | OSV | JSR planned | N/A | Minimal | Minimal | OSV, pip-audit | NVD | N/A | OSV |
| **Provenance Attestation** | Planned | Limited | Planned | N/A | N/A | N/A | Planned | N/A | N/A | N/A |

**Key Risks:**

1. **Typosquatting** (Python > others)
   - PyPI highly vulnerable (no namespacing)
   - Rust/Zig less so (exact URLs required)

2. **Supply Chain Attacks**
   - SLSA provenance tracking emerging (Rust, Python)
   - Content-addressed (Unison, Zig) inherently resistant

3. **Dependency Confusion** (Go, potentially others)
   - Using same module name from different sources
   - Go Module Proxy helps mitigate

4. **Account Compromise**
   - All registries vulnerable
   - MFA adoption improving (PyPI, Swift, Rust)
   - Zig avoids this (no central registry)

5. **Abandoned Packages**
   - Python (minimal maintenance)
   - All registries struggle with this
   - No universal solution

**Best Practices (across all):**
- Verify checksums/hashes
- Monitor dependency trees (tools like OWASP DependencyCheck)
- Use recent versions (security patches)
- Vendoring for critical infrastructure
- Lockfiles essential (immutability)

---

## Summary Comparison Table

| Aspect | Most Innovative | Most Mature | Most Practical | Most Flexible | Most Strict |
|--------|-----------------|------------|-----------------|----------------|------------|
| **Dependency Resolution** | Nix (lazy evaluation) | Python (pip) | Go (MVS) | Rust (PubGrub) | Elm (exact pins) |
| **Reproducibility** | Unison (content-addr) | Rust (checksums) | Nix (flake.lock) | Go (vendoring) | Zig (hash mandatory) |
| **Version Control** | Zig (hash-based) | Rust (semver + checksums) | Go (semver variant) | Deno (URLs) | Elm (enforced semver) |
| **Security** | Zig (decentralized) | Python (centralizing) | Rust (crates.io + checks) | Go (decentralized) | Elm (curated registry) |
| **Monorepo Support** | Nix (flakes) | Rust (native workspaces) | Go (workspaces, Go 1.18+) | Deno (workspaces, new) | Elm (not designed for) |
| **Ecosystem Size** | Python | Python | Rust | Go | Elm |
| **Ease of Use** | Go | Python | Deno | Rust | Elm |

---

# Conclusions

## Key Trends

1. **Content-Addressed Dominance**: Newer systems (Zig, Unison, Nix) embrace content-addressing for security & reproducibility

2. **Decentralization Preference**: Go, Swift, Zig avoid single registry dependency

3. **SAT Solving**: Modern resolution (Rust PubGrub, Python uv) outpaces greedy algorithms

4. **Hash Verification**: Near-universal adoption for integrity verification

5. **Lockfile Standardization**: All systems converging on explicit version/hash pinning

## Language Positioning

- **Safe & Reproducible**: Rust, Elm, Nix, Unison
- **Fast & Practical**: Go, Deno, Zig, Python (uv)
- **Simple & Minimal**: Deno, Zig, Go
- **Powerful & Complex**: Nix, Rust, Swift
- **Niche/Specialized**: Roc (platform-based), Unison (content-addressed)

This research demonstrates that modern package management is converging on several principles:
1. Explicit version/content pinning
2. Hash-based integrity verification
3. Reproducible builds as default
4. Decentralized or hybrid registries
5. Clear error messages on conflicts

The most successful systems (Rust, Go) balance ecosystem size with strong tooling and clear semantics.