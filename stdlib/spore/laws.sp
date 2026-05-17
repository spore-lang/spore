import spore.combine
import spore.merge
import spore.order

// Spore compositional stdlib — reusable law-oriented helpers built with today's
// `spec { example ... property ... }` surface. These are executable patterns,
// not trusted rewrites.

@unbounded
pub fn merge_self_i32(items: List[I32]) -> List[I32] cost [O(items), O(items), 0, 0]
spec {
    example "deduplicates_overlap": len(merge_self_i32([1i32, 1i32, 2i32])) == 2i64
    example "keeps_members": contains(merge_self_i32([1i32, 1i32, 2i32]), 2i32) == true
    property "idempotent_self_merge": |items: List[I32]| merge_self_i32(merge_self_i32(items))
}
{ merge_unique_i32([], items) }

@unbounded
pub fn canonical_members_i32(items: List[I32]) -> List[I32] cost [O(items), O(items), 0, 0]
spec {
    example "deduplicates_overlap": len(canonical_members_i32([1i32, 1i32, 2i32])) == 2i64
    example "keeps_members": contains(canonical_members_i32([2i32, 1i32, 2i32, 3i32, 1i32]), 3i32) == true
    property "idempotent": |items: List[I32]| canonical_members_i32(canonical_members_i32(items))
}
{ merge_unique_i32([], items) }

pub fn sum3_left_assoc_i32(a: I32, b: I32, c: I32) -> I32
spec {
    example "adds_values": sum3_left_assoc_i32(20i32, 10i32, 12i32) == 42i32
    example "keeps_zero_identity": sum3_left_assoc_i32(0i32, 5i32, 7i32) == 12i32
    property "associative": |a: I32, b: I32, c: I32|
        combine_pair(a, combine_pair(b, c, |x: I32, y: I32| x + y), |x: I32, y: I32| x + y)
}
{ combine_pair(combine_pair(a, b, |x: I32, y: I32| x + y), c, |x: I32, y: I32| x + y) }

pub fn compare_self_i32(value: I32) -> Bool cost [6, 0, 0, 0]
spec {
    example "reflexive": compare_self_i32(7i32) == true
    example "zero": compare_self_i32(0i32) == true
}
{ ordering_is_eq(compare_i32(value, value)) }
