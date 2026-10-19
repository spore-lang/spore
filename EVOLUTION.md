# 项目演进

## 初始意图

软件开发的核心工作是表达意图，但类型签名、文档、测试和口头约定常常各说一部分。
Spore 希望把意图作为一等公民，让函数签名携带蓝图，hole 表达尚未完成的部分，
填洞后形成可执行程序。开发者先写签名与类型，编译器验证骨架，再按依赖顺序逐洞实现；
人和 Agent 通过同一份自包含 HoleReport 协作。

[2026-03-30 的初始设计](https://github.com/spore-lang/spore/commit/243ebe1b2351bedfde9bd234231756596f655e27)已确定以下方向：

- 构建编译型通用语言，让输入、输出、错误、副作用与代价意图进入可检查的签名。
- 带 hole 的部分程序可以编译、分析与模拟，不把未完成状态当成类型错误或可执行成果。
- 无状态编译器 sporec 与有状态 Codebase Manager spore 分工，诊断同时服务人和机器。
- 以接口 hash 和实现 hash 区分兼容性与缓存；内容寻址取代 semver，不同 hash 可共存，
  模块图禁止循环依赖。
- 采用四维计算代价、结构化并发与消息传递；不引入 async/await 着色或共享可变状态。
- Rust bootstrap，先考虑 Cranelift，后续再评估 LLVM；纯计算组件逐步自举。

设计参考 Agda 的 hole、Idris 的 elaboration、Unison 的内容寻址、Elm 的诊断和
Roc 的 Platform。面向希望先固定意图再实现的开发者、需要精确上下文的 Agent，
以及对副作用和计算预算敏感的场景。

类型表达力优先用泛型、关联类型和 GAT 覆盖，避免 HKT 损害签名可读性。
不追求 Rust 式所有权/借用检查器、自定义操作符或独立 MIR；成功的标志是开发者写完
签名即可得到骨架反馈，Agent 能据 HoleReport 填洞，接口不变的实现修改无需级联重检。

## 2026-04-13

### 触发

诊断、handler、Platform 和 native backend 需要共同的边界，避免每个功能形成自己的语言合同。

### 变化

以共享 Diagnostic IR 作为终端、JSON 和 LSP 的事实源；无明确下一步时不伪造 help。
Handler 采用词法作用域、one-shot、non-resumable 语义，优先证明可实现的边界。
Platform 是提供 IO handler 的普通包，也是唯一 FFI 表面；项目入口选择与 startup contract
分开，每个项目绑定一个 Platform。

Cranelift 先做单模块、纯标量 skeleton，不一次接管完整运行时。
HostValue、ADT、Option、Result 的布局冻结前不扩展 aggregate ABI；Platform host model
泛化前不推进 foreign fn 与 effect lowering。

相关决策见
[诊断合同](https://github.com/spore-lang/spore/pull/82)、
[handler 语义](https://github.com/spore-lang/spore/pull/91)、
[Platform 定义](https://github.com/spore-lang/spore/pull/92)和
[backend 里程碑](https://github.com/spore-lang/spore/pull/93)。

### 理由

先冻结共用合同和最小 native 对照切片，可以将语言语义、数据布局与宿主运行时分开验证，避免用尚未落地的全量后端承诺掩盖不确定性。

## 2026-04-24

### 触发

早期 capability 与 trait 混用，以及 property 表达式的含义，需要统一到可检查的 effect 与行为规范。

### 变化

统一使用 effect / uses / perform，trait 约束与 effect system 在类型层分离。
函数签名以 where / uses / cost / spec 承载约束、副作用、四维预算和行为断言。
Property 返回受限输入上的期望值，由测试基础设施与实际返回值比较。

Cranelift 作为实验性纯标量 backend 接入，复杂数据、foreign、effect 和 package runtime
继续受到 skeleton 边界限制，不作静默回退。

见 [effect 术语统一](https://github.com/spore-lang/spore/pull/114)、
[property 语义](https://github.com/spore-lang/spore/pull/116)与
[实验后端](https://github.com/spore-lang/spore/pull/115)。

### 理由

显式且统一的语义能同时服务用户、类型检查与 Agent。行为规范表达期望结果，避免让同一 spec 在文档、测试和编译器中产生不同解释。

## 2026-05-16

### 触发

effect、错误集、代价和 laws 都需要组合与检查，为它们各造一套规则会增加实现和规范之间的分歧。

### 变化

将这些对象视为可组合、可规范化、可检查的语义，复用合成、合并、顺序和规则检查。
工程侧采用 spore.combine / spore.merge / spore.order / spore.laws 组织，不新增用户可见的
std.algebra 表面。目标覆盖 handler discharge、错误集等价与 checked residual budget。

见 [组合语义设计](https://github.com/spore-lang/spore/pull/140)。

### 理由

组合机制首先服务工程实现与规范对齐，避免为了内部共性扩张表面语言。细化类型 L1 的工程化路径和运行时代价收敛仍待解决，with_cost_limit 不因此成为可用表面语法。

## 2026-05-17

### 触发

HoleReport 需要携带更完整的 effect、residual context 和拒绝原因，同时让现有消费者继续依赖清楚的协议谱系。

### 变化

扩展现有 v0.x HoleReport，而不是另起协议命名。
报告继续提供类型、可见绑定、可用 effect、预算、候选函数和约束，协作遵循
DISCOVER → ANALYZE → PROPOSE → VERIFY → ACCEPT/REJECT，拒绝必须返回结构化原因。

详细语义由 [SEP 规范](https://github.com/spore-lang/spore-evolution/tree/main/seps)维护；
实际语法和工具行为以本仓库 [README](README.md)、编译器与测试为准。
见 [HoleReport 扩展](https://github.com/spore-lang/spore/pull/145)。

### 理由

沿着已有协议扩展上下文，才能让人和 Agent 使用同一协作界面。单 hole 原子修改后重新验证，比让自由文本或过时实现快照承担合同更可靠。
