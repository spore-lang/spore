import spore.merge
import spore.order

// Spore compositional stdlib — executable law-shaped examples.
// These stay within current `spec { example ... property ... }` support.

@unbounded
pub fn merge_self_i32(items: List[I32]) -> List[I32] cost [O(items), O(items), 0, 0]
spec {
    example "deduplicates_overlap": len(merge_self_i32([1, 1, 2])) == 2
    example "keeps_members": contains(merge_self_i32([1, 1, 2]), 2) == true
    property "idempotent_self_merge": |items: List[I32]| merge_unique_i32([], items)
}
{ merge_unique_i32([], items) }

pub fn compare_self_i32(value: I32) -> Bool cost [6, 0, 0, 0]
spec {
    example "reflexive": compare_self_i32(7) == true
    example "zero": compare_self_i32(0) == true
}
{ ordering_is_eq(compare_i32(value, value)) }
