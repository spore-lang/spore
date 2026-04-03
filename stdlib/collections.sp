// Spore standard library — collection utilities
// Runtime builtins (do NOT redefine): map, filter, fold, each, append,
// prepend, head, tail, reverse, range, contains, len.

fn list_is_empty[T](list: List[T]) -> Bool cost <= 2 { len(list) == 0 }

fn head_option[T](list: List[T]) -> Option[T] cost <= 3 {
    match list {
        [x, ..rest] => Some(x),
        [] => None,
    }
}

@unbounded
fn last[T](list: List[T]) -> Option[T] {
    match list {
        [] => None,
        [x] => Some(x),
        [_, ..rest] => last(rest),
    }
}

@unbounded
fn take[T](list: List[T], n: Int) -> List[T] {
    if n <= 0 { [] }
    else {
        match list {
            [] => [],
            [x, ..rest] => prepend(x, take(rest, n - 1)),
        }
    }
}

@unbounded
fn drop[T](list: List[T], n: Int) -> List[T] {
    if n <= 0 { list }
    else {
        match list {
            [] => [],
            [_, ..rest] => drop(rest, n - 1),
        }
    }
}

@unbounded
fn zip[T, U](a: List[T], b: List[U]) -> List[Pair[T, U]] {
    match a {
        [] => [],
        [x, ..xs] => match b {
            [] => [],
            [y, ..ys] => prepend(Pair { first: x, second: y }, zip(xs, ys)),
        },
    }
}

@unbounded
fn enumerate_from[T](list: List[T], start: Int) -> List[Pair[Int, T]] {
    match list {
        [] => [],
        [x, ..rest] => prepend(Pair { first: start, second: x }, enumerate_from(rest, start + 1)),
    }
}

fn enumerate[T](list: List[T]) -> List[Pair[Int, T]] cost <= 2 {
    enumerate_from(list, 0)
}

@unbounded
fn any[T](list: List[T], pred: (T) -> Bool) -> Bool {
    match list {
        [] => false,
        [x, ..rest] => if pred(x) { true } else { any(rest, pred) },
    }
}

@unbounded
fn all[T](list: List[T], pred: (T) -> Bool) -> Bool {
    match list {
        [] => true,
        [x, ..rest] => if pred(x) { all(rest, pred) } else { false },
    }
}

@unbounded
fn find[T](list: List[T], pred: (T) -> Bool) -> Option[T] {
    match list {
        [] => None,
        [x, ..rest] => if pred(x) { Some(x) } else { find(rest, pred) },
    }
}

@unbounded
fn find_index[T](list: List[T], pred: (T) -> Bool) -> Option[Int] {
    find_index_from(list, pred, 0)
}

@unbounded
fn find_index_from[T](list: List[T], pred: (T) -> Bool, i: Int) -> Option[Int] {
    match list {
        [] => None,
        [x, ..rest] => if pred(x) { Some(i) } else { find_index_from(rest, pred, i + 1) },
    }
}

@unbounded
fn flatten[T](list: List[List[T]]) -> List[T] {
    fold(list, [], |acc: List[T], xs: List[T]| fold(xs, acc, |a: List[T], x: T| append(a, x)))
}

@unbounded
fn flat_map[T, U](list: List[T], f: (T) -> List[U]) -> List[U] {
    flatten(map(list, f))
}

@unbounded
fn sort_asc(list: List[Int]) -> List[Int] {
    match list {
        [] => [],
        [pivot, ..rest] => {
            let smaller = filter(rest, |y: Int| y <= pivot);
            let larger = filter(rest, |y: Int| y > pivot);
            let left = sort_asc(smaller);
            let right = sort_asc(larger);
            fold(reverse(append(left, pivot)), right, |acc: List[Int], item: Int| prepend(item, acc))
        },
    }
}

@unbounded
fn sum(list: List[Int]) -> Int {
    fold(list, 0, |acc: Int, x: Int| acc + x)
}

@unbounded
fn product(list: List[Int]) -> Int {
    fold(list, 1, |acc: Int, x: Int| acc * x)
}

@unbounded
fn count[T](list: List[T], pred: (T) -> Bool) -> Int {
    fold(list, 0, |acc: Int, x: T| if pred(x) { acc + 1 } else { acc })
}

@unbounded
fn min_list(list: List[Int]) -> Option[Int] {
    match list {
        [] => None,
        [x, ..rest] => Some(fold(rest, x, |a: Int, b: Int| if b < a { b } else { a })),
    }
}

@unbounded
fn max_list(list: List[Int]) -> Option[Int] {
    match list {
        [] => None,
        [x, ..rest] => Some(fold(rest, x, |a: Int, b: Int| if b > a { b } else { a })),
    }
}

@unbounded
fn nth[T](list: List[T], n: Int) -> Option[T] {
    if n < 0 { None }
    else {
        match list {
            [] => None,
            [x, ..rest] => if n == 0 { Some(x) } else { nth(rest, n - 1) },
        }
    }
}

@unbounded
fn dedup(list: List[Int]) -> List[Int] {
    match list {
        [] => [],
        [x] => [x],
        [x, y, ..rest] => if x == y { dedup(prepend(y, rest)) } else { prepend(x, dedup(prepend(y, rest))) },
    }
}
