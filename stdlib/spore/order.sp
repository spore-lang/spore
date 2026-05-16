// Spore compositional stdlib — small ordering helpers over the current Ordering type.

pub trait PartialOrder[T] {
    fn compare(left: T, right: T) -> Ordering
}

pub trait TotalOrder[T] {
    fn compare(left: T, right: T) -> Ordering
}

pub fn compare_i32(left: I32, right: I32) -> Ordering cost [3, 0, 0, 0]
spec {
    example "less": ordering_is_lt(compare_i32(1i32, 2i32)) == true
    example "equal": ordering_is_eq(compare_i32(2i32, 2i32)) == true
}
{
    if left < right { Less } else {
        if left > right { Greater } else { Equal }
    }
}

pub fn compare_bool(left: Bool, right: Bool) -> Ordering cost [4, 0, 0, 0]
spec {
    example "false_before_true": ordering_is_lt(compare_bool(false, true)) == true
    example "equal": ordering_is_eq(compare_bool(true, true)) == true
}
{
    if left == right { Equal } else {
        if left { Greater } else { Less }
    }
}

pub fn ordering_is_lt(ordering: Ordering) -> Bool cost [3, 0, 0, 0] {
    match ordering {
        Less => true,
        Equal => false,
        Greater => false,
    }
}

pub fn ordering_is_eq(ordering: Ordering) -> Bool cost [3, 0, 0, 0] {
    match ordering {
        Less => false,
        Equal => true,
        Greater => false,
    }
}

pub fn ordering_then(first: Ordering, second: Ordering) -> Ordering cost [3, 0, 0, 0]
spec {
    example "keeps_first_decision": ordering_is_lt(ordering_then(Less, Greater)) == true
    example "uses_second_when_equal": ordering_is_lt(ordering_then(Equal, Less)) == true
}
{
    match first {
        Equal => second,
        _ => first,
    }
}
