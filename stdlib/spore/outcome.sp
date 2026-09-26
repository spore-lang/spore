// First-class outcome helpers for the Spore standard library.

pub fn map_ok[A, B, E](value: A ! E, f: (A) -> B) -> B ! E {
    match value {
        ok success => f(success),
        fail failure => fail failure,
    }
}

pub fn map_fail[A, E, F](value: A ! E, f: (E) -> F) -> A ! F {
    match value {
        ok success => success,
        fail failure => fail f(failure),
    }
}

pub fn is_ok[A, E](value: A ! E) -> Bool {
    match value {
        ok _ => true,
        fail _ => false,
    }
}

pub fn is_fail[A, E](value: A ! E) -> Bool {
    match value {
        ok _ => false,
        fail _ => true,
    }
}
