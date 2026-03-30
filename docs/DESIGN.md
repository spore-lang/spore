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

### 函数签名语法（混合式 v0.2）
```
fn name(params) -> ReturnType ! [Errors]
where
    T: Constraint
    effects: pure, deterministic
    cost ≤ N
    uses [Capabilities]
{ body }
```

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
- 细化类型: L0 可判定谓词 + L1 抽象解释传播（无 SMT）
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

### 语法设计（v0.1）
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
- 基本类型: I32/I64/U32/U64/F32/F64/Bool/Str + List[T]/Map[K,V]/Set[T]

## 设计文档索引

### 规格文档
- files/signature-syntax-v0.2.md — 签名语法完整草案
- files/cost-model-v0.1.md — 代价模型完整设计
- files/hole-system-v0.2.md — Hole 系统完整设计
- files/module-system-v0.1.md — 模块系统设计（含双 hash）
- files/type-system-v0.1.md — 类型系统设计
- files/compiler-output-v0.1.md — 编译器输出格式设计
- files/concurrency-model-v0.1.md — 并发模型设计
- files/package-management-v0.1.md — 包管理系统设计
- files/incremental-compilation-v0.1.md — 增量编译与 Watch 模式
- files/syntax-spec-v0.1.md — 语法规格（生成中）

### 调研文档
- files/syntax-comparison-v0.1.md — 参考语言语法对比
- files/module-system-research.md — 10 语言模块系统调研
- files/type-research-dependent.md — 依赖类型调研（7 语言）
- files/type-research-practical.md — 实用类型系统调研（7 语言）
- files/type-research-tradeoffs.md — 类型系统权衡分析
- files/concurrency-research.md — 13 并发模型调研
- files/pkg-management-research.md — 10 语言包管理调研
- files/hot-reload-research.md — 12 系统热重载调研
- files/syntax-research.md — 10 语言语法设计调研

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

### 实现技术栈
- **实现语言**: Rust
- **自举策略**: Rust bootstrap → 部分自举（Parser/TypeChecker/CostAnalyzer 等纯计算部分用 Spore 重写）
- **代码生成**: Cranelift（先）→ 后期可选加 LLVM
  - Cranelift 优势: 10x 编译速度、纯 Rust、函数级粒度（契合内容寻址）、原生 WASM
  - 14% 输出性能差距可接受，新语言不需要和 C 竞争
- **增量编译框架**: salsa（rust-analyzer 同款）
- **解析器**: Rust 生态（pest/LALRPOP/手写递归下降，待定）
- **错误报告**: ariadne 或 miette
- **LSP 服务器**: tower-lsp
- **内容寻址 Hash**: blake3

### 调研文档索引（补充）
- files/impl-stack-research.md — 10 语言编译器实现栈调研
- files/codegen-comparison.md — LLVM vs Cranelift 深度对比
- files/platform-research.md — 9 语言 Platform/Effect 系统调研

## 下一步
- [ ] 创建 GitHub 仓库和项目骨架
- [ ] 实现 Phase 1: Parser + 基本类型检查 + Cranelift codegen
