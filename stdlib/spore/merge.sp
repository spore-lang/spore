import spore.combine as combine

// Spore compositional stdlib — merge-oriented helpers.
pub trait Merge[T] {
        fn merge(left: T, right: T) -> T;

}

pub fn merge_unique_i32(left: List[I32], right: List[I32]) -> List[I32]
properties {
    adds_new_member(): contains(merge_unique_i32([1i32], [2i32]), 2i32) == true
    overlap_stays_unique(): len(merge_unique_i32([1i32], [1i32, 1i32])) == 1
}
{ fold(right, left, |acc: List[I32], item: I32| if contains(acc, item) { acc } else { append(acc, item) }) }

pub fn merge_unique_str(left: List[Str], right: List[Str]) -> List[Str]
properties {
    adds_new_member(): contains(merge_unique_str(["a"], ["b"]), "b") == true
    overlap_stays_unique(): len(merge_unique_str(["a"], ["a", "a"])) == 1
}
{ fold(right, left, |acc: List[Str], item: Str| if contains(acc, item) { acc } else { append(acc, item) }) }

pub fn merge_all_i32(parts: List[List[I32]]) -> List[I32]
properties {
    empty(): len(merge_all_i32([])) == 0
    deduplicates_across_parts(): len(merge_all_i32([[1i32, 2i32], [2i32, 3i32]])) == 3
}
{ combine_all(parts, [], |acc: List[I32], part: List[I32]| merge_unique_i32(acc, part)) }
