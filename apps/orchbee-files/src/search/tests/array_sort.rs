//! The sort Quick Open orders its first matches with, against node's own `Array.prototype.sort`,
//! for an order, an order that treats near values as equal, and no order at all.

use std::cmp::Ordering;

use serde::Deserialize;

use super::super::array_sort;

#[derive(Deserialize)]
struct Corpus {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    comparator: String,
    pattern: String,
    values: Vec<i64>,
    order: Vec<usize>,
}

fn comparator(name: &str) -> fn(i64, i64) -> Ordering {
    match name {
        "numbers" => |a, b| a.cmp(&b),
        "tolerance" => |a, b| {
            if (a - b).abs() <= 2 {
                Ordering::Equal
            } else {
                a.cmp(&b)
            }
        },
        "chaos" => |a, b| ((a * 7 + b * 13) % 5 - 2).cmp(&0),
        _ => panic!("no comparator {name}"),
    }
}

#[test]
fn sorts_as_v8_sorts() {
    let corpus: Corpus = serde_json::from_str(include_str!("array-sort.json")).unwrap();
    for case in corpus.cases {
        let compare = comparator(&case.comparator);
        let mut order: Vec<usize> = (0..case.values.len()).collect();
        array_sort::sort(&mut order, |a, b| compare(case.values[*a], case.values[*b]));
        let context = (&case.comparator, &case.pattern, case.values.len());
        assert_eq!(order, case.order, "{context:?}");
    }
}
