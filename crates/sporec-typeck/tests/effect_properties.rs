use proptest::prelude::*;
use sporec_typeck::effect_set::{EffectHierarchy, EffectSet};

fn effects() -> impl Strategy<Value = EffectSet> {
    prop::collection::vec("[A-Z][a-z]{0,8}", 0..12).prop_map(EffectSet::from_names)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn effect_union_and_difference_obey_set_laws(a in effects(), b in effects(), c in effects()) {
        prop_assert_eq!(a.union(&b), b.union(&a));
        prop_assert_eq!(a.union(&b).union(&c), a.union(&b.union(&c)));
        prop_assert_eq!(a.intersection(&b), b.intersection(&a));
        prop_assert_eq!(a.difference(&b).union(&a.intersection(&b)), a.clone());
        prop_assert_eq!(a.is_subset_of(&b), b.is_superset_of(&a));
        prop_assert_eq!(a.missing_from(&b), b.difference(&a).iter().cloned().collect::<Vec<_>>());
        prop_assert!(a.is_subset_of(&a.union(&b)));
    }

    #[test]
    fn hierarchy_expansion_terminates_and_is_idempotent(
        edges in prop::collection::vec((0_u8..8, 0_u8..8), 0..32),
        roots in prop::collection::vec(0_u8..8, 0..8),
    ) {
        let mut hierarchy = EffectHierarchy::new();
        for (parent, child) in edges {
            hierarchy.add_implies(format!("Effect{parent}"), format!("Effect{child}"));
        }
        let roots = EffectSet::from_names(roots.into_iter().map(|n| format!("Effect{n}")));
        let expanded = hierarchy.expand(&roots);
        prop_assert!(roots.is_subset_of(&expanded));
        prop_assert_eq!(hierarchy.expand(&expanded), expanded.clone());
        prop_assert!(hierarchy.check_propagation(&roots, &expanded).is_ok());
        let missing = EffectSet::from_names(["NotDeclared".to_owned()]);
        prop_assert_eq!(hierarchy.check_propagation(&roots, &missing), Err(vec!["NotDeclared".to_owned()]));
    }
}

#[test]
fn standard_collection_adapters_preserve_effect_requirements() {
    let names = ["Console".to_owned(), "FileRead".to_owned()];
    let expected = names
        .clone()
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    let from_collection = EffectSet::from_btreeset(expected.clone());
    let from_iterator: EffectSet = names.into_iter().collect();
    for effects in [from_collection, from_iterator] {
        assert!(!effects.is_empty());
        assert_eq!(effects.len(), 2);
        assert!(effects.contains("Console"));
        assert!(effects.contains("FileRead"));
        assert_eq!(effects.to_btreeset(), expected);
        assert_eq!(effects.to_string(), "[Console, FileRead]");
    }
    assert!(EffectSet::new().is_empty());
}
