import spore.combine
import spore.merge

// Spore compositional stdlib — reusable law-oriented helpers built with today's
// `spec { example ... property ... }` surface. These are executable patterns,
// not trusted rewrites.

@unbounded
pub fn canonical_members_i32(items: List[I32]) -> List[I32] cost [O(items), O(items), 0, 0]
spec {
    example "deduplicates_overlap": len(canonical_members_i32([1, 1, 2])) == 2
    example "keeps_members": contains(canonical_members_i32([2, 1, 2, 3, 1]), 3) == true
    property "idempotent": |items: List[I32]| canonical_members_i32(canonical_members_i32(items))
}
{ merge_unique_i32([], items) }

pub fn sum3_left_assoc_i32(a: I32, b: I32, c: I32) -> I32
spec {
    example "adds_values": sum3_left_assoc_i32(20, 10, 12) == 42
    example "keeps_zero_identity": sum3_left_assoc_i32(0, 5, 7) == 12
    property "associative": |a: I32, b: I32, c: I32|
        combine_pair(a, combine_pair(b, c, |x: I32, y: I32| x + y), |x: I32, y: I32| x + y)
}
{ combine_pair(combine_pair(a, b, |x: I32, y: I32| x + y), c, |x: I32, y: I32| x + y) }
