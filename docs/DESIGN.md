# Spore 语言设计

Spore（孢子）是一门编译型、通用目的的编程语言，以**意图作为一等公民**。
一个紧凑的孢子携带完整蓝图，在适宜环境中发育为完整有机体 — 这正是 Spore 程序的形态：
签名（孢子）携带完整上下文 → hole（萌芽） → 完整程序（有机体）。

## 核心原则

1. **签名是重力中心** — 函数签名就是完整规范：输入 / 输出、错误集、能力集、代价预算、spec。下游分析全部从签名展开，函数体可以为空。
2. **Hole 是协作点而非错误** — 带 hole 的程序编译成功。它是“部分”，不是“坏掉”。Hole 是人、Agent、团队之间结构化协作的主要机制，编译器为每个 hole 生成自包含的 HoleReport。
3. **先架构，后细节** — 推荐自顶向下：先定签名与类型，编译器验证骨架，再按依赖顺序填洞。
4. **编译器是文档助手** — 所有输出同时提供人类可读文本与机器可读 JSON，共享同一份 Diagnostic IR。
5. **显式的能力与代价模型** — 每个副作用必须声明 capability，每个计算代价可被编译期验证。从签名就能完整评估一个函数的影响。
6. **内容寻址，不用版本号** — 模块以内容 hash 标识（签名 hash 决定接口兼容性，实现 hash 决定缓存），无 semver，无钻石依赖冲突。

## 工具链定位

- **CLI**: `spore build` / `spore run` / `spore check` / `spore watch`
- **`sporec`**: 无状态编译器
- **`spore`**: 有状态的 Codebase Manager（依赖、lock、缓存、watch）

## 函数签名

```
fn name(params) -> ReturnType ! Errors
    where T: Constraint, U: Constraint
    uses [Capabilities]
    cost [compute, alloc, io, parallel]
    spec {
        example "name": expr
        property "law": |x: I32 when self >= 0| x
    }
{ body }
```

- 四个子句 `where` / `uses` / `cost` / `spec` 可以任意顺序出现；文档与 formatter 统一推荐这一顺序。
- `where` 用于 trait 约束，单一 `where` 子句内以逗号分隔多条约束。
- 函数属性（pure / deterministic / total）**从 `uses` 自动推断**，无需手动声明：`uses []` 即纯函数；非空 `uses` 依赖对应 capability 边界。
- `spec` 里 `example` 是具体断言，`property` 是“受限输入上的返回值规范”lambda；其参数必须与函数输入逐一对应，但可以把某个参数收窄成细化后的**输入子集**，lambda 本身直接返回该子集上的期望结果。例如 `fn abs(x: I32) -> I32` 可写 `property "law": |x: I32 when self >= 0| x`，表示在非负输入上 `abs` 应直接返回 `x`，由测试基础设施对比函数实际返回值。
- 细化类型谓词用 `when self > 0`，隐式 `self` 绑定。
- 错误集写作 `! E1 | E2`，管道符分隔。
- 代价固定为四维向量 `cost [compute, alloc, io, parallel]`。

完整表面语法决策见 [`decisions/syntax.md`](decisions/syntax.md)，规范见 SEP-0001。

## 类型系统

- **Nominal 为主**，配合匿名结构体记录的 structural 形态。
- **签名显式标注，函数体内推断**（双向推断）。
- **Sealed enum + 穷尽匹配**；完整模式匹配（嵌套 / guard / or-pattern）。
- **关联类型 + GAT**；不引入 HKT。
- **细化类型** 分两层：L0 可判定谓词（`when self > 0`）+ L1 抽象解释传播，不依赖 SMT。
- **Const generics**（值级类型参数）。
- **Trait 约束与 effect / capability 在类型层分离**：`where` 用于 trait，`effect` / `uses` / `perform` 用于 effect。
- **`@allows` 注解**：在 hole 级别约束 Agent 可用函数集。
- 基本类型：`I32` / `I64` / `U32` / `U64` / `F32` / `F64` / `Int` / `Float` / `Bool` / `Str` / `()` + `List[T]` / `Map[K,V]` / `Set[T]`。`Int` / `Float` 是 `I64` / `F64` 的别名。

## 能力与 Effect

- **内置 capability**：`Console` / `FileRead` / `FileWrite` / `NetConnect` / `NetListen` / `Env` / `Spawn` / `Clock` / `Random` / `Exit`。
- **自定义 effect 接口**：`effect Name { ... }`；支持 `effect Name = A | B` 聚合别名。
- **调用语法**：`perform Effect.op(...)`；该 effect 必须有显式声明。
- **Handler**：`handle { ... } with { ... }` 为词法作用域、不可恢复（non-resumable）、one-shot。
  - `with { ... }` 内部可以 `use HandlerName { ... }` 安装命名 handler 实例，或以 `on Effect.op(...) => ...` 定义匿名 inline arm。
  - 顶层命名 handler 语法：`handler <Effect> as <HandlerName>(...) { ... }`。
  - 命中规则：**内层优先**；同一 `with` 块内两个绑定命中同一 `Effect.op` 即编译错误。
- **推断规则**：`uses` 为空 → 自由函数；有依赖未声明 → 不完整函数；已声明 → 验证一致性。

完整语义见 SEP-0003。

## 代价模型

- 四维 **CostVector**：`compute(op) + alloc(cell) + io(call) + parallel(lane)`。
- 编译期通过**抽象解释**模拟执行，验证 ≤ 声明上界。
- 支持符号代价表达式；编译器从被调用函数自动推导。
- `unbounded` 函数必须由 `with_cost_limit` 包裹。

完整规范见 SEP-0004。

## Hole 系统

- 语法：`?name` 或 `?name : Type`。
- 带 hole 的 partial 函数**可编译、可模拟、不可执行**。
- 编译器按依赖图对 hole 排序（传递依赖者数量降序），推荐填洞顺序。
- **HoleReport (JSON)** 包含完整上下文：类型、可见绑定、能力预算、代价预算、候选函数、`@allows` 约束。
- 填洞遵循**单 hole 原子提交**：一次替换一个 hole，再交增量编译验证。
- Hole 诊断是 *note / partial-state signal*，不导致编译失败；只有真实类型 / 能力 / 代价 / 模块错误才返回非零状态。
- 协作主循环：`DISCOVER → ANALYZE → PROPOSE → VERIFY → ACCEPT/REJECT`，`REJECT` 必须返回结构化 root cause 与 fix hints。

完整协议见 SEP-0005。

## 模块与包

- **一文件一模块**：`src/billing/invoice.sp` → `billing.invoice`。无 `module` 头声明。
- **可见性**：private（默认）/ `pub(pkg)` / `pub`。Hole 候选搜索、诊断、导出 API 全部尊重同一套边界。
- **双 hash 身份**：
  - **sig hash** — 公开接口 + 能力声明 + cost 声明的 hash，用于接口兼容性与下游是否需要重查。
  - **impl hash** — 类型检查通过后的完整 AST hash，用于缓存与 codegen 跳过；含 hole 的函数为 `None`。
- **双 hash 决策树**：`impl hash` 不变 → 跳过本模块；`impl` 变但 `sig hash` 不变 → 只重编本模块；`sig hash` 变 → 沿依赖图向下游传播。
- **禁止循环依赖**（Elm 风格）。
- **导入**：`import mod as alias`，无通配符、无选择性导入、无隐式嵌套。
- **依赖粒度**：`sig`-only 依赖服务接口耦合与增量检查；`sig+impl` 依赖服务实际构建。
- **包管理**：完全内容寻址。`spore.toml` 声明依赖意图，`.spore-lock` pin 精确 hash；哈希是兼容性与复现的权威，human-readable tag 仅用于发现与沟通。
- **存储**：本地 `.spore-store` + Git-first，后端可插拔（local path / registry / IPFS）。
- **维护工作流**：`spore add` / `update` / `remove` / `gc`；GC 语义是“以锁文件可达集为根清理未引用哈希”。
- **钻石依赖**：不同 hash 直接共存，无冲突。
- **无 Functor**：用 generics + traits 替代。

完整规范见 SEP-0008。

## Platform 系统

Platform 是语言级概念，以**普通包**形态存在，提供全部 IO effect handler — 应用代码保持纯净。

- **单 Platform / 项目**：一个 manifest-backed 项目绑定一个 Platform；多可执行目标通过命名 `entry` 建模，而不是同时绑定多个 Platform。
- **Platform 契约**：manifest 的 `[platform]` 元数据 + 专门的 **contract module** 共同构成：
  - manifest 定位 contract module、startup contract symbol、adapter、handled capabilities；
  - contract module 通过带 hole 的 `startup function` 持有权威签名与 spec。
- **项目 entry vs startup contract**：`entry` 是项目层选择的执行目标，Platform 对该模块中的 `startup function` 施加契约校验；“选哪个模块运行”与“该模块里函数签名必须长什么样”分开。
- **Spec stacking**：Platform contract 的 startup spec 与应用实现侧 spec 是叠加约束。
- **Platform 的 effect / foreign API** 由 Platform 导出的普通模块定义（如 `basic_cli.stdout`、`basic_cli.file` 中的 `foreign fn`），应用代码像导入普通依赖一样导入它们。
- **FFI 边界**：**Platform 是唯一 FFI 表面**，应用代码不直接声明裸 native FFI。
- **测试**：换用 mock Platform 即得到确定性 handler，支持 record-replay。

## 并发模型

- **结构化并发**：子任务树，父等所有子完成；父取消 → 子自动取消，协作式。
- **`Spawn` 是 capability**，没有 async/await 着色。
- **消息传递**：`Channel[T]`，无共享可变状态。
- **代价**：lane 作为 `parallel` 维度，编译器从被调用函数推导。模拟时保守取 lane 间 max cost。

完整规范见 SEP-0007。

## 语法

- 完全 **expression-based**（`if` / `match` 都有返回值）。
- 大括号 `{}`；分号 Rust 语义（有分号 = 语句，无分号 = 返回表达式）。
- **管道 `|>`** 是唯一特殊操作符；不允许自定义操作符。
- 字符串：`f"..."`（插值）/ `t"..."`（类型化）/ `r"..."`（raw）。
- 错误：`! Errors` 签名契约 + `throw expr` + `?` 传播糖；`?` 在调用边界受检。
- Lambda：`|x, y| x + y`。
- 绑定：`let` 不可变 + shadowing；`Ref[T]` 可变容器。
- 无循环：递归 + 高阶函数（`map` / `fold` / `filter`），编译器保证 TCO。
- 泛型用 `[T]`（非 `<T>`）。
- 模式匹配：`match`，穷尽 + 嵌套 + guard + or-pattern。
- 类型注解后置：`name: Type`。
- trait 实现：`impl Trait for Type { ... }`。
- 注释：`//` / `///` / `/* */`（可嵌套）。

## 标准库（极简）

- **Prelude（自动可用）**：基本数值 / `Bool` / `Str` / `()` / `Option[T]` / `Result[T,E]` / 基本操作符 / `|>` / `?`。
- `spore.list` — `List[T]`：map / fold / filter / zip / head / tail / len / reverse / sort / ...
- `spore.map` — `Map[K,V]`：insert / get / remove / keys / values / merge / ...
- `spore.set` — `Set[T]`：add / remove / contains / union / intersect / diff / ...
- `spore.str` — `Str` 扩展：split / join / trim / contains / starts_with / replace / ...
- `spore.math` — abs / min / max / pow / sqrt / ...
- `spore.ref` — `Ref[T]` 可变容器。
- 其余（JSON / HTTP / 正则 / 时间 / ...）全部第三方。

完整规范见 SEP-0009。

## 编译器输出与诊断

编译器输出同时服务**人类开发者 / CI / LSP / Agent**。稳定契约锚定在共享的 **Diagnostic IR** 上，默认文本 / `--verbose` / `--json` / LSP adapter 只是同一诊断对象的不同投影。

**最小稳定字段集**：

```
code / severity / message / primary_span / secondary_labels / notes / help / related
```

- **错误码族**：`E`（类型）/ `W`（警告）/ `C`（能力）/ `K`（代价）/ `H`（hole）/ `M`（模块）。
- `sporec explain CODE` 是统一的长解释入口，避免把长篇错误说明散落在文档中。
- 默认文本输出应 help-rich，但 `help` 字段本身保持可选：没有明确下一步时不伪造建议。
- Auto-fix 作为后续 code-action / edit 层，不并入最小 IR。

## 编译器 Pipeline

```
Source → [Lex] → Tokens → [Parse] → AST
       → [Resolve+Desugar] → HIR
       → [TypeCheck+CapCheck+CostCheck] → TypedHIR
       → [Codegen] → Cranelift IR → Native
```

**3 层 IR + Cranelift IR 作为 LIR**，无独立 flat IR，无 MIR。

### AST
- 与源码 1:1 对应，节点带 `Span`；保留所有语法糖（`|>` / `?` / `f"..."`）。
- 用途：错误报告、IDE 语法高亮。

### HIR
- Resolve + Desugar pass 产物。
- **脱糖**：`|>` → 函数调用，`?` → match on `Result`，`f"..."` → format 调用。
- 名称解析、导入解析、hole 记录。
- **`sig hash` 在此层计算**。

### TypedHIR
- TypeCheck + CapCheck + CostCheck 合并 pass 产物。
- 类型推断、能力验证（函数体使用 ⊆ 声明）、代价抽象解释、穷尽检查、错误集传播、细化类型检查、HoleReport 生成。
- **`impl hash` 在此层计算**（含 hole 函数为 `None`）。

### 设计决策

- **不需要 MIR**：无 borrow checker，不需要 CFG 级分析。
- **不需要独立 flat IR**：当前 hash + 依赖图已覆盖增量缓存。
- **能力 + 代价合并进 TypeCheck**：减少 IR 转换；trait 约束与 effect 接口在该层交汇但不合并为同一语法实体。
- **脱糖全在 Resolve 层**：TypeCheck 不处理语法糖。
- **不支持 Comptime**：const generics + 细化类型 + 代价模型已足够。

## 实现技术栈

- **实现语言**：Rust（edition 2024）。
- **自举策略**：Rust bootstrap → 纯计算组件（Parser / TypeChecker / CostAnalyzer 等）逐步用 Spore 重写。
- **解析器**：手写递归下降 + Pratt。
- **Backend**：Cranelift 为主（纯 Rust、快编译、原生 WASM、函数级粒度契合内容寻址）；LLVM 作为未来可选高性能后端。
- **CLI 框架**：bpaf。
- **LSP**：自研 JSON-RPC / LSP 实现。
- **增量**：自研依赖追踪 + hash 驱动缓存。
- **诊断栈**：`thiserror` 用于 crate 边界结构化错误；`ariadne` 仅做人类可读 renderer；`sporec-diagnostics` 持有共享 Diagnostic IR；`tracing` 用于开发侧观测，不与用户诊断混用。
- **内容寻址 Hash**：blake3。

## Watch 与增量

- `spore watch` 语义是**保存后编译**，不是每击键分析；也不是运行时状态保持式热重载。
- watch / batch / LSP 共享同一份 Diagnostic IR；`watch --json` 的 NDJSON 事件流只是其上的 transport 层。
- `sig hash` 仅覆盖公开接口 / 能力 / cost；私有实现、注释、内部 hole 状态不触发下游级联 — “改实现不改接口”仍是局部反馈。
- watch 失败后仍须继续工作，保留最近一次可用依赖图。

## 文档治理

- 本文件是仓库内**唯一主设计文档**，维护跨主题的 durable 设计结论。
- 规范级长文存放于 sibling repo `spore-evolution/seps/`（SEP-0001 ~ SEP-0009）。
