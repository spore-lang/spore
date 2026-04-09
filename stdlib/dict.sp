// Spore standard library — dictionary type stubs
// Pure Spore dict implemented as List[Pair[K, V]].

fn dict_new[K, V]() -> List[Pair[K, V]] cost [1, 0, 0, 0] { [] }

fn dict_len[K, V](d: List[Pair[K, V]]) -> I32 cost [2, 0, 0, 0]
spec {
    example "empty": dict_len(dict_new()) == 0
    example "one": dict_len(dict_insert(dict_new(), 1, 10)) == 1
}
{ len(d) }

fn dict_is_empty[K, V](d: List[Pair[K, V]]) -> Bool cost [2, 0, 0, 0]
spec {
    example "empty": dict_is_empty(dict_new()) == true
    example "nonempty": dict_is_empty(dict_insert(dict_new(), 1, 10)) == false
}
{ len(d) == 0 }

@unbounded
fn dict_insert[K, V](d: List[Pair[K, V]], key: K, value: V) -> List[Pair[K, V]] {
    append(dict_remove(d, key), Pair { first: key, second: value })
}

@unbounded
fn dict_get[V](d: List[Pair[I32, V]], key: I32) -> Option[V]
spec {
    example "found": unwrap_or(dict_get(dict_insert(dict_new(), 1, 42), 1), 0) == 42
    example "missing": unwrap_or(dict_get(dict_new(), 1), 0) == 0
}
{
    match d {
        [] => None,
        [Pair { first: k, second: v }, ..rest] => if k == key { Some(v) } else { dict_get(rest, key) },
    }
}

@unbounded
fn dict_get_str[V](d: List[Pair[Str, V]], key: Str) -> Option[V]
spec {
    example "found": unwrap_or(dict_get_str(dict_insert(dict_new(), "a", 99), "a"), 0) == 99
    example "missing": unwrap_or(dict_get_str(dict_new(), "z"), 0) == 0
}
{
    match d {
        [] => None,
        [Pair { first: k, second: v }, ..rest] => if k == key { Some(v) } else { dict_get_str(rest, key) },
    }
}

@unbounded
fn dict_remove[K, V](d: List[Pair[K, V]], key: K) -> List[Pair[K, V]]
spec {
    example "remove": dict_len(dict_remove(dict_insert(dict_new(), 1, 10), 1)) == 0
    example "noop": dict_len(dict_remove(dict_new(), 1)) == 0
}
{
    filter(d, |entry: Pair[K, V]| entry.first != key)
}

@unbounded
fn dict_contains_key[K, V](d: List[Pair[K, V]], key: K) -> Bool {
    any(d, |entry: Pair[K, V]| entry.first == key)
}

@unbounded
fn dict_keys[K, V](d: List[Pair[K, V]]) -> List[K] {
    map(d, |entry: Pair[K, V]| entry.first)
}

@unbounded
fn dict_values[K, V](d: List[Pair[K, V]]) -> List[V] {
    map(d, |entry: Pair[K, V]| entry.second)
}
