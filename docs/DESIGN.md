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

## 已确定的设计

### 函数签名语法（混合式 v0.3）
```
fn name(params) -> ReturnType ! [Errors]
    where T: Constraint
    uses [Capabilities]
    cost ≤ N
{ body }
```

> **v0.2→v0.3 变更**: 原 `where { ... }` 统一块拆分为独立子句：
> - `where T: Constraint` — 泛型约束（保留 where 关键字）
> - `uses [Capabilities]` — 能力集声明（独立子句）
> - `cost ≤ N` — 代价上界（独立子句）
>
> 细化类型谓词语法同步变更: `where |n| n > 0` → `if |n| n > 0`
>
> **v0.3→v0.4 变更**: 移除 `with [...]` 子句。函数属性（pure, deterministic, total）从 `uses` 自动推断：
> - `uses []` → pure + deterministic + total
> - `uses [Compute]` → deterministic（纯计算，无副作用）
> - 含 IO/State capability → 非 pure
> - 编译器自动验证一致性，无需手动声明

### 能力集系统
- 内置: Compute, FileRead, FileWrite, NetRead, NetWrite, Clock, Random, StateRead, StateWrite, Spawn
- 自定义: `capability Name = [...]`（capability = trait，统一机制）
- 推断规则: 无依赖→自由函数 / 有依赖未声明→不完整函数 / 声明→验证一致性
- FuncCall/Module 已移除，用 @allows + 调用图查询替代

### 抽象代价模型
- 四维: compute(op) + alloc(cell) + io(call) + parallel(lane)
- 签名显示标量，查询显示明细
- 编译时模拟执行（抽象解释）
- 符号代价表达式支持
- unbounded 函数需 with_cost_limit 包裹

### Hole 系统（v0.2）
- 语法: `?name` 或 `?name : Type`（无 @priority，已移除）
- 填洞顺序: 编译器依赖分析推荐，按传递依赖者数量降序排列
- Partial 函数可编译、可模拟、不可执行
- HoleReport JSON: 完整上下文（类型、绑定、能力、代价预算、候选函数）
- CLI: `sporec --holes`, `sporec --query-hole ?name`, `spore holes --suggest-order`

### 模块系统（v0.1）
- 一文件一模块: `src/billing/invoice.spore` → `billing.invoice`
- 双 hash 寻址:
  - 签名 hash (sig): 接口兼容性检查，接口不变则不变
  - 实现 AST hash (impl): 内容寻址，编译成功后分配，部分函数为 None
- 可见性: 默认私有 / `pub(pkg)` 包内可见 / `pub` 完全公开
- 禁止循环依赖（Elm 风格）
- Platform 概念（Roc 风格）: Platform 提供所有 IO 能力，纯包无法直接 IO
- 无 Functor: 泛型 + 能力集替代
- 模块能力封顶: `module billing uses [...]`，可省略自动推断 + `--fixes` 修复
- 导入: `import mod as alias` / `alias x = mod.item`（无通配符、无隐式嵌套）

### 类型系统（v0.1）
- Nominal 为主 + 匿名结构体记录（structural）
- Capability = Trait（统一机制，capability 是 trait 语法糖）
- 关联类型 + GAT 支持
- 无 HKT（关联类型 + GAT 已足够，避免错误信息灾难）
- 细化类型: L0 可判定谓词（`if |n| n > 0`）+ L1 抽象解释传播（无 SMT）
- Sealed enum（穷尽匹配）
- 签名必须完整注解，函数体内推断
- Const generics（值级类型参数）
- 完整模式匹配: 穷尽 + 嵌套 + guard + or-pattern
- @allows: hole 级 Agent 约束（限制填洞可用函数）
- FuncCall/Module 已移除，用调用图查询替代

### 编译器输出格式（v0.1）
- 三种模式（非分级）: 默认文本 / --verbose / --json
- 默认: Rust 风格简洁指向 + 色彩 + 总是附带 help 修复建议
- --verbose: 默认 + 推导链 + 候选类型 + 能力/代价上下文
- --json: LSP 兼容 + Spore 扩展，完整超集
- 错误编码: E0xxx(类型) / W0xxx(警告) / C0xxx(能力) / K0xxx(代价) / H0xxx(hole) / M0xxx(模块)
- `sporec --explain CODE` 查看详细解释
- `sporec --fixes` / `sporec --unsafe-fixes` 自动修复

### 并发模型（v0.1）
- 结构化并发: 子任务树，父等所有子完成
- 效果处理器: Spawn 是 capability/effect，无 async/await 着色
- 代价推导: 编译器从被调用函数推导，无需手动分配
- 消息传递: Channel[T]，无共享可变状态
- 取消传播: 父取消 → 子自动取消，协作式
- 模拟: 保守取 max cost across lanes

### 包管理（v0.1）
- 完全内容寻址，无传统注册中心
- 模块级寻址粒度（一个 .spore 文件 = 一个可寻址单元）
- 存储: 本地 .spore-store + Git 默认，后端可插拔
- 无 semver，纯 hash + 命名别名（sig hash = 兼容性检查）
- 清单: spore.toml（元数据+依赖）+ .spore-lock（精确 hash pin）
- 钻石依赖: 无冲突，不同 hash 共存
- Platform 不特殊化，只是提供 IO 能力的普通包
- 能力封顶两层: 项目级 spore.toml ≥ 模块级
- 编译器自动推导 + `--fixes` 补全

### 增量编译与 Watch 模式（v0.1）
- 核心: 增量编译 + 实时诊断 + hole 状态更新（非运行时热替换）
- 粒度: 模块级（sig hash 不变 → 下游免检）
- 触发: 文件保存即触发（`spore watch`）
- 输出: 实时编译诊断 + hole 进度报告
- LSP 集成: `spore watch --json` 作为 LSP 后端

### 语法设计（v0.2）
- 完全 expression-based（if/match 都有返回值）
- 大括号 `{}`，分号 Rust 语义（有分号=语句，无分号=返回表达式）
- 管道 `|>` 操作符，不允许自定义操作符
- 字符串: f-string `f"..."` + t-string `t"..."` + raw `r"..."`
- 错误: `! [Errors]` + `?` 操作符
- Lambda: `|x, y| x + y`
- 注释: `//` / `///` / `/* */`（可嵌套）
- 绑定: `let` 不可变 + shadowing，`Ref[T]` 可变容器（需 StateWrite）
- 模式匹配: `match`（穷尽 + 嵌套 + guard + or-pattern）
- 无循环: 递归 + 高阶函数（map/fold/filter），编译器保证 TCO
- 条件: `if cond { a } else { b }` 表达式
- 类型注解后置: `name: Type`
- trait 实现内联: `implements [...]`（Roc 风格）
- `struct` 记录 + `type` 枚举/ADT
- `capability` 关键字（= trait）
- 函数属性（pure, deterministic, total）— 从 `uses` 自动推断，无需关键字
- `if` 子句用于细化类型谓词（`if |n| n > 0`），不再使用 `where`
- `where` 关键字仅保留用于泛型约束（`where T: Constraint`）
- 基本类型: I32/I64/U32/U64/F32/F64/Bool/Str + List[T]/Map[K,V]/Set[T]

## 设计文档索引

### 规格文档 (docs/specs/)
- [syntax-spec-v0.1.md](specs/syntax-spec-v0.1.md) — 语法规格（含签名语法详解，原 signature-syntax-v0.2 已合入附录 B）
- [type-system-v0.1.md](specs/type-system-v0.1.md) — 类型系统设计
- [effect-algebra-v0.1.md](specs/effect-algebra-v0.1.md) — 效果代数设计
- [cost-analysis-v0.1.md](specs/cost-analysis-v0.1.md) — 代价分析综合规范（整合原 cost-model / cost-decidability / recursion-analysis）
- [hole-report-v0.3.md](specs/hole-report-v0.3.md) — Hole 报告规范（正式版）
- [hole-dependency-graph-v0.1.md](specs/hole-dependency-graph-v0.1.md) — Hole 依赖图
- [compiler-output-v0.1.md](specs/compiler-output-v0.1.md) — 编译器输出格式设计
- [incremental-compilation-v0.1.md](specs/incremental-compilation-v0.1.md) — 增量编译与 Watch 模式
- [module-system-v0.1.md](specs/module-system-v0.1.md) — 模块系统设计（含双 hash）
- [package-management-v0.1.md](specs/package-management-v0.1.md) — 包管理系统设计
- [platform-system-v0.1.md](specs/platform-system-v0.1.md) — Platform 系统设计
- [concurrency-model-v0.1.md](specs/concurrency-model-v0.1.md) — 并发模型设计

### 调研文档 (docs/research/)
- [syntax-comparison-v0.1.md](research/syntax-comparison-v0.1.md) — 参考语言语法对比
- [module-system-research.md](research/module-system-research.md) — 10 语言模块系统调研
- [type-research-dependent.md](research/type-research-dependent.md) — 依赖类型调研（7 语言）
- [type-research-practical.md](research/type-research-practical.md) — 实用类型系统调研（7 语言）
- [type-research-tradeoffs.md](research/type-research-tradeoffs.md) — 类型系统权衡分析
- [concurrency-research.md](research/concurrency-research.md) — 13 并发模型调研
- [pkg-management-research.md](research/pkg-management-research.md) — 10 语言包管理调研
- [hot-reload-research.md](research/hot-reload-research.md) — 12 系统热重载调研
- [syntax-research.md](research/syntax-research.md) — 10 语言语法设计调研
- [impl-stack-research.md](research/impl-stack-research.md) — 10 语言编译器实现栈调研
- [codegen-comparison.md](research/codegen-comparison.md) — LLVM vs Cranelift 深度对比
### 归档文档 (docs/archive/)
- [hole-system-v0.2.md](archive/hole-system-v0.2.md) — ⚠️ 已被 hole-report-v0.3.md 取代，保留供历史参考

### 标准库（极简）
- **Prelude（自动可用）**: I32/I64/U32/U64/F32/F64/Bool/Str, Option[T], Result[T,E], 基本操作符, |>, ?
- **spore.list** — List[T]: map/fold/filter/zip/head/tail/len/reverse/sort/...
- **spore.map** — Map[K,V]: insert/get/remove/keys/values/merge/...
- **spore.set** — Set[T]: add/remove/contains/union/intersect/diff/...
- **spore.str** — Str 扩展: split/join/trim/contains/starts_with/replace/...
- **spore.math** — 数学函数: abs/min/max/pow/sqrt/...
- **spore.ref** — Ref[T] 可变容器（需 StateWrite capability）
- 其余全部第三方（JSON/HTTP/正则/时间等）

### Platform 系统（v0.1）
- 语言级概念，spore.toml 中声明 `platform = "git:url"`
- 提供所有 IO effect handler（应用代码完全纯净）
- Effect handler 风格（与并发模型统一）
- 不内置官方 Platform，全部第三方
- 支持多 Platform（优先级指定，编译器检查无冲突）
- Platform 契约: capability 集合 + handler 实现 + 入口点类型
- 实现语言: 原生代码（Rust/C/编译后的 Spore）
- 测试: 换 mock Platform（确定性 handler）

### 实现技术栈（已确定）
- **实现语言**: Rust（edition 2024, MSRV 1.90）
- **自举策略**: Rust bootstrap → 部分自举（Parser/TypeChecker/CostAnalyzer 等纯计算部分用 Spore 重写）
- **解析器**: 手写递归下降 + Pratt 运算符解析（调研 Rust/Zig/Roc/Unison/Elm/Gleam 全部手写）
- **代码生成**: Cranelift（先）→ 后期可选加 LLVM
  - Cranelift 优势: 10x 编译速度、纯 Rust、函数级粒度（契合内容寻址）、原生 WASM
  - 14% 输出性能差距可接受，新语言不需要和 C 竞争
- **增量编译框架**: salsa（rust-analyzer 同款）
- **错误报告**: ariadne 0.6（gonidium 同款，JSON 模式自行序列化）
- **错误处理**: thiserror 2（gonidium 同款，结构化错误枚举）
- **CLI 框架**: clap 4 + derive（多子命令场景优于 bpaf）
- **LSP 服务器**: tower-lsp
- **内容寻址 Hash**: blake3
- **无 Comptime**: 不支持图灵完备的编译期执行（Zig 风格），const generics + 细化类型 + 代价模型已覆盖 95% 场景；v1.1 按需可加轻量 `const fn`

### 编译器 Pipeline 架构（v0.1）

```
Source → [Lex] → Tokens → [Parse] → AST → [Resolve+Desugar] → HIR → [TypeCheck+CapCheck+CostCheck] → TypedHIR → [Codegen] → Cranelift IR → Native
```

**3 层 IR + Cranelift IR 充当 LIR**（无独立 flat IR，无 MIR）

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
- **错误集传播**: `! [Errors]` 类型一致性
- **细化类型检查**: L0 可判定谓词 + L1 抽象解释传播
- **Hole 报告生成**: 完整上下文（类型/绑定/能力/代价预算/候选函数）
- **impl hash 在此层计算**: 类型检查通过后的完整 AST hash，部分函数（含 hole）为 None

#### salsa 集成
```
salsa::input  → SourceFile { path, contents }
salsa::tracked → lex(file) → parse(tokens) → resolve(ast) → type_check(hir) → codegen(typed_hir)
```
- 文件内容变更 → 重新 lex/parse
- sig hash 不变 → 下游模块跳过 resolve + type_check
- impl hash 不变 → 跳过 codegen（Cranelift 缓存命中）

#### 设计决策记录
- **不需要 MIR**: 无 borrow checker，无需 CFG 级别分析
- **不需要 flat IR**: 无 comptime，salsa 提供增量缓存
- **能力+代价合并到 TypeCheck**: capability = trait，与类型信息交叉使用，减少 IR 转换
- **脱糖全在 Resolve 层**: `|>`/`?`/`f"..."` 均在进入 HIR 前脱糖，TypeCheck 不处理语法糖
- **不支持 Comptime**: const generics + 细化类型 + 代价模型已足够；Elm/Roc/Gleam 均无 comptime

## 下一步
- [ ] 设计具体 IR 数据结构（AST with spans, HIR types, TypedHIR types）
- [ ] 规划 Phase 1 实现范围和任务
- [ ] 实现 Phase 1: Lexer → Parser → 基本类型检查 → hello-world codegen
- [ ] 11 份规格文档一致性审计
