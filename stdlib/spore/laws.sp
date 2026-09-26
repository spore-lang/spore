import spore.combine as combine

import spore.merge as merge

import spore.order as order

// Spore compositional stdlib — reusable law-oriented helpers built with
// source properties. These are executable patterns, not trusted rewrites.
pub fn merge_self_i32(items: List[I32]) -> List[I32]
properties {
    deduplicates_overlap(): len(merge_self_i32([1i32, 1i32, 2i32])) == 2i64
    keeps_members(): contains(merge_self_i32([1i32, 1i32, 2i32]), 2i32) == true
    idempotent_self_merge(items: List[I32]): merge_self_i32(items) == merge_self_i32(merge_self_i32(items))
}
{ merge_unique_i32([], items) }

pub fn canonical_members_i32(items: List[I32]) -> List[I32]
properties {
    deduplicates_overlap(): len(canonical_members_i32([1i32, 1i32, 2i32])) == 2i64
    keeps_members(): contains(canonical_members_i32([2i32, 1i32, 2i32, 3i32, 1i32]), 3i32) == true
    idempotent(items: List[I32]): canonical_members_i32(items) == canonical_members_i32(canonical_members_i32(items))
}
{ merge_unique_i32([], items) }

pub fn sum3_left_assoc_i32(a: I32, b: I32, c: I32) -> I32
properties {
    adds_values(): sum3_left_assoc_i32(20i32, 10i32, 12i32) == 42i32
    keeps_zero_identity(): sum3_left_assoc_i32(0i32, 5i32, 7i32) == 12i32
    associative(a: I32, b: I32, c: I32): sum3_left_assoc_i32(a, b, c) == combine_pair(a, combine_pair(b, c, |x: I32, y: I32| x + y), |x: I32, y: I32| x + y)
}
{ combine_pair(combine_pair(a, b, |x: I32, y: I32| x + y), c, |x: I32, y: I32| x + y) }

pub fn compare_self_i32(value: I32) -> Bool
properties {
    reflexive(): compare_self_i32(7i32) == true
    zero(): compare_self_i32(0i32) == true
}
{ ordering_is_eq(compare_i32(value, value)) }
