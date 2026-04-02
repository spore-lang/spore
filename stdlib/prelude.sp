// Spore standard library — prelude
// Auto-loaded into every compilation unit.

// ── Option type ─────────────────────────────────────────────────────

type Option[T] { Some(T), None }

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

fn is_some[T](opt: Option[T]) -> Bool {
    match opt { Some(_) => true, None => false }
}

fn is_none[T](opt: Option[T]) -> Bool {
    match opt { Some(_) => false, None => true }
}

// ── Result type ─────────────────────────────────────────────────────

type Result[T, E] { Ok(T), Err(E) }

fn unwrap_or_result[T, E](res: Result[T, E], default: T) -> T {
    match res {
        Ok(v) => v,
        Err(_) => default,
    }
}

fn map_result[T, U, E](res: Result[T, E], f: (T) -> U) -> Result[U, E] {
    match res {
        Ok(v) => Ok(f(v)),
        Err(e) => Err(e),
    }
}

fn is_ok[T, E](res: Result[T, E]) -> Bool {
    match res { Ok(_) => true, Err(_) => false }
}

fn is_err[T, E](res: Result[T, E]) -> Bool {
    match res { Ok(_) => false, Err(_) => true }
}

// ── Ordering type ───────────────────────────────────────────────────

type Ordering { Less, Equal, Greater }
