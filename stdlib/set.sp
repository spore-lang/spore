// Spore standard library — set type (list-backed)
// Pure Spore implementation using sorted lists.

fn set_new() -> List[Int] cost <= 1 { [] }

@unbounded
fn set_insert(s: List[Int], item: Int) -> List[Int] {
    if contains(s, item) { s } else { append(s, item) }
}

@unbounded
fn set_remove(s: List[Int], item: Int) -> List[Int] {
    filter(s, |x: Int| x != item)
}

fn set_contains(s: List[Int], item: Int) -> Bool cost <= 2 {
    contains(s, item)
}

fn set_len(s: List[Int]) -> Int cost <= 2 { len(s) }

fn set_is_empty(s: List[Int]) -> Bool cost <= 2 { len(s) == 0 }

@unbounded
fn set_union(a: List[Int], b: List[Int]) -> List[Int] {
    fold(b, a, |acc: List[Int], x: Int| set_insert(acc, x))
}

@unbounded
fn set_intersection(a: List[Int], b: List[Int]) -> List[Int] {
    filter(a, |x: Int| contains(b, x))
}

@unbounded
fn set_difference(a: List[Int], b: List[Int]) -> List[Int] {
    filter(a, |x: Int| if contains(b, x) { false } else { true })
}

// ── String set variants ─────────────────────────────────────────────

fn set_new_str() -> List[String] cost <= 1 { [] }

@unbounded
fn set_insert_str(s: List[String], item: String) -> List[String] {
    if contains(s, item) { s } else { append(s, item) }
}

@unbounded
fn set_remove_str(s: List[String], item: String) -> List[String] {
    filter(s, |x: String| x != item)
}

fn set_contains_str(s: List[String], item: String) -> Bool cost <= 2 {
    contains(s, item)
}
