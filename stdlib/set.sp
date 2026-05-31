// Spore standard library — set type (list-backed)
// Pure Spore implementation using sorted lists.
fn set_new() -> List[I64]
properties {
    empty(): set_is_empty(set_new()) == true
}
{ [] }

fn set_insert(s: List[I64], item: I64) -> List[I64]
properties {
    add(): set_contains(set_insert(set_new(), 5), 5) == true
    idempotent(): set_len(set_insert(set_insert(set_new(), 5), 5)) == 1
}
{
    if contains(s, item) { s } else { append(s, item) }
}

fn set_remove(s: List[I64], item: I64) -> List[I64]
properties {
    remove(): set_contains(set_remove(set_insert(set_new(), 5), 5), 5) == false
    noop(): set_len(set_remove(set_new(), 5)) == 0
}
{ filter(s, |x: I64| x != item) }

fn set_contains(s: List[I64], item: I64) -> Bool
properties {
    present(): set_contains(set_insert(set_new(), 3), 3) == true
    absent(): set_contains(set_new(), 3) == false
}
{ contains(s, item) }

fn set_len(s: List[I64]) -> I64
properties {
    empty(): set_len(set_new()) == 0
    one(): set_len(set_insert(set_new(), 1)) == 1
}
{ len(s) }

fn set_is_empty(s: List[I64]) -> Bool
properties {
    empty(): set_is_empty(set_new()) == true
    nonempty(): set_is_empty(set_insert(set_new(), 1)) == false
}
{ len(s) == 0 }

fn set_union(a: List[I64], b: List[I64]) -> List[I64]
properties {
    merge(): set_len(set_union(set_insert(set_new(), 1), set_insert(set_new(), 2))) == 2
    overlap(): set_len(set_union(set_insert(set_new(), 1), set_insert(set_new(), 1))) == 1
}
{ fold(b, a, |acc: List[I64], x: I64| set_insert(acc, x)) }

fn set_intersection(a: List[I64], b: List[I64]) -> List[I64]
properties {
    overlap(): set_len(set_intersection(set_insert(set_insert(set_new(), 1), 2), set_insert(set_insert(set_new(), 2), 3))) == 1
    none(): set_len(set_intersection(set_insert(set_new(), 1), set_insert(set_new(), 2))) == 0
}
{ filter(a, |x: I64| contains(b, x)) }

fn set_difference(a: List[I64], b: List[I64]) -> List[I64]
properties {
    basic(): set_len(set_difference(set_insert(set_insert(set_new(), 1), 2), set_insert(set_new(), 2))) == 1
    empty(): set_len(set_difference(set_new(), set_insert(set_new(), 1))) == 0
}
{ filter(a, |x: I64| if contains(b, x) { false } else { true }) }

// ── Str set variants ─────────────────────────────────────────────
fn set_new_str() -> List[Str] { [] }

fn set_insert_str(s: List[Str], item: Str) -> List[Str] {
    if contains(s, item) { s } else { append(s, item) }
}

fn set_remove_str(s: List[Str], item: Str) -> List[Str] { filter(s, |x: Str| x != item) }

fn set_contains_str(s: List[Str], item: Str) -> Bool
properties {
    present(): set_contains_str(set_insert_str(set_new_str(), "hi"), "hi") == true
    absent(): set_contains_str(set_new_str(), "hi") == false
}
{ contains(s, item) }
