# Spore Package Management Specification v0.1

**版本**: 0.1.0
**状态**: Draft
**日期**: 2024-01

---

## 1. 概述 (Overview)

Spore 包管理系统采用**内容寻址** (content-addressed) 模型,通过模块的接口签名和实现内容的双重哈希来唯一标识依赖。本规范定义了 Spore 生态中模块的声明、解析、获取、存储和版本管理机制。

### 1.1 设计哲学 (Design Philosophy)

- **确定性构建** (Deterministic Builds): 相同的源代码和依赖哈希保证相同的构建结果
- **去中心化** (Decentralized): 无中心 registry,任何 Git 仓库或本地路径均可作为模块源
- **细粒度缓存** (Fine-grained Caching): 模块级别的内容寻址,接口变更不影响未依赖接口的模块
- **透明性** (Transparency): 所有依赖关系通过哈希显式声明,不依赖隐式版本协商

### 1.2 核心概念 (Core Concepts)

#### 1.2.1 模块标识 (Module Identity)

每个 Spore 模块由**双重哈希** (dual hash) 唯一标识:

```
<module-name>@sig:<signature-hash>+impl:<implementation-hash>
```

- **signature hash**: 模块公开接口 (public API) 的哈希,仅包含类型签名、函数声明等
- **implementation hash**: 完整实现内容的哈希,包括私有函数、实现逻辑等

**示例**:
```
mylib@sig:a3f9c2e1+impl:7b8d4f2a
```

#### 1.2.2 依赖类型 (Dependency Types)

- **接口依赖** (signature dependency): 仅依赖模块的公开接口,使用 `sig:` 哈希
- **完整依赖** (full dependency): 依赖完整实现,使用 `sig:+impl:` 双哈希
- **开发依赖** (dev dependency): 仅在测试/构建时需要
- **可选依赖** (optional dependency): 由 feature flag 控制

### 1.3 与传统包管理的差异 (Differences from Traditional Package Management)

| 特性 | 传统包管理 (npm, cargo) | Spore |
|------|------------------------|-------|
| 版本标识 | Semantic versioning | Content hash |
| 依赖解析 | 范围协商 (^1.2.3) | 精确哈希 |
| Registry | 中心化 (npmjs.com) | 去中心化 (Git/local) |
| Diamond 依赖 | 单版本共享 | 多哈希共存 |
| 缓存粒度 | 包版本 | 模块内容 |

---

## 2. spore.toml 格式 (spore.toml Format)

`spore.toml` 是项目清单文件,声明模块元数据和依赖关系。

### 2.1 完整示例 (Complete Example)

```toml
[package]
name = "web-server"
version = "0.2.3"  # 人类可读版本号,非强制
authors = ["Alice <alice@example.com>"]
description = "高性能 HTTP 服务器"
license = "MIT"
repository = "https://github.com/alice/web-server"

# 可选: 指定 Spore 编译器版本
spore-version = ">=0.5.0"

[dependencies]
# 使用命名别名 (named alias)
http = { alias = "std-http-v2", sig = "a3f9c2e1", impl = "7b8d4f2a" }
json = { alias = "fast-json", sig = "b4e7d3c2" }  # 仅接口依赖

# Git 源
logger = { git = "https://github.com/logger/logger", sig = "c9a8b7e1", impl = "2f3e4d5c" }

# 本地路径 (开发阶段)
utils = { path = "../utils", sig = "d1c2b3a4", impl = "8e9f0a1b" }

# 条件依赖
metrics = { alias = "prometheus", sig = "e2f3a4b5", optional = true }

[dev-dependencies]
test-framework = { alias = "spore-test", sig = "f3e4d5c6", impl = "1a2b3c4d" }
benchmark = { git = "https://github.com/perf/bench", sig = "a1b2c3d4" }

[features]
default = ["tls"]
tls = ["dep:tls-lib"]
metrics = ["dep:metrics"]
all = ["tls", "metrics"]

# 依赖覆盖 (用于 monorepo 或测试场景)
[overrides]
# 强制所有依赖使用特定版本
"http" = { sig = "a3f9c2e1", impl = "OVERRIDE" }

[build]
# 构建脚本
script = "build.spore"

[metadata]
# 自定义元数据,不影响构建
tags = ["http", "server", "async"]
stability = "experimental"
```

### 2.2 字段说明 (Field Descriptions)

#### 2.2.1 [package] 节

| 字段 | 类型 | 必需 | 说明 |
|------|------|------|------|
| `name` | String | ✓ | 模块名称,用于本地引用 |
| `version` | String | ✗ | 人类可读版本号,不参与解析 |
| `authors` | Array | ✗ | 作者列表 |
| `description` | String | ✗ | 模块描述 |
| `license` | String | ✗ | 许可证标识 |
| `repository` | String | ✗ | 源代码仓库 URL |
| `spore-version` | String | ✗ | 要求的 Spore 编译器版本 |

#### 2.2.2 依赖声明 (Dependency Declaration)

**Git 源**:
```toml
[dependencies]
mylib = {
    git = "https://github.com/user/repo",
    branch = "main",  # 可选: 分支
    tag = "v1.2.3",   # 可选: tag
    rev = "abc123",   # 可选: commit hash
    sig = "a1b2c3d4",
    impl = "e5f6a7b8"
}
```

**本地路径**:
```toml
[dependencies]
mylib = {
    path = "../mylib",
    sig = "a1b2c3d4",
    impl = "e5f6a7b8"
}
```

**命名别名** (Named Alias):
```toml
[dependencies]
# alias 用于在锁文件中引用已知的"发布版本"
mylib = {
    alias = "mylib-stable-2023",
    sig = "a1b2c3d4",
    impl = "e5f6a7b8"
}
```

**仅接口依赖**:
```toml
[dependencies]
# 省略 impl 表示仅需要接口
mylib = { sig = "a1b2c3d4" }
```

#### 2.2.3 Features

```toml
[features]
# default feature 自动启用
default = ["logging"]

# feature 可以启用其他 features
full = ["logging", "metrics", "tls"]

# feature 可以启用可选依赖
logging = ["dep:logger"]

# feature 可以影响编译配置
tls = []  # 由代码中的 #[cfg(feature = "tls")] 控制
```

### 2.3 Workspace 支持 (Workspace Support)

Spore **不强制** workspace 概念,但支持通过 `path` 依赖实现 monorepo:

```toml
# 项目 A 的 spore.toml
[package]
name = "project-a"

[dependencies]
shared = { path = "../shared", sig = "...", impl = "..." }

# 项目 B 的 spore.toml
[package]
name = "project-b"

[dependencies]
shared = { path = "../shared", sig = "...", impl = "..." }
```

不需要顶层 workspace.toml,每个项目独立解析依赖。

---

## 3. .spore-lock 格式 (.spore-lock Format)

`.spore-lock` 记录依赖解析后的完整依赖图和获取位置,确保可重现构建。

### 3.1 完整示例 (Complete Example)

```toml
# 此文件由 spore lock 自动生成,不应手动编辑
version = 1

# 锁文件元数据
[metadata]
generated-at = "2024-01-15T10:30:00Z"
spore-version = "0.5.2"

# 根模块
[[package]]
name = "web-server"
source = "local"
sig = "1a2b3c4d5e6f7a8b"
impl = "9c8d7e6f5a4b3c2d"

# 直接依赖
[[package]]
name = "http"
alias = "std-http-v2"
source = { type = "git", url = "https://github.com/std/http", rev = "abc1234" }
sig = "a3f9c2e1d8b7a6f5"
impl = "7b8d4f2a1e9c3d5b"
location = ".spore-store/git/github.com/std/http/abc1234"

# 接口依赖的解析
[[package]]
name = "json"
alias = "fast-json"
source = { type = "registry", url = "https://pkgs.example.com/json" }
sig = "b4e7d3c2a1f9e8d7"
impl = null  # 仅接口依赖,未获取实现
location = ".spore-store/sigs/b4e7d3c2a1f9e8d7"

# 传递依赖
[[package]]
name = "io"
source = { type = "git", url = "https://github.com/std/io", rev = "def5678" }
sig = "c5d6e7f8a9b0c1d2"
impl = "e3f4a5b6c7d8e9f0"
location = ".spore-store/git/github.com/std/io/def5678"

# 依赖图
[dependency-graph]
# 依赖关系: 包名 -> 依赖列表
"web-server" = ["http", "json"]
"http" = ["io"]
"json" = []
"io" = []

# 哈希验证表
[verification]
# 包名 -> 完整性校验信息
"http" = {
    sig-algo = "blake3",
    sig-hash = "a3f9c2e1d8b7a6f5",
    impl-algo = "blake3",
    impl-hash = "7b8d4f2a1e9c3d5b",
    verified-at = "2024-01-15T10:30:05Z"
}
"json" = {
    sig-algo = "blake3",
    sig-hash = "b4e7d3c2a1f9e8d7",
    impl-algo = null,
    impl-hash = null,
    verified-at = "2024-01-15T10:30:06Z"
}

# 能力记录
[capabilities]
"http" = ["network:listen", "filesystem:read"]
"io" = ["filesystem:read", "filesystem:write"]

# 平台特定锁
[[platform-locks]]
platform = "linux-x86_64"
packages = ["http", "io", "json"]

[[platform-locks]]
platform = "wasm32"
packages = ["json"]  # http 不支持 wasm
```

### 3.2 字段说明 (Field Descriptions)

#### 3.2.1 Package 条目

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | String | 模块名称 |
| `alias` | String | 命名别名 (可选) |
| `source` | Object | 源位置信息 |
| `sig` | String | 签名哈希 |
| `impl` | String/null | 实现哈希,null 表示仅接口依赖 |
| `location` | String | 本地存储路径 |

#### 3.2.2 Source 类型

```toml
# Git 源
source = { type = "git", url = "https://...", rev = "abc123" }

# 本地路径
source = { type = "path", path = "../mylib" }

# Registry (可插拔后端)
source = { type = "registry", url = "https://pkgs.example.com" }

# IPFS
source = { type = "ipfs", cid = "Qm..." }
```

---

## 4. 内容寻址模型 (Content-Addressed Model)

### 4.1 哈希计算 (Hash Computation)

#### 4.1.1 签名哈希 (Signature Hash)

签名哈希仅包含公开接口的规范化表示:

```
sig_hash = BLAKE3(canonicalize(
    module_name,
    public_functions,
    public_types,
    public_constants,
    doc_comments  // 可选,由配置控制
))
```

**示例**: 假设模块 `math.spore`

```spore
// 公开接口
pub fn add(a: Int, b: Int) -> Int
pub fn sub(a: Int, b: Int) -> Int
pub type Vector = { x: Float, y: Float }

// 私有实现
fn internal_helper() { ... }
```

签名哈希计算输入:
```
module: math
pub fn add(a: Int, b: Int) -> Int
pub fn sub(a: Int, b: Int) -> Int
pub type Vector = { x: Float, y: Float }
```

**规范化规则**:
- 移除所有空白字符
- 按字母序排序声明
- 移除注释 (除非配置保留 doc comments)
- 类型别名展开

#### 4.1.2 实现哈希 (Implementation Hash)

实现哈希包含完整模块内容:

```
impl_hash = BLAKE3(
    module_source_code,
    dependency_sigs,  // 依赖的签名哈希
    compiler_version
)
```

**示例**:
```
math.spore 完整源代码 (500 lines)
+ dependency http@sig:a3f9c2e1
+ dependency json@sig:b4e7d3c2
+ compiler spore-0.5.2
= impl_hash: 7b8d4f2a1e9c3d5b
```

### 4.2 哈希算法选择 (Hash Algorithm)

默认使用 **BLAKE3**:
- 速度: 比 SHA-256 快 10x+
- 安全性: 128-bit 安全级别
- 并行性: 原生支持多线程
- 前缀一致: 支持增量哈希和流式处理

哈希输出截断为 **64-bit 十六进制** (256-bit),示例: `a3f9c2e1d8b7a6f5`

### 4.3 冲突处理 (Collision Handling)

理论上 256-bit 哈希空间冲突概率极低 (< 2^-128),但仍需处理:

1. **检测**: 在 fetch 时验证哈希与实际内容
2. **拒绝**: 发现冲突立即报错,不自动降级
3. **记录**: 在 `.spore-lock` 中记录完整性校验时间戳

```toml
[verification]
"suspicious-pkg" = {
    sig-hash = "a3f9...",
    collision-detected = true,
    error = "Hash collision detected, refusing to use"
}
```

---

## 5. 依赖解析 (Dependency Resolution)

### 5.1 解析算法 (Resolution Algorithm)

Spore 依赖解析**不需要 SAT solver**,因为不存在版本范围协商:

```
1. 读取 spore.toml 的 [dependencies]
2. 对每个依赖:
   a. 检查 .spore-lock 是否已有精确哈希
   b. 如果存在,验证本地缓存
   c. 如果不存在,从 source 获取并计算哈希
3. 递归解析传递依赖
4. 构建依赖图 (允许同一模块的不同哈希共存)
5. 写入 .spore-lock
```

**伪代码**:
```python
def resolve_dependencies(manifest: SporeToml) -> DepGraph:
    graph = DepGraph()
    queue = [(manifest.package, manifest.dependencies)]

    // NOTE: Algorithm pseudocode — Spore has no loop constructs.
    // Recursive resolution: process queue until empty
    fn resolve_next(queue, graph):
        match queue:
            [] => graph
            [(pkg, deps), ...rest] =>
                let (new_graph, new_queue) = deps |> fold((graph, rest), fn((g, q), dep) {
                    match dep.sig in g:
                        true => (g, q)
                        false =>
                            let source = fetch_source(dep.source)
                            verify_hash(source, dep.sig, dep.impl)
                            (g.add(dep), q ++ [(dep, source.dependencies)])
                })
                resolve_next(new_queue, new_graph)

    resolve_next(queue, graph)
```

### 5.2 Diamond 依赖 (Diamond Dependencies)

允许**多哈希共存**:

```
       A
      / \
     B1  C1
      \ /
       D

B1 依赖 D@sig:aaa+impl:bbb
C1 依赖 D@sig:aaa+impl:ccc  # 不同实现哈希
```

**解析结果**: 两个 D 的版本同时存在于依赖图:
- `D@sig:aaa+impl:bbb` (被 B1 使用)
- `D@sig:aaa+impl:ccc` (被 C1 使用)

**链接时处理**:
- 如果 A 仅依赖 D 的接口 (sig),选择任一实现
- 如果 A 依赖 D 的完整实现,编译器报错,要求显式选择

### 5.3 循环依赖 (Cyclic Dependencies)

Spore **禁止循环依赖**:

```toml
# A 依赖 B
[dependencies]
b = { sig = "..." }

# B 依赖 A ❌ 编译错误
[dependencies]
a = { sig = "..." }
```

检测算法: 标准拓扑排序,发现环立即报错。

### 5.4 接口依赖优化 (Signature Dependency Optimization)

仅接口依赖时,跳过实现获取:

```toml
[dependencies]
# 仅需要类型定义
types = { sig = "a1b2c3d4" }  # 不需要 impl
```

**编译流程**:
1. 获取 `types` 的接口定义 (可能是 .spore-sig 文件)
2. 类型检查时使用接口信息
3. 链接时不包含 `types` 的实现代码

**好处**:
- 减少不必要的代码下载
- 缩短编译时间
- 接口变更时依赖模块无需重新编译

---

## 6. 存储后端 (Storage Backend)

### 6.1 本地存储结构 (Local Storage Structure)

```
.spore-store/
├── sigs/                    # 签名缓存
│   ├── a3f9c2e1/
│   │   └── interface.spore-sig
│   └── b4e7d3c2/
│       └── interface.spore-sig
├── impls/                   # 实现缓存
│   ├── 7b8d4f2a/
│   │   └── module.spore
│   └── 2f3e4d5c/
│       └── module.spore
├── git/                     # Git 源缓存
│   └── github.com/
│       └── user/
│           └── repo/
│               └── abc1234/
│                   └── src/
├── ipfs/                    # IPFS 缓存
│   └── Qm.../
└── metadata/                # 元数据
    └── index.db             # SQLite 索引
```

### 6.2 后端接口 (Backend Interface)

Spore 存储后端可插拔,实现以下 trait:

```spore
pub trait StorageBackend {
    fn fetch_sig(hash: Hash) -> Result<Signature>
    fn fetch_impl(hash: Hash) -> Result<Module>
    fn store_sig(sig: Signature) -> Hash
    fn store_impl(module: Module) -> Hash
    fn list_cached() -> Vec<Hash>
    fn gc() -> Result<()>  // 垃圾回收
}
```

### 6.3 后端实现 (Backend Implementations)

#### 6.3.1 Git Backend

```toml
[dependencies]
mylib = {
    git = "https://github.com/user/repo",
    rev = "abc123",
    sig = "...",
    impl = "..."
}
```

**获取流程**:
1. `git clone --bare https://github.com/user/repo .spore-store/git/github.com/user/repo`
2. `git worktree add .spore-store/git/github.com/user/repo/abc123 abc123`
3. 计算哈希并验证
4. 符号链接到 `.spore-store/impls/<hash>/`

#### 6.3.2 Local Path Backend

```toml
[dependencies]
mylib = { path = "../mylib", sig = "...", impl = "..." }
```

**获取流程**:
1. 读取 `../mylib/src/`
2. 计算哈希
3. 验证与声明的哈希匹配
4. **不复制**到 `.spore-store`,直接引用原路径

#### 6.3.3 IPFS Backend

```toml
[dependencies]
mylib = {
    ipfs = "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG",
    sig = "...",
    impl = "..."
}
```

**获取流程**:
1. `ipfs get QmYw... -o .spore-store/ipfs/QmYw...`
2. 计算哈希并验证
3. 符号链接到 `.spore-store/impls/<hash>/`

#### 6.3.4 HTTP Registry Backend

```toml
[dependencies]
mylib = {
    registry = "https://pkgs.example.com",
    alias = "mylib-v2",
    sig = "...",
    impl = "..."
}
```

**Registry API**:
```
GET /modules/<alias>/sig/<sig-hash>
GET /modules/<alias>/impl/<impl-hash>
GET /modules/<alias>/latest  # 返回最新哈希
```

### 6.4 缓存与垃圾回收 (Cache and GC)

**缓存策略**:
- 所有获取的模块缓存到 `.spore-store/`
- 全局共享: 所有项目共用同一 `.spore-store/` (默认 `~/.spore-store`)
- 内容寻址: 相同哈希的模块只存储一次

**垃圾回收**:
```bash
# 清理未被任何锁文件引用的缓存
spore gc

# 强制清理所有缓存
spore gc --all
```

**GC 算法**:
1. 扫描所有 `.spore-lock` 文件
2. 标记被引用的哈希
3. 删除未标记的缓存

---

## 7. 能力封顶 (Capability Ceiling)

Spore 当前明确的是**项目/包级能力系统** (package/project capability system)。若未来需要模块级 carrier，将另行设计（TBD）:

### 7.1 能力声明 (Capability Declaration)

模块必须在 `spore.toml` 中声明所需能力:

```toml
[package]
name = "web-server"

[capabilities]
require = [
    "network:listen:8080",     # 监听端口 8080
    "filesystem:read:/data",   # 读取 /data 目录
    "filesystem:write:/logs",  # 写入 /logs 目录
    "env:read:API_KEY",        # 读取环境变量 API_KEY
]

# 可选: 声明能力的用途
[capabilities.descriptions]
"network:listen:8080" = "HTTP 服务器监听端口"
"filesystem:read:/data" = "读取配置文件"
```

### 7.2 能力类别 (Capability Categories)

```
network:
  - listen:<port>
  - connect:<host>:<port>
  - http:client

filesystem:
  - read:<path>
  - write:<path>
  - delete:<path>

env:
  - read:<var>
  - write:<var>

process:
  - spawn:<cmd>
  - signal:<pid>

ipc:
  - unix-socket:<path>

time:
  - realtime
  - monotonic

random:
  - secure
  - fast
```

### 7.3 能力传播 (Capability Propagation)

依赖的能力**不自动传播**,必须显式声明:

```toml
# A 依赖 B,B 需要 network:listen
[dependencies]
b = { sig = "...", impl = "..." }

# A 必须显式授权 B 的能力
[capabilities]
grant = [
    "b:network:listen:8080"  # 授权 b 模块监听 8080
]
```

### 7.4 能力审计 (Capability Audit)

```bash
# 审计项目的能力使用
spore audit capabilities

# 输出示例:
# web-server requires:
#   - network:listen:8080 ✓ granted
#   - filesystem:read:/data ✓ granted
#
# web-server dependencies:
#   - http requires:
#     - network:connect:* ✗ NOT GRANTED
#     - suggestion: add [capabilities.grant] "http:network:connect:*"
```

### 7.5 运行时封顶 (Runtime Ceiling)

编译时生成能力清单,运行时强制执行:

```bash
# 运行时传入允许的能力
spore run --allow network:listen:8080 --allow filesystem:read:/data

# 或使用配置文件
spore run --caps-from caps.toml
```

**caps.toml**:
```toml
[allow]
network = ["listen:8080"]
filesystem = { read = ["/data"], write = ["/logs"] }
env = { read = ["API_KEY"] }
```

---

## 8. 发布与发现 (Publishing and Discovery)

### 8.1 发布流程 (Publishing Workflow)

Spore **无中心 registry**,发布即推送到 Git 仓库:

```bash
# 1. 确保代码已提交
git add .
git commit -m "Release v0.2.0"

# 2. 打 tag (可选,人类可读)
git tag v0.2.0

# 3. 推送到远程
git push origin main --tags

# 4. 记录哈希
spore hash --output release-notes.md

# release-notes.md 内容:
# mylib v0.2.0
# sig: a3f9c2e1d8b7a6f5
# impl: 7b8d4f2a1e9c3d5b
#
# Dependencies:
#   http@sig:b4e7d3c2+impl:2f3e4d5c
#   json@sig:c5d6e7f8
```

### 8.2 发现机制 (Discovery Mechanisms)

#### 8.2.1 命名别名 (Named Aliases)

发布者在 README 或文档中声明稳定别名:

```markdown
# mylib

## Installation

Add to your `spore.toml`:

​```toml
[dependencies]
mylib = {
    git = "https://github.com/user/mylib",
    alias = "mylib-stable-2024",
    sig = "a3f9c2e1d8b7a6f5",
    impl = "7b8d4f2a1e9c3d5b"
}
​```
```

#### 8.2.2 Registry 聚合 (Registry Aggregation)

社区可运行可选的 registry 服务器,索引 Git 仓库:

```bash
# 查询 registry
spore search "http server"

# 输出:
# - web-server (https://github.com/alice/web-server)
#   alias: web-server-v2
#   sig: a3f9c2e1, impl: 7b8d4f2a
#   description: 高性能 HTTP 服务器
```

**Registry 不存储代码**,仅提供搜索和元数据:

```json
{
  "name": "web-server",
  "source": "https://github.com/alice/web-server",
  "aliases": {
    "web-server-v2": {
      "sig": "a3f9c2e1d8b7a6f5",
      "impl": "7b8d4f2a1e9c3d5b",
      "published": "2024-01-15T10:00:00Z"
    }
  }
}
```

#### 8.2.3 Awesome 列表

社区维护 `awesome-spore` 仓库:

```markdown
# Awesome Spore

## Web Frameworks
- **web-server**: https://github.com/alice/web-server
  - alias: `web-server-v2`
  - sig: `a3f9c2e1`, impl: `7b8d4f2a`

## JSON Libraries
- **fast-json**: https://github.com/json/fast
  - alias: `fast-json-2024`
  - sig: `b4e7d3c2`, impl: `2f3e4d5c`
```

---

## 9. 迁移与兼容性 (Migration and Compatibility)

### 9.1 从语义版本迁移 (Migrating from Semver)

假设现有项目使用语义版本:

**之前 (package.json)**:
```json
{
  "dependencies": {
    "lodash": "^4.17.21",
    "express": "~4.18.0"
  }
}
```

**之后 (spore.toml)**:
```toml
[dependencies]
lodash = {
    git = "https://github.com/lodash/lodash",
    tag = "4.17.21",  # 对应 Git tag
    alias = "lodash-4.17",
    sig = "...",  # 运行 spore hash 计算
    impl = "..."
}
express = {
    git = "https://github.com/expressjs/express",
    tag = "4.18.0",
    alias = "express-4.18",
    sig = "...",
    impl = "..."
}
```

**迁移工具**:
```bash
# 自动将 package.json 转换为 spore.toml
spore migrate --from package.json --to spore.toml

# 工具会:
# 1. 读取 package.json
# 2. 查找对应的 Git 仓库
# 3. 根据版本号找到 Git tag
# 4. 计算哈希
# 5. 生成 spore.toml
```

### 9.2 兼容性策略 (Compatibility Strategy)

**向后兼容**:
- `.spore-lock` 版本号确保向后兼容读取
- 新增字段不影响旧版本工具

**向前兼容**:
- 旧工具遇到未知字段发出警告但继续
- 关键字段变更通过版本号隔离

### 9.3 破坏性变更 (Breaking Changes)

需要破坏性变更时:
1. 递增 `.spore-lock` 的 `version` 字段
2. 旧工具拒绝处理新版本锁文件
3. 提供迁移脚本

```bash
# 升级锁文件格式
spore lock upgrade --from 1 --to 2
```

---

## 10. CLI 命令 (CLI Commands)

### 10.1 spore init

**功能**: 初始化新项目

```bash
spore init [name]

# 交互式
spore init
# > Project name: my-project
# > Author: Alice <alice@example.com>
# > License: MIT

# 带参数
spore init my-project --author "Alice" --license MIT
```

**生成的 spore.toml**:
```toml
[package]
name = "my-project"
version = "0.1.0"
authors = ["Alice <alice@example.com>"]
license = "MIT"

[dependencies]
```

### 10.2 spore add

**功能**: 添加依赖

```bash
# Git 源
spore add https://github.com/user/repo

# 自动计算哈希并更新 spore.toml:
# [dependencies]
# repo = { git = "https://github.com/user/repo", sig = "...", impl = "..." }

# 指定别名
spore add https://github.com/user/repo --alias my-lib

# 指定 tag/branch
spore add https://github.com/user/repo --tag v1.2.3

# 仅接口依赖
spore add https://github.com/user/repo --sig-only

# 本地路径
spore add ../mylib

# 开发依赖
spore add https://github.com/test/framework --dev

# 可选依赖
spore add https://github.com/metrics/prom --optional --feature metrics
```

**自动哈希推断**:
```bash
# 如果未指定哈希,spore add 会:
# 1. 克隆仓库
# 2. 计算签名和实现哈希
# 3. 写入 spore.toml
# 4. 运行 spore lock 更新锁文件
```

### 10.3 spore remove

**功能**: 移除依赖

```bash
spore remove mylib

# 移除后:
# 1. 从 spore.toml 删除依赖声明
# 2. 运行 spore lock 更新锁文件
# 3. 可选: 运行 spore gc 清理缓存
```

### 10.4 spore update

**功能**: 更新依赖

```bash
# 更新所有依赖到最新 commit
spore update

# 更新特定依赖
spore update mylib

# 更新到特定 tag
spore update mylib --tag v2.0.0

# 仅更新哈希 (适用于 path 依赖)
spore update mylib --recompute-hash
```

**更新流程**:
1. 获取最新代码 (Git pull, 或重新读取 path)
2. 重新计算哈希
3. 更新 `spore.toml` 中的 `sig` 和 `impl`
4. 运行 `spore lock` 更新锁文件

### 10.5 spore lock

**功能**: 生成或更新 `.spore-lock`

```bash
# 根据 spore.toml 生成锁文件
spore lock

# 仅验证锁文件有效性,不更新
spore lock --verify

# 更新锁文件但不获取代码
spore lock --no-fetch
```

### 10.6 spore fetch

**功能**: 获取依赖到本地缓存

```bash
# 根据 .spore-lock 获取所有依赖
spore fetch

# 获取特定依赖
spore fetch mylib

# 并行获取 (默认)
spore fetch --jobs 8

# 仅获取签名 (用于类型检查)
spore fetch --sigs-only
```

### 10.7 spore deps

**功能**: 显示依赖树

```bash
# 显示依赖树
spore deps

# 输出:
# my-project
# ├── http@sig:a3f9c2e1+impl:7b8d4f2a
# │   └── io@sig:c5d6e7f8+impl:e3f4a5b6
# ├── json@sig:b4e7d3c2 (sig-only)
# └── logger@sig:c9a8b7e1+impl:2f3e4d5c

# 显示详细信息
spore deps --verbose

# 输出:
# http@sig:a3f9c2e1+impl:7b8d4f2a
#   source: git+https://github.com/std/http@abc1234
#   location: .spore-store/git/github.com/std/http/abc1234
#   capabilities: network:listen, network:connect

# 仅显示直接依赖
spore deps --depth 1

# 输出 JSON
spore deps --json > deps.json
```

### 10.8 spore audit

**功能**: 审计项目

```bash
# 审计能力使用
spore audit capabilities

# 审计哈希完整性
spore audit hashes

# 审计依赖循环
spore audit cycles

# 审计未使用依赖
spore audit unused

# 全面审计
spore audit --all
```

**输出示例**:
```
🔍 Auditing capabilities...
✓ web-server: network:listen:8080 (granted)
✗ http: network:connect:* (NOT GRANTED)
  suggestion: add [capabilities.grant] "http:network:connect:*"

🔍 Auditing hashes...
✓ http@sig:a3f9c2e1+impl:7b8d4f2a (verified)
✓ json@sig:b4e7d3c2 (verified)

🔍 Auditing cycles...
✓ No cyclic dependencies

🔍 Auditing unused dependencies...
⚠ logger is declared but never imported
  suggestion: run `spore remove logger` or add import
```

### 10.9 spore hash

**功能**: 计算模块哈希

```bash
# 计算当前模块的哈希
spore hash

# 输出:
# sig: a3f9c2e1d8b7a6f5
# impl: 7b8d4f2a1e9c3d5b

# 计算指定模块
spore hash --module ../mylib

# 仅计算签名哈希
spore hash --sig-only

# 输出到文件
spore hash --output HASH.txt
```

### 10.10 自动推断与修复 (Auto-inference and Fixes)

**--fixes 标志**:

```bash
# 自动修复常见问题
spore check --fixes

# 会自动:
# 1. 推断缺失的哈希
# 2. 移除未使用的依赖
# 3. 添加缺失的能力声明
# 4. 修复格式问题
```

**示例**:

```toml
# spore.toml (before)
[dependencies]
http = { git = "https://github.com/std/http" }  # 缺失哈希

# 运行: spore check --fixes

# spore.toml (after)
[dependencies]
http = {
    git = "https://github.com/std/http",
    sig = "a3f9c2e1d8b7a6f5",  # ✓ 自动推断
    impl = "7b8d4f2a1e9c3d5b"  # ✓ 自动推断
}
```

---

## 11. 设计决策记录 (Design Decision Records)

### 11.1 决策 1: 无中心 Registry

**决策**: Spore 不运营中心 registry,使用 Git 等去中心化存储。

**理由**:
- **避免单点故障**: 中心 registry 宕机影响全生态
- **降低审查风险**: 无中心机构可删除包
- **减少运营成本**: 无需维护大规模存储和带宽
- **利用现有基础设施**: Git 已广泛部署

**缺点**:
- 发现困难: 需依赖搜索引擎或社区索引
- 命名冲突: 无全局命名空间

**缓解措施**:
- 社区运营可选 registry 服务 (仅索引,不存储)
- 使用完整 URL 作为真实标识符

### 11.2 决策 2: 模块级粒度

**决策**: 以模块 (单文件或目录) 为依赖单位,非整个仓库。

**理由**:
- **细粒度缓存**: 仅重新获取变更的模块
- **减少依赖膨胀**: 避免引入不需要的代码
- **提升构建速度**: 仅编译必要的模块

**实现**:
```
仓库: https://github.com/user/repo
模块: repo/http/server.spore
      repo/http/client.spore
      repo/json/parser.spore

依赖声明:
[dependencies]
http-server = {
    git = "https://github.com/user/repo",
    module = "http/server",
    sig = "...",
    impl = "..."
}
```

### 11.3 决策 3: 本地 .spore-store + 可插拔后端

**决策**: 默认本地缓存,但支持自定义后端 (IPFS, S3 等)。

**理由**:
- **离线开发**: 本地缓存确保离线可工作
- **全局共享**: 多项目共享缓存,减少磁盘占用
- **灵活性**: 团队可部署自定义缓存服务

**配置示例**:
```toml
# ~/.spore/config.toml
[storage]
backend = "local"
path = "~/.spore-store"

# 可选: 使用团队缓存服务
[[storage.mirrors]]
type = "http"
url = "https://cache.example.com"
priority = 1

[[storage.mirrors]]
type = "ipfs"
gateway = "https://ipfs.io"
priority = 2
```

### 11.4 决策 4: 无语义版本

**决策**: 使用内容哈希代替语义版本。

**理由**:
- **消除歧义**: 哈希唯一对应代码,无解释空间
- **可重现性**: 相同哈希保证相同代码
- **简化解析**: 无需 SAT solver

**人类可读性缓解**:
- 保留可选的 `version` 字段用于文档
- 使用命名别名 (alias) 提供稳定标识
- 工具显示时附加人类可读信息

**示例**:
```bash
spore deps

# 输出:
# http@sig:a3f9c2e1+impl:7b8d4f2a (alias: std-http-v2, version: 2.3.1)
```

### 11.5 决策 5: spore.toml + .spore-lock 双文件

**决策**: 分离意图 (spore.toml) 和状态 (.spore-lock)。

**理由**:
- **可读性**: spore.toml 人类编辑,简洁
- **可重现性**: .spore-lock 机器生成,完整
- **协作**: spore.toml 合并冲突少

**类似工具**: Cargo.toml + Cargo.lock, package.json + package-lock.json

### 11.6 决策 6: Diamond 依赖多哈希共存

**决策**: 允许同一模块的不同哈希同时存在。

**理由**:
- **避免版本地狱**: 无需强制统一版本
- **并行升级**: 不同依赖可独立升级

**缺点**:
- 二进制膨胀: 相似代码重复链接

**缓解措施**:
- 链接器优化: 合并相同代码
- 推荐工具提示: 检测可统一的版本

### 11.7 决策 7: Platform 不特殊化

**决策**: 平台特定依赖与普通依赖同等处理。

**实现**:
```toml
[dependencies]
# 所有平台
io = { sig = "...", impl = "..." }

# 仅 Linux
[target.linux.dependencies]
epoll = { sig = "...", impl = "..." }

# 仅 WASM
[target.wasm32.dependencies]
wasm-bindgen = { sig = "...", impl = "..." }
```

### 11.8 决策 8: 无强制 Workspace

**决策**: 不强制 monorepo 使用 workspace 配置。

**理由**:
- **简化**: 减少概念负担
- **灵活性**: path 依赖已足够

**可选支持**:
社区可开发 workspace 工具,但非核心功能。

### 11.9 决策 9: 两层能力系统

**决策**: 编译时声明 + 运行时封顶。

**理由**:
- **安全性**: 防止依赖滥用权限
- **透明性**: 用户可审计能力使用

**挑战**:
- 学习曲线: 需要理解能力模型
- 兼容性: 现有代码需添加声明

**缓解措施**:
- 工具自动推断能力需求
- 提供宽松模式 (allow-all) 用于快速原型

### 11.10 决策 10: 自动推断 + --fixes

**决策**: 提供 `--fixes` 标志自动修复常见问题。

**理由**:
- **降低门槛**: 新手无需完全理解哈希
- **加速迁移**: 从现有项目迁移更轻松

**自动推断范围**:
- 缺失的哈希值
- 未声明的能力
- 冗余的依赖

**不自动修复**:
- 破坏性变更 (需人工确认)
- 歧义情况 (多个可能值)

---

## 12. 附录 (Appendix)

### 12.1 完整 spore.toml 示例

```toml
[package]
name = "web-framework"
version = "1.0.0"
authors = ["Alice <alice@example.com>", "Bob <bob@example.com>"]
description = "A modern web framework for Spore"
license = "MIT OR Apache-2.0"
repository = "https://github.com/spore-lang/web-framework"
keywords = ["web", "http", "framework"]
categories = ["web-programming"]
readme = "README.md"
homepage = "https://webframework.spore.dev"
documentation = "https://docs.webframework.spore.dev"

spore-version = ">=0.5.0"

[dependencies]
# 核心依赖
http = {
    git = "https://github.com/spore-std/http",
    tag = "v2.0.0",
    alias = "std-http-v2",
    sig = "a3f9c2e1d8b7a6f5",
    impl = "7b8d4f2a1e9c3d5b"
}

router = {
    git = "https://github.com/spore-std/router",
    rev = "abc1234567890def",
    sig = "b4e7d3c2a1f9e8d7",
    impl = "2f3e4d5c6a7b8e9f"
}

# 仅接口依赖
json = {
    git = "https://github.com/spore-std/json",
    alias = "std-json",
    sig = "c5d6e7f8a9b0c1d2"
}

# 本地开发依赖
logger = {
    path = "../logger",
    sig = "d1c2b3a4e5f6a7b8",
    impl = "8e9f0a1b2c3d4e5f"
}

# 可选依赖
tls = {
    git = "https://github.com/crypto/tls",
    sig = "e2f3a4b5c6d7e8f9",
    impl = "9f0a1b2c3d4e5f6a",
    optional = true
}

metrics = {
    git = "https://github.com/observability/metrics",
    sig = "f3e4d5c6b7a8f9e0",
    optional = true
}

[dev-dependencies]
test-framework = {
    git = "https://github.com/spore-std/test",
    alias = "spore-test-v1",
    sig = "a1b2c3d4e5f6a7b8",
    impl = "1a2b3c4d5e6f7a8b"
}

benchmark = {
    git = "https://github.com/perf/benchmark",
    sig = "b2c3d4e5f6a7b8c9",
    impl = "2b3c4d5e6f7a8b9c"
}

mock-server = {
    path = "../testing/mock-server",
    sig = "c3d4e5f6a7b8c9d0",
    impl = "3c4d5e6f7a8b9c0d"
}

[features]
default = ["logging"]
full = ["tls", "metrics", "compression"]
logging = ["dep:logger"]
tls = ["dep:tls"]
metrics = ["dep:metrics"]
compression = []

[capabilities]
require = [
    "network:listen:0.0.0.0:8080",
    "network:connect:*:443",
    "filesystem:read:/etc/ssl",
    "filesystem:write:/var/log/app",
    "env:read:PORT",
    "env:read:DATABASE_URL",
]

grant = [
    "http:network:listen:*",
    "http:network:connect:*",
    "tls:filesystem:read:/etc/ssl",
    "logger:filesystem:write:/var/log",
]

[capabilities.descriptions]
"network:listen:0.0.0.0:8080" = "HTTP server main listener"
"filesystem:write:/var/log/app" = "Application logging"

[build]
script = "build.spore"

[overrides]
# 强制所有依赖使用特定 http 版本 (测试用)
"http" = { sig = "a3f9c2e1d8b7a6f5", impl = "OVERRIDE" }

[metadata]
tags = ["production-ready", "async", "high-performance"]
stability = "stable"
minimum-rust-version = "1.70"

[target.linux.dependencies]
epoll = {
    git = "https://github.com/linux/epoll",
    sig = "1a2b3c4d5e6f7a8b",
    impl = "a1b2c3d4e5f6a7b8"
}

[target.wasm32.dependencies]
wasm-bindgen = {
    git = "https://github.com/wasm/bindgen",
    sig = "2b3c4d5e6f7a8b9c",
    impl = "b2c3d4e5f6a7b8c9"
}

[profile.dev]
opt-level = 0
debug = true

[profile.release]
opt-level = 3
lto = true
```

### 12.2 完整 .spore-lock 示例

```toml
version = 1

[metadata]
generated-at = "2024-01-15T10:30:00Z"
spore-version = "0.5.2"
lock-file-version = "1.0"

[[package]]
name = "web-framework"
source = "local"
sig = "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d"
impl = "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6"
features = ["default", "logging"]

[[package]]
name = "http"
alias = "std-http-v2"
source = { type = "git", url = "https://github.com/spore-std/http", rev = "abc1234567890def" }
sig = "a3f9c2e1d8b7a6f5e4d3c2b1a0f9e8d7"
impl = "7b8d4f2a1e9c3d5b6a7f8e9d0c1b2a3"
location = ".spore-store/git/github.com/spore-std/http/abc1234"
checksum = "blake3:a3f9c2e1d8b7a6f5e4d3c2b1a0f9e8d77b8d4f2a1e9c3d5b6a7f8e9d0c1b2a3"

[[package]]
name = "router"
source = { type = "git", url = "https://github.com/spore-std/router", rev = "def4567890abc123" }
sig = "b4e7d3c2a1f9e8d7c6b5a4f3e2d1c0b9"
impl = "2f3e4d5c6a7b8e9f0a1b2c3d4e5f6a7"
location = ".spore-store/git/github.com/spore-std/router/def4567"
checksum = "blake3:b4e7d3c2a1f9e8d7c6b5a4f3e2d1c0b92f3e4d5c6a7b8e9f0a1b2c3d4e5f6a7"

[[package]]
name = "json"
alias = "std-json"
source = { type = "git", url = "https://github.com/spore-std/json", rev = "fed6789012bcd345" }
sig = "c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0"
impl = null
location = ".spore-store/sigs/c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0"
checksum = "blake3:c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0"

[[package]]
name = "logger"
source = { type = "path", path = "../logger" }
sig = "d1c2b3a4e5f6a7b8c9d0e1f2a3b4c5d6"
impl = "8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3"
location = "../logger"
checksum = "blake3:d1c2b3a4e5f6a7b8c9d0e1f2a3b4c5d68e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3"

[[package]]
name = "io"
source = { type = "git", url = "https://github.com/spore-std/io", rev = "123456789abcdef0" }
sig = "e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8"
impl = "f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9"
location = ".spore-store/git/github.com/spore-std/io/1234567"
checksum = "blake3:e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9"

[dependency-graph]
"web-framework" = ["http", "router", "json", "logger"]
"http" = ["io"]
"router" = ["http"]
"json" = []
"logger" = ["io"]
"io" = []

[verification]
"http" = {
    sig-algo = "blake3",
    sig-hash = "a3f9c2e1d8b7a6f5e4d3c2b1a0f9e8d7",
    impl-algo = "blake3",
    impl-hash = "7b8d4f2a1e9c3d5b6a7f8e9d0c1b2a3",
    verified-at = "2024-01-15T10:30:05Z",
    size-bytes = 125648
}
"router" = {
    sig-algo = "blake3",
    sig-hash = "b4e7d3c2a1f9e8d7c6b5a4f3e2d1c0b9",
    impl-algo = "blake3",
    impl-hash = "2f3e4d5c6a7b8e9f0a1b2c3d4e5f6a7",
    verified-at = "2024-01-15T10:30:06Z",
    size-bytes = 87432
}
"json" = {
    sig-algo = "blake3",
    sig-hash = "c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0",
    impl-algo = null,
    impl-hash = null,
    verified-at = "2024-01-15T10:30:07Z",
    size-bytes = 12456
}

[capabilities]
"web-framework" = ["network:listen:0.0.0.0:8080", "filesystem:write:/var/log/app"]
"http" = ["network:listen:*", "network:connect:*"]
"logger" = ["filesystem:write:/var/log"]
"io" = ["filesystem:read", "filesystem:write"]

[[platform-locks]]
platform = "linux-x86_64"
packages = ["web-framework", "http", "router", "json", "logger", "io"]

[[platform-locks]]
platform = "linux-aarch64"
packages = ["web-framework", "http", "router", "json", "logger", "io"]

[[platform-locks]]
platform = "darwin-x86_64"
packages = ["web-framework", "http", "router", "json", "logger", "io"]

[[platform-locks]]
platform = "wasm32"
packages = ["json"]  # 仅 json 支持 wasm

[build-metadata]
compiler-version = "spore-0.5.2"
build-timestamp = "2024-01-15T10:30:10Z"
build-host = "linux-x86_64"
```

### 12.3 示例场景

#### 场景 1: 创建新项目

```bash
# 1. 初始化项目
mkdir my-app && cd my-app
spore init

# 2. 添加依赖
spore add https://github.com/spore-std/http --alias std-http
spore add https://github.com/spore-std/json --sig-only

# 3. 编写代码
cat > src/main.spore <<EOF
import http from "std-http"
import json from "json"

fn main() {
    http.serve(8080, handle_request)
}

fn handle_request(req) {
    let data = json.parse(req.body)
    http.respond(200, json.stringify(data))
}
EOF

# 4. 运行
spore run --allow network:listen:8080
```

#### 场景 2: 更新依赖

```bash
# 查看当前依赖
spore deps

# 更新单个依赖到最新
spore update http

# 验证更新
spore test
spore audit

# 提交锁文件
git add .spore-lock
git commit -m "Update http to latest"
```

#### 场景 3: 处理 Diamond 依赖

```
项目结构:
  my-app
  ├── lib-a (依赖 util@sig:v1+impl:v1)
  └── lib-b (依赖 util@sig:v1+impl:v2)
```

```bash
# spore 自动处理,两个版本共存
spore deps

# 输出:
# my-app
# ├── lib-a@sig:aaa+impl:bbb
# │   └── util@sig:v1+impl:v1
# └── lib-b@sig:ccc+impl:ddd
#     └── util@sig:v1+impl:v2

# 如果 my-app 也依赖 util,需显式选择
spore add util --impl v1  # 或 v2
```

#### 场景 4: 本地开发 Monorepo

```
monorepo/
├── packages/
│   ├── core/
│   │   └── spore.toml
│   ├── utils/
│   │   └── spore.toml
│   └── app/
│       └── spore.toml
```

```toml
# packages/app/spore.toml
[dependencies]
core = { path = "../core", sig = "...", impl = "..." }
utils = { path = "../utils", sig = "...", impl = "..." }

# 开发时自动更新哈希
# spore update --recompute-hash
```

#### 场景 5: 发布模块

```bash
# 1. 确保测试通过
spore test
spore audit

# 2. 计算哈希
spore hash --output RELEASE.md

# 3. 提交并打 tag
git add .
git commit -m "Release v1.0.0"
git tag v1.0.0
git push origin main --tags

# 4. 在 README.md 中记录
cat >> README.md <<EOF
## Installation

\`\`\`toml
[dependencies]
my-lib = {
    git = "https://github.com/user/my-lib",
    tag = "v1.0.0",
    alias = "my-lib-v1",
    sig = "a1b2c3d4e5f6a7b8",
    impl = "1a2b3c4d5e6f7a8b"
}
\`\`\`
EOF

git add README.md
git commit -m "Update installation instructions"
git push
```

### 12.4 术语表 (Glossary)

| 术语 | 英文 | 说明 |
|------|------|------|
| 内容寻址 | Content-addressed | 通过内容哈希标识资源 |
| 签名哈希 | Signature hash | 模块公开接口的哈希 |
| 实现哈希 | Implementation hash | 完整实现的哈希 |
| 命名别名 | Named alias | 人类可读的模块标识符 |
| 依赖图 | Dependency graph | 模块间依赖关系 |
| 能力 | Capability | 模块可执行的操作权限 |
| 封顶 | Ceiling | 限制能力的最大范围 |
| 锁文件 | Lock file | 记录依赖解析结果的文件 |
| Diamond 依赖 | Diamond dependency | 同一模块被多个路径依赖 |
| 可重现构建 | Reproducible build | 相同输入产生相同输出 |

### 12.5 参考资料 (References)

1. **内容寻址系统**
   - IPFS: https://ipfs.io
   - Git: https://git-scm.com
   - Nix: https://nixos.org

2. **包管理系统**
   - Cargo: https://doc.rust-lang.org/cargo/
   - npm: https://docs.npmjs.com
   - Go modules: https://go.dev/ref/mod

3. **能力系统**
   - Deno permissions: https://deno.land/manual/basics/permissions
   - WebAssembly Component Model: https://github.com/WebAssembly/component-model

4. **哈希算法**
   - BLAKE3: https://github.com/BLAKE3-team/BLAKE3

---

**文档版本**: v0.1.0
**最后更新**: 2024-01-15
**贡献者**: Spore Language Team
**许可证**: CC BY 4.0
