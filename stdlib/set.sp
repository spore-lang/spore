// Spore standard library — set type (list-backed)
// Pure Spore implementation using sorted lists.

fn set_new() -> List[I32] cost [1, 0, 0, 0]
spec {
    example "empty": set_is_empty(set_new()) == true
}
{ [] }

@unbounded
fn set_insert(s: List[I32], item: I32) -> List[I32]
spec {
    example "add": set_contains(set_insert(set_new(), 5), 5) == true
    example "idempotent": set_len(set_insert(set_insert(set_new(), 5), 5)) == 1
}
{
    if contains(s, item) { s } else { append(s, item) }
}

@unbounded
fn set_remove(s: List[I32], item: I32) -> List[I32]
spec {
    example "remove": set_contains(set_remove(set_insert(set_new(), 5), 5), 5) == false
    example "noop": set_len(set_remove(set_new(), 5)) == 0
}
{
    filter(s, |x: I32| x != item)
}

fn set_contains(s: List[I32], item: I32) -> Bool cost [2, 0, 0, 0]
spec {
    example "present": set_contains(set_insert(set_new(), 3), 3) == true
    example "absent": set_contains(set_new(), 3) == false
}
{
    contains(s, item)
}

fn set_len(s: List[I32]) -> I32 cost [2, 0, 0, 0]
spec {
    example "empty": set_len(set_new()) == 0
    example "one": set_len(set_insert(set_new(), 1)) == 1
}
{ len(s) }

fn set_is_empty(s: List[I32]) -> Bool cost [2, 0, 0, 0]
spec {
    example "empty": set_is_empty(set_new()) == true
    example "nonempty": set_is_empty(set_insert(set_new(), 1)) == false
}
{ len(s) == 0 }

@unbounded
fn set_union(a: List[I32], b: List[I32]) -> List[I32]
spec {
    example "merge": set_len(set_union(set_insert(set_new(), 1), set_insert(set_new(), 2))) == 2
    example "overlap": set_len(set_union(set_insert(set_new(), 1), set_insert(set_new(), 1))) == 1
}
{
    fold(b, a, |acc: List[I32], x: I32| set_insert(acc, x))
}

@unbounded
fn set_intersection(a: List[I32], b: List[I32]) -> List[I32]
spec {
    example "overlap": set_len(set_intersection(set_insert(set_insert(set_new(), 1), 2), set_insert(set_insert(set_new(), 2), 3))) == 1
    example "none": set_len(set_intersection(set_insert(set_new(), 1), set_insert(set_new(), 2))) == 0
}
{
    filter(a, |x: I32| contains(b, x))
}

@unbounded
fn set_difference(a: List[I32], b: List[I32]) -> List[I32]
spec {
    example "basic": set_len(set_difference(set_insert(set_insert(set_new(), 1), 2), set_insert(set_new(), 2))) == 1
    example "empty": set_len(set_difference(set_new(), set_insert(set_new(), 1))) == 0
}
{
    filter(a, |x: I32| if contains(b, x) { false } else { true })
}

// ── Str set variants ─────────────────────────────────────────────

fn set_new_str() -> List[Str] cost [1, 0, 0, 0] { [] }

@unbounded
fn set_insert_str(s: List[Str], item: Str) -> List[Str] {
    if contains(s, item) { s } else { append(s, item) }
}

@unbounded
fn set_remove_str(s: List[Str], item: Str) -> List[Str] {
    filter(s, |x: Str| x != item)
}

fn set_contains_str(s: List[Str], item: Str) -> Bool cost [2, 0, 0, 0]
spec {
    example "present": set_contains_str(set_insert_str(set_new_str(), "hi"), "hi") == true
    example "absent": set_contains_str(set_new_str(), "hi") == false
}
{
    contains(s, item)
}
