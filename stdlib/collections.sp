// Spore standard library — collection utilities
// Runtime builtins (do NOT redefine): map, filter, fold, each, append,
// prepend, head, tail, reverse, range, contains, len.
fn list_is_empty[T](list: List[T]) -> Bool { len(list) == 0 }

fn head_option[T](list: List[T]) -> Option[T] {
    match list {
        [x, ..rest] => Some(x),
        [] => None,
    }
}

fn last[T](list: List[T]) -> Option[T] {
    match list {
        [] => None,
        [x] => Some(x),
        [_, ..rest] => last(rest),
    }
}

fn take[T](list: List[T], n: I64) -> List[T]
properties {
    len(): len(take([1, 2, 3, 4], 2)) == 2
    sum(): sum(take([1, 2, 3, 4], 2)) == 3
    zero(): len(take([1, 2, 3], 0)) == 0
}
{
    if n <= 0 { [] } else {
        match list {
            [] => [],
            [x, ..rest] => prepend(x, take(rest, n - 1)),
        }
    }
}

fn drop[T](list: List[T], n: I64) -> List[T]
properties {
    len(): len(drop([1, 2, 3, 4], 2)) == 2
    sum(): sum(drop([1, 2, 3, 4], 2)) == 7
    zero(): len(drop([1, 2, 3], 0)) == 3
}
{
    if n <= 0 { list } else {
        match list {
            [] => [],
            [_, ..rest] => drop(rest, n - 1),
        }
    }
}

fn zip[T, U](a: List[T], b: List[U]) -> List[Pair[T, U]] {
    match a {
        [] => [],
        [x, ..xs] => match b {
            [] => [],
            [y, ..ys] => prepend(Pair { first: x, second: y }, zip(xs, ys)),
        },
    }
}

fn enumerate_from[T](list: List[T], start: I64) -> List[Pair[I64, T]] {
    match list {
        [] => [],
        [x, ..rest] => prepend(Pair { first: start, second: x }, enumerate_from(rest, start + 1)),
    }
}

fn enumerate[T](list: List[T]) -> List[Pair[I64, T]] { enumerate_from(list, 0) }

fn any[T](list: List[T], pred: (T) -> Bool) -> Bool
properties {
    found(): any([1, 2, 3], |x: I64| x > 2) == true
    not_found(): any([1, 2, 3], |x: I64| x > 5) == false
    empty(): any([], |x: I64| x > 0) == false
}
{
    match list {
        [] => false,
        [x, ..rest] => if pred(x) { true } else { any(rest, pred) },
    }
}

fn all[T](list: List[T], pred: (T) -> Bool) -> Bool
properties {
    all_true(): all([2, 4, 6], |x: I64| x % 2 == 0) == true
    some_false(): all([2, 3, 6], |x: I64| x % 2 == 0) == false
    empty(): all([], |x: I64| x > 0) == true
}
{
    match list {
        [] => true,
        [x, ..rest] => if pred(x) { all(rest, pred) } else { false },
    }
}

fn find[T](list: List[T], pred: (T) -> Bool) -> Option[T] {
    match list {
        [] => None,
        [x, ..rest] => if pred(x) { Some(x) } else { find(rest, pred) },
    }
}

fn find_index[T](list: List[T], pred: (T) -> Bool) -> Option[I64] { find_index_from(list, pred, 0) }

fn find_index_from[T](list: List[T], pred: (T) -> Bool, i: I64) -> Option[I64] {
    match list {
        [] => None,
        [x, ..rest] => if pred(x) { Some(i) } else { find_index_from(rest, pred, i + 1) },
    }
}

fn flatten[T](list: List[List[T]]) -> List[T]
properties {
    sum(): sum(flatten([[1, 2], [3], [4, 5]])) == 15
    len(): len(flatten([[1, 2], [3]])) == 3
}
{ fold(list, [], |acc: List[T], xs: List[T]| fold(xs, acc, |a: List[T], x: T| append(a, x))) }

fn flat_map[T, U](list: List[T], f: (T) -> List[U]) -> List[U] { flatten(map(list, f)) }

fn sort_asc(list: List[I64]) -> List[I64]
properties {
    preserves_sum(): sum(sort_asc([3, 1, 4, 1, 5])) == 14
    preserves_len(): len(sort_asc([3, 1, 4])) == 3
    empty(): len(sort_asc([])) == 0
}
{
    match list {
        [] => [],
        [pivot, ..rest] => {
            let smaller = filter(rest, |y: I64| y <= pivot);
            let larger = filter(rest, |y: I64| y > pivot);
            let left = sort_asc(smaller);
            let right = sort_asc(larger);
            fold(reverse(append(left, pivot)), right, |acc: List[I64], item: I64| prepend(item, acc))
        },
    }
}

fn sum(list: List[I64]) -> I64
properties {
    basic(): sum([1, 2, 3]) == 6
    empty(): sum([]) == 0
    single(): sum([42]) == 42
}
{ fold(list, 0, |acc: I64, x: I64| acc + x) }

fn product(list: List[I64]) -> I64
properties {
    basic(): product([2, 3, 4]) == 24
    empty(): product([]) == 1
}
{ fold(list, 1, |acc: I64, x: I64| acc * x) }

fn count[T](list: List[T], pred: (T) -> Bool) -> I64
properties {
    basic(): count([1, 2, 3, 4, 5], |x: I64| x > 3) == 2
    none(): count([1, 2], |x: I64| x > 5) == 0
    empty(): count([], |x: I64| x > 0) == 0
}
{ fold(list, 0, |acc: I64, x: T| if pred(x) { acc + 1 } else { acc }) }

fn min_list(list: List[I64]) -> Option[I64] {
    match list {
        [] => None,
        [x, ..rest] => Some(fold(rest, x, |a: I64, b: I64| if b < a { b } else { a })),
    }
}

fn max_list(list: List[I64]) -> Option[I64] {
    match list {
        [] => None,
        [x, ..rest] => Some(fold(rest, x, |a: I64, b: I64| if b > a { b } else { a })),
    }
}

fn nth[T](list: List[T], n: I64) -> Option[T] {
    if n < 0 { None } else {
        match list {
            [] => None,
            [x, ..rest] => if n == 0 { Some(x) } else { nth(rest, n - 1) },
        }
    }
}

fn dedup(list: List[I64]) -> List[I64]
properties {
    reduces_len(): len(dedup([1, 1, 2, 2, 3])) == 3
    sum(): sum(dedup([1, 1, 2, 2, 3])) == 6
    no_dups(): len(dedup([1, 2, 3])) == 3
}
{
    match list {
        [] => [],
        [x] => [x],
        [x, y, ..rest] => if x == y { dedup(prepend(y, rest)) } else { prepend(x, dedup(prepend(y, rest))) },
    }
}
