# 并发模型设计 — 完整规范 v0.1

> **状态**: 初稿
> **前置依赖**: signature-syntax-v0.2, cost-model-v0.1, type-system-v0.1, concurrency-research
> **核心公式**: `Spore Concurrency = Koka Effects + Kotlin Structured Scoping + Zig Explicit Resources + Spore Cost Budgets`

---

## 目录

1. [概述](#一概述)
2. [结构化并发](#二结构化并发)
3. [效果处理器](#三效果处理器)
4. [Channel](#四channel)
5. [代价模型集成](#五代价模型集成)
6. [取消](#六取消)
7. [设计决策记录](#七设计决策记录adr)
8. [附录：语法速查](#八附录语法速查)

---

## 一、概述

### 1.1 设计目标

Spore 的并发模型追求五个性质的统一——目前无已知语言同时达成其中三个以上：

| 性质 | 含义 | 受益者 |
|------|------|--------|
| **无色函数** | 并发不引入 `async`/`await` 语法分裂 | 所有开发者 |
| **结构化生命周期** | 子任务不可逃逸父作用域 | 资源管理、调试 |
| **显式能力** | 并发任务只能使用声明的 capability | 安全审计 |
| **有界代价** | 编译器静态验证 `cost ≤ N` 与 `parallel(lane)` | 性能可预测性 |
| **可模拟执行** | handler 可替换，同一代码编译时模拟 / 测试时确定性 / 生产时并行 | Agent、CI |

### 1.2 核心机制

```
Structured Concurrency  ← 任务生命周期（Kotlin/Swift/Trio 模型）
       +
Effect Handlers          ← 并发原语的实现方式（Koka/OCaml 5 模型）
       +
Capability Narrowing     ← 子任务能力集 ⊆ 父任务能力集（Spore 独有）
       +
Cost Budgets             ← parallel(lane) 与标量代价约束（Spore 独有）
```

### 1.3 无色函数：为什么不要 async/await

Bob Nystrom 的 *"What Color is Your Function?"*（2015）指出：`async` 把函数分为两种颜色，
红色（async）只能从红色上下文调用，导致整个调用链病毒式传染。

Spore 的解决路径：**并发是一种 effect，effect 在签名的 `uses` 子句中声明，
函数体内的代码完全不需要 `await` 关键字**。effect-polymorphic 函数天然无色。

```spore
-- 无需 async 标注——并发通过 Spawn effect 声明
fn fetch_all(urls: List[Url]) -> List[Response] ! [NetError]
effects: idempotent
cost ≤ urls.len * per_fetch
uses [Spawn, NetRead]
{
    parallel_scope {
        urls.map(|url| spawn { fetch(url) })
            .map(|task| task.await)
    }
}

-- 纯函数：无 Spawn、无 IO，编译器保证单线程
fn transform(data: List[Item]) -> List[Item]
effects: pure, deterministic
cost ≤ data.len * 3
{
    data.map(|item| item.process())
}
```

两个函数使用相同的调用语法——没有红色/蓝色之分。
`fetch_all` 的并发性完全由其 handler 在调用点决定。

### 1.4 与已有子系统的交互

| Spore 子系统 | 与并发模型的交互点 |
|-------------|-------------------|
| 签名语法 | `uses [Spawn, ...]` 声明并发能力；`cost ≤ N` 限定总代价 |
| 代价模型 | 第 4 维度 `parallel(lane)` 由本模型定义 |
| 能力系统 | `Spawn` 是内置 capability；子任务能力集 ⊆ 父 |
| 效果注解 | `pure` 排除 `Spawn`；`deterministic` + `Spawn` 要求调度无关结果 |
| Hole 系统 | 部分函数的并发 hole 仍参与代价预算分析 |
| Platform | Platform 提供 `Spawn` handler 的运行时实现 |

---

## 二、结构化并发

### 2.1 核心原则

> **并发任务构成树，而非图。**
> ——Nathaniel J. Smith, *"Go statement considered harmful"*, 2018

Spore 强制执行以下不变量：

1. **Nursery 规则**: 所有并发任务必须在 `parallel_scope` 内 spawn。
2. **父等子**: `parallel_scope` 在所有子任务完成前不会退出。
3. **取消向下传播**: 取消父任务 → 递归取消所有子任务。
4. **错误向上冒泡**: 子任务异常传播至父 scope，并取消尚存的 siblings。
5. **资源安全**: 因为子任务总在 scope 退出前结束，`defer` 清理总是有效。

### 2.2 parallel_scope 语法

`parallel_scope` 是引入并发的**唯一入口**——不存在 `GlobalScope` 等逃生舱。

```spore
-- 基础形式
parallel_scope {
    let a = spawn { compute_part1() }
    let b = spawn { compute_part2() }
    (a.await, b.await)
}

-- 带显式 lane 数限制（可选，省略时由编译器从函数签名推断）
parallel_scope(lanes: 4) {
    for chunk in data.chunks(4) {
        spawn { process_chunk(chunk) }
    }
}
```

**语义**:
- `parallel_scope` 返回块内最后一个表达式的值。
- 块退出时，所有 spawn 的任务要么已完成、要么被取消。
- 嵌套 `parallel_scope` 形成树形结构——内层 scope 消耗外层的 lane 预算。

### 2.3 spawn 表达式

`spawn` 在当前 `parallel_scope` 内启动一个子任务，返回 `Task[T]`。

```spore
-- spawn 返回 Task[T]
let task: Task[Response] = spawn { fetch(url) }

-- Task[T] 只有一个方法：.await -> T ! [子任务的错误集]
let response: Response = task.await
```

**spawn 不是独立语句**——它总是绑定到 `parallel_scope`。
离开 scope 时未被 await 的 task 会被自动取消（见 §六 取消）。

### 2.4 Task 树

每个 `parallel_scope` 创建一个 nursery 节点，`spawn` 在其下挂载叶子：

```
main()
 └─ parallel_scope          ← nursery A
     ├─ spawn { fetch(url1) }   ← task A1
     ├─ spawn { fetch(url2) }   ← task A2
     └─ spawn {                 ← task A3
           parallel_scope       ← nursery B（嵌套）
            ├─ spawn { parse(data1) }  ← task B1
            └─ spawn { parse(data2) }  ← task B2
         }
```

编译器在**编译时**构建此静态 task 树，用于：
- 代价预算分配验证
- 能力集继承检查
- lane 消耗计算

### 2.5 错误传播

子任务错误遵循 **fail-fast** 策略（默认），可选 **supervisor** 模式：

```spore
-- 默认模式：任一子任务失败 → 取消所有 siblings → 传播错误至父
parallel_scope {
    let a = spawn { fetch(url1) }  -- 如果此处抛 NetError...
    let b = spawn { fetch(url2) }  -- ...b 会被取消
    (a.await, b.await)             -- 整个 scope 向上传播 NetError
}
```

```spore
-- Supervisor 模式：子任务独立失败，互不影响
parallel_scope(on_error: .collect) {
    let a = spawn { fetch(url1) }
    let b = spawn { fetch(url2) }
    -- a.await 返回 Result[Response, NetError]
    -- b.await 返回 Result[Response, NetError]
    -- 两者独立，不取消对方
    match (a.await, b.await) {
        (Ok(r1), Ok(r2)) => (r1, r2),
        (Err(e), _) | (_, Err(e)) => handle_partial_failure(e),
    }
}
```

错误传播规则摘要：

| 模式 | 子任务失败时 | 返回类型 | 适用场景 |
|------|------------|---------|---------|
| `fail_fast`（默认） | 取消 siblings，传播错误 | `T ! [E]` | 所有子结果都需要的场景 |
| `collect` | 不取消 siblings，收集结果 | `Result[T, E]` per task | 部分结果有意义的场景 |

### 2.6 parallel_scope 返回值语义

`parallel_scope` 本身是一个表达式，其值为块内最后一行：

```spore
fn load_dashboard(user_id: UserId) -> Dashboard ! [DbError, NetError]
uses [Spawn, NetRead, DbRead]
{
    -- parallel_scope 的返回值直接作为函数返回值
    parallel_scope {
        let profile = spawn { db.get_profile(user_id) }
        let feed    = spawn { api.get_feed(user_id) }
        let notifs  = spawn { api.get_notifications(user_id) }
        Dashboard {
            profile: profile.await,
            feed: feed.await,
            notifications: notifs.await,
        }
    }
}
```

---

## 三、效果处理器

### 3.1 Spawn 作为 Effect

在 Spore 中，`Spawn` 不是关键字魔法——它是一个**标准 effect 声明**：

```spore
-- 内置 effect 定义（概念性，由 Platform 提供实现）
effect Spawn {
    fn spawn[T](task: () -> T) -> Task[T]
}
```

函数通过 `uses [Spawn]` 声明使用此 effect：

```spore
fn concurrent_work() -> (Int, Int)
uses [Spawn]
{
    parallel_scope {
        let a = spawn { heavy_compute_1() }
        let b = spawn { heavy_compute_2() }
        (a.await, b.await)
    }
}
```

**关键约束**: 没有 `uses [Spawn]` 的函数**不可能**调用 `spawn`——编译器静态拒绝。

### 3.2 Handler 机制

Effect handler 是 effect 操作的具体解释。不同 handler 为同一代码提供不同运行时行为：

```spore
-- 生产环境：真实并行执行
handle concurrent_work()
    with platform.spawn_handler(thread_pool)

-- 测试环境：确定性顺序执行
handle concurrent_work()
    with sequential_handler()

-- 编译时模拟：抽象解释，不实际运行
handle concurrent_work()
    with cost_analysis_handler()
```

### 3.3 内置 Handler

Platform 提供以下 Spawn handler：

| Handler | 行为 | 用途 |
|---------|------|------|
| `ParallelHandler(pool)` | 真实 OS thread / green thread 并行 | 生产环境 |
| `SequentialHandler` | 按 spawn 顺序逐个执行 | 单元测试 |
| `DeterministicHandler(seed)` | 固定调度顺序，可重放 | 回归测试、CI |
| `CostAnalysisHandler` | 抽象执行，统计代价 | 编译时模拟 |
| `TracingHandler(inner)` | 包装任意 handler，记录事件时间线 | 调试、profiling |

### 3.4 SequentialHandler：测试利器

`SequentialHandler` 是测试并发代码的核心策略——
它将所有 `spawn` 变为同步调用，使并发代码变得确定性可测：

```spore
-- 待测试函数
fn fetch_and_merge(urls: List[Url]) -> MergedData ! [NetError]
uses [Spawn, NetRead]
{
    parallel_scope {
        let tasks = urls.map(|url| spawn { fetch(url) })
        let results = tasks.map(|t| t.await)
        merge(results)
    }
}

-- 测试
test "fetch_and_merge 合并结果正确" {
    let mock_net = MockNetHandler {
        responses: Map.from([
            (url1, response1),
            (url2, response2),
        ])
    }

    let result = handle fetch_and_merge([url1, url2])
        with sequential_handler()
        with mock_net

    assert_eq(result, expected_merged)
}
```

**关键洞察**: 因为并发是 effect，测试代码无需引入任何异步测试框架——
只需替换 handler 即可。

### 3.5 自定义 Effect 与 Handler

用户可定义自己的并发相关 effect：

```spore
-- 定义 rate-limiting effect
effect RateLimit {
    fn acquire_permit() -> Unit ! [RateLimitExceeded]
    fn release_permit() -> Unit
}

-- 使用
fn throttled_fetch(url: Url) -> Response ! [NetError, RateLimitExceeded]
uses [Spawn, NetRead, RateLimit]
{
    acquire_permit()
    defer { release_permit() }
    fetch(url)
}

-- 提供 handler
handler token_bucket_handler(rate: Int, per: Duration) for RateLimit {
    fn acquire_permit() -> Unit ! [RateLimitExceeded] {
        -- token bucket 实现
        if tokens > 0 { tokens -= 1; resume(()) }
        else { raise(RateLimitExceeded) }
    }
    fn release_permit() -> Unit {
        tokens += 1
        resume(())
    }
}
```

### 3.6 Effect 与 Capability 的关系

在 Spore 中，**effect ≈ capability**——但职责分离：

| 维度 | Capability (`uses [...]`) | Effect (`effect Foo`) |
|------|--------------------------|----------------------|
| 定义层 | 签名元数据，编译器验证 | 可被 handler 拦截的操作 |
| 语义 | "这个函数可以做什么" | "这个操作如何被执行" |
| 验证时机 | 编译时能力集检查 | 编译时 + handler 绑定时 |
| 示例 | `uses [Spawn]` 允许使用 `spawn` | `effect Spawn { fn spawn... }` 定义操作 |

关系：每个 effect 自动成为一个 capability。声明 `uses [Spawn]` 等价于"此函数可能 perform `Spawn` effect"。

---

## 四、Channel

### 4.1 Channel[T] 类型

Channel 是 Spore 中并发任务之间通信的首选机制——
**无共享内存，通过通信共享数据**（CSP 哲学）。

```spore
-- 创建 buffered channel
let (tx, rx) = Channel.new[Message](buffer: 10)

-- 创建 unbuffered channel（同步传递）
let (tx, rx) = Channel.new[Message](buffer: 0)
```

`Channel.new` 返回一对 `(Sender[T], Receiver[T])`，两者均不可 Clone：

| 类型 | 操作 | 阻塞行为 |
|------|------|---------|
| `Sender[T]` | `tx.send(value)` | buffer 满时挂起 |
| `Receiver[T]` | `rx.recv()` | buffer 空时挂起 |

### 4.2 send/recv 作为 Effect

Channel 操作也是 effect——可被 handler 拦截以实现模拟、追踪等：

```spore
effect ChanSend[T] {
    fn send(value: T) -> Unit ! [ChannelClosed]
}

effect ChanRecv[T] {
    fn recv() -> T ! [ChannelClosed]
}
```

函数使用 channel 需声明对应 capability：

```spore
fn producer(tx: Sender[Int]) -> Unit ! [ChannelClosed]
uses [ChanSend[Int]]
{
    for i in 0..100 {
        tx.send(i)
    }
}

fn consumer(rx: Receiver[Int]) -> List[Int] ! [ChannelClosed]
uses [ChanRecv[Int]]
{
    let mut results = []
    for msg in rx {
        results.push(msg)
    }
    results
}
```

### 4.3 select 表达式

`select` 同时等待多个 channel，取最先就绪者：

```spore
select {
    msg from rx1 => handle_message(msg),
    msg from rx2 => handle_command(msg),
    timeout(5.seconds) => handle_timeout(),
}
```

**语义**:
- `select` 在所有分支上同时等待。
- 多个分支同时就绪时，选择顺序由调度器决定（非确定性）。
- `timeout` 是内置特殊分支，到期后触发。
- `select` 是表达式——每个分支必须返回相同类型。

完整示例：

```spore
fn event_loop(
    commands: Receiver[Command],
    events: Receiver[Event],
    shutdown: Receiver[Unit],
) -> Unit ! [ChannelClosed]
uses [ChanRecv[Command], ChanRecv[Event], ChanRecv[Unit], Spawn]
{
    loop {
        select {
            cmd from commands => {
                execute(cmd)
            },
            evt from events => {
                log_event(evt)
            },
            _ from shutdown => {
                break
            },
            timeout(30.seconds) => {
                heartbeat()
            },
        }
    }
}
```

### 4.4 Fan-out / Fan-in 模式

**Fan-out**: 一个 producer，多个 consumer 共享同一 channel：

```spore
fn fan_out_example(urls: List[Url]) -> List[Response] ! [NetError]
uses [Spawn, NetRead]
{
    let (tx, rx) = Channel.new[Url](buffer: urls.len)
    let (result_tx, result_rx) = Channel.new[Response](buffer: urls.len)

    parallel_scope {
        -- 分发者
        spawn {
            for url in urls {
                tx.send(url)
            }
            tx.close()
        }

        -- N 个 worker（fan-out）
        for _ in 0..4 {
            spawn {
                for url in rx {
                    let resp = fetch(url)
                    result_tx.send(resp)
                }
            }
        }

        -- 收集（fan-in）
        spawn {
            let mut results = []
            for _ in 0..urls.len {
                results.push(result_rx.recv())
            }
            result_tx.close()
            results
        }
    }
}
```

**Fan-in**: 多个 producer 向同一 channel 发送，单个 consumer 聚合：

```spore
fn fan_in_example() -> List[Event]
uses [Spawn, NetRead, FileRead]
{
    let (tx, rx) = Channel.new[Event](buffer: 100)

    parallel_scope {
        -- 多个 producer
        spawn { watch_network(tx.clone()) }
        spawn { watch_filesystem(tx.clone()) }
        spawn { watch_timer(tx.clone()) }

        -- 单个 consumer（fan-in）
        spawn {
            let mut events = []
            for event in rx {
                events.push(event)
                if events.len >= 1000 { break }
            }
            events
        }
    }
}
```

> **注**: `tx.clone()` 创建同一 channel 的额外 Sender。
> 所有 Sender 关闭后 channel 自动关闭（引用计数语义）。

### 4.5 Channel 关闭语义

| 操作 | channel 已关闭时的行为 |
|------|---------------------|
| `tx.send(value)` | 返回 `Err(ChannelClosed)` |
| `rx.recv()` | buffer 非空则返回值；buffer 空则返回 `Err(ChannelClosed)` |
| `tx.close()` | 标记 sender 端关闭 |
| `for msg in rx` | 迭代至 channel 关闭 + buffer 耗尽 |

---

## 五、代价模型集成

### 5.1 parallel(lane) 维度

代价模型 v0.1 定义了四个维度。并发模型负责第 4 维度的语义：

| 维度 | 缩写 | 单位 | 含义 |
|------|------|------|------|
| Compute | `C` | op | CPU 操作步数 |
| Allocation | `A` | cell | 堆内存分配量 |
| IO | `W` | call | 外部调用次数 |
| **Parallel** | **`P`** | **lane** | **并发执行通道数** |

`lane` 是逻辑概念——不直接等于 OS thread 或 CPU core，
而是编译器可静态追踪的并发资源单位。

### 5.2 代价组合规则

#### 顺序执行
```
cost(A; B) = cost(A) + cost(B)        -- C, A, W 各维度求和
parallel(A; B) = max(A.P, B.P)        -- P 维度取峰值
```

#### 并行执行（parallel_scope 内）
```
cost(parallel { A, B }) = max(cost(A), cost(B))   -- C, A, W 各维度取 max
parallel(parallel { A, B }) = A.P + B.P            -- P 维度求和
```

直觉：并行任务的墙钟时间由最慢分支决定（max），
但占用的并发资源是各分支之和（sum）。

#### spawn 开销
```
cost(spawn { body }) = spawn_overhead + cost(body)
    其中 spawn_overhead = 5 op + 1 cell  -- 常数，可在 spore.toml 配置
```

### 5.3 编译器推断：无需手动分配

**核心设计决策**: 开发者不需要手动分配 lane——编译器从被调函数的签名推断。

```spore
-- 开发者只需写：
fn fetch_all(urls: List[Url]) -> List[Response] ! [NetError]
uses [Spawn, NetRead]
{
    parallel_scope {
        urls.map(|url| spawn { fetch(url) })
            .map(|task| task.await)
    }
}

-- 编译器推断输出（sporec --query-cost fetch_all）：
-- {
--   "cost_symbolic": "urls.len × (5 + per_fetch) + urls.len",
--   "parallel_lanes": "urls.len",
--   "note": "lane 数依赖运行时 urls.len，编译时无法确定上界"
-- }
```

如果调用方需要限定 lane：

```spore
fn bounded_fetch(urls: List[Url, max: 100]) -> List[Response] ! [NetError]
cost ≤ 100 * per_fetch + 500
uses [Spawn, NetRead]
{
    parallel_scope(lanes: 10) {
        -- 最多 10 个并行任务，剩余排队
        for url in urls {
            spawn { fetch(url) }
        }
    }
}
```

编译器可验证：`parallel(lane=10)` ≤ 签名声明的约束。

### 5.4 Lane 预算分配

嵌套 `parallel_scope` 从父 scope 的 lane 预算中分配：

```spore
fn pipeline(data: Data) -> Result
cost ≤ 5000
uses [Spawn, FileRead, NetWrite]
{
    -- 阶段 1：使用 4 个 lane
    let prepared = parallel_scope(lanes: 4) {
        let a = spawn { parse_part1(data) }
        let b = spawn { parse_part2(data) }
        let c = spawn { parse_part3(data) }
        let d = spawn { validate(data) }
        merge(a.await, b.await, c.await, d.await)
    }
    -- 阶段 1 结束，4 个 lane 释放

    -- 阶段 2：使用 2 个 lane
    parallel_scope(lanes: 2) {
        let uploaded = spawn { upload(prepared) }
        let logged   = spawn { log_result(prepared) }
        (uploaded.await, logged.await)
    }

    -- 编译器推断: 峰值 lane = max(4, 2) = 4
}
```

### 5.5 代价查询示例

```bash
$ sporec --query-cost pipeline
{
  "function": "pipeline",
  "cost_scalar": 4200,
  "cost_declared": "≤ 5000",
  "status": "within_bound",
  "dimensions": {
    "compute": 3800,
    "alloc": 45,
    "io": 3,
    "parallel_lanes_peak": 4
  },
  "breakdown": [
    {
      "scope": "parallel_scope#1",
      "lanes": 4,
      "wall_cost": 1200,
      "children": [
        { "task": "parse_part1", "cost": 1000 },
        { "task": "parse_part2", "cost": 1200 },
        { "task": "parse_part3", "cost": 900 },
        { "task": "validate",    "cost": 800 }
      ]
    },
    {
      "scope": "parallel_scope#2",
      "lanes": 2,
      "wall_cost": 2600,
      "children": [
        { "task": "upload",     "cost": 2600 },
        { "task": "log_result", "cost": 400 }
      ]
    }
  ]
}
```

### 5.6 与 Hole 系统的交互

含 hole 的并发函数参与代价预算分析：

```spore
fn concurrent_pipeline(data: Data) -> Result
cost ≤ 3000
uses [Spawn, NetRead]
{
    parallel_scope(lanes: 2) {
        let a = spawn { fetch(data.url) }     -- cost: 800
        let b = spawn { ?process_logic }      -- hole: 剩余预算
        merge(a.await, b.await)
    }
}
```

```json
{
  "hole": "process_logic",
  "cost_consumed": 800,
  "cost_budget_remaining": 2200,
  "parallel_context": {
    "scope_lanes": 2,
    "lane_index": 1,
    "sibling_costs": [800]
  },
  "note": "此 hole 在 parallel_scope 的第 2 个 lane 中，代价不超过 2200"
}
```

---

## 六、取消

### 6.1 取消传播模型

取消沿 task 树**自顶向下**传播——与错误的自底向上传播方向互补：

```
错误传播方向:  子 → 父  (向上冒泡)
取消传播方向:  父 → 子  (向下级联)
```

```spore
parallel_scope {
    let a = spawn { long_running_1() }
    let b = spawn {
        parallel_scope {                    -- 嵌套 scope
            let c = spawn { subtask_1() }
            let d = spawn { subtask_2() }
            (c.await, d.await)
        }
    }
    -- 如果外层 scope 被取消：
    -- 1. a 收到取消信号
    -- 2. b 收到取消信号
    -- 3. b 的内层 scope 向 c, d 传播取消
    -- 4. 所有任务响应取消后，scope 退出
}
```

### 6.2 协作式取消

Spore 使用**协作式**取消——任务不会被强制终止，
而是通过 `is_cancelled()` 检查或在 effect 调用点自动检查：

```spore
fn long_running_download(url: Url) -> Data ! [NetError, Cancelled]
uses [Spawn, NetRead]
{
    let chunks = []
    for chunk_url in split_download(url) {
        -- 每次 effect 调用（如 fetch）自动检查取消状态
        -- 如果已取消，fetch 内部抛出 Cancelled 而非真正请求
        chunks.push(fetch(chunk_url))
    }
    merge_chunks(chunks)
}
```

显式取消检查点：

```spore
fn cpu_bound_work(data: List[Item]) -> List[Item]
uses [Spawn]
{
    let mut results = []
    for (i, item) in data.enumerate() {
        -- CPU 密集型代码需要手动插入取消检查点
        if i % 100 == 0 {
            check_cancelled()  -- 如果已取消，抛出 Cancelled
        }
        results.push(transform(item))
    }
    results
}
```

### 6.3 Cleanup Handler（defer + 取消）

`defer` 块在取消时仍然执行——保证资源清理：

```spore
fn with_temp_file() -> Data ! [IoError, Cancelled]
uses [Spawn, FileRead, FileWrite]
{
    let file = create_temp_file()
    defer { delete_file(file) }  -- 取消时也会执行

    -- 长时间操作，可能被取消
    write_data(file, generate_large_data())
    read_and_process(file)
    -- 无论正常返回、异常、还是取消，defer 都会执行
}
```

### 6.4 取消语义总结

| 场景 | 行为 |
|------|------|
| 父 scope 被取消 | 所有子任务收到取消信号 |
| 子任务在 IO effect 处 | handler 检查取消状态，抛 `Cancelled` |
| 子任务在纯计算中 | 需在循环内手动调用 `check_cancelled()` |
| `defer` 块 | 取消时正常执行，用于资源清理 |
| 已完成的 task 被"取消" | 无操作——结果已可用 |
| `select` 等待中被取消 | 所有分支放弃，抛 `Cancelled` |

### 6.5 超时作为取消

`with_timeout` 是取消的语法糖——超时即取消：

```spore
fn fetch_with_timeout(url: Url, limit: Duration) -> Response ! [NetError, Timeout]
uses [Spawn, NetRead, Clock]
{
    with_timeout(limit) {
        fetch(url)
    }
    -- 如果 fetch 在 limit 内未完成，被取消并返回 Err(Timeout)
}
```

实现：`with_timeout` 在内部 spawn 一个计时任务和主任务，
使用类似 `select` 的机制——主任务先完成则返回结果，计时先到则取消主任务。

---

## 七、设计决策记录（ADR）

### ADR-C1: 选择 Effect Handlers 而非 async/await

**状态**: 已确定

**上下文**: 需要为 Spore 选择并发机制。候选方案：
1. async/await（Rust、JS、Python）
2. Green threads（Go、Java Loom）
3. Effect handlers（Koka、OCaml 5）
4. Actor model（Erlang）

**决策**: 选择 **effect handlers**。

**理由**:
- **无色函数**: async/await 引入函数颜色分裂，与 Spore "签名即完整规范"的理念冲突。
- **可替换 handler**: effect handlers 天然支持编译时模拟执行——
  生产用并行 handler，测试用顺序 handler，CI 用确定性 handler。
  这与 Spore 的"模拟执行"核心特性深度对齐。
- **与能力系统统一**: effect ≈ capability。`Spawn` effect 自动成为 capability，
  无需维护两套并发相关的类型标注。
- **排除 green threads**: 虽然无色（好），但缺乏 effect 追踪（坏）。
  任何 thread 可做任何事——违反 Spore 的能力约束模型。
- **排除 actor model**: 无共享内存 + 消息复制开销对代价模型不友好。
  无类型消息协议与 Spore 的类型系统哲学冲突。

**后果**:
- 需要设计完整的 handler 绑定语法。
- Platform 负责提供生产环境的 Spawn handler 实现。
- 编译器需支持 effect polymorphism 以实现真正的无色函数。

---

### ADR-C2: 选择结构化并发而非 fire-and-forget

**状态**: 已确定

**上下文**: 并发任务生命周期管理方式的选择。

**决策**: 强制**结构化并发**。`parallel_scope` 是唯一入口，不提供 `GlobalScope` 逃生舱。

**理由**:
- **代价可分析**: 结构化的 task 树在编译时可枚举——
  编译器需要此属性来验证 `cost ≤ N` 和 `parallel(lane=K)` 约束。
  fire-and-forget 的 task 图不可静态分析。
- **资源安全**: 子任务不逃逸父 scope → `defer` 清理总是有效 → 无资源泄漏。
- **调试友好**: task 树镜像调用栈，backtrace 中可见完整并发结构。
- **符合 Spore 哲学**: "编译器能看到一切" —— 结构化并发让并发结构成为编译器可分析的静态信息。

**被排除的替代方案**:
- Go 的 goroutine 泄漏问题直接违反 Spore 的资源安全保证。
- Kotlin 的 `GlobalScope` 是安全阀，但 Spore 的代价模型不允许未追踪的并发。

**后果**:
- 某些模式（如长生命周期后台任务）需通过 Platform 层实现，而非用户代码直接 spawn。
- 开发者需适应"所有并发都在 scope 内"的心智模型。

---

### ADR-C3: Spawn 是 Capability 而非关键字

**状态**: 已确定

**上下文**: `spawn` 应作为语言关键字还是 effect/capability？

**决策**: `spawn` 是 `Spawn` effect 的操作，`Spawn` 同时是 capability。

**理由**:
- **能力追踪**: `uses [Spawn]` 使编译器可在签名层面知道函数是否涉及并发。
  纯函数（无 `Spawn`）保证单线程执行。
- **handler 可替换**: 因为 `spawn` 是 effect 操作，
  handler 可将其解释为真实线程、协程、或顺序调用——对调用者透明。
- **能力缩窄**: 子任务可声明 `uses [NetRead]`（不含 `Spawn`）→
  编译器保证该子任务不会再嵌套 spawn——递归并发深度可控。
- **与代价模型一致**: capability 出现在签名中 → 代价分析可在签名层面推断并发度。

**后果**:
- `parallel_scope` 内必须有 `Spawn` capability 才能使用 `spawn`。
- 纯函数和无 `Spawn` 能力的函数永远不会引入并发——保证确定性。

---

### ADR-C4: Channel 使用消息传递而非共享内存

**状态**: 已确定

**上下文**: 并发任务之间的数据共享机制选择。

**决策**: Channel[T] 是主要通信机制。不提供 `Mutex`/`RwLock` 等共享内存原语。

**理由**:
- **代价可预测**: channel send/recv 的代价是确定的（1 call per operation）。
  Mutex 竞争的代价不可预测——retry/spin 次数取决于运行时调度。
- **与 effect 系统集成**: send/recv 是 effect 操作，可被 handler 拦截。
  Mutex 的 lock/unlock 语义与 effect handler 的 continuation 模型不兼容。
- **死锁自由**: 单向 channel + 结构化 scope 的组合大幅减少死锁可能。
  共享内存 + 锁的死锁是 NP-hard 的静态检测问题。
- **CSP 哲学**: *"Don't communicate by sharing memory; share memory by communicating."*

**被排除的替代方案**:
- STM（Haskell）: retry 代价不可界定，违反代价模型。
- Actor mailbox: 无类型协议，与 Spore 类型系统冲突。
- Mutex/RwLock: 代价不可预测，死锁检测困难。

**后果**:
- 需要高效的 channel 实现（参考 Go channel / Tokio mpsc）。
- 某些性能极端场景（如无锁计数器）可能需要 unsafe 原语——预留 `unsafe_shared` 作为未来可能的扩展点，不在 v0.1 范围内。

---

### ADR-C5: 编译器推断 Lane 而非手动分配

**状态**: 已确定

**上下文**: `parallel(lane)` 的值由谁决定？

**决策**: 编译器从被调函数的签名和 task 树结构**自动推断** lane 需求。
开发者可在 `parallel_scope(lanes: K)` 中**可选**设置上限，但不是必须。

**理由**:
- **降低心智负担**: 手动分配 lane 要求开发者理解硬件拓扑——违反 Spore "机器无关代价" 的设计原则。
- **编译器更擅长**: 编译器能遍历完整 task 树，计算峰值 lane 需求，
  远比人类手动推算准确。
- **签名层面约束**: 函数签名中可声明 `cost ≤ N`，编译器验证推断的 lane 数是否合理。
  不需要在签名中显式写 `parallel(lane=K)`——除非开发者有特定意图。

**推断规则**:
1. 无 `Spawn` → `P = 0`（纯顺序）
2. `parallel_scope { spawn × N }` → `P = N`（N 个 spawn 全部并行时）
3. `parallel_scope(lanes: K) { spawn × N }` → `P = min(K, N)`（有上限时）
4. 嵌套 scope: `P = max(P_scope1, P_scope2)`（顺序 scope 取峰值）
5. 嵌套 scope 内部: `P = sum(P_children)`（并行子任务取和）

**后果**:
- `sporec --query-cost fn_name` 总是报告推断的 lane 数。
- 开发者可在签名中手动约束以实现特定资源限制。

---

### ADR-C6: 协作式取消而非强制终止

**状态**: 已确定

**上下文**: 取消运行中的并发任务的机制。

**决策**: **协作式取消**。任务在 effect 操作点或显式 checkpoint 处检查取消标志。

**理由**:
- **资源安全**: 强制终止（如 `pthread_cancel`）可能在任意代码行中断——
  `defer` 清理可能未被执行，文件句柄泄漏，数据处于中间状态。
  协作式确保任务在安全点退出。
- **与 effect handler 对齐**: 每次 effect 操作（IO、send、recv 等）都经过 handler——
  handler 在 resume 前检查取消标志是零成本的拦截点。
  纯计算不经过 handler，所以需要显式 `check_cancelled()`。
- **确定性**: 取消的时机由代码中的检查点决定，而非操作系统的调度——
  模拟执行可精确预测取消行为。

**与 Kotlin/Swift 的对比**:
- Kotlin: `isActive` check + `suspend` 函数自动检查——非常接近 Spore 的设计。
- Swift: `Task.checkCancellation()` + `await` 点自动检查——同样是协作式。
- Go: `context.Context` + `select`——手动但有效。

Spore 取上述三者的优点：
- 自动检查点在所有 effect 调用处（类似 Kotlin/Swift 的 `await`）。
- 显式 `check_cancelled()` 用于 CPU 密集循环（类似 Go 的 `select` / Kotlin 的 `isActive`）。
- `defer` 保证清理（类似所有三者的 scope cleanup）。

**后果**:
- CPU 密集循环若不插入 `check_cancelled()`，取消响应会延迟。
- 编译器可发出 warning: 当检测到较长循环体无取消检查点时。

---

## 八、附录：语法速查

### A.1 parallel_scope

```spore
-- 基本形式
parallel_scope {
    <body>
}

-- 带 lane 限制
parallel_scope(lanes: <N>) {
    <body>
}

-- Supervisor 模式
parallel_scope(on_error: .collect) {
    <body>
}

-- 组合
parallel_scope(lanes: 4, on_error: .collect) {
    <body>
}
```

### A.2 spawn

```spore
-- 返回 Task[T]
let task = spawn { <expr> }

-- 直接 await
let result = spawn { <expr> }.await

-- 带能力缩窄（可选）
let task = spawn uses [NetRead] { <expr> }
```

### A.3 Task[T]

```spore
-- 等待结果
let value: T = task.await

-- 显式取消（罕见——通常由 scope 管理）
task.cancel()

-- 检查状态
task.is_done      -- Bool
task.is_cancelled  -- Bool
```

### A.4 Channel

```spore
-- 创建
let (tx, rx) = Channel.new[T](buffer: N)

-- 发送 / 接收
tx.send(value)          -- ! [ChannelClosed]
let value = rx.recv()   -- ! [ChannelClosed]

-- 关闭
tx.close()

-- 克隆 sender（多 producer）
let tx2 = tx.clone()

-- 迭代
for msg in rx {
    handle(msg)
}
```

### A.5 select

```spore
select {
    <binding> from <receiver> => <expr>,
    timeout(<duration>) => <expr>,
}

-- 示例
select {
    msg from rx1 => handle1(msg),
    msg from rx2 => handle2(msg),
    _   from quit_rx => break,
    timeout(5.seconds) => default_action(),
}
```

### A.6 取消

```spore
-- 超时
with_timeout(duration) {
    <body>
}

-- 显式取消检查
check_cancelled()

-- 清理
defer { <cleanup> }
```

### A.7 Effect 与 Handler

```spore
-- 声明 effect
effect MyEffect {
    fn operation(param: T) -> U
}

-- 使用 effect（在 uses 中声明）
fn my_func() -> Result
uses [MyEffect]
{
    operation(value)
}

-- 提供 handler
handle my_func()
    with my_handler()

-- 定义 handler
handler my_handler for MyEffect {
    fn operation(param: T) -> U {
        -- 实现
        resume(result)  -- 继续执行调用方
    }
}
```

### A.8 函数签名中的并发声明

```spore
-- 完整形式
fn name(params) -> ReturnType ! [Errors]
where T: Constraints
effects: <annotations>
cost ≤ <N>
uses [Spawn, Channel, ...]
{
    <body>
}

-- 推断输出示例
-- sporec 推断:
--   effects: idempotent
--   cost = 3200
--   parallel_lanes_peak = 4
--   uses [Spawn, NetRead]
```

### A.9 完整示例：HTTP 服务器处理器

```spore
capability HttpHandler = [Spawn, NetRead, NetWrite, DbRead, Clock]

fn handle_request(req: Request) -> Response ! [DbError, Timeout]
effects: idempotent
cost ≤ 5000
uses [HttpHandler]
{
    with_timeout(10.seconds) {
        parallel_scope(lanes: 3) {
            let auth   = spawn uses [DbRead]   { verify_token(req.token) }
            let user   = spawn uses [DbRead]   { load_user(req.user_id) }
            let config = spawn uses [NetRead]   { fetch_remote_config() }

            let auth_result = auth.await
            if !auth_result.valid {
                return Response.unauthorized()
            }

            Response.ok(
                render_page(user.await, config.await)
            )
        }
    }
}
```

### A.10 完整示例：ETL Pipeline

```spore
fn etl_pipeline(
    source: DataSource,
    sink: DataSink,
    batch_size: Int,
) -> EtlReport ! [ExtractError, TransformError, LoadError]
cost ≤ batch_size * 200
uses [Spawn, NetRead, DbRead, DbWrite]
{
    let (raw_tx, raw_rx)     = Channel.new[RawRecord](buffer: batch_size)
    let (clean_tx, clean_rx) = Channel.new[CleanRecord](buffer: batch_size)

    parallel_scope(lanes: 6) {
        -- Extract: 1 lane
        let extractor = spawn uses [NetRead] {
            for record in source.stream() {
                raw_tx.send(record)
            }
            raw_tx.close()
        }

        -- Transform: 4 lanes (fan-out)
        for _ in 0..4 {
            spawn {
                for raw in raw_rx {
                    match transform(raw) {
                        Ok(clean) => clean_tx.send(clean),
                        Err(e)    => log_error(e),
                    }
                }
            }
        }

        -- Load: 1 lane (fan-in)
        let loader = spawn uses [DbWrite] {
            let mut count = 0
            for record in clean_rx {
                sink.write(record)
                count += 1
            }
            count
        }

        -- 等待并汇总
        extractor.await
        clean_tx.close()
        let loaded = loader.await

        EtlReport { records_loaded: loaded }
    }
}
```

---

## 参考文献

- Smith, Nathaniel J. *"Notes on structured concurrency, or: Go statement considered harmful"*. 2018.
- Nystrom, Bob. *"What Color is Your Function?"*. 2015.
- Leijen, Daan. *"Algebraic Effects for Functional Programming"*. Microsoft Research. 2016.
- Kotlin Coroutines: https://kotlinlang.org/docs/coroutines-basics.html
- Koka Language: https://koka-lang.github.io/koka/doc/book.html
- OCaml 5 Effects: https://ocaml.org/manual/5.2/effects.html
- Go Concurrency: https://go.dev/wiki/LearnConcurrency
