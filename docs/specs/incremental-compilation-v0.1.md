# Spore 增量编译与 Watch Mode 规格说明

> **版本**: v0.1 (Draft)  |  **状态**: 设计阶段
> **范围**: Developer-time DX — 快速迭代、实时诊断、hole 状态追踪

---

## 1. 概述

### 1.1 为什么 Spore 天然适合增量编译

传统语言的增量编译依赖文件时间戳或手工接口边界。Spore 的 content-addressed 设计提供更精确的路径：

- **impl hash** — 模块实现内容的 hash。内容未变 → hash 不变 → 跳过编译。
- **sig hash** — 模块对外接口的 hash。接口未变 → 下游依赖者无需重检。

两层 hash 构成天然的增量编译边界：impl hash 决定「是否重编本模块」，sig hash 决定「是否级联重编下游」。

### 1.2 精简落地的三个能力

| 能力               | 说明                                       |
|--------------------|-------------------------------------------|
| 快速增量编译       | 文件保存后仅重编变化部分                   |
| 实时诊断           | 错误、警告实时推送到终端或编辑器           |
| Hole 状态追踪      | 实时显示剩余 hole 数量及可填充状态         |

### 1.3 非目标

生产环境热替换、运行时动态加载、分布式编译、跨项目缓存（后续可扩展）。

---

## 2. 增量编译策略

### 2.1 核心决策树

```
文件变化检测
  ├─ 计算新 impl hash
  │   ├─ impl hash 未变 → 跳过（content-addressed dedup）
  │   └─ impl hash 已变
  │       ├─ sig hash 未变 → 重编本模块，下游跳过
  │       └─ sig hash 已变 → 重编本模块 + 标记下游重检
  └─ 处理完毕 → 输出诊断
```

### 2.2 变化检测与 hash 计算

```
fn on_file_changed(path: Path) -> ChangeResult:
    let source = read_file(path)
    let module_id = resolve_module(path)
    let new_impl_hash = hash_impl(source)

    if new_impl_hash == cache.get_impl_hash(module_id):
        return ChangeResult::Unchanged

    let new_sig_hash = compute_sig_hash(source)
    let old_sig_hash = cache.get_sig_hash(module_id)
    cache.update(module_id, new_impl_hash, new_sig_hash)

    if new_sig_hash == old_sig_hash:
        return ChangeResult::ImplOnly(module_id)
    else:
        return ChangeResult::SigChanged(module_id)
```

### 2.3 依赖图遍历与并行编译

sig hash 变化时，沿依赖图向下游传播（topological order）。每层重编后判断 sig hash 是否变化，决定是否继续传播——确保范围尽可能小。同层内独立模块可并行编译。

```
// NOTE: This is algorithm pseudocode; Spore itself has no loop constructs —
//       use recursion + higher-order functions (each/fold/map/filter).

fn compile_batch(modules: Set<ModuleId>, graph: DepGraph):
    let levels = topological_levels(modules, graph)
    levels |> each(fn(level) {
        level |> parallel_each(fn(module_id) {
            compile(module_id)
            update_diagnostics(module_id)
            update_hole_report(module_id)
        })
    })
```

### 2.4 三种场景

**场景 A：impl hash 未变（最快路径）**
```
[watch] 文件变化: src/math/vector.spore
[watch] impl hash 未变 (0xa3f2...b7c1) → 跳过
[watch] 耗时: <1ms
```

**场景 B：impl hash 变，sig hash 未变**
```
[watch] 文件变化: src/math/vector.spore
[watch] impl hash 变化: 0xa3f2...b7c1 → 0x91d0...e4a2
[watch] sig hash 未变 → 仅重编本模块 ... OK (47ms)
[watch] ✓ 0 错误，0 警告
```

**场景 C：sig hash 变化（级联）**
```
[watch] 文件变化: src/math/vector.spore
[watch] sig hash 变化 → 检查下游:
         ├─ src/physics/rigid_body.spore ... OK (38ms)
         ├─ src/renderer/mesh.spore ... OK (41ms)
         └─ src/game/player.spore ... OK (29ms)
[watch] ✓ 4 模块重编，0 错误 (163ms)
```

### 2.5 Hash 包含内容

**sig hash 包含**：导出的类型/函数签名、capability 要求、cost annotation。
**sig hash 不包含**：函数体实现、私有定义、注释、内部 hole 状态。

「只改注释」→ impl hash 变（重编本模块），sig hash 不变（下游跳过）。

---

## 3. spore watch 命令

### 3.1 基本用法

```bash
spore watch                          # 监视当前项目
spore watch --json                   # JSON 输出（IDE / agent 消费）
spore watch --project ./my-project   # 指定项目路径
spore watch --quiet                  # 只显示错误
spore watch --jobs 4                 # 指定并行度
```

### 3.2 终端输出

```
$ spore watch
[watch] 监视项目: ./my-project (42 模块)
[watch] ✓ 初始编译完成 (1.2s)，0 错误，2 警告，5 holes

  警告: src/net/http.spore:23:5  未使用的导入: `Timeout`
  警告: src/db/query.spore:45:12 capability `FileSystem` 已声明但未使用

  Holes (5):
    ◯ src/auth/login.spore:30   authenticate : User -> Token
    ◯ src/auth/login.spore:45   validate_token : Token -> Bool
    ◯ src/db/query.spore:12     execute_query : Query -> Result
    ◯ src/net/http.spore:67     handle_request : Request -> Response
    ◯ src/net/http.spore:89     parse_headers : Bytes -> Headers

[watch] 等待文件变化...
```

文件变化时：

```
[watch] ─── 变化检测 ────────────────────────
[watch] 文件变化: src/auth/login.spore
[watch] 重编: src/auth/login.spore ... OK (34ms)
[watch] sig hash 未变 → 跳过下游

  ✓ 0 错误，2 警告，4 holes (之前: 5)
  Holes 更新:
    ● src/auth/login.spore:30   authenticate  [已填充]
    ◯ src/auth/login.spore:45   validate_token : Token -> Bool
    ...
```

### 3.3 JSON 输出（NDJSON）

`spore watch --json` 输出 newline-delimited JSON，每行一个事件：

```jsonc
// 增量编译事件
{
  "type": "incremental_compile",
  "timestamp": "2025-01-15T10:30:45Z",
  "trigger": { "file": "src/auth/login.spore", "change_type": "impl_only" },
  "recompiled": ["src/auth/login.spore"],
  "skipped_downstream": true,
  "duration_ms": 34,
  "diagnostics": [],
  "holes": {
    "total": 4,
    "filled_this_cycle": ["src/auth/login.spore:30"],
    "ready_to_fill": ["src/auth/login.spore:45"]
  }
}
```

```jsonc
// 诊断事件
{
  "type": "diagnostic",
  "file": "src/net/http.spore",
  "line": 23, "column": 5,
  "severity": "warning",
  "code": "W001",
  "message": "未使用的导入: `Timeout`"
}
```

### 3.4 Debounce 策略

文件系统事件可能短时间内大量触发（如 `git checkout`）。

- **debounce 窗口**: 100ms（合并窗口内所有变化）
- **最大等待**: 500ms（变化持续到来时的上限）

```
t=0ms    文件 A 变化
t=20ms   文件 B 变化  ─┐ debounce 窗口
t=80ms   文件 C 变化   │
t=100ms  窗口到期     ─┘
t=100ms  开始增量编译 {A, B, C}
```

### 3.5 错误恢复

| 情况             | 处理策略                                     |
|-----------------|----------------------------------------------|
| 语法/类型错误    | 报告错误，保留上次成功编译状态               |
| 循环依赖         | 报告错误，中断受影响模块                     |
| 文件被删除       | 从依赖图移除，标记依赖者为错误               |
| 编译器 panic     | 捕获并报告内部错误，继续监视                 |

原则：**watch mode 永不退出**（除非 Ctrl+C）。

---

## 4. Hole 状态实时更新

### 4.1 HoleReport 结构

```
type HoleReport:
    total: Nat
    filled_this_cycle: List Hole
    ready_to_fill: List Hole        // 依赖已满足，可立即实现
    blocked: List BlockedHole       // 附阻塞原因

type Hole:
    module: ModuleId
    location: SourceLocation
    name: String
    signature: Type

type BlockedHole:
    hole: Hole
    blocked_by: List ModuleId
```

### 4.2 Hole 填充的完整 Watch 会话

```
$ spore watch
[watch] ✓ 初始编译完成，0 错误，3 holes

  Holes (3):
    ◯ src/auth.spore:10  hash_password : String -> HashedPassword
    ◯ src/auth.spore:20  verify_password : String -> HashedPassword -> Bool
    ◯ src/app.spore:50   create_user : UserInput -> Result User Error
         ⚠ 被阻塞: 依赖 hash_password, verify_password
  可立即填充: hash_password, verify_password

# --- 开发者填充 hash_password ---

[watch] 文件变化: src/auth.spore → 重编 OK (28ms)
  ✓ 0 错误，2 holes (之前: 3)
    ● hash_password  [已填充 ✓]
    ◯ verify_password : String -> HashedPassword -> Bool
    ◯ create_user ⚠ 被阻塞: 依赖 verify_password
  可立即填充: verify_password

# --- 开发者填充 verify_password ---

[watch] 文件变化: src/auth.spore → 重编 OK (31ms)
[watch] sig hash 变化 → 下游:
         └─ src/app.spore ... OK (22ms)
  ✓ 0 错误，1 hole
    ● verify_password  [已填充 ✓]
    ◯ create_user → 不再被阻塞！
  可立即填充: create_user

# --- 开发者填充 create_user ---

[watch] 文件变化: src/app.spore → 重编 OK (35ms)
  ✓ 0 错误，0 holes 🎉 所有 hole 已填充！
```

### 4.3 Hole 状态 JSON 事件

```jsonc
{
  "type": "hole_update",
  "summary": { "total": 2, "ready_to_fill": 1, "blocked": 1 },
  "changes": [
    { "action": "filled", "hole": { "file": "src/auth.spore", "line": 10, "name": "hash_password" } },
    { "action": "unblocked", "hole": { "file": "src/auth.spore", "line": 20, "name": "verify_password" },
      "reason": "dependency hash_password is now filled" }
  ]
}
```

### 4.4 Agent 订阅工作流

AI agent 通过 `--json` 输出订阅 hole 状态，实现自动化填充：

```
1. 启动 spore watch --json
2. 解析初始 hole 列表
3. 选择 ready_to_fill 的 hole → 生成实现 → 写入文件
4. 等待 watch 输出编译结果
5. 成功 → 下一个 hole；失败 → 修正重试
6. 重复直到 holes = 0
```

---

## 5. 依赖图与传播

### 5.1 数据结构

```
type DepGraph:
    nodes: Map ModuleId ModuleInfo
    edges: Map ModuleId (Set ModuleId)    // 模块 → 它依赖的模块
    reverse: Map ModuleId (Set ModuleId)  // 模块 → 依赖它的模块

type ModuleInfo:
    id: ModuleId
    path: Path
    impl_hash: Hash
    sig_hash: Hash
    capabilities: Set Capability
    cost_annotation: CostBound
    holes: List Hole
```

### 5.2 传播规则

1. **impl hash 不变** → 不传播
2. **sig hash 不变** → 重编本模块，不向下游传播
3. **sig hash 变** → 向直接下游传播，每层重编后再判断是否继续

### 5.3 Capability 传播

```
# 修改前
module HttpClient:
    capabilities: {Network}

# 修改后 — 新增 FileSystem
module HttpClient:
    capabilities: {Network, FileSystem}
```

```
[watch] sig hash 变化（capability 集合变化）
[watch] capability 变化: HttpClient: {Network} → {Network, FileSystem}
[watch] 检查项目 capability ceiling...
         ceiling 允许: {Network, FileSystem, Clock} → ✓ 在范围内
[watch] 下游 capability 兼容性检查:
         ├─ src/api/client.spore → OK
         └─ src/app.spore → 传播终止
[watch] ✓ 3 模块重编，0 错误，1 capability 警告
```

超出 ceiling 时：

```
[watch] ✗ 错误: HttpClient 新增 capability `Crypto`
         项目 ceiling: {Network, FileSystem, Clock}
         `Crypto` 不在 ceiling 中 — 需更新 ceiling 或移除使用
```

### 5.4 Cost 传播

```
# 修改: sort 的 cost 从 O(n·log n) 退化为 O(n²)
```

```
[watch] cost 变化: sort: O(n·log n) → O(n²)
[watch] 检查下游 cost budget:
         ├─ src/data/table.spore
         │   sort_column budget: O(n·log n) → ✗ 超出
         └─ src/app.spore
             process_data budget: O(n²) → ✓ 在范围内
[watch] ✗ 1 错误
```

### 5.5 依赖图增量更新

模块 import 声明变化时更新图结构，并检查循环依赖：

```
// NOTE: This is algorithm pseudocode; Spore itself has no loop constructs.

fn update_dep_graph(module_id, old_deps, new_deps):
    (old_deps - new_deps) |> each(fn(dep) {  // 移除
        graph.edges[module_id].remove(dep)
        graph.reverse[dep].remove(module_id)
    })
    (new_deps - old_deps) |> each(fn(dep) {  // 新增
        graph.edges[module_id].insert(dep)
        graph.reverse[dep].insert(module_id)
    })
    if has_cycle(graph, module_id):
        emit_error("循环依赖", module_id)
```

---

## 6. 与 LSP 的集成

### 6.1 架构

```
┌──────────────┐   LSP Protocol   ┌──────────────┐  stdin/stdout  ┌──────────────┐
│  编辑器 (IDE) │ ←──────────────→ │ Spore LSP    │ ←────────────→ │ spore watch  │
│              │                   │ Server       │    (JSON)      │ --json       │
└──────────────┘                   └──────────────┘                └──────────────┘
```

LSP server 启动 `spore watch --json` 子进程，解析 JSON 流转换为 LSP 消息。

### 6.2 消息映射

| watch 事件              | LSP 消息                              |
|------------------------|---------------------------------------|
| `diagnostic`           | `textDocument/publishDiagnostics`     |
| `incremental_compile`  | diagnostics 刷新                      |
| `hole_update`          | 自定义 `spore/holeUpdate`             |

### 6.3 Diagnostics → LSP

```jsonc
// watch 输出 → 转换为 LSP publishDiagnostics
{
  "method": "textDocument/publishDiagnostics",
  "params": {
    "uri": "file:///project/src/auth.spore",
    "diagnostics": [{
      "range": { "start": {"line": 22, "character": 9}, "end": {"line": 22, "character": 24} },
      "severity": 1,
      "code": "E042",
      "source": "spore",
      "message": "类型不匹配: 期望 `Token`，实际 `String`"
    }]
  }
}
```

### 6.4 Hole 自定义 LSP 扩展

```jsonc
{
  "method": "spore/holeUpdate",
  "params": {
    "holes": [
      {
        "uri": "file:///project/src/auth.spore",
        "range": { "start": {"line": 29, "character": 4}, "end": {"line": 29, "character": 50} },
        "name": "authenticate",
        "signature": "User -> Token",
        "status": "ready_to_fill"
      }
    ],
    "summary": { "total": 2, "ready": 1, "blocked": 1 }
  }
}
```

### 6.5 触发时机

Watch mode 监听文件系统事件而非 LSP didChange。编译只在**文件保存**后触发——避免对半编辑状态做无意义编译。

---

## 7. 性能目标

以下为**期望目标**，非硬性保证。

| 操作                     | 目标延迟   |
|--------------------------|-----------|
| 单模块重编               | < 100ms   |
| 依赖图遍历               | < 10ms    |
| Hash 计算（单模块）      | < 5ms     |
| 全项目初始分析 (~100 模块) | < 5s      |
| 端到端延迟（含 debounce） | < 200ms   |
| Hole report 更新          | < 50ms    |

**缓存策略**：watch mode 维护 hash 缓存、依赖图、编译产物、hole 状态的内存缓存。退出后可选持久化（非 v0.1 必需）。

**并行度**：默认 = CPU 核心数，可 `--jobs N` 指定。

---

## 8. 设计决策记录

### ADR-001: 核心场景为开发时快速迭代

**决策**: 限定为 developer-time DX，不涉及 production hot-swap。
**理由**: DX 是刚需且投入产出比最高；content addressing 天然适合增量编译。
**后果**: 无需运行时模块替换协议，只考虑单机开发场景。

### ADR-002: 增量编译 + 实时诊断 + Hole 状态

**决策**: "Program-as-Service" 落地为三个具体能力。
**理由**: 覆盖编码时最核心的反馈需求——正确性、问题定位、进度追踪。
**后果**: `spore watch` 成为统一载体，JSON 输出包含全部三种信息。

### ADR-003: 模块级增量粒度

**决策**: 以模块为编译单元，sig hash 不变则下游跳过。
**理由**: 模块是自然的编译边界（有明确导出接口），函数级过细收益有限。
**后果**: hash 计算、依赖图、并行编译均以模块为粒度。

### ADR-004: 文件保存触发

**决策**: 监听文件系统事件，保存时触发编译。
**理由**: 保存是「值得检查」的自然意图信号；按键触发导致大量无意义编译。
**后果**: 需要 debounce；LSP 通过 FS 事件而非 didChange 触发。

### ADR-005: 双格式输出

**决策**: 人类可读（默认）+ JSON（`--json`）。
**理由**: 终端用户需要易读输出，IDE/agent 需要结构化输出。
**后果**: 两种格式信息一致；JSON 格式成为 LSP 和 agent 集成基础。

---

## 术语表

| 术语               | 说明                                                  |
|--------------------|-------------------------------------------------------|
| impl hash          | 模块实现内容的 hash，判断是否需要重编本模块           |
| sig hash           | 模块对外接口的 hash，判断下游是否需要重检             |
| hole               | 源码中待实现的占位符，带类型签名                      |
| capability         | 模块可使用的系统能力（Network, FileSystem 等）        |
| capability ceiling | 项目级 capability 上限                                |
| cost annotation    | 函数的计算复杂度标注                                  |
| debounce           | 合并短时间内多次文件变化为一次编译触发                |
| NDJSON             | Newline-Delimited JSON                                |
