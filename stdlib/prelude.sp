// Spore standard library — prelude
// Auto-loaded into every compilation unit.

// ── Option type ─────────────────────────────────────────────────────

type Option[T] { Some(T), None }

fn unwrap_or[T](opt: Option[T], default: T) -> T cost [3, 0, 0, 0] {
    match opt {
        Some(v) => v,
        None => default,
    }
}

fn map_option[T, U](opt: Option[T], f: (T) -> U) -> Option[U] cost [3, 0, 0, 0] {
    match opt {
        Some(v) => Some(f(v)),
        None => None,
    }
}

fn and_then[T, U](opt: Option[T], f: (T) -> Option[U]) -> Option[U] cost [4, 0, 0, 0] {
    match opt {
        Some(v) => f(v),
        None => None,
    }
}

fn or_else[T](opt: Option[T], f: () -> Option[T]) -> Option[T] cost [4, 0, 0, 0] {
    match opt {
        Some(v) => Some(v),
        None => f(),
    }
}

fn is_some[T](opt: Option[T]) -> Bool cost [3, 0, 0, 0] {
    match opt { Some(_) => true, None => false }
}

fn is_none[T](opt: Option[T]) -> Bool cost [3, 0, 0, 0] {
    match opt { Some(_) => false, None => true }
}

fn flatten_option[T](opt: Option[Option[T]]) -> Option[T] cost [4, 0, 0, 0] {
    match opt {
        Some(inner) => inner,
        None => None,
    }
}

// ── Result type ─────────────────────────────────────────────────────

type Result[T, E] { Ok(T), Err(E) }

fn unwrap_or_result[T, E](res: Result[T, E], default: T) -> T cost [3, 0, 0, 0] {
    match res {
        Ok(v) => v,
        Err(_) => default,
    }
}

fn map_result[T, U, E](res: Result[T, E], f: (T) -> U) -> Result[U, E] cost [3, 0, 0, 0] {
    match res {
        Ok(v) => Ok(f(v)),
        Err(e) => Err(e),
    }
}

fn and_then_result[T, U, E](res: Result[T, E], f: (T) -> Result[U, E]) -> Result[U, E] cost [4, 0, 0, 0] {
    match res {
        Ok(v) => f(v),
        Err(e) => Err(e),
    }
}

fn map_err[T, E, F](res: Result[T, E], f: (E) -> F) -> Result[T, F] cost [3, 0, 0, 0] {
    match res {
        Ok(v) => Ok(v),
        Err(e) => Err(f(e)),
    }
}

fn or_else_result[T, E, F](res: Result[T, E], f: (E) -> Result[T, F]) -> Result[T, F] cost [4, 0, 0, 0] {
    match res {
        Ok(v) => Ok(v),
        Err(e) => f(e),
    }
}

fn is_ok[T, E](res: Result[T, E]) -> Bool cost [3, 0, 0, 0] {
    match res { Ok(_) => true, Err(_) => false }
}

fn is_err[T, E](res: Result[T, E]) -> Bool cost [3, 0, 0, 0] {
    match res { Ok(_) => false, Err(_) => true }
}

fn flatten_result[T, E](res: Result[Result[T, E], E]) -> Result[T, E] cost [3, 0, 0, 0] {
    match res {
        Ok(inner) => inner,
        Err(e) => Err(e),
    }
}

// ── Ordering type ───────────────────────────────────────────────────

type Ordering { Less, Equal, Greater }

fn compare(a: I32, b: I32) -> Ordering cost [3, 0, 0, 0] {
    if a < b { Less }
    else { if a > b { Greater } else { Equal } }
}

// ── Bool combinators ────────────────────────────────────────────────

fn not(b: Bool) -> Bool cost [1, 0, 0, 0] {
    if b { false } else { true }
}

fn bool_to_int(b: Bool) -> I32 cost [1, 0, 0, 0] {
    if b { 1 } else { 0 }
}

// ── Function combinators ────────────────────────────────────────────

fn identity[T](x: T) -> T cost [1, 0, 0, 0] { x }

fn always[T, U](x: T, _y: U) -> T cost [1, 0, 0, 0] { x }

// ── Pair type ───────────────────────────────────────────────────────

struct Pair[A, B] { first: A, second: B }
