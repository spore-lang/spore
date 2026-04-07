// Spore standard library — set type (list-backed)
// Pure Spore implementation using sorted lists.

fn set_new() -> List[Int] cost <= 1
spec {
    example "empty": set_is_empty(set_new()) == true
}
{ [] }

@unbounded
fn set_insert(s: List[Int], item: Int) -> List[Int]
spec {
    example "add": set_contains(set_insert(set_new(), 5), 5) == true
    example "idempotent": set_len(set_insert(set_insert(set_new(), 5), 5)) == 1
}
{
    if contains(s, item) { s } else { append(s, item) }
}

@unbounded
fn set_remove(s: List[Int], item: Int) -> List[Int]
spec {
    example "remove": set_contains(set_remove(set_insert(set_new(), 5), 5), 5) == false
    example "noop": set_len(set_remove(set_new(), 5)) == 0
}
{
    filter(s, |x: Int| x != item)
}

fn set_contains(s: List[Int], item: Int) -> Bool cost <= 2
spec {
    example "present": set_contains(set_insert(set_new(), 3), 3) == true
    example "absent": set_contains(set_new(), 3) == false
}
{
    contains(s, item)
}

fn set_len(s: List[Int]) -> Int cost <= 2
spec {
    example "empty": set_len(set_new()) == 0
    example "one": set_len(set_insert(set_new(), 1)) == 1
}
{ len(s) }

fn set_is_empty(s: List[Int]) -> Bool cost <= 2
spec {
    example "empty": set_is_empty(set_new()) == true
    example "nonempty": set_is_empty(set_insert(set_new(), 1)) == false
}
{ len(s) == 0 }

@unbounded
fn set_union(a: List[Int], b: List[Int]) -> List[Int]
spec {
    example "merge": set_len(set_union(set_insert(set_new(), 1), set_insert(set_new(), 2))) == 2
    example "overlap": set_len(set_union(set_insert(set_new(), 1), set_insert(set_new(), 1))) == 1
}
{
    fold(b, a, |acc: List[Int], x: Int| set_insert(acc, x))
}

@unbounded
fn set_intersection(a: List[Int], b: List[Int]) -> List[Int]
spec {
    example "overlap": set_len(set_intersection(set_insert(set_insert(set_new(), 1), 2), set_insert(set_insert(set_new(), 2), 3))) == 1
    example "none": set_len(set_intersection(set_insert(set_new(), 1), set_insert(set_new(), 2))) == 0
}
{
    filter(a, |x: Int| contains(b, x))
}

@unbounded
fn set_difference(a: List[Int], b: List[Int]) -> List[Int]
spec {
    example "basic": set_len(set_difference(set_insert(set_insert(set_new(), 1), 2), set_insert(set_new(), 2))) == 1
    example "empty": set_len(set_difference(set_new(), set_insert(set_new(), 1))) == 0
}
{
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

fn set_contains_str(s: List[String], item: String) -> Bool cost <= 2
spec {
    example "present": set_contains_str(set_insert_str(set_new_str(), "hi"), "hi") == true
    example "absent": set_contains_str(set_new_str(), "hi") == false
}
{
    contains(s, item)
}
