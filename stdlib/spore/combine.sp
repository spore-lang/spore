// Spore compositional stdlib — target-named helper surface.
// This first slice stays within today's trait/spec/function capabilities.

pub trait Combine[T] {
    fn combine(left: T, right: T) -> T
}

pub fn combine_pair[T](left: T, right: T, step: (T, T) -> T) -> T cost [3, 0, 0, 0]
spec {
    example "sum": combine_pair(20, 22, |a: I32, b: I32| a + b) == 42
    example "keep_left": combine_pair(true, false, |a: Bool, _b: Bool| a) == true
}
{ step(left, right) }

@unbounded
pub fn combine_all[T](items: List[T], seed: T, step: (T, T) -> T) -> T cost [O(items), 0, 0, 0]
spec {
    example "empty_keeps_seed": combine_all([], 10, |acc: I32, item: I32| acc + item) == 10
    example "sum": combine_all([1, 2, 3], 0, |acc: I32, item: I32| acc + item) == 6
}
{ fold(items, seed, step) }
