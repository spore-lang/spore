// Spore standard library — prelude
// Auto-loaded into every compilation unit.

// ── Option type ─────────────────────────────────────────────────────
enum Option[T] {
    Some(T),
    None,
}

fn unwrap_or[T](opt: Option[T], default: T) -> T {
    match opt {
        Some(v) => v,
        None => default,
    }
}

fn map_option[T, U](opt: Option[T], f: (T) -> U) -> Option[U] {
    match opt {
        Some(v) => Some(f(v)),
        None => None,
    }
}

fn and_then[T, U](opt: Option[T], f: (T) -> Option[U]) -> Option[U] {
    match opt {
        Some(v) => f(v),
        None => None,
    }
}

fn or_else[T](opt: Option[T], f: () -> Option[T]) -> Option[T] {
    match opt {
        Some(v) => Some(v),
        None => f(),
    }
}

fn is_some[T](opt: Option[T]) -> Bool {
    match opt {
        Some(_) => true,
        None => false,
    }
}

fn is_none[T](opt: Option[T]) -> Bool {
    match opt {
        Some(_) => false,
        None => true,
    }
}

fn flatten_option[T](opt: Option[Option[T]]) -> Option[T] {
    match opt {
        Some(inner) => inner,
        None => None,
    }
}

// ── Ordering type ───────────────────────────────────────────────────
enum Ordering {
    Less,
    Equal,
    Greater,
}

fn compare(a: I64, b: I64) -> Ordering {
    if a < b { Less } else {
        if a > b { Greater } else { Equal }
    }
}

// ── Bool combinators ────────────────────────────────────────────────
fn not(b: Bool) -> Bool {
    if b { false } else { true }
}

fn bool_to_int(b: Bool) -> I64 {
    if b { 1 } else { 0 }
}

// ── Function combinators ────────────────────────────────────────────
fn identity[T](x: T) -> T { x }

fn always[T, U](x: T, _y: U) -> T { x }

// ── Pair type ───────────────────────────────────────────────────────
struct Pair[A, B] {
    first: A,
    second: B,
}
