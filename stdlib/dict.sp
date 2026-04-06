// Spore standard library — dictionary type stubs
// Pure Spore dict implemented as List[Pair[K, V]].

fn dict_new[K, V]() -> List[Pair[K, V]] cost <= 1 { [] }

fn dict_len[K, V](d: List[Pair[K, V]]) -> Int cost <= 2 { len(d) }

fn dict_is_empty[K, V](d: List[Pair[K, V]]) -> Bool cost <= 2 { len(d) == 0 }

@unbounded
fn dict_insert[K, V](d: List[Pair[K, V]], key: K, value: V) -> List[Pair[K, V]] {
    append(dict_remove(d, key), Pair { first: key, second: value })
}

@unbounded
fn dict_get[V](d: List[Pair[Int, V]], key: Int) -> Option[V] {
    match d {
        [] => None,
        [Pair { first: k, second: v }, ..rest] => if k == key { Some(v) } else { dict_get(rest, key) },
    }
}

@unbounded
fn dict_get_str[V](d: List[Pair[String, V]], key: String) -> Option[V] {
    match d {
        [] => None,
        [Pair { first: k, second: v }, ..rest] => if k == key { Some(v) } else { dict_get_str(rest, key) },
    }
}

@unbounded
fn dict_remove[K, V](d: List[Pair[K, V]], key: K) -> List[Pair[K, V]] {
    filter(d, |entry: Pair[K, V]| entry.first != key)
}

@unbounded
fn dict_contains_key[K, V](d: List[Pair[K, V]], key: K) -> Bool {
    any(d, |entry: Pair[K, V]| entry.first == key)
}

fn dict_keys[K, V](d: List[Pair[K, V]]) -> List[K] cost <= 2 {
    map(d, |entry: Pair[K, V]| entry.first)
}

fn dict_values[K, V](d: List[Pair[K, V]]) -> List[V] cost <= 2 {
    map(d, |entry: Pair[K, V]| entry.second)
}
