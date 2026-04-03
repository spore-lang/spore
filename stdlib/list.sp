// Spore standard library — list operations
// Higher-level combinators built on builtins (map, filter, fold, etc.)

fn is_empty_list[T](xs: List[T]) -> Bool cost <= 2 { len(xs) == 0 }

fn last[T](xs: List[T]) -> T cost <= 100 {
    head(reverse(xs))
}

fn take[T](xs: List[T], n: Int) -> List[T] cost <= 1000 {
    if n <= 0 { [] }
    else {
        if len(xs) == 0 { [] }
        else { prepend(take(tail(xs), n - 1), head(xs)) }
    }
}

fn drop[T](xs: List[T], n: Int) -> List[T] cost <= 1000 {
    if n <= 0 { xs }
    else {
        if len(xs) == 0 { [] }
        else { drop(tail(xs), n - 1) }
    }
}

fn any[T](xs: List[T], pred: (T) -> Bool) -> Bool cost <= 1000 {
    fold(xs, false, |acc: Bool, x: T| if acc { true } else { pred(x) })
}

fn all[T](xs: List[T], pred: (T) -> Bool) -> Bool cost <= 1000 {
    fold(xs, true, |acc: Bool, x: T| if acc { pred(x) } else { false })
}

fn find[T](xs: List[T], pred: (T) -> Bool) -> Option[T] cost <= 1000 {
    if len(xs) == 0 { None }
    else {
        let h = head(xs);
        if pred(h) { Some(h) }
        else { find(tail(xs), pred) }
    }
}

fn flatten[T](xss: List[List[T]]) -> List[T] cost <= 10000 {
    fold(xss, [], |acc: List[T], xs: List[T]| concat(acc, xs))
}

fn flat_map[T, U](xs: List[T], f: (T) -> List[U]) -> List[U] cost <= 10000 {
    flatten(map(xs, f))
}

fn zip[A, B](as_list: List[A], bs: List[B]) -> List[Pair[A, B]] cost <= 1000 {
    if len(as_list) == 0 { [] }
    else {
        if len(bs) == 0 { [] }
        else {
            let pair = Pair { first: head(as_list), second: head(bs) };
            prepend(zip(tail(as_list), tail(bs)), pair)
        }
    }
}

fn enumerate[T](xs: List[T]) -> List[Pair[Int, T]] cost <= 1000 {
    zip(range(0, len(xs)), xs)
}

fn count[T](xs: List[T], pred: (T) -> Bool) -> Int cost <= 1000 {
    len(filter(xs, pred))
}

fn sum(xs: List[Int]) -> Int cost <= 1000 {
    fold(xs, 0, |acc: Int, x: Int| acc + x)
}

fn product(xs: List[Int]) -> Int cost <= 1000 {
    fold(xs, 1, |acc: Int, x: Int| acc * x)
}

fn index_of[T](xs: List[T], item: T) -> Int cost <= 1000 {
    if len(xs) == 0 { 0 - 1 }
    else {
        if head(xs) == item { 0 }
        else {
            let rest = index_of(tail(xs), item);
            if rest < 0 { 0 - 1 }
            else { rest + 1 }
        }
    }
}

fn unique[T](xs: List[T]) -> List[T] cost <= 10000 {
    fold(xs, [], |acc: List[T], x: T|
        if contains(acc, x) { acc }
        else { append(acc, x) }
    )
}

fn repeat[T](item: T, n: Int) -> List[T] cost <= 1000 {
    if n <= 0 { [] }
    else { prepend(repeat(item, n - 1), item) }
}

fn sort_asc(xs: List[Int]) -> List[Int] cost <= 100000 {
    if len(xs) <= 1 { xs }
    else {
        let pivot = head(xs);
        let rest = tail(xs);
        let lo = filter(rest, |x: Int| x < pivot);
        let hi = filter(rest, |x: Int| x >= pivot);
        concat(concat(sort_asc(lo), [pivot]), sort_asc(hi))
    }
}

fn sort_desc(xs: List[Int]) -> List[Int] cost <= 100000 {
    reverse(sort_asc(xs))
}
