---
description: 以意图为一等公民的编译型通用编程语言——签名携带完整规范，hole 驱动人机协作
owner: spore-lang
created: 2026-03-30
updated: 2026-05-05
inspired_by:
  - Roc — Platform 系统、纯函数式应用架构
  - Elm — 禁止循环依赖、极简设计哲学
  - Unison — 内容寻址代码管理
  - Rust — 类型系统表达力、代数 effect 方向
  - Koka — effect handler 语义参考
---

## 起源

软件开发的核心工作是"表达意图"——告诉编译器、队友和 AI 你要什么。然而绝大多数语言把"意图"散落在类型签名、文档注释、测试用例和口头约定之间，无法被统一验证。

Spore（孢子）从一个类比出发：一粒孢子携带完整蓝图，在适宜环境中发育为有机体。Spore 程序也是如此——函数签名即完整蓝图，hole 是萌芽态，填满 hole 后成为可执行的有机体。

这意味着：开发者先写签名、再编译骨架、最后按依赖顺序逐洞填充。编译器在每一步都能给出结构化反馈，人和 Agent 以同一份 HoleReport 作为协作界面。

## 产品/设计目标

Spore 是一门编译型、通用目的的编程语言，目标是让**签名成为唯一的规范来源**。

- 一个函数签名同时承载输入/输出类型、错误集、effect 声明、实现形态预算和 properties——不需要在别处重复表达。
- 带 hole 的程序合法编译，编译器为每个 hole 输出自包含的上下文报告，使"部分完成"成为正常的工作状态而非错误。
- 编译器输出同时面向人和机器：同一份 Diagnostic IR 投影为终端文本、JSON 和 LSP 事件。
- 模块以内容 hash 标识，无版本号，无钻石依赖冲突。

## 目标用户

- 希望在编码前先固化意图的开发者——先写签名与类型，再逐步实现。
- 使用 AI Agent 辅助编程的团队——Agent 通过 HoleReport 获取精确上下文，完成结构化填洞。
- 对副作用和实现形态敏感的场景——effect 和 budget 在签名层面显式声明并编译期验证。

## 核心原则

1. **签名是重力中心** — 输入/输出、错误集、effect、预算、properties 全部在签名声明，函数体可以为空。
2. **Hole 是协作点而非错误** — 带 hole 的程序编译成功；hole 是人、Agent、团队之间结构化协作的接口。
3. **先架构后细节** — 自顶向下：先定签名与类型，编译器验证骨架，再按依赖顺序填洞。
4. **编译器是文档助手** — 所有输出同时提供人类可读文本与机器可读 JSON，共享同一份 Diagnostic IR。
5. **显式的 effect 与预算** — 每个副作用必须声明，实现形态约束可被编译期验证。从签名就能完整评估一个函数的影响。
6. **内容寻址取代版本号** — 签名 hash 决定接口兼容性，实现 hash 决定缓存，无 semver，无钻石依赖冲突。

## 能力地图（方向性）

- **函数签名即规范**：inline bounds / `uses` / `budget` / `properties` 覆盖类型约束、effect、预算和行为断言。
- **Hole 系统**：`?name` 语法；编译器按依赖图排序推荐填洞顺序；HoleReport 包含类型、可见绑定、可用 effect、预算上下文、候选函数。
- **Effect 系统**：内置 atomic effect（Console / FileRead / NetConnect 等）+ 自定义 effect 接口 + 聚合别名；`handle ... with ...` 词法作用域 handler。
- **预算模型**：命名的实现形态约束（分支、嵌套、递归、并行度、调用、effect、hole）；编译期按源码形态验证。
- **内容寻址模块**：双 hash（sig hash / impl hash）驱动增量编译与缓存；禁止循环依赖。
- **Platform 系统**：语言级概念，以普通包形态提供全部 IO effect handler；应用代码保持纯净；Platform 是唯一 FFI 表面。
- **结构化并发**：`Spawn` 作为 effect，无 async/await 着色；`Channel[T]` 消息传递，无共享可变状态。
- **双格式诊断**：Diagnostic IR 投影为默认文本 / `--json` / LSP adapter；统一错误码族（E / W / F / B / H / P / M）。
- **编译器工具链**：`sporec`（无状态编译器）+ `spore`（有状态 Codebase Manager：依赖、lock、缓存、watch）。

## 成功信号

- Agent 读取 HoleReport 后能在无额外对话的情况下正确填洞。
- 开发者写完签名即可编译，获得骨架验证反馈。
- 签名变更时编译器精确报告受影响的下游模块——改实现不改接口的修改不触发级联。
- `spore watch` 保存后秒级反馈，增量编译只重检 hash 变化的模块。

## 生态关系

- **SEP 规范**：详细语义存放于 [`../spore-evolution/seps/`](../spore-evolution/seps/)（SEP-0001 ~ SEP-0009），SPARK.md 只保留方向性描述。
- **实现路线**：[`ROADMAP.md`](ROADMAP.md) 跟踪编译器、工具链与标准库的实现计划，并链接对应 SEP。
- **Backend**：Cranelift 为主（纯 Rust、快编译、原生 WASM）；LLVM 作为未来可选高性能后端。
- **实现语言**：Rust（edition 2024，MSRV 1.95）；自举路线为纯计算组件逐步用 Spore 重写。

## 什么不是本项目要做的（Non-goals）

- **所有权 / 借用检查器** — 不引入 Rust 式生命周期系统。
- **HKT** — 用 generics + traits + 关联类型 + GAT 覆盖，不引入 higher-kinded types。
- **Async/await 着色** — 并发通过 `Spawn` effect 建模，不设专门的 async 语法。
- **自定义操作符** — 管道 `|>` 是唯一特殊操作符。
- **Comptime** — const generics + 细化类型 + 预算模型已足够，不引入编译期求值宏。
- **独立 MIR** — 无 borrow checker，不需要 CFG 级中间表示。
- **Semver** — 内容寻址 hash 取代语义版本号。
- **循环依赖** — 模块图必须无环。

## 已考虑的替代方案 & 理由

| 方向           | 替代方案              | 选择 & 理由                                                               |
| -------------- | --------------------- | ------------------------------------------------------------------------- |
| 版本管理       | Semver                | 内容 hash 消除版本兼容猜测；不同 hash 直接共存，钻石依赖不冲突            |
| 泛型表达力     | HKT                   | 实际需求由 GAT + 关联类型覆盖；HKT 引入的类型层复杂度与"签名可读"原则冲突 |
| 中间表示       | 独立 MIR（CFG-based） | 无 borrow checker，hash + 依赖图已覆盖增量缓存，额外 IR 无收益            |
| 并发模型       | Async/await           | effect 系统天然统一同步与异步；着色问题在 Spore 不存在                    |
| Effect handler | 可恢复 continuation   | 先实现 one-shot non-resumable handler，降低运行时复杂度                   |
| 循环引用       | 允许并做拓扑延迟      | Elm 证明了禁止循环依赖带来的架构清晰度；编译器可更自由地并行与缓存        |

## 开放问题

- Cranelift backend 当前只覆盖单模块纯标量程序；`HostValue` / ADT / `Option` / outcome 的数据布局何时冻结？
- `@foreign` 导入边界与 effect lowering 需等 Platform host model 泛化后再推进——如何界定 skeleton 阶段边界？
- 细化类型 L1 层（抽象解释传播）的工程化路径尚未确定。
- 运行时资源收敛机制暂未进入表面语法；当前预算只约束源码实现形态。

## 修订记录

- 2026-03-30：初稿（作为 `docs/DESIGN.md`）。
- 2026-05-05：迁移至 `SPARK.md`，重组为 SPARK 格式，精简实现细节至方向性描述。
