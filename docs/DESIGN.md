# Spore 编程语言设计

## 语言名称
**Spore**（孢子）— 一个紧凑的生物单元，携带完整蓝图，可在适宜环境中发育为完整有机体。
映射：签名(孢子) 携带完整上下文 → hole(萌芽) → 完整程序(有机体)。

CLI: `spore build`, `spore run`, `spore check`
编译器: `sporec`（无状态）
Codebase Manager: `spore`（有状态）

## 背景
从 Claude.ai 会话迁移。已完成设计：
- 核心哲学：意图一等公民，函数签名作为重力中心
- 五语言提炼：Agda(hole) / Idris(elaboration) / Unison(内容寻址) / Elm(错误信息) / Roc(managed effects)
- 架构分层：sporec（无状态编译器）+ spore（Codebase Manager）
- Snapshot 系统：.spore-lock + --permit
- 错误分级：v0–v4

## 语法规范决策记录

以下决策已最终确定，所有文档和实现必须遵守：

| # | 决策 | 说明 |
|---|------|------|
| D1 | `struct` 用于积类型 | 不使用 `type = {}`，parser 仅支持 `struct` |
| D2 | `capability` 废弃 → `trait` | formatter 自动重写；parser 发出诊断 |
| D3 | SEP spec-clause 分支优先合入 | 实现先于文档 |
| D4 | `perform` 为规范 effect 调用语法 | parser 和 AST 已支持 |
| D5 | `throw` = `return Err()` 语法糖 | 暂定，SEP-0001 开放问题 #4 |
| D6 | `when` 用于 refinement types | 避免 `if` 表达式歧义 |
| D7 | 删除 `module` 关键字 | 文件路径推导 |
| D8 | `[T]` 用于泛型 | 避免 `<>` 解析歧义 |
| D9 | `Str` 为规范类型名 | 与 `Int`、`Bool`、`Float` 一致 |
| D10 | `when self > 0` refinement 谓词 | 隐式 self 绑定，非 lambda |
| D11 | `! E1 \| E2` 错误集语法 | 管道符，无方括号，fn-def 和 type-expr 通用 |
| D12 | 禁止选择性/通配符导入 | SEP-0008 规则 |
| D13 | Range `a..b` 已知实现差距 | token 已词法化，无 parser 路径 |
| N1 | `cost [c, a, i, p]` 向量形式 | 已实现；避免非 ASCII `≤`；SEP map 形式留待未来 |
| N2 | `type Name { Variant(T) }` 用于枚举 | 花括号界定，位置参数字段，按 parser 实现 |
| N3 | v0.1 无 tuple structs | parser 仅支持 `struct Name { field: Type }` |
| N4 | `where` 子句不支持 `+` 多重约束 | 每个参数单一约束；`where T: Bound, U: Bound` |
| N5 | Spec 子句使用 `:` 分隔符 | `example "name": expr`，非 `=>` |
| N6 | `Int`/`Float` = `I64`/`F64` 别名 | 尺寸类型是具体类型；抽象名是便利别名 |
| N7 | `struct` = 积类型，`type` = 和类型 | 按 D1；`type` 关键字仅用于 enum/ADT |

## 已确定的设计

### 函数签名语法（混合式 v0.3）
```
fn name(params) -> ReturnType ! Errors
    where T: Constraint
    uses [Capabilities]
    cost [compute, alloc, io, parallel]
    spec {
        example "...": ...
    }
{ body }
```

> 解析器接受 `where`、`uses`、`cost`、`spec` 子句按任意顺序出现；文档与格式化输出统一推荐顺序为 `where` → `uses` → `cost` → `spec`。

> **v0.2→v0.3 变更**: 原 `where { ... }` 统一块拆分为独立子句：
> - `where T: Constraint` — 泛型约束（保留 where 关键字）
> - `uses [Capabilities]` — 能力集声明（独立子句）
> - `cost [compute, alloc, io, parallel]` — 四维代价声明（固定顺序独立子句）
>
> 细化类型谓词语法同步变更: `where |n| n > 0` → `when self > 0`
>
> **v0.3→v0.4 变更**: 移除 `with [...]` 子句。函数属性（pure, deterministic, total）从 `uses` 自动推断：
> - `uses []` → pure + deterministic + total
> - `uses [Compute]` → deterministic（纯计算，无副作用）
> - 含 IO/State capability → 非 pure
> - 编译器自动验证一致性，无需手动声明

### 能力集系统
- 内置: Compute, FileRead, FileWrite, NetRead, NetWrite, Clock, Random, StateRead, StateWrite, Spawn
- 自定义: `trait Name = [...]`（capability = trait，统一机制）
- 推断规则: 无依赖→自由函数 / 有依赖未声明→不完整函数 / 声明→验证一致性
- FuncCall/Module 已移除，用 @allows + 调用图查询替代

### 抽象代价模型
- 四维: compute(op) + alloc(cell) + io(call) + parallel(lane)
- 签名语法固定为 `cost [compute, alloc, io, parallel]`
- 文档与格式化输出统一采用 compute → alloc → io → parallel 顺序
- 旧的 `cost <= expr` 标量写法已移除；`log/max/min` 风格的标量表面语法留待后续讨论
- 编译时模拟执行（抽象解释）
- 符号代价表达式支持
- unbounded 函数需 with_cost_limit 包裹

### Hole 系统（v0.2）
- 语法: `?name` 或 `?name : Type`（无 @priority，已移除）
- 填洞顺序: 编译器依赖分析推荐，按传递依赖者数量降序排列
- Partial 函数可编译、可模拟、不可执行
- HoleReport JSON: 完整上下文（类型、绑定、能力、代价预算、候选函数）
- CLI: `sporec holes <file>`, `sporec query-hole <file> <hole>`, `spore holes <file>`

### 模块系统（v0.1）
- 一文件一模块: `src/billing/invoice.sp` → `billing.invoice`
- 双 hash 寻址:
  - 签名 hash (sig): 接口兼容性检查，接口不变则不变
  - 实现 AST hash (impl): 内容寻址，编译成功后分配，部分函数为 None
- 可见性: 默认私有 / `pub(pkg)` 包内可见 / `pub` 完全公开
- 禁止循环依赖（Elm 风格）
- Platform 概念（Roc 风格）: Platform 提供所有 IO 能力，纯包无法直接 IO
- 无 Functor: 泛型 + 能力集替代
- 模块名仅由文件路径决定；无 `module ...` 头声明
- 不存在模块级 capability ceiling / carrier；能力检查仅在函数级 `uses [...]` 与项目 / Platform 边界发生
- 导入: `import mod as alias` / `alias x = mod.item`（无通配符、无隐式嵌套）

### 类型系统（v0.1）
- Nominal 为主 + 匿名结构体记录（structural）
- Capability = Trait（统一机制，capability 是 trait 语法糖）
- 关联类型 + GAT 支持
- 无 HKT（关联类型 + GAT 已足够，避免错误信息灾难）
- 细化类型: L0 可判定谓词（`when self > 0`）+ L1 抽象解释传播（无 SMT）
- Sealed enum（穷尽匹配）
- 签名必须完整注解，函数体内推断
- Const generics（值级类型参数）
- 完整模式匹配: 穷尽 + 嵌套 + guard + or-pattern
- @allows: hole 级 Agent 约束（限制填洞可用函数）
- FuncCall/Module 已移除，用调用图查询替代

### 编译器输出格式（当前实现 + 下一步）
- 当前实现：
  - 默认文本输出已存在，但仍有不少 ad hoc `eprintln!` / 字符串拼接路径
  - 部分 `--json` 已存在，但不同命令仍是命令私有 schema
  - `sporec::compile_diagnostics` 目前只有 `message / span / severity`
  - 错误码族 `E/W/C/K/H/M` 已在 typeck 层落地
  - `sporec explain CODE` 已是统一长解释入口
- 下一步目标：
  - 引入共享 `sporec-diagnostics` crate 作为 canonical Diagnostic IR
  - 最小字段集固定为：
    - `code`
    - `severity`
    - `message`
    - `primary_span`
    - `secondary_labels`
    - `notes`
    - `help`
    - `related`
  - 默认文本 / `--verbose` / `--json` 都变成同一诊断对象的不同投影
  - auto-fix 作为后续 code-action / edit 层，不并入最小 IR

### 并发模型（v0.1）
- 结构化并发: 子任务树，父等所有子完成
- 效果处理器: Spawn 是 capability/effect，无 async/await 着色
- 代价推导: 编译器从被调用函数推导，无需手动分配
- 消息传递: Channel[T]，无共享可变状态
- 取消传播: 父取消 → 子自动取消，协作式
- 模拟: 保守取 max cost across lanes

### 包管理（v0.1）
- 完全内容寻址，无传统注册中心
- 模块级寻址粒度（一个 `.sp` 文件 = 一个可寻址单元）
- 存储: 本地 .spore-store + Git 默认，后端可插拔
- 无 semver，纯 hash + 命名别名（sig hash = 兼容性检查）
- 清单: spore.toml（元数据+依赖）+ .spore-lock（精确 hash pin）
- 钻石依赖: 无冲突，不同 hash 共存
- Platform 不特殊化，只是提供 IO 能力的普通包
- 能力封顶仅存在于项目级 `spore.toml` / Platform 边界；源码层没有模块级 capability carrier
- 编译器自动推导；未来可在稳定 Diagnostic IR 之上叠加 code-action / auto-fix

### 增量编译与 Watch 模式（当前实现 + 下一步）
- 当前实现：
  - 核心仍是“保存后编译”，不是运行时热替换
  - 模块级增量决策仍围绕 `sig hash` / `impl hash`
  - `spore watch --json` 已有粗粒度机器输出，但还不是稳定的通用协议
  - LSP 当前是进程内编译 + 薄映射，不依赖 `watch --json` 作为唯一后端
- 下一步目标：
  - watch / batch / LSP 都建立在同一份 Diagnostic IR 上
  - richer NDJSON 事件流作为后续 transport 层，而不是先定义另一套诊断 schema
  - hole 协议、watch 事件和 LSP adapter 都从同一份结构化诊断对象派生

### 语法设计（v0.2）
- 完全 expression-based（if/match 都有返回值）
- 大括号 `{}`，分号 Rust 语义（有分号=语句，无分号=返回表达式）
- 管道 `|>` 操作符，不允许自定义操作符
- 字符串: f-string `f"..."` + t-string `t"..."` + raw `r"..."`
- 错误: `! Errors` 签名契约 + `throw expr` + `?` 传播糖（调用边界受检）
- Lambda: `|x, y| x + y`
- 注释: `//` / `///` / `/* */`（可嵌套）
- 绑定: `let` 不可变 + shadowing，`Ref[T]` 可变容器（需 StateWrite）
- 模式匹配: `match`（穷尽 + 嵌套 + guard + or-pattern）
- 无循环: 递归 + 高阶函数（map/fold/filter），编译器保证 TCO
- 条件: `if cond { a } else { b }` 表达式
- 类型注解后置: `name: Type`
- trait 实现内联: `implements [...]`（Roc 风格）
- `struct` 记录 + `type` 枚举/ADT
- `trait` 关键字（= trait）
- 函数属性（pure, deterministic, total）— 从 `uses` 自动推断，无需关键字
- `when` 子句用于细化类型谓词（`when self > 0`），不再使用 `where` / `if`
- `where` 关键字仅保留用于单一泛型约束（`where T: Constraint`）；多重 / 分组形式暂不纳入 v0.1
- 基本类型（文档规范写法）: I32/I64/U32/U64/F32/F64/Bool/Char/Str/() + List[T]/Map[K,V]/Set[T]

## 文档治理与规范映射

`docs/DESIGN.md` 现在是仓库内唯一的主设计文档：
- **表面语法统一决策**、跨主题约束、实现栈结论统一维护在此文件。
- **规范级长文** 迁移为 SEP 体系；当前 SEP 文本位于 sibling repo `../../spore-evolution/seps/`。
- 旧 `docs/specs/` 与 `docs/research/` 草案不再独立维护，目录仅保留最小重定向说明，避免设计漂移。

### 主题摘要与 SEP 对照

| 主题 | 本文保留的 durable 摘要 | 相关 SEP / 外部规范 |
|---|---|---|
| 核心语法与签名 | expression-based 语言；签名子句推荐顺序 `where → uses → cost → spec`；`struct`/`type`/`trait`/`perform`/`when`/`[T]`/`Str` 等统一决策以本文 D1–D13、N1–N7 为当前权威。若任何 SEP 草案仍保留旧表面写法，以本文为准，待后续回写。 | `SEP-0001-core-syntax.md` |
| 类型系统 | nominal 为主、局部推断、显式签名、sealed enum、关联类型/GAT、const generics、L0/L1 细化类型；v0.1 不引入 HKT 或完整 dependent types。 | `SEP-0002-type-system.md` |
| 能力 / effect 语义 | 语义层保持 capability-set 检查、推断 purity/determinism、handler 由 Platform / runtime 承载；语法层继续采用本文统一后的 `uses [...]` 与 trait/capability 约定。 | `SEP-0003-effect-capability-system.md` |
| 代价模型 | 保留四维 CostVector（compute/alloc/io/parallel）与静态验证目标；现行 surface syntax 固定为 `cost [c, a, i, p]`，复杂代数与更丰富表达式留待后续。 | `SEP-0004-cost-analysis.md` |
| Hole 协作协议 | typed holes、依赖感知排序、JSON 报告、跨模块聚合、Open→Filling→Filled→Accepted 状态机；本文保留工作流摘要，完整协议见 SEP。 | `SEP-0005-hole-system.md` |
| 编译器输出 / 架构 / watch | 诊断编码、默认/verbose/json 三模式、内容寻址缓存、增量编译、watch/LSP 后端、6 阶段 pipeline 的高层约束保留在本文；详细数据模型与协议交给 SEP。 | `SEP-0006-compiler-architecture.md` |
| 并发模型 | 结构化并发、`Spawn` 能力、Channel 消息传递、取消传播、lane 作为 parallel cost 维度；本文保留用户心智模型，形式化语义见 SEP。 | `SEP-0007-concurrency-model.md` |
| 模块 / 包 / Platform | 一文件一模块、双 hash、private-by-default、内容寻址依赖、Git-first 存储、Platform 提供 IO handler、无 wildcard import；本文的“无模块级 carrier / ceiling、仅项目与 Platform 边界检查”是当前统一口径。 | `SEP-0008-module-package-system.md` |
| 标准库边界 | prelude + 少量核心集合容器 + `Ref[T]`；绝大多数功能走第三方包，IO 由 Platform 提供。 | `SEP-0009-standard-library.md` |

### 跨语言调研沉淀（持久结论）

以下内容来自原 `docs/research/`，仅保留仍然影响语言方向的结论：

| 调研主题 | 沉淀结论 |
|---|---|
| 语法设计 | 采用 expression-based 核心、有限关键字、无自定义操作符、显式签名承载错误/能力/代价信息。 |
| 实用类型系统 | 函数边界显式、函数体内推断；穷尽匹配与错误信息质量是语言可用性的核心投资。 |
| 依赖类型光谱 | 选择 refinement + 抽象解释 + const generics 的 80/20 路线，而非 SMT / theorem proving / 全 dependent types。 |
| 模块系统 | 路径导出模块名、private-by-default、避免独立 module language / functor，参数化优先通过 generics + traits 完成。 |
| 包管理 | 内容寻址、锁文件 pin、哈希校验、去中心化或 Git-first 分发优于传统 semver-first registry。 |
| 热重载 | v0.1 聚焦增量编译、watch、诊断与 hole 进度；不把“运行时状态保持式热重载”作为首要目标。 |
| 实现技术栈 | Rust 在 ADT、增量编译、WASM、LSP、FFI、内容寻址与工具链成熟度上是最佳折中。 |
| 代码生成 | 先用 Cranelift 获得纯 Rust 实现、快编译与 WASM 友好性；LLVM 保留为未来可选高性能后端。 |

#### 语言方向与非目标（研究长文折叠）
- **语法方向**：保留 expression-based、有限关键字、无自定义操作符、签名集中承载约束这四个总原则；具体表面语法由本文 D1–D13 / N1–N7 统一维护，完整语法论证与替代方案比较不再留在仓库内旧草案里。
- **类型方向**：继续坚持“函数边界显式、函数体内推断”的工程化路线，把错误信息质量、穷尽匹配、签名可读性放在优先级前列；不把 HKT、全 dependent types、SMT 驱动证明或 theorem-proving 作为 v0.1 目标。
- **模块方向**：文件系统就是模块声明；避免单独的 module language、functor、模块级 capability carrier/cap ceiling，把参数化能力留给 generics + traits + package / Platform 边界。
- **运行时方向**：watch mode 的目标是“保存后快速反馈”，不是 Erlang/Smalltalk 式运行时热替换；v0.1 不承诺状态迁移、热升级、分布式热重载或动态装载协议。
- **实现方向**：编译器继续以 Rust bootstrap 起步，优先保证可维护的 parser/typechecker/codegen/tooling 闭环；纯计算组件未来可逐步自举，但不以“尽快全量自举”压过当前语言设计收敛。

#### 已冻结 / 仍开放的设计契约（2026-04）
- **已冻结**
  - **内存模型方向**：不引入 borrow checker / lifetime system；当前方向是 **Perceus 风格 RC + region optimization**。
  - **验证策略**：以 **spec / property / refinement** 为主线；mutation testing 不进入近期关键路径。
  - **互操作边界**：**Platform 是唯一 FFI 表面**；应用代码不直接声明裸 native FFI。
  - **handler v0.1 语义**：`handle { ... } with { ... }` 是**词法作用域**、**不可恢复（non-resumable）**、**one-shot** 的；命中规则为**内层优先**，handler arm 在当前活动 handler 栈内求值，因此 arm 内继续触发的 `perform` 仍会按同一套活动 handler 继续匹配；handler arm 的返回值就是对应 `perform` 表达式的值。
- **仍开放**
  - `Ref[T]` 作为语言内可变单元 vs 平台包装器的精确边界
  - 若未来引入 continuation / resume，handler 的生命周期、逃逸与取消规则如何定义
  - `HostValue` / ADT / `Result` / `Option` 的跨边界映射
  - L0 refinement enforcement 的精确边界

### 标准库（极简）
- **Prelude（自动可用）**: I32/I64/U32/U64/F32/F64/Bool/Char/Str/(), Option[T], Result[T,E], 基本操作符, |>, ?
- **spore.list** — List[T]: map/fold/filter/zip/head/tail/len/reverse/sort/...
- **spore.map** — Map[K,V]: insert/get/remove/keys/values/merge/...
- **spore.set** — Set[T]: add/remove/contains/union/intersect/diff/...
- **spore.str** — Str 扩展: split/join/trim/contains/starts_with/replace/...
- **spore.math** — 数学函数: abs/min/max/pow/sqrt/...
- **spore.ref** — Ref[T] 可变容器（需 StateWrite capability）
- 其余全部第三方（JSON/HTTP/正则/时间等）

### Platform 系统（v0.1）
- 语言级概念，在 `spore.toml` 中声明；一个项目只绑定一个 Platform
- 提供所有 IO effect handler（应用代码完全纯净）
- Effect handler 风格（与并发模型统一）
- 长期目标是不内置官方 Platform；当前实现仍保留 `cli` / `web` / `embedded` built-ins 作为 legacy / 过渡路径，但 package-backed Platform 已是主方向
- 命名 `entry` 选择项目的可执行目标，并解析到对应的 `entry module`
- Platform 契约由 manifest 中的 `[platform]` 元数据与专门的 contract module 共同构成：manifest 负责定位 contract module / startup contract symbol / adapter / handled capabilities，contract module 则通过带 hole 的 `startup function` 持有权威签名与 spec
- 设计契约上，`entry module` 中提供满足 `startup contract` 的 `startup function`；Platform contract module 中的 startup `spec` 与应用实现侧 `spec` 视为叠加约束。当前编译器已经强制 hole-backed contract + startup 名称/签名匹配，但 merged `spec` enforcement 仍未落地
- 启动函数名与签名的当前查找路径是：先由项目 manifest 的 `[project].platform` 选中 Platform 包，再由该包的 `[platform].contract-module` 与 `[platform].startup-contract` 定位 contract module 中的权威 startup 定义；编译器从该 hole-backed 定义读取签名，并要求 entry module 中实现同名同签名的 `startup function`
- Platform 的 effect / foreign API 由 Platform 自己导出的普通模块定义（例如 `basic_cli.stdout`、`basic_cli.file` 一类模块中的 `foreign fn`）；应用代码像导入普通依赖模块一样导入这些 Platform 模块，编译器 / typechecker 通过依赖根与传递依赖解析去加载它们
- 当前 runtime 仍不是“从 Platform 包动态装载宿主实现”：`sporec` 现在只对 package-backed `basic-cli` 提供显式 host profile，把导入后的 `basic_cli.*` foreign surface 路由到内建 runtime handler；`handles` 元数据与 startup `spec` stacking 已写入契约，但除 startup shape 校验外，通用 enforcement 仍是后续工作
- 实现语言: 原生代码（Rust/C/编译后的 Spore）
- 测试: 换 mock Platform（确定性 handler）

### 实现技术栈（当前状态 + diagnostics 方向）
- **实现语言**: Rust（edition 2024, MSRV 1.90）
- **自举策略**: Rust bootstrap → 部分自举（Parser/TypeChecker/CostAnalyzer 等纯计算部分用 Spore 重写）
- **解析器**: 手写递归下降 + Pratt 运算符解析（调研 Rust/Zig/Roc/Unison/Elm/Gleam 全部手写）
- **当前执行 backend**: `sporec-codegen` 仍是 tree-walking interpreter（PoC）
- **native backend roadmap**: Cranelift（先）→ 后期可选加 LLVM
  - Cranelift 优势: 10x 编译速度、纯 Rust、函数级粒度（契合内容寻址）、原生 WASM
  - 14% 输出性能差距可接受，新语言不需要和 C 竞争
- **当前 CLI 框架**: bpaf 0.9
- **当前 LSP 服务器**: 轻量自研 JSON-RPC / LSP 实现（非 tower-lsp）
- **当前增量机制**: 自研依赖追踪与 hash 驱动缓存（非 salsa）
- **当前错误层**: parser / typeck / module 已有手写 typed error，其他边界仍混有 `Result<_, String>` / `Vec<String>`
- **diagnostics 下一步**:
  - `thiserror 2`：用于 crate 边界的结构化错误枚举
  - `ariadne 0.6`：只做人类可读 renderer，不下沉到 core compiler IR
  - `sporec-diagnostics`：共享 Diagnostic IR
  - `tracing` + `tracing-subscriber`：开发侧观测，不与用户诊断混用
- **内容寻址 Hash**: blake3
- **无 Comptime**: 不支持图灵完备的编译期执行（Zig 风格），const generics + 细化类型 + 代价模型已覆盖 95% 场景；v1.1 按需可加轻量 `const fn`

### 编译器输出与工具消费约定
- 这是仓库内保留的**高层行为约束**；错误码枚举、JSON schema、字段级协议由 `SEP-0006-compiler-architecture.md` 继续持有。
- 编译器输出同时服务 **人类开发者 / CI / LSP / Agent**。因此稳定约束应先锚定在**共享 Diagnostic IR**，再由默认文本、`--verbose`、`--json`、LSP adapter 各自投影出去。
- 当前 CLI JSON 仍有命令私有 schema；下一步要消除这种分叉，把稳定机器契约收敛到最小字段集：`code / severity / message / primary_span / secondary_labels / notes / help / related`。
- 默认文本输出应尽量 help-rich，但 `help` 本身保持可选：有明确下一步时就给 `help:`，没有时也不能伪造建议。
- 错误码分类继续固定为 `E/W/C/K/H/M` 六大族：类型、警告、能力、代价、hole、模块。`sporec explain CODE` 是统一的长解释入口，避免把长篇错误说明散落在旧文档中。
- Hole 诊断是 **note / partial-state signal**，不是单独导致退出失败的错误；只有真实类型/能力/代价/模块错误才使编译返回非零状态。这一点是后续 hole workflow、watch mode 与 CI 兼容性的基础。
- 第一阶段先稳定 **batch diagnostics**；`watch --json` 的 richer event stream 是后续 transport 层，不应先于共享 Diagnostic IR 自己发散成另一套字段语义。

### 增量编译、Watch 与 Hole 协作
- 完整协议已交给 `SEP-0005-hole-system.md` 与 `SEP-0006-compiler-architecture.md`；这里保留不应丢失的**工作流与架构约束**。
- **双 hash 决策树** 是 watch / cache 设计核心：`impl hash` 不变则本模块直接跳过；`impl` 变但 `sig hash` 不变则只重编本模块；`sig hash` 改变时才沿依赖图向下游传播。
- `sig hash` 只覆盖公开接口、能力要求与 cost annotation；私有实现、注释、内部 hole 状态不应触发下游级联。这样保证“改实现不改接口”仍是局部反馈。
- `spore watch` 的触发语义是 **保存后编译**，不是对半编辑 buffer 做每击键分析；watch 输出面向终端与 LSP/Agent 共用，默认要能在失败后继续工作并保留最近一次可用依赖图。
- Hole 协作保留单一主循环：`DISCOVER → ANALYZE → PROPOSE → VERIFY → ACCEPT/REJECT`。其中 `DISCOVER` 在长期目标里可来自 `spore watch --json` 的 hole 图事件；当前稳定面仍可依赖 batch diagnostics 与 `sporec query-hole <file> <id> --json` 组合完成 `ANALYZE`。`REJECT` 必须返回结构化 root cause 与 fix hints，而不是只给一段文本。
- 填洞仍采用**单 hole 原子提交**的约束：一次只替换一个 hole、再交给增量编译验证。这样能把诊断、候选排序、依赖图更新和 agent 重试策略都锚定在可回滚的最小步上。
- v0.1 watch 的目标是“增量编译 + 实时诊断 + hole 进度”，而不是运行时状态保持式 hot reload；因此旧热重载调研中的 Erlang/Smalltalk/Pharo 路线只作为参考，不进入当前承诺面。

### 模块、包与 Platform 的工程约束
- 细节规范归 `SEP-0008-module-package-system.md`；本文保留的是仓库内部最稳定的工程结论。
- **模块布局**：`src/` 下路径直接决定模块名；推荐 `types.sp` 承载目录共享类型、`shortcuts.sp` 承载公开别名；测试模块位于 `test/`，可消费 `pub` 与 `pub(pkg)` API。模块段名继续采用 lowercase `snake_case`。
- **可见性模型**：仅保留 private / `pub(pkg)` / `pub` 三层；Hole 候选搜索、诊断与导出 API 视图都必须尊重同一套可见性边界，不再另有旧文档中的模块例外规则。
- **包管理心智模型**：`spore.toml` 负责声明依赖意图，`.spore-lock` 负责 pin 精确解析结果；哈希才是兼容性与复现的权威，human-readable version/tag 只作发现与沟通用途，不重新引入 semver-first 解析。
- **依赖粒度**：继续支持 `sig`-only 依赖与 `sig+impl` 完整依赖。前者服务接口耦合和增量检查，后者服务实际构建与发布；两者都建立在双 hash 身份模型上。
- **缓存与分发**：默认保持 Git-first、内容寻址、全局去重缓存；`.spore-store` 作为本地缓存与后端抽象入口，可兼容 local path、registry、IPFS 等来源，但仓库内主叙述仍以 Git / 本地路径为第一优先级。
- **清理与维护工作流**：依赖增删改仍围绕 `spore add` / `spore update` / `spore remove` / `spore gc` 展开；GC 的语义是“以锁文件可达集为根清理未引用哈希”，而不是按版本号或发布时间做启发式删除。
- **Platform 契约**：Platform 仍是普通包形态的语言级概念。当前 MVP 约定 **manifest 中的 `[platform]` 元数据 + 专门的 contract module** 共同构成权威：manifest 负责定位 contract module / startup contract symbol / adapter / handled capabilities，contract module 则通过带 hole 的 startup 定义持有权威签名与 spec；应用代码不直接持有 IO 实现。
- **项目 entry vs Platform startup contract**：`entry` 是项目层选择的执行目标，解析到某个源文件 / 模块；Platform 不拥有这个模块，但会对该模块中的 `startup function` 施加契约校验。这样可以把“选哪个模块运行”和“该模块里函数签名必须长什么样”明确拆开。
- **单 Platform / 项目**：一个 manifest-backed 项目只绑定一个 Platform。之所以先收缩到这一模型，是为了避免 handler 路由、priority 规则、测试替身和诊断语义同时膨胀；多个可执行目标先通过命名 `entry` 建模，而不是通过同一项目同时绑定多个 Platform 建模。跨 Platform 组合保留为未来专题设计，而不是 v0.1 承诺面。
- **测试与替身**：mock / test / record-replay Platform 是 durable 设计结论，不是附带示例。Spore 的“应用代码保持纯净、IO 由 Platform 承担”必须直接转化为可重复测试、确定性重放和平台替换能力。

### 编译器 Pipeline 架构（当前执行路径 + 目标 native pipeline）

- 当前默认可运行路径仍是 parser / typechecker / module resolution 之后进入解释器。
- 下图描述的是 **Cranelift skeleton 及其后续扩展落地后的目标 native pipeline**，不是当前默认执行实现。

```
Source → [Lex] → Tokens → [Parse] → AST → [Resolve+Desugar] → HIR → [TypeCheck+CapCheck+CostCheck] → TypedHIR → [Codegen] → Cranelift IR → Native
```

**目标上保持 3 层 IR + Cranelift IR 充当 LIR**（无独立 flat IR，无 MIR）

#### AST（Abstract Syntax Tree）
- 原始语法树，与源码 1:1 对应
- 所有节点带 `Span`（源码位置）
- 保留所有语法糖（`|>`、`?`、`f"..."`）
- 用途: 错误报告指向源码、IDE 语法高亮

#### HIR（High-level IR）
- 由 Resolve+Desugar pass 生成
- **脱糖**: `|>` → 函数调用, `?` → match on Result, `f"..."` → format 调用
- **名称解析**: 所有标识符绑定到声明
- **导入解析**: 模块路径解析为具体模块引用
- **Hole 记录**: 标记 `?name` 位置，记录上下文
- **sig hash 在此层计算**: 签名（参数/返回/错误集/效果/能力/代价声明）hash，签名不变则下游免重新检查

#### TypedHIR（Typed High-level IR）
- 由 TypeCheck+CapCheck+CostCheck 统一 pass 生成
- **类型推断**: 所有表达式都有确定类型（双向类型推断）
- **能力验证**: 函数体能力使用 ⊆ 声明能力集
- **代价计算**: 抽象解释计算代价，验证 ≤ 声明上界
- **穷尽检查**: match 表达式穷尽性验证
- **错误集传播**: `! Errors` 类型一致性
- **细化类型检查**: L0 可判定谓词 + L1 抽象解释传播
- **Hole 报告生成**: 完整上下文（类型/绑定/能力/代价预算/候选函数）
- **impl hash 在此层计算**: 类型检查通过后的完整 AST hash，部分函数（含 hole）为 None

#### 增量依赖追踪（当前实现与目标）
```text
SourceFile { path, contents }
  → lex
  → parse
  → resolve
  → type_check
  → execute_backend
```
- 当前实现依赖自研 hash / 依赖图决策，而不是把 `salsa` 当成已落地前提
- 当前 compile/run 路径仍偏 eager：文件内容变更后会重新 lex/parse/check，而不是已经把基于 `sig hash` / `impl hash` 的跳过规则完整接进主路径
- 目标上的增量规则仍然是：`sig hash` 不变 → 下游模块跳过 resolve + type_check
- 当前默认执行路径在这里进入解释器；进入 native backend 后，`impl hash` 不变才允许跳过 codegen（future Cranelift 缓存命中）

#### Cranelift skeleton 里程碑（下一 backend slice）
- 这是一条**里程碑定义**，不是“后端立刻全量替换解释器”的承诺。目标是先证明 native lowering 路径、最小 ABI 和回归比较方式成立，再逐步扩展到 richer runtime / data layout。
- **最小可编译子集**
  - 只覆盖 **单模块 / 单 entry** 的纯计算程序；不碰 package imports、project runtime、Platform adapter、`foreign fn`、`perform` / `handle`、并发、`Ref[T]`、spec runner。
  - 只覆盖**单态（monomorphic）顶层函数**；不把 closure capture、运行时泛型实例化、task/channel 值放进第一阶段 backend。
  - 表达式子集限定为：字面量、`let`、block、`if`、一元/二元算术与比较、布尔逻辑、具名函数直接调用、tail-expression 返回。
  - 第一阶段跨 backend 边界只接受**固定大小标量值**；`Str`、`List`、`Struct`、`Enum`、`Map`、`Option`、`Result` 以及任何 HostValue 映射都继续留在解释器路径，等数据布局 / 跨边界语义冻结后再扩展。
- **runtime hook 假设**
  - `sporec-codegen` 继续保持“统一执行 crate”；Cranelift skeleton 先作为其内部 backend 选项存在，而不是立即拆出新的公开运行时产品面。
  - `spore run`、manifest project runtime、package-backed Platform execution、spec 测试、watch 与 hole 相关执行路径在 skeleton 阶段继续默认走解释器；backend 先只服务内部 parity 测试与最小 native smoke。
  - skeleton backend 遇到 effect / foreign / package runtime / unsupported aggregate 时必须**显式报错**，不做静默回退或半支持。
  - 仍坚持“无独立 MIR”的方向：第一阶段可以从 checked IR/HIR 直接 lower 到极小的 backend-local function lowering，或直接 lower 到 Cranelift IR，但不要先为 backend 引入一层失控的新中间表示。
- **interpreter vs backend 对照里程碑**
  1. **M0 — scalar parity**：同一组纯函数 fixture 在解释器与 Cranelift skeleton 下得到相同结果 / 相同失败形状；覆盖整数 / 布尔 / unit、局部绑定、分支、直接调用。
  2. **M1 — multi-function parity**：加入多函数调用链、递归或等价控制流 fixture，确认 lowering 后的调用约定与返回约定稳定。
  3. **M2 — data-layout gate**：等 `HostValue` / ADT / `Option` / `Result` 边界冻结后，再引入结构体、枚举和更丰富返回值；这一步才允许讨论 backend ↔ host ABI。
  4. **M3 — platform gate**：等 package-backed Platform host model 不再只特判 `basic-cli` 后，再讨论 `foreign fn` / effect lowering，而不是把这些问题挤进 skeleton 阶段。
- **验收口径**
  - skeleton backend 不是默认执行路径；
  - 纯标量 fixture 的 parity 测试稳定通过；
  - 当前 package / Platform / diagnostics / watch 工作流不因 backend 原型而被迫改协议。

#### 设计决策记录
- **不需要 MIR**: 无 borrow checker，无需 CFG 级别分析
- **不需要 flat IR**: 无 comptime，当前 hash + 依赖图方案已覆盖主要增量缓存需求
- **能力+代价合并到 TypeCheck**: capability = trait，与类型信息交叉使用，减少 IR 转换
- **脱糖全在 Resolve 层**: `|>`/`?`/`f"..."` 均在进入 HIR 前脱糖，TypeCheck 不处理语法糖
- **不支持 Comptime**: const generics + 细化类型 + 代价模型已足够；Elm/Roc/Gleam 均无 comptime

## 后续维护重点
- [ ] 将仍与本文表面语法不一致的 SEP 草案回写统一（重点：syntax / effect / module-package）
- [ ] 补齐已知实现差距：Range `a..b`、并发 runtime、 richer cost expressions
- [ ] 在实现推进时保持本文、SEP 与 README 的交叉引用同步
