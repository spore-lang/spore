# Spore 语言设计

Spore（孢子）是一门编译型、通用目的的编程语言，以**意图作为一等公民**。
一个紧凑的孢子携带完整蓝图，在适宜环境中发育为完整有机体 — 这正是 Spore 程序的形态：
签名（孢子）携带完整上下文 → hole（萌芽） → 完整程序（有机体）。

## 核心原则

1. **签名是重力中心** — 函数签名就是完整规范：输入 / 输出、错误集、effect 集、代价预算、spec。下游分析全部从签名展开，函数体可以为空。
2. **Hole 是协作点而非错误** — 带 hole 的程序编译成功。它是“部分”，不是“坏掉”。Hole 是人、Agent、团队之间结构化协作的主要机制，编译器为每个 hole 生成自包含的 HoleReport。
3. **先架构，后细节** — 推荐自顶向下：先定签名与类型，编译器验证骨架，再按依赖顺序填洞。
4. **编译器是文档助手** — 所有输出同时提供人类可读文本与机器可读 JSON，共享同一份 Diagnostic IR。
5. **显式的 effect 与代价模型** — 每个副作用必须声明 required effects，每个计算代价可被编译期验证。从签名就能完整评估一个函数的影响。
6. **内容寻址，不用版本号** — 模块以内容 hash 标识（签名 hash 决定接口兼容性，实现 hash 决定缓存），无 semver，无钻石依赖冲突。

## 组合语义（compositional semantics）rationale

- Spore 把 **effect / 错误集 / 代价 / laws** 都视为可组合、可规范化、可检查的语义对象；编译器内部优先复用同一套组合语义，而不是为每个系统各造一套特殊规则。
- 这套“组合”首先服务**工程实现与规范对齐**：handler discharge、错误集等价、剩余预算、law 组合都共享同一套合成 / 合并 / 顺序 / 规则检查思路。
- 这并**不**意味着引入用户可见的 `std.algebra` 风格表面。面向工程实现与文档组织的命名采用 `spore.combine` / `spore.merge` / `spore.order` / `spore.laws`；若某个模块在当前仓库里尚未完整落地，应视为本波目标组织，而不是已稳定交付事实。

## 工具链定位

- **CLI**: `spore build` / `spore run` / `spore check` / `spore watch`
- **`sporec`**: 无状态编译器
- **`spore`**: 有状态的 Codebase Manager（依赖、lock、缓存、watch）

## 函数签名

```
fn name(params) -> ReturnType ! Errors
    where T: Constraint, U: Constraint
    uses [Effects]
    cost [compute, alloc, io, parallel]
    spec {
        example "name": expr
        property "law": |x: I32 when self >= 0| x
    }
{ body }
```

- 四个子句 `where` / `uses` / `cost` / `spec` 可以任意顺序出现；文档与 formatter 统一推荐这一顺序。
- `where` 用于 trait 约束，单一 `where` 子句内以逗号分隔多条约束。
- 函数属性（pure / deterministic / total）**从 `uses` 自动推断**，无需手动声明：`uses []` 即纯函数；非空 `uses` 依赖对应 effect 边界。
- `spec` 里 `example` 是具体断言，`property` 是“受限输入上的返回值规范”lambda；其参数必须与函数输入逐一对应，但可以把某个参数收窄成细化后的**输入子集**，lambda 本身直接返回该子集上的期望结果。例如 `fn abs(x: I32) -> I32` 可写 `property "law": |x: I32 when self >= 0| x`，表示在非负输入上 `abs` 应直接返回 `x`，由测试基础设施对比函数实际返回值。
- 细化类型谓词用 `when self > 0`，隐式 `self` 绑定。
- 错误集写作 `! E1 | E2`，管道符分隔。
- 错误集采用 **canonicalization-first**：编译器先把传播链上的错误项展开、去重、规范化，再判断等价与兼容性；第一波只承诺规范化传播 / 等价语义，不要求新增 `error Alias = ...` 声明表面。
- 代价固定为四维向量 `cost [compute, alloc, io, parallel]`。

完整表面语法决策见 [`decisions/syntax.md`](decisions/syntax.md)，规范见 SEP-0001。

## 类型系统

- **Nominal 为主**，配合匿名结构体记录的 structural 形态。
- **签名显式标注，函数体内推断**（双向推断）。
- **Sealed enum + 穷尽匹配**；完整模式匹配（嵌套 / guard / or-pattern）。
- **关联类型 + GAT**；不引入 HKT。
- **细化类型** 分两层：L0 可判定谓词（`when self > 0`）+ L1 抽象解释传播，不依赖 SMT。
- **Const generics**（值级类型参数）。
- **Trait 约束与 effect system 在类型层分离**：`where` 用于 trait，`effect` / `uses` / `perform` 用于 effect。
- **`@allows` 注解**：在 hole 级别约束 Agent 可用函数集。
- 基本类型：`I32` / `I64` / `U32` / `U64` / `F32` / `F64` / `Int` / `Float` / `Bool` / `Str` / `()` + `List[T]` / `Map[K,V]` / `Set[T]`。`Int` / `Float` 是 `I64` / `F64` 的别名。

## Effect 系统

- **内置 atomic effects**：`Console` / `FileRead` / `FileWrite` / `NetConnect` / `NetListen` / `Env` / `Spawn` / `Clock` / `Random` / `Exit`。
- **自定义 effect 接口**：`effect Name { ... }`；支持 `effect Name = A | B` 聚合别名。
- **调用语法**：`perform Effect.op(...)`；该 effect 必须有显式声明。
- **统一 handler 模型（本波目标行为）**：普通 handler、mock handler、Platform handler 在语义上都是同一种 handler。目标声明形态是 `handler Name(params?) handles [EffectA, EffectB] uses [ImplEffects] { impl EffectA { ... } impl EffectB { ... } }`。
- **Discharge 规则（本波目标行为）**：`handle { ... } with { ... }` 仍是词法作用域、不可恢复（non-resumable）、one-shot；进入块后，被覆盖的 handled effects 会从内部 residual effect set 中 discharge，而 handler 自身 `uses [ImplEffects]` 会并回外层 residual。
- **覆盖规则**：**内层优先**；每个 `perform Effect.op(...)` 要么命中最近的匹配 handler，要么保留在外层 `uses` 集；同一作用域对同一 `Effect.op` 的重复命中是编译错误。
- 当前实现仍在逐步补齐这套统一声明与运行时，因此这里描述的是**本波目标语义**，不是“所有语法已落地”的现状宣称。
- **推断规则**：`uses` 为空 → 自由函数；有 effect 依赖未声明 → 不完整函数；已声明 → 验证一致性。

完整语义见 SEP-0003。

## 代价模型

- 四维 **CostVector**：`compute(op) + alloc(cell) + io(call) + parallel(lane)`。
- 编译期通过**抽象解释**模拟执行，验证 ≤ 声明上界。
- 支持符号代价表达式；编译器从被调用函数自动推导。
- `@unbounded` 是显式逃生舱：它跳过函数体预算验证、向调用者传播
  unbounded taint，但当前策略仍要求声明期望向量
  `cost [compute, alloc, io, parallel]`，保留签名中的代价意图。
- 编译器在检查过程中会为调用点、handler 安装点、hole 点位维护**checked residual budget**（声明总预算扣除已消耗 obligation 后的剩余空间）；这是验证器 / 诊断 / HoleReport 共享的内部概念，不引入用户可写的“代价减法”表面语法。
  `with_cost_limit` 属于未来收敛机制，不是当前表面语法。

完整规范见 SEP-0004。

## Hole 系统

- 语法：`?name` 或 `?name : Type`。
- 带 hole 的 partial 函数**可编译、可模拟、不可执行**。
- 编译器按依赖图对 hole 排序（传递依赖者数量降序），推荐填洞顺序。
- **HoleReport (JSON)** 包含完整上下文：类型、可见绑定、available effects、代价预算、候选函数、`@allows` 约束。
- HoleReport 的协议命名继续沿用 **v0.x lineage**；未来即使增补 effect context / residual context / rejection reasons，也应作为当前 v0.x 家族上的扩展，而不是跳到脱离现有 lineage 的新命名故事。
- 填洞遵循**单 hole 原子提交**：一次替换一个 hole，再交增量编译验证。
- Hole 诊断是 *note / partial-state signal*，不导致编译失败；只有真实类型 / effect / 代价 / 模块错误才返回非零状态。
- 协作主循环：`DISCOVER → ANALYZE → PROPOSE → VERIFY → ACCEPT/REJECT`，`REJECT` 必须返回结构化 root cause 与 fix hints。

完整协议见 SEP-0005。

## 模块与包

- **一文件一模块**：`src/billing/invoice.sp` → `billing.invoice`。无 `module` 头声明。
- **可见性**：private（默认）/ `pub(pkg)` / `pub`。Hole 候选搜索、诊断、导出 API 全部尊重同一套边界。
- **双 hash 身份**：
  - **sig hash** — 公开接口 + required effects 声明 + cost 声明的 hash，用于接口兼容性与下游是否需要重查。
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
  - manifest 定位 contract module、startup contract symbol、adapter、handled effects；
  - contract module 通过带 hole 的 `startup function` 持有权威签名与 spec。
- **项目 entry vs startup contract**：`entry` 是项目层选择的执行目标，Platform 对该模块中的 `startup function` 施加契约校验；“选哪个模块运行”与“该模块里函数签名必须长什么样”分开。
- **Spec stacking**：Platform contract 的 startup spec 与应用实现侧 spec 是叠加约束。
- **Platform 的 effect / foreign API** 由 Platform 导出的普通模块定义（如 `basic_cli.stdout`、`basic_cli.file` 中的 `foreign fn`），应用代码像导入普通依赖一样导入它们。
- **FFI 边界**：**Platform 是唯一 FFI 表面**，应用代码不直接声明裸 native FFI。
- **测试**：换用 mock Platform 即得到确定性 handler，支持 record-replay。

## 并发模型

- **结构化并发**：子任务树，父等所有子完成；父取消 → 子自动取消，协作式。
- **`Spawn` 是 atomic effect**，没有 async/await 着色。
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
- **组合语义辅助命名（本波目标组织）**：`spore.combine` / `spore.merge` / `spore.order` / `spore.laws` 用于承载工程侧组合器、合并规则、顺序 / 偏序语义与 law helpers；这里明确**不**引入用户表面的 `std.algebra`。
- 其余（JSON / HTTP / 正则 / 时间 / ...）全部第三方。

完整规范见 SEP-0009。

## 编译器输出与诊断

编译器输出同时服务**人类开发者 / CI / LSP / Agent**。稳定契约锚定在共享的 **Diagnostic IR** 上，默认文本 / `--verbose` / `--json` / LSP adapter 只是同一诊断对象的不同投影。

**最小稳定字段集**：

```
code / severity / message / primary_span / secondary_labels / notes / help / related
```

- **错误码族**：`E`（类型）/ `W`（警告）/ `C`（effect）/ `K`（代价）/ `H`（hole）/ `M`（模块）。
- `sporec explain CODE` 是统一的长解释入口，避免把长篇错误说明散落在文档中。
- 默认文本输出应 help-rich，但 `help` 字段本身保持可选：没有明确下一步时不伪造建议。
- Auto-fix 作为后续 code-action / edit 层，不并入最小 IR。

## 编译器 Pipeline

```
Source → [Lex] → Tokens → [Parse] → AST
       → [Resolve+Desugar] → HIR
       → [TypeCheck+EffectCheck+CostCheck] → TypedHIR
       → [Codegen] → Cranelift IR → Native
```

**3 层 IR + Cranelift IR 作为 LIR**，无独立 flat IR，无 MIR。

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
  - 当前已落地的最小 slice 是 `sporec-codegen` / `sporec-driver` 内部 opt-in Cranelift 标量 backend：只覆盖单模块、纯、标量程序与 parity 测试，CLI 默认执行路径仍保持解释器。

### AST
- 与源码 1:1 对应，节点带 `Span`；保留所有语法糖（`|>` / `?` / `f"..."`）。
- 用途：错误报告、IDE 语法高亮。

### HIR
- Resolve + Desugar pass 产物。
- **脱糖**：`|>` → 函数调用，`?` → match on `Result`，`f"..."` → format 调用。
- 名称解析、导入解析、hole 记录。
- **`sig hash` 在此层计算**。

### TypedHIR
- TypeCheck + EffectCheck + CostCheck 合并 pass 产物。
- 类型推断、effect 验证（函数体使用 ⊆ 声明的 effect set）、代价抽象解释、穷尽检查、错误集传播、细化类型检查、HoleReport 生成。
- **`impl hash` 在此层计算**（含 hole 函数为 `None`）。

### 设计决策

- **不需要 MIR**：无 borrow checker，不需要 CFG 级分析。
- **不需要独立 flat IR**：当前 hash + 依赖图已覆盖增量缓存。
- **effect checking + 代价合并进 TypeCheck**：减少 IR 转换；trait 约束与 effect 接口在该层交汇但不合并为同一语法实体。
- **脱糖全在 Resolve 层**：TypeCheck 不处理语法糖。
- **不支持 Comptime**：const generics + 细化类型 + 代价模型已足够。

## 实现技术栈

- **实现语言**：Rust（edition 2024）。
- **编译器开发基线**：当前仓库 MSRV 为 Rust 1.95。
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
- `sig hash` 仅覆盖公开接口 / required effects / cost；私有实现、注释、内部 hole 状态不触发下游级联 — “改实现不改接口”仍是局部反馈。
- watch 失败后仍须继续工作，保留最近一次可用依赖图。

## 文档治理

- 本文件是仓库内**唯一主设计文档**，维护跨主题的 durable 设计结论。
- 规范级长文存放于 sibling repo `spore-evolution/seps/`（SEP-0001 ~ SEP-0009）。
