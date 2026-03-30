# Concurrency & Parallelism Models: A Comprehensive Survey for Spore Language Design

> **Purpose**: Inform the concurrency model design for the Spore programming language.
> Spore already has: a 4-dimensional cost model with `parallel(lane)`, a capability system (`NetRead`, `FileWrite`, …), effect annotations (`pure`, `deterministic`, `idempotent`, `total`), compile-time simulated execution, and a Platform concept.

---

## Table of Contents

1. [Go — Goroutines + Channels (CSP)](#1-go--goroutines--channels-csp)
2. [Rust — async/await + Ownership](#2-rust--asyncawait--ownership)
3. [Erlang/Elixir — Actor Model (BEAM)](#3-erlangelixir--actor-model-beam)
4. [Kotlin — Coroutines + Structured Concurrency](#4-kotlin--coroutines--structured-concurrency)
5. [Swift — Structured Concurrency + Actors](#5-swift--structured-concurrency--actors)
6. [Java — Virtual Threads (Project Loom)](#6-java--virtual-threads-project-loom)
7. [Haskell — STM + Lightweight Threads](#7-haskell--stm--lightweight-threads)
8. [Koka — Effect Handlers for Concurrency](#8-koka--effect-handlers-for-concurrency)
9. [Unison — Abilities for Concurrency](#9-unison--abilities-for-concurrency)
10. [Zig — Async/await + Explicit I/O](#10-zig--asyncawait--explicit-io)
11. [OCaml 5 — Algebraic Effects + Domains](#11-ocaml-5--algebraic-effects--domains)
12. [Structured Concurrency (Concept)](#12-structured-concurrency-concept)
13. [Linear/Affine Types for Concurrency](#13-linearaffine-types-for-concurrency)
14. [Cross-Cutting Analysis](#cross-cutting-analysis)
15. [Recommendation for Spore](#recommendation-for-spore)

---

## 1. Go — Goroutines + Channels (CSP)

### Design Summary

Go's concurrency is directly inspired by Tony Hoare's **Communicating Sequential Processes (CSP)**. The core philosophy: *"Don't communicate by sharing memory; share memory by communicating."*

```go
// Goroutines: lightweight (2KB stack), multiplexed onto OS threads
func worker(id int, jobs <-chan int, results chan<- int) {
    for j := range jobs {
        results <- j * 2
    }
}

func main() {
    jobs := make(chan int, 100)
    results := make(chan int, 100)

    // Fan-out: 3 worker goroutines
    for w := 1; w <= 3; w++ {
        go worker(w, jobs, results)
    }

    // Fan-in: send work, collect results
    for j := 1; j <= 5; j++ {
        jobs <- j
    }
    close(jobs)

    for a := 1; a <= 5; a++ {
        fmt.Println(<-results)
    }
}
```

```go
// select: multiplexing channels
select {
case msg := <-ch1:
    fmt.Println("received from ch1:", msg)
case msg := <-ch2:
    fmt.Println("received from ch2:", msg)
case <-time.After(1 * time.Second):
    fmt.Println("timeout")
}
```

### Key Design Rationale

- **Accessible concurrency**: Goroutines + channels are simpler than raw threads + locks.
- **Efficient**: Goroutines start at 2KB stack; millions can coexist. The runtime's M:N scheduler multiplexes goroutines onto OS threads.
- **CSP over Actors**: Go chose CSP because channels are first-class values that can be passed around, composed, and selected on. Actors couple identity to communication; CSP decouples them.

### Strengths

- Extremely low barrier to entry for concurrent programming
- Millions of goroutines in a single process
- `select` provides powerful channel multiplexing
- Garbage-collected goroutine stacks grow/shrink dynamically
- "Colorless" — no async/sync function split

### Weaknesses / Pain Points

- **Goroutine leaks**: A goroutine blocked on a channel that's never written to lives forever. No built-in structured lifetime.
- **No structured concurrency**: Goroutines are fire-and-forget. `context.Context` is the manual workaround for cancellation/timeout, but it's opt-in and error-prone.
- **Error handling**: Goroutines can't return errors to their spawner. Errors must be sent through channels or shared state.
- **Race detector is runtime-only**: Data races on shared memory are caught only by the race detector at runtime, not the type system.

### Interaction with Spore

| Spore Feature | Interaction |
|---|---|
| **Cost model / `parallel(lane)`** | Go has no cost model. Goroutines are free to spawn, making cost prediction impossible. Spore's explicit lane budget is strictly better. |
| **Capabilities** | Go has no capability system. Any goroutine can access any global. Spore's capability model would prevent a goroutine-equivalent from accessing `FileWrite` without declaration. |
| **Effects** | Go has no effect tracking. A goroutine can do anything. Spore's `pure`/`deterministic` annotations would be violated freely. |
| **Simulated execution** | Go's runtime scheduling is non-deterministic, making compile-time simulation extremely difficult. |

---

## 2. Rust — async/await + Ownership

### Design Summary

Rust prevents data races at compile time through its **ownership and borrowing** system. Concurrency is layered on top via `std::thread` (OS threads) and `async/await` (cooperative tasks on executor runtimes).

```rust
// Ownership prevents data races at compile time
use std::sync::Arc;
use tokio::sync::Mutex;

async fn increment(counter: Arc<Mutex<i32>>) {
    let mut lock = counter.lock().await;
    *lock += 1;
}

#[tokio::main]
async fn main() {
    let counter = Arc::new(Mutex::new(0));

    let mut handles = vec![];
    for _ in 0..10 {
        let counter = Arc::clone(&counter);
        handles.push(tokio::spawn(async move {
            increment(counter).await;
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }
}
```

**Send and Sync traits**:
- `Send`: A type can be transferred across thread boundaries (ownership moves to another thread).
- `Sync`: A type can be shared (via `&T`) across threads safely.
- These are **auto-traits** — the compiler derives them automatically and errors if a non-Send type crosses a `.await` boundary on a multi-threaded executor.

```rust
// Rc is NOT Send — this won't compile on a multi-threaded runtime
async fn bad(x: Rc<u32>) { /* ... */ }
tokio::spawn(bad(Rc::new(42))); // ERROR: Rc<u32> is not Send

// Arc IS Send + Sync — this works
async fn good(x: Arc<u32>) { /* ... */ }
tokio::spawn(good(Arc::new(42))); // OK
```

### Key Design Rationale

- **Zero-cost abstractions**: Futures are state machines compiled at build time — no heap allocation for the future itself.
- **Ownership = thread safety**: The borrow checker statically guarantees no two threads can mutably alias the same data.
- **No runtime by default**: The language provides `Future` as a trait; the executor (tokio, async-std, smol) is a library choice.

### Strengths

- **Compile-time data-race prevention** — the strongest guarantee of any mainstream language
- Zero-cost futures (state machine transformation)
- `Send`/`Sync` auto-traits make thread safety composable and inferrable
- Rich ecosystem (tokio, async-std, smol, rayon for data parallelism)

### Weaknesses / Pain Points

- **Colored functions problem**: `async fn` returns a `Future`, which can only be `.await`-ed inside another `async fn`. This "infects" the entire call stack.
- **Pin complexity**: `Future::poll` takes `self: Pin<&mut Self>`. Self-referential state machines require pinning, which is notoriously confusing.
- **Runtime fragmentation**: tokio, async-std, and smol are incompatible ecosystems. A library written for tokio may not work with async-std.
- **Async traits**: Only recently stabilized. Previously required the `async-trait` crate with dynamic dispatch overhead.

### Interaction with Spore

| Spore Feature | Interaction |
|---|---|
| **Cost model** | Rust's futures have predictable per-operation cost (no hidden allocation). This aligns well with Spore's cost tracking. |
| **Capabilities** | Rust has no capability system, but `Send`/`Sync` are an analogous mechanism for thread-safety capabilities. Spore could generalize this pattern. |
| **Effects** | No effect system, but ownership serves a similar role for mutability tracking. `pure` functions in Spore could be enforced similarly. |
| **Simulated execution** | Deterministic future polling is simulator-friendly if the executor is controlled. |

---

## 3. Erlang/Elixir — Actor Model (BEAM)

### Design Summary

Erlang implements the **actor model** directly in its virtual machine (BEAM). Every actor is a **lightweight process** (~300 bytes) with its own heap, communicating exclusively via **message passing**. There is **no shared memory**.

```erlang
%% Erlang: Spawn a process, send/receive messages
-module(counter).
-export([start/0, loop/1]).

start() ->
    Pid = spawn(?MODULE, loop, [0]),
    Pid ! {increment},
    Pid ! {get, self()},
    receive
        {count, N} -> io:format("Count: ~p~n", [N])
    end.

loop(Count) ->
    receive
        {increment} ->
            loop(Count + 1);
        {get, From} ->
            From ! {count, Count},
            loop(Count)
    end.
```

```elixir
# Elixir: GenServer (OTP pattern)
defmodule Counter do
  use GenServer

  def init(initial), do: {:ok, initial}
  def handle_cast(:increment, state), do: {:noreply, state + 1}
  def handle_call(:get, _from, state), do: {:reply, state, state}
end

# Supervision tree
children = [
  {Counter, 0},
  {AnotherWorker, []}
]
Supervisor.start_link(children, strategy: :one_for_one)
```

### Key Design Rationale

- **"Let it crash" philosophy**: Instead of defensive error handling, processes crash and are restarted by supervisors. This produces self-healing systems.
- **Isolation**: Each process has its own heap. Garbage collection is per-process with no stop-the-world pauses.
- **Supervisors form a tree**: `one_for_one` (restart crashed child), `one_for_all` (restart all if one crashes), `rest_for_one` (restart crashed + subsequent children).

### Strengths

- Battle-tested in telecoms (Ericsson AXD 301: 99.9999999% uptime — "nine nines")
- Millions of processes per node
- Hot code reloading
- Built-in distribution: processes communicate transparently across nodes
- Per-process GC eliminates stop-the-world pauses

### Weaknesses / Pain Points

- **No shared state**: All inter-process communication copies data. Large data structures (e.g., images, matrices) incur significant copying overhead.
- **Untyped messages**: Message protocol violations are runtime crashes. No static typing for mailbox protocols.
- **Debugging distributed actors**: Tracing message flows across hundreds of processes is non-trivial.
- **Single-threaded per process**: Individual processes cannot exploit multi-core parallelism.

### Interaction with Spore

| Spore Feature | Interaction |
|---|---|
| **Cost model** | Message copying has predictable cost. Process creation is O(1). Maps naturally to cost budgets. |
| **Capabilities** | Erlang has no capabilities, but process isolation + message passing is an implicit capability boundary. Spore's explicit `uses FileWrite` is strictly more precise. |
| **Effects** | Process isolation means effects are confined per-process. Spore could model each concurrent task as having an isolated effect scope. |
| **Simulated execution** | Deterministic message ordering within a single process makes simulation tractable. Non-determinism comes from inter-process scheduling. |

---

## 4. Kotlin — Coroutines + Structured Concurrency

### Design Summary

Kotlin is the **poster child for structured concurrency**. Coroutines are launched within a `CoroutineScope`, and the scope cannot complete until all children finish. This is enforced by the language/library design.

```kotlin
import kotlinx.coroutines.*

// Structured concurrency: parent waits for all children
suspend fun fetchData(): Pair<User, List<Post>> = coroutineScope {
    val user = async { fetchUser() }         // child coroutine
    val posts = async { fetchPosts() }       // child coroutine
    Pair(user.await(), posts.await())
    // coroutineScope doesn't exit until both complete
}

// Cancellation propagates down the tree
fun main() = runBlocking {
    val job = launch {
        coroutineScope {
            launch { delay(Long.MAX_VALUE) }  // will be cancelled
            launch { delay(Long.MAX_VALUE) }  // will be cancelled
        }
    }
    delay(100)
    job.cancel()  // cancels all children
}
```

**Dispatchers** determine where coroutines execute:
- `Dispatchers.Main` — UI thread
- `Dispatchers.IO` — blocking I/O thread pool
- `Dispatchers.Default` — CPU-bound work

### Key Design Rationale

JetBrains designed structured concurrency to solve:
1. **Goroutine leak problem**: Coroutines cannot outlive their scope.
2. **Error propagation**: Exceptions in children propagate to parents and cancel siblings (unless `SupervisorJob`).
3. **Lifecycle binding**: Android `viewModelScope` ties coroutines to ViewModel lifecycle.

### Strengths

- Clean structured concurrency model — the best mainstream implementation
- Cancellation is cooperative and propagates down the tree
- `SupervisorJob` allows independent child failure when needed
- Works on JVM, Android, Native, JS

### Weaknesses / Pain Points

- **`suspend` is a color**: Functions must be marked `suspend` to use `delay`, `await`, etc. This is the colored functions problem (though mitigated by structured scoping).
- **`GlobalScope` escape hatch**: Developers can bypass structured concurrency via `GlobalScope.launch`, defeating its guarantees.
- **JVM limitations**: On the JVM, coroutines are still backed by thread pools; they don't have the per-coroutine isolation of Erlang processes.

### Interaction with Spore

| Spore Feature | Interaction |
|---|---|
| **Cost model / `parallel(lane)`** | `coroutineScope` naturally defines a "budget boundary". Spore could enforce `cost ≤ N` at scope boundaries. `parallel(lane)` maps to the number of `async` children. |
| **Capabilities** | Each `coroutineScope` could carry a capability set. Children inherit parent capabilities (or a subset). |
| **Effects** | `suspend` is essentially an effect annotation. Spore's `pure` could mean "no suspend, no IO". |
| **Simulated execution** | Structured scoping makes the concurrent task tree statically known → amenable to compile-time simulation. |

**This is the closest existing model to what Spore needs.**

---

## 5. Swift — Structured Concurrency + Actors

### Design Summary

Swift combines **structured concurrency** (task trees) with **actor isolation** (mutable state protection).

```swift
// Structured concurrency with TaskGroup
func fetchAllUsers(ids: [String]) async throws -> [User] {
    try await withThrowingTaskGroup(of: User.self) { group in
        for id in ids {
            group.addTask {
                try await fetchUser(id: id)
            }
        }
        return try await group.reduce(into: [User]()) { $0.append($1) }
    }
}

// Actor: thread-safe mutable state
actor BankAccount {
    private var balance: Double = 0

    func deposit(_ amount: Double) {
        balance += amount  // no lock needed — actor isolation
    }

    func getBalance() -> Double {
        balance
    }
}

// MainActor: UI thread isolation
@MainActor
class ViewModel: ObservableObject {
    @Published var items: [Item] = []

    func load() async {
        items = try await fetchItems()  // guaranteed on main thread
    }
}
```

**Sendable protocol** (analogous to Rust's `Send`):
```swift
struct Point: Sendable { let x: Double; let y: Double }  // OK — immutable value
// class MutableThing { var x = 0 }  // NOT Sendable — mutable reference type
```

### Key Design Rationale

- **Actor isolation**: Mutable state is confined to an actor. Cross-actor access requires `await`, enforced by the compiler.
- **Sendable**: Types must opt in to cross-concurrency-domain transfer. The compiler enforces this in Swift 6 strict concurrency mode.
- **MainActor**: A special global actor for UI code, eliminating an entire class of "wrong thread" bugs.

### Strengths

- Combines structured concurrency (task lifetime) with actor isolation (state safety)
- `Sendable` checking at compile time (like Rust's `Send`)
- `@MainActor` is a pragmatic, real-world innovation
- Task cancellation propagates through the task tree

### Weaknesses / Pain Points

- **Colored functions**: `async` functions can only be called with `await`, creating the familiar split.
- **Actor reentrancy**: Actors can interleave between `await` points, which can violate expected invariants.
- **Migration pain**: Moving existing codebases to strict Sendable checking is difficult.
- **Limited to Apple ecosystems** (though Swift is open-source, adoption is mainly iOS/macOS).

### Interaction with Spore

| Spore Feature | Interaction |
|---|---|
| **Cost model** | TaskGroup provides natural parallelism boundaries. Actor isolation means cross-actor calls have measurable overhead. |
| **Capabilities** | Actors naturally form capability boundaries — only the actor can access its own state. |
| **Effects** | Actor methods are implicitly effectful (they can mutate state). `nonisolated` methods are pure. |
| **Simulated execution** | Actor isolation + structured tasks make the concurrency graph statically analyzable. |

---

## 6. Java — Virtual Threads (Project Loom)

### Design Summary

Java's Project Loom introduces **virtual threads** (JEP 444, GA in Java 21) — lightweight threads managed by the JVM runtime, not the OS. Combined with **structured concurrency** (JEP 462, preview).

```java
// Virtual threads: millions of lightweight threads
try (var executor = Executors.newVirtualThreadPerTaskExecutor()) {
    for (int i = 0; i < 100_000; i++) {
        executor.submit(() -> {
            // Each task runs on its own virtual thread
            var response = httpClient.send(request, bodyHandler);
            process(response);
        });
    }
}

// Structured concurrency with StructuredTaskScope
try (var scope = new StructuredTaskScope.ShutdownOnFailure()) {
    Subtask<User> user = scope.fork(() -> fetchUser(id));
    Subtask<List<Order>> orders = scope.fork(() -> fetchOrders(id));

    scope.join();           // Wait for both
    scope.throwIfFailed();  // Propagate exceptions

    return new Response(user.get(), orders.get());
}  // Automatic cleanup on scope exit
```

### Key Design Rationale

- **Thread-per-request without scalability cliff**: Virtual threads are ~1KB. Traditional platform threads are 1-2MB. Millions of virtual threads are feasible.
- **No colored functions**: Virtual threads look and behave exactly like regular threads. Blocking I/O calls are transparently non-blocking under the hood.
- **Backward compatible**: Existing `Thread` APIs work unchanged with virtual threads.

### Strengths

- **Colorless**: No async/await syntax. Functions don't need special annotations.
- Millions of concurrent virtual threads
- Structured concurrency via `StructuredTaskScope`
- Backward compatible with 25+ years of Java threading code

### Weaknesses / Pain Points

- **Structured concurrency still in preview** (JEP 462 as of Java 23)
- **`synchronized` pinning**: Virtual threads pin to carrier threads when in `synchronized` blocks, reducing concurrency.
- **No effect tracking**: Any virtual thread can do anything. No compile-time guarantees about thread safety beyond `synchronized`/`volatile`.
- **Debugging**: Stack traces for millions of virtual threads need new tooling.

### Interaction with Spore

| Spore Feature | Interaction |
|---|---|
| **Cost model** | Virtual threads make threading "free" in memory, but actual parallelism is still bound by cores. Spore's `parallel(lane)` would make the physical parallelism budget explicit. |
| **Capabilities** | Java has no capability system. A virtual thread can access any global state. |
| **Effects** | No effect tracking. The colorless model trades explicitness for convenience. |
| **Simulated execution** | Thread scheduling is non-deterministic. Hard to simulate without controlling the scheduler. |

---

## 7. Haskell — STM + Lightweight Threads

### Design Summary

Haskell leverages its **purity** to make concurrency safer. **Software Transactional Memory (STM)** provides composable atomic transactions, and `forkIO` spawns lightweight green threads.

```haskell
import Control.Concurrent
import Control.Concurrent.STM

-- STM: composable atomic transactions
transfer :: TVar Int -> TVar Int -> Int -> STM ()
transfer from to amount = do
    balFrom <- readTVar from
    check (balFrom >= amount)  -- retry if insufficient
    writeTVar from (balFrom - amount)
    writeTVar to =<< ((+ amount) <$> readTVar to)

-- Usage: atomically runs the entire transaction
main :: IO ()
main = do
    acc1 <- newTVarIO 1000
    acc2 <- newTVarIO 500
    atomically $ transfer acc1 acc2 200
    -- If another thread modifies acc1/acc2 during this transaction,
    -- STM automatically retries. No deadlocks possible.

-- async library: structured concurrency for Haskell
import Control.Concurrent.Async

main :: IO ()
main = do
    (user, posts) <- concurrently fetchUser fetchPosts
    -- Both run concurrently; exceptions propagate; both are cancelled on failure
    print (user, posts)
```

### Key Design Rationale

- **STM eliminates deadlocks**: Transactions are composable — you can combine two STM actions into one atomic action without knowing their internals.
- **Purity makes concurrency safer**: Pure functions (the majority of Haskell code) can run in parallel trivially since they have no side effects.
- **`async` library**: Provides structured-concurrency-like combinators (`concurrently`, `race`, `withAsync`).

### Strengths

- STM is the **gold standard** for composable shared-state concurrency
- Purity + types make reasoning about concurrency tractable
- Lightweight green threads via `forkIO`
- `async` library provides cancellation, error propagation, and structured patterns

### Weaknesses / Pain Points

- **STM retry overhead**: Transactions that conflict frequently cause repeated retries, wasting CPU.
- **Lazy evaluation gotchas**: Unevaluated thunks in `TVar`s can cause space leaks and evaluation in unexpected threads. Requires discipline with `deepseq`/strict evaluation.
- **IO monad is a color**: `IO` actions cannot be called from pure code. This is the monad "color" problem.
- **Transaction log cost**: STM read/write cost is O(n) in transaction log length for conflict detection.

### Interaction with Spore

| Spore Feature | Interaction |
|---|---|
| **Cost model** | STM transactions have unpredictable retry cost. Hard to bound. Spore would need deterministic alternatives or bounded-retry semantics. |
| **Capabilities** | Haskell's `IO` monad acts as a coarse capability. Spore's fine-grained capabilities (`FileWrite`, `NetRead`) are more precise. |
| **Effects** | Haskell's type system tracks `IO` vs pure. Spore's effect system (`pure`, `deterministic`) is the direct successor. |
| **Simulated execution** | Pure computations are trivially simulatable. STM requires modeling the retry mechanism. |

---

## 8. Koka — Effect Handlers for Concurrency

### Design Summary

Koka uses **algebraic effect handlers** as the universal mechanism for side effects, including concurrency. Effects are tracked in the type system. Concurrency is modeled as *just another effect*.

```koka
// Effect declaration for async operations
effect async
  ctl fork(action : () -> <async,exn> ()) : ()
  ctl yield() : ()

// A concurrent computation — its effects are tracked in the type
fun concurrent-work() : <async, console> ()
  fork fn()
    println("Task A running")
    yield()
    println("Task A done")
  fork fn()
    println("Task B running")
    yield()
    println("Task B done")

// Handler: interprets the async effect with a round-robin scheduler
fun round-robin(action : () -> <async|e> a) : e a
  // ... scheduler implementation using effect handler resume/discard
```

```koka
// Effect rows track all effects precisely
fun pure-compute(x: int) : int           // no effects at all
  x * 2

fun io-compute(x: int) : <io> int        // has IO effect
  println("computing...")
  x * 2

fun concurrent(x: int) : <async, io> int  // has both
  // ...
```

### Key Design Rationale

- **Concurrency as a composable effect**: Async/await, green threads, and coroutines are all implementable as effect handlers — they are user-definable, not baked into the language.
- **No colored functions**: Effects are inferred and compose automatically. A function that is effect-polymorphic works with any combination of effects.
- **Structured scoping via handlers**: Effect handlers are lexically scoped. When a handler exits, its effect is no longer available — enforcing structured concurrency naturally.

### Strengths

- **Most compositional** approach to concurrency — swap the handler, change the model
- No colored functions problem — effect polymorphism subsumes it
- Concurrency, exceptions, state, and async are all unified under effects
- Type system tracks all effects precisely

### Weaknesses / Pain Points

- **Still experimental**: Koka's concurrency via effects is not production-battle-tested.
- **Performance**: Evidence-passing and continuation-based compilation add overhead vs. native coroutines.
- **Ecosystem**: Small community, limited libraries.
- **Learning curve**: Algebraic effects are unfamiliar to most programmers.

### Interaction with Spore

| Spore Feature | Interaction |
|---|---|
| **Cost model** | Effect handlers can be instrumented with cost tracking. Each `fork` or `yield` can debit from a cost budget. |
| **Capabilities** | Effects ARE capabilities. `<async, io>` is equivalent to declaring `uses Async, IO`. **This is the strongest alignment with Spore's design.** |
| **Effects** | Koka's effect system is the direct inspiration. Spore's `pure` = no effects. `deterministic` = no non-deterministic effects. |
| **Simulated execution** | Effect handlers can be replaced with mock/simulation handlers at compile time. This is exactly how Spore's simulated execution should work. |

**Koka's effect system is the single most important model for Spore's concurrency design.**

---

## 9. Unison — Abilities for Concurrency

### Design Summary

Unison's **abilities** are its version of algebraic effects. Combined with **content-addressed code** (every definition is identified by its AST hash), this creates a unique platform for distributed concurrency.

```unison
-- IO and Exception are abilities (effects)
helloWorld : '{IO, Exception} ()
helloWorld = do printLine "Hello World"

-- Fork ability for concurrency
concurrent : '{IO, Fork} ()
concurrent = do
  fork do
    printLine "Task A"
  fork do
    printLine "Task B"
  printLine "Main task"

-- Remote ability: distributed computing
distributed : '{Remote} Nat
distributed = do
  at node1 do
    computeExpensiveResult()
```

### Key Design Rationale

- **Content-addressed code enables transparent distribution**: Since every function is identified by hash, sending code to a remote node is trivial — just send the hash, and the remote node can fetch the implementation.
- **Abilities as capabilities**: A function's type declares exactly what it can do. `'{IO, Exception} ()` says "this needs IO and may throw".
- **No build system**: Code is stored in a database by hash. Dependencies are always resolved. No version conflicts.

### Strengths

- **Distribution is first-class**: Moving computation between nodes is a language feature, not infrastructure.
- Content-addressed code eliminates dependency hell
- Abilities provide fine-grained effect/capability tracking
- Immutable, hash-identified definitions make caching and memoization trivial

### Weaknesses / Pain Points

- **Very early ecosystem**: Small community, limited production use.
- **Untyped fork**: Current `Fork` ability doesn't enforce structured concurrency.
- **No shared-memory parallelism**: Focused on distributed computing over shared-nothing.
- **Tooling**: Different development model (UCM, no files) has steep learning curve.

### Interaction with Spore

| Spore Feature | Interaction |
|---|---|
| **Cost model** | Content-addressed code could enable cross-node cost accounting. |
| **Capabilities** | Abilities ARE capabilities — direct alignment with Spore. |
| **Effects** | Same as Koka, but with less mature type-level tracking. |
| **Simulated execution** | Hash-identified code is perfectly reproducible — excellent for simulation. |

---

## 10. Zig — Async/await + Explicit I/O

### Design Summary

Zig's async model was **removed and completely redesigned** (2024-2025). The new design (Zig 0.16+) treats I/O as an **explicitly passed interface** — like allocators. No hidden allocations, no hidden control flow, no "magic".

```zig
const std = @import("std");
const Io = std.Io;

// Every async function receives an explicit Io parameter
fn saveData(io: Io, data: []const u8) !void {
    var future = io.async(saveFile, .{ io, data, "output.txt" });
    try future.await(io);
}

fn saveFile(io: Io, data: []const u8, name: []const u8) !void {
    var file = try io.open(name, .{ .write = true });
    defer file.close();
    try file.writeAll(data);
}

// The caller chooses the I/O strategy (sync, async, io_uring, etc.)
pub fn main() !void {
    const io = std.io.getDefaultIo();  // or io_uring, kqueue, thread pool...
    try saveData(io, "hello");
}
```

### Key Design Rationale

- **No hidden allocations**: Zig's core philosophy. Every async frame's allocation is explicit and user-controlled.
- **No colored functions**: By passing `Io` as a parameter, the same function works synchronously or asynchronously based on the `Io` implementation.
- **No runtime**: No built-in scheduler, event loop, or garbage collector. The user chooses everything.
- **Comptime for metaprogramming, not async**: Previous versions used `comptime` to generate async frames, which was removed for being too opaque.

### Strengths

- **Most explicit** model — zero hidden behavior
- No colored functions — same code works sync or async
- User controls allocation, scheduling, and I/O strategy
- Perfect alignment with Zig's "no hidden control flow, no hidden allocations" philosophy

### Weaknesses / Pain Points

- **Still in development**: The new async model is landing in Zig 0.16, not yet production-stable.
- **Boilerplate**: Passing `Io` everywhere is verbose compared to implicit async.
- **No structured concurrency**: Up to the user to implement.
- **Ecosystem fragmentation risk**: Different I/O implementations may not interoperate.

### Interaction with Spore

| Spore Feature | Interaction |
|---|---|
| **Cost model** | Explicit I/O + explicit allocation makes cost accounting trivial. Every operation's cost is visible at the call site. |
| **Capabilities** | The `Io` parameter IS a capability. If you don't have `Io`, you can't do I/O. Direct alignment with Spore. |
| **Effects** | No formal effect system, but the `Io` parameter serves as a manual effect marker. |
| **Simulated execution** | Explicit I/O can be replaced with mock implementations for simulation — perfect fit. |

**Zig's `Io`-as-parameter pattern is an important inspiration for how Spore could pass capability bundles.**

---

## 11. OCaml 5 — Algebraic Effects + Domains

### Design Summary

OCaml 5 introduces two orthogonal mechanisms:
- **Domains** for true parallelism (OS threads on multiple cores)
- **Algebraic effect handlers** for cooperative concurrency (fibers within a domain)

```ocaml
(* Effect declaration *)
type _ Effect.t +=
  | Fork : (unit -> unit) -> unit Effect.t
  | Yield : unit Effect.t

(* A concurrent computation using effects *)
let concurrent_work () =
  perform (Fork (fun () ->
    print_endline "Task A";
    perform Yield;
    print_endline "Task A done"));
  perform (Fork (fun () ->
    print_endline "Task B";
    perform Yield;
    print_endline "Task B done"))

(* Handler: round-robin scheduler *)
let round_robin f =
  let queue = Queue.create () in
  let enqueue k = Queue.push k queue in
  let dequeue () =
    if Queue.is_empty queue then ()
    else Effect.Deep.continue (Queue.pop queue) ()
  in
  Effect.Deep.match_with f ()
    { retc = (fun () -> dequeue ());
      exnc = raise;
      effc = fun (type a) (eff : a Effect.t) ->
        match eff with
        | Fork f ->
          Some (fun (k : (a, _) Effect.Deep.continuation) ->
            enqueue k; round_robin f)
        | Yield ->
          Some (fun k -> enqueue k; dequeue ())
        | _ -> None }

(* Domains for parallelism *)
let () =
  let d1 = Domain.spawn (fun () -> heavy_compute_1 ()) in
  let d2 = Domain.spawn (fun () -> heavy_compute_2 ()) in
  let r1 = Domain.join d1 in
  let r2 = Domain.join d2 in
  print_int (r1 + r2)
```

### Key Design Rationale

- **Separation of concerns**: Domains handle parallelism (multi-core). Effects handle concurrency (interleaving within a core). These are orthogonal.
- **Effects as library-level concurrency**: The language provides the mechanism (effect handlers with resumable continuations); libraries (Eio, Domainslib) provide the policy.
- **Direct-style code**: No monads needed for concurrency. Code looks sequential but can be cooperatively scheduled.

### Strengths

- **Clean separation** of concurrency (fibers/effects) and parallelism (domains)
- Effect handlers are more powerful than async/await — they support generators, coroutines, exceptions, and concurrency uniformly
- **No colored functions**: Effectful code looks like regular code
- Libraries (Eio) provide production-grade I/O on top of effects

### Weaknesses / Pain Points

- **Effects are untyped in OCaml 5**: The effect system doesn't track effects in types (yet). This loses the composability guarantee Koka has.
- **Shared memory between domains**: Unlike Erlang, domains share a heap. Requires careful synchronization.
- **Ecosystem migration**: Existing OCaml libraries written for the single-core model need updating.
- **Limited adoption**: OCaml 5 is still new (released December 2022).

### Interaction with Spore

| Spore Feature | Interaction |
|---|---|
| **Cost model** | Domains map cleanly to `parallel(lane)`. Each domain = one lane. |
| **Capabilities** | Untyped effects in OCaml miss this opportunity. Spore's typed effects/capabilities are strictly better. |
| **Effects** | OCaml 5's effects are the runtime mechanism; Spore needs typed effects (like Koka) on top. |
| **Simulated execution** | Effect handlers can be swapped with simulation handlers — same pattern as Koka. |

---

## 12. Structured Concurrency (Concept)

### Origin

Nathaniel J. Smith's 2018 blog post *"Notes on structured concurrency, or: Go statement considered harmful"* argued that `go`/`spawn`/`fork` is the concurrent equivalent of `goto` — it creates unstructured control flow.

### Core Principle

> **Concurrent tasks form a tree, not a web.**

Just as structured programming replaced `goto` with blocks (`if`, `while`, `for`), structured concurrency replaces fire-and-forget spawning with **scoped task groups**.

### The Rules

1. **Nurseries / TaskGroups**: A parent opens a scope and spawns children within it.
2. **Parent waits**: The scope cannot exit until ALL children have completed.
3. **Cancellation propagates down**: Cancelling a parent cancels all children recursively.
4. **Errors bubble up**: Exceptions in children propagate to the parent scope.
5. **Resources are safe**: Because all children finish before the scope exits, resource cleanup (`defer`/`finally`) works correctly.

```python
# Trio (Python): The original structured concurrency library
import trio

async def fetch_with_timeout(url):
    async with trio.open_nursery() as nursery:
        nursery.start_soon(fetch, url)
        nursery.start_soon(timeout, 5.0)
        # nursery exits only when BOTH complete
        # if timeout fires, it cancels fetch

async def parent():
    async with trio.open_nursery() as nursery:
        nursery.start_soon(fetch_with_timeout, "https://example.com")
        nursery.start_soon(fetch_with_timeout, "https://example.org")
    # GUARANTEE: both fetches are done or cancelled when we reach here
    print("All done — safe to clean up!")
```

### Implementations Across Languages

| Language | Construct | Notes |
|---|---|---|
| **Python (Trio)** | `async with trio.open_nursery()` | The original implementation |
| **Kotlin** | `coroutineScope { }` | Most polished mainstream implementation |
| **Swift** | `withTaskGroup { }` | Combined with actor isolation |
| **Java** | `StructuredTaskScope` (preview) | JEP 462, try-with-resources |
| **Rust** | Planned / crates (`moro`, `async-scoped`) | Not yet in std |
| **Go** | Manual via `context.Context` + `errgroup` | Opt-in, not enforced |

### Why Structured Concurrency Is Winning

1. **No resource leaks**: Children can't outlive parents.
2. **Debuggable**: The task tree mirrors the call stack.
3. **Composable**: You can nest task groups without worrying about lifetime management.
4. **Analyzable**: The compiler can see the task tree → enables static cost analysis.

### Interaction with Spore

**Structured concurrency is the single most important constraint for Spore's concurrency model.** It enables:
- **Cost budgets**: A `coroutineScope`-like construct bounds the total work of its children. The cost of the scope = sum(cost of children).
- **Capability scoping**: Children inherit parent capabilities (or a narrowed subset). No capability can be acquired beyond what the parent provides.
- **Effect confinement**: All effects within a scope are visible to the parent handler.
- **Simulated execution**: The compiler can enumerate the task tree and simulate all interleavings.

---

## 13. Linear/Affine Types for Concurrency

### Overview

| Type System | Rule | Example Language |
|---|---|---|
| **Linear types** | Value must be used **exactly once** | Linear Haskell, Clean |
| **Affine types** | Value must be used **at most once** | Rust (ownership) |
| **Session types** | Communication channel follows a **typed protocol** | Ferrite, Hibana (Rust libs) |

### How Rust's Ownership Prevents Data Races

Rust's type system is an **affine type system**. The key rules:
1. A value has exactly one owner.
2. Ownership can be moved (transferred) but not duplicated (unless `Copy`/`Clone`).
3. You can have EITHER one `&mut T` OR many `&T`, never both simultaneously.

These rules make data races **impossible in safe Rust**:
- Two threads can't hold `&mut T` to the same data.
- `Send` ensures only transferable types cross thread boundaries.
- `Sync` ensures only shareable types are accessed from multiple threads.

### Session Types for Protocol Verification

Session types encode **communication protocols in the type system**:

```rust
// Using the `par` library for session types in Rust
// Type-level protocol: Client sends i32, receives String, then Done
type MyProtocol = Send<i32, Recv<String, Done>>;

fn client(chan: Chan<MyProtocol>) {
    let chan = chan.send(42);           // must send i32 first
    let (greeting, chan) = chan.recv(); // then receive String
    chan.close();                       // then close
    // Protocol violations are COMPILE ERRORS
}
```

**Multiparty Session Types (MPST)**: Extends session types to multiple participants, verifying that all participants follow the protocol and that the system is deadlock-free.

### Interaction with Spore

| Spore Feature | Interaction |
|---|---|
| **Cost model** | Linear types ensure resources (including cost budget tokens) are consumed exactly once. |
| **Capabilities** | Capabilities could be linear — they must be passed, not duplicated. This prevents capability amplification. |
| **Effects** | Session types can encode effect protocols: "first you initialize, then you read, then you close". |
| **Channel protocols** | If Spore has channels, session types can verify message ordering at compile time. |

---

## Cross-Cutting Analysis

### The Colored Functions Problem

**What is it?** Bob Nystrom's 2015 essay *"What Color is Your Function?"* describes the problem: in async/await languages, `async` functions can only call other `async` functions from within `async` contexts. This "color" (sync vs async) virally infects the entire call stack.

| Language | Has Colored Functions? | Why? |
|---|---|---|
| **Rust** | ✅ Yes | `async fn` returns `impl Future`; must `.await` it |
| **JavaScript** | ✅ Yes | `async function` returns `Promise`; must `await` |
| **Python** | ✅ Yes | `async def` returns coroutine; must `await` |
| **Kotlin** | ✅ Mild | `suspend` functions, but structured scoping mitigates |
| **Swift** | ✅ Mild | `async` functions, but actor isolation adds value |
| **Go** | ❌ No | All functions are "the same color" — goroutines are transparent |
| **Java (Loom)** | ❌ No | Virtual threads are transparent — blocking is cheap |
| **Erlang** | ❌ No | Message passing, not function calls, drives concurrency |
| **Koka** | ❌ No | Effect polymorphism subsumes the sync/async distinction |
| **OCaml 5** | ❌ No | Effect handlers provide concurrency in direct style |
| **Zig (new)** | ❌ No | `Io` parameter makes sync/async a caller choice |
| **Haskell** | ⚠️ Partial | `IO` monad is a "color", but STM within `IO` is composable |

**Solutions**:
1. **Green/virtual threads** (Go, Java Loom): Make blocking cheap. No need for async syntax.
2. **Effect handlers** (Koka, OCaml 5): Concurrency is an effect. Effect-polymorphic functions are colorless.
3. **Explicit I/O parameter** (Zig): The caller decides sync vs async by choosing the `Io` implementation.

**For Spore**: Effect handlers are the right solution. Concurrency should be an effect, and effect-polymorphic functions should not need to declare whether they are "async".

---

### Structured vs Unstructured Concurrency

| Aspect | Structured | Unstructured |
|---|---|---|
| Task lifetime | Bounded by parent scope | Unbounded (fire-and-forget) |
| Cancellation | Propagates down the tree | Manual / ad-hoc |
| Error handling | Exceptions bubble up | Silently swallowed or logged |
| Resource cleanup | Guaranteed at scope exit | Hope and pray |
| Static analysis | Task tree is visible | Task graph is unknowable |
| Examples | Kotlin, Swift, Trio, Java Loom | Go goroutines, bare `pthread_create` |

**Why structured concurrency is winning**:
1. The fire-and-forget model is the concurrent equivalent of `goto`.
2. Resource cleanup requires knowing when all tasks are done.
3. Cost budgets (Spore's `cost ≤ N`) require bounded task trees.
4. Simulated execution requires enumerating the task tree.

**Interaction with cost budgets**:
```
// Conceptual Spore code
fn process(data: List[Item]) -> Result
  cost ≤ 1000, parallel(lane=4)
{
    // Structured scope: 4 parallel lanes, total cost ≤ 1000
    parallel_scope(lanes: 4) {
        for chunk in data.chunks(4) {
            spawn { process_chunk(chunk) }  // each chunk gets 250 cost budget
        }
    }
    // GUARANTEE: all chunks done, total cost ≤ 1000
}
```

---

### Concurrency + Effect Systems

The three languages with algebraic effects — **Koka**, **OCaml 5**, and **Unison** — demonstrate that effects can model concurrency **without colored functions**:

```
// Pseudocode showing the effect approach

// 1. Declare concurrency as an effect
effect Concurrent {
    fork(task: () -> Unit): Unit
    yield(): Unit
}

// 2. Write code using the effect
fn work(): <Concurrent, IO> Unit {
    fork(() => print("A"))
    fork(() => print("B"))
    yield()
}

// 3. Choose the scheduler at the call site
handle work() with
    | RoundRobinScheduler   // cooperative, single-threaded
    | WorkStealingScheduler // parallel, multi-core
    | SimulationScheduler   // deterministic, for testing
```

**The composition advantage**: You can combine effects freely:
- `<Concurrent, State, IO>` — concurrent with mutable state and I/O
- `<Concurrent, STM>` — concurrent with transactional memory
- `<Concurrent>` only — pure concurrent computation (deterministic!)

**This is exactly what Spore needs**: Effects track what a concurrent task can do, and handlers provide the runtime strategy.

---

### Concurrency + Capabilities

Capabilities can control concurrent access in powerful ways:

```
// Conceptual Spore
fn web_handler(req: Request) -> Response
  uses NetRead, DbRead
  cost ≤ 500, parallel(lane=2)
{
    // This function can read from network and database
    // It CANNOT write to files (no FileWrite capability)
    // It has 2 parallel lanes and 500 cost units

    parallel_scope(lanes: 2) {
        let user = spawn { db.query(req.user_id) }   // uses DbRead, 1 lane
        let data = spawn { fetch(req.data_url) }      // uses NetRead, 1 lane
    }
    // Each spawn inherits ONLY the capabilities it needs
}
```

**Race condition prevention through capabilities**:
- If two concurrent tasks both need `&mut State`, the capability system can reject this at compile time.
- Capabilities can encode exclusive access: `ExclusiveWrite(resource)` can only be held by one task at a time.
- The combination of structured concurrency + capabilities + effects means the compiler can verify that concurrent tasks don't interfere.

---

## Recommendation for Spore

### The Ideal Model: Effects + Structured Concurrency + Capabilities

Based on this survey, Spore should combine:

1. **Koka-style algebraic effects** for the concurrency mechanism
2. **Kotlin/Swift-style structured concurrency** for task lifetime management
3. **Rust-inspired ownership/capabilities** for thread-safety guarantees
4. **Zig-inspired explicit resource passing** for predictable cost accounting

### Concrete Design Proposal

#### 1. Concurrency as an Effect

```spore
// Concurrency is declared as an effect, not special syntax
effect Spawn {
    fn fork<T>(task: () -> T) -> Future<T>
    fn yield() -> ()
}

// A concurrent function declares its effects
fn fetch_all(urls: List[Url]) -> List[Response]
    uses Spawn, NetRead
    cost ≤ 1000, parallel(lane=4)
{
    parallel_scope(lanes: 4) {
        urls.map(|url| fork(|| fetch(url)))
            .map(|f| f.await())
    }
}

// A pure function has no Spawn effect — it's guaranteed sequential
fn transform(data: List[Item]) -> List[Item]
    pure, deterministic
{
    data.map(|item| item.process())
}
```

#### 2. Structured Scoping Enforces Cost Budgets

```spore
// parallel_scope is the ONLY way to introduce concurrency
// It creates a structured scope that:
//   - bounds the number of parallel lanes
//   - divides the parent's cost budget among children
//   - guarantees all children complete before the scope exits
//   - propagates cancellation downward

fn process(data: Data) -> Result
    uses Spawn, FileRead
    cost ≤ 2000, parallel(lane=8)
{
    parallel_scope(lanes: 4) {
        // Each child gets cost ≤ 500, lane allocation from parent
        let a = fork(|| phase_1(data.left))   // uses FileRead, cost ≤ 500
        let b = fork(|| phase_1(data.right))  // uses FileRead, cost ≤ 500
        merge(a.await(), b.await())
    }
    // 4 lanes remain for the next scope
    parallel_scope(lanes: 4) {
        // ...
    }
}
```

#### 3. Capabilities Narrow in Concurrent Tasks

```spore
// Parent has FileRead + NetWrite
fn handler(req: Request) -> Response
    uses FileRead, NetWrite
{
    parallel_scope(lanes: 2) {
        // Child 1: only gets FileRead (capability narrowing)
        let config = fork uses FileRead {
            read_config("app.toml")
        }
        // Child 2: only gets NetWrite
        let sent = fork uses NetWrite {
            send_response(req.origin, data)
        }
        // COMPILE ERROR: child cannot use capability not in parent
        // fork uses DbWrite { ... }  // ERROR: handler doesn't have DbWrite
    }
}
```

#### 4. Effect Handlers Enable Simulation

```spore
// Production handler: uses real I/O and OS threads
handle fetch_all(urls)
    with RealSpawnHandler(thread_pool)
    with RealNetHandler(tcp_stack)

// Test handler: deterministic, simulated execution
handle fetch_all(urls)
    with SimulatedSpawnHandler(deterministic_scheduler)
    with MockNetHandler(recorded_responses)

// Compile-time analysis handler: abstract interpretation
handle fetch_all(urls)
    with CostAnalysisHandler()  // tracks cost without executing
    with CapabilityCheckHandler()  // verifies no capability violations
```

#### 5. How This Interacts with Spore's Existing Features

| Spore Feature | Interaction with Proposed Concurrency Model |
|---|---|
| **`cost ≤ N`** | `parallel_scope` divides cost budget among children. `fork` deducts from parent's budget. Violations are compile-time errors. |
| **`parallel(lane=K)`** | `parallel_scope(lanes: K)` is the runtime realization. Each `fork` consumes one lane. Exceeding lanes → compile error or queue. |
| **`pure`** | A `pure` function cannot have the `Spawn` effect. It is guaranteed single-threaded. |
| **`deterministic`** | A `deterministic` function with `Spawn` must produce the same result regardless of scheduling order. The compiler can verify this via simulation. |
| **`idempotent`** | An `idempotent` concurrent task can be safely retried on failure — important for fault tolerance. |
| **Capabilities** | Capabilities narrow monotonically from parent to child scope. No capability amplification in concurrent tasks. |
| **Simulated execution** | Effect handlers at compile time can simulate the task tree, checking cost bounds and capability violations. |
| **Platform** | The Platform provides the root capability set and the Spawn handler implementation. Different platforms (Linux, WASM, embedded) provide different concurrency strategies. |

### Why NOT Other Models

| Model | Why Not for Spore |
|---|---|
| **Go CSP** | No structured concurrency, no effect tracking, no cost model. Goroutine leaks are a design flaw Spore must avoid. |
| **Rust async/await** | Colored functions problem. Pin complexity. Runtime fragmentation. Spore's effects avoid all three. |
| **Erlang actors** | No shared state means costly data copying. No fine-grained effect/capability tracking. |
| **Java virtual threads** | Colorless (good), but no effect tracking, no cost model, no capability system. |
| **Haskell STM** | STM retry cost is unbounded and unpredictable — violates Spore's cost budget guarantee. |

### Summary: Spore's Concurrency DNA

```
Spore Concurrency = Koka Effects
                   + Kotlin Structured Scoping
                   + Swift Actor Isolation Ideas
                   + Zig Explicit Resource Passing
                   + Rust Ownership for Thread Safety
                   + Spore-unique Cost Budgets + Capability Narrowing
```

The result is a concurrency model where:
- **Every concurrent construct is an effect** → no colored functions
- **Every scope is structured** → no resource leaks, no orphaned tasks
- **Every capability is explicit** → no unauthorized I/O in concurrent tasks
- **Every cost is bounded** → the compiler can reject programs that exceed budgets
- **Every execution can be simulated** → because handlers are swappable

This is, to my knowledge, **the first language design that unifies all five properties**. No existing language achieves more than three.

---

## References

### Primary Sources
- Hoare, C.A.R. *Communicating Sequential Processes*. 1978.
- Nystrom, Bob. *"What Color is Your Function?"*. 2015. https://journal.stuffwithstuff.com/2015/02/01/what-color-is-your-function/
- Smith, Nathaniel J. *"Notes on structured concurrency"*. 2018. https://vorpus.org/blog/notes-on-structured-concurrency-or-go-statement-considered-harmful/
- Leijen, Daan. *"Algebraic Effects for Functional Programming"*. Microsoft Research. 2016.

### Language-Specific
- **Go**: https://go.dev/wiki/LearnConcurrency
- **Rust**: https://doc.rust-lang.org/book/ch17-05-traits-for-async.html
- **Erlang/OTP**: https://www.erlang.org/doc/system/design_principles.html
- **Kotlin**: https://kotlinlang.org/docs/coroutines-basics.html
- **Swift**: https://docs.swift.org/swift-book/documentation/the-swift-programming-language/concurrency/
- **Java Loom**: https://openjdk.org/jeps/444 (JEP 444), https://openjdk.org/jeps/462 (JEP 462)
- **Haskell STM**: https://hackage.haskell.org/package/stm
- **Koka**: https://koka-lang.github.io/koka/doc/book.html
- **Unison**: https://www.unison-lang.org/docs/the-big-idea/
- **Zig async redesign**: https://andrewkelley.me/post/zig-new-async-io-text-version.html
- **OCaml 5 effects**: https://ocaml.org/manual/5.2/effects.html
- **Trio (Python)**: https://trio.readthedocs.io/
- **Session types in Rust**: https://github.com/faiface/par, https://hibanaworks.dev/
