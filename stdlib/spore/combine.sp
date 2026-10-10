// Spore compositional stdlib — target-named helper surface.
// This first slice stays within today's trait/property/function capabilities.
pub trait Combine[T] {
        fn combine(left: T, right: T) -> T;

}

pub fn combine_pair[T](left: T, right: T, step: (T, T) -> T) -> T
properties {
    sum(): combine_pair(20i32, 22i32, |a: I32, b: I32| a + b) == 42i32
    keep_left(): combine_pair(true, false, |a: Bool, _b: Bool| a) == true
}
{ step(left, right) }

pub fn combine_all[T](items: List[T], seed: T, step: (T, T) -> T) -> T
properties {
    empty_keeps_seed(): combine_all([], 10i32, |acc: I32, item: I32| acc + item) == 10i32
    sum(): combine_all([1i32, 2i32, 3i32], 0i32, |acc: I32, item: I32| acc + item) == 6i32
}
{ fold(items, seed, step) }
