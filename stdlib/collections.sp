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
fn take[T](list: List[T], n: I32) -> List[T]
spec {
    example "len": len(take([1, 2, 3, 4], 2)) == 2
    example "sum": sum(take([1, 2, 3, 4], 2)) == 3
    example "zero": len(take([1, 2, 3], 0)) == 0
}
{
    if n <= 0 { [] }
    else {
        match list {
            [] => [],
            [x, ..rest] => prepend(x, take(rest, n - 1)),
        }
    }
}

@unbounded
fn drop[T](list: List[T], n: I32) -> List[T]
spec {
    example "len": len(drop([1, 2, 3, 4], 2)) == 2
    example "sum": sum(drop([1, 2, 3, 4], 2)) == 7
    example "zero": len(drop([1, 2, 3], 0)) == 3
}
{
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
fn enumerate_from[T](list: List[T], start: I32) -> List[Pair[I32, T]] {
    match list {
        [] => [],
        [x, ..rest] => prepend(Pair { first: start, second: x }, enumerate_from(rest, start + 1)),
    }
}

fn enumerate[T](list: List[T]) -> List[Pair[I32, T]] cost <= 2 {
    enumerate_from(list, 0)
}

@unbounded
fn any[T](list: List[T], pred: (T) -> Bool) -> Bool
spec {
    example "found": any([1, 2, 3], |x: I32| x > 2) == true
    example "not_found": any([1, 2, 3], |x: I32| x > 5) == false
    example "empty": any([], |x: I32| x > 0) == false
}
{
    match list {
        [] => false,
        [x, ..rest] => if pred(x) { true } else { any(rest, pred) },
    }
}

@unbounded
fn all[T](list: List[T], pred: (T) -> Bool) -> Bool
spec {
    example "all_true": all([2, 4, 6], |x: I32| x % 2 == 0) == true
    example "some_false": all([2, 3, 6], |x: I32| x % 2 == 0) == false
    example "empty": all([], |x: I32| x > 0) == true
}
{
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
fn find_index[T](list: List[T], pred: (T) -> Bool) -> Option[I32] {
    find_index_from(list, pred, 0)
}

@unbounded
fn find_index_from[T](list: List[T], pred: (T) -> Bool, i: I32) -> Option[I32] {
    match list {
        [] => None,
        [x, ..rest] => if pred(x) { Some(i) } else { find_index_from(rest, pred, i + 1) },
    }
}

@unbounded
fn flatten[T](list: List[List[T]]) -> List[T]
spec {
    example "sum": sum(flatten([[1, 2], [3], [4, 5]])) == 15
    example "len": len(flatten([[1, 2], [3]])) == 3
}
{
    fold(list, [], |acc: List[T], xs: List[T]| fold(xs, acc, |a: List[T], x: T| append(a, x)))
}

@unbounded
fn flat_map[T, U](list: List[T], f: (T) -> List[U]) -> List[U] {
    flatten(map(list, f))
}

@unbounded
fn sort_asc(list: List[I32]) -> List[I32]
spec {
    example "preserves_sum": sum(sort_asc([3, 1, 4, 1, 5])) == 14
    example "preserves_len": len(sort_asc([3, 1, 4])) == 3
    example "empty": len(sort_asc([])) == 0
}
{
    match list {
        [] => [],
        [pivot, ..rest] => {
            let smaller = filter(rest, |y: I32| y <= pivot);
            let larger = filter(rest, |y: I32| y > pivot);
            let left = sort_asc(smaller);
            let right = sort_asc(larger);
            fold(reverse(append(left, pivot)), right, |acc: List[I32], item: I32| prepend(item, acc))
        },
    }
}

@unbounded
fn sum(list: List[I32]) -> I32
spec {
    example "basic": sum([1, 2, 3]) == 6
    example "empty": sum([]) == 0
    example "single": sum([42]) == 42
}
{
    fold(list, 0, |acc: I32, x: I32| acc + x)
}

@unbounded
fn product(list: List[I32]) -> I32
spec {
    example "basic": product([2, 3, 4]) == 24
    example "empty": product([]) == 1
}
{
    fold(list, 1, |acc: I32, x: I32| acc * x)
}

@unbounded
fn count[T](list: List[T], pred: (T) -> Bool) -> I32
spec {
    example "basic": count([1, 2, 3, 4, 5], |x: I32| x > 3) == 2
    example "none": count([1, 2], |x: I32| x > 5) == 0
    example "empty": count([], |x: I32| x > 0) == 0
}
{
    fold(list, 0, |acc: I32, x: T| if pred(x) { acc + 1 } else { acc })
}

@unbounded
fn min_list(list: List[I32]) -> Option[I32] {
    match list {
        [] => None,
        [x, ..rest] => Some(fold(rest, x, |a: I32, b: I32| if b < a { b } else { a })),
    }
}

@unbounded
fn max_list(list: List[I32]) -> Option[I32] {
    match list {
        [] => None,
        [x, ..rest] => Some(fold(rest, x, |a: I32, b: I32| if b > a { b } else { a })),
    }
}

@unbounded
fn nth[T](list: List[T], n: I32) -> Option[T] {
    if n < 0 { None }
    else {
        match list {
            [] => None,
            [x, ..rest] => if n == 0 { Some(x) } else { nth(rest, n - 1) },
        }
    }
}

@unbounded
fn dedup(list: List[I32]) -> List[I32]
spec {
    example "reduces_len": len(dedup([1, 1, 2, 2, 3])) == 3
    example "sum": sum(dedup([1, 1, 2, 2, 3])) == 6
    example "no_dups": len(dedup([1, 2, 3])) == 3
}
{
    match list {
        [] => [],
        [x] => [x],
        [x, y, ..rest] => if x == y { dedup(prepend(y, rest)) } else { prepend(x, dedup(prepend(y, rest))) },
    }
}
