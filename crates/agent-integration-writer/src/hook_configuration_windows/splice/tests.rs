use super::*;
use serde_json::json;

fn array(elements: Vec<(usize, Change)>, append: Vec<Value>) -> Change {
    Change::Array { elements, append }
}

fn object(members: Vec<(&'static str, Change)>, append: Vec<(&'static str, Value)>) -> Change {
    Change::Object { members, append }
}

/// Removing an element takes exactly that element and one separator, wherever it sits; every
/// other byte stays, and appending then removing a value restores the original bytes.
#[test]
fn removal_takes_one_element_and_append_round_trips() {
    let text = "{\r\n\t\"list\" :  [\r\n\t\t1.50,\r\n\t\t\"two\",\r\n\t\t{ \"three\" : 3 }\r\n\t],\r\n\t\"other\": 1.0E+2\r\n}\r\n";
    for (index, expected) in [
        (
            0,
            "{\r\n\t\"list\" :  [\r\n\t\t\"two\",\r\n\t\t{ \"three\" : 3 }\r\n\t],\r\n\t\"other\": 1.0E+2\r\n}\r\n",
        ),
        (
            1,
            "{\r\n\t\"list\" :  [\r\n\t\t1.50,\r\n\t\t{ \"three\" : 3 }\r\n\t],\r\n\t\"other\": 1.0E+2\r\n}\r\n",
        ),
        (
            2,
            "{\r\n\t\"list\" :  [\r\n\t\t1.50,\r\n\t\t\"two\"\r\n\t],\r\n\t\"other\": 1.0E+2\r\n}\r\n",
        ),
    ] {
        let change = object(
            vec![("list", array(vec![(index, Change::Remove)], vec![]))],
            vec![],
        );
        assert_eq!(apply(text, &change).unwrap(), expected, "{index}");
    }
    let appended = apply(
        text,
        &object(
            vec![("list", array(vec![], vec![json!({"new": [true]})]))],
            vec![("added", json!({"a": 1}))],
        ),
    )
    .unwrap();
    let parsed: Value = serde_json::from_str(&appended).unwrap();
    assert_eq!(parsed["list"][3], json!({"new": [true]}));
    assert_eq!(parsed["added"], json!({"a": 1}));
    assert!(!appended.replace("\r\n", "").contains('\n'));
    let removed = apply(
        &appended,
        &object(
            vec![
                ("list", array(vec![(3, Change::Remove)], vec![])),
                ("added", Change::Remove),
            ],
            vec![],
        ),
    )
    .unwrap();
    assert_eq!(removed, text);
}

/// A container whose last members a change removes goes too, up to but excluding the root, so a
/// container that appending created leaves with the values appended into it.
#[test]
fn emptied_containers_leave_and_the_root_stays() {
    for text in [
        "{\"keep\":[]}",
        "{\n  \"keep\": [],\n  \"user\": {\"x\": 1}\n}\n",
    ] {
        let added = apply(
            text,
            &object(vec![], vec![("hooks", json!({"Start": [{"run": "x"}]}))]),
        )
        .unwrap();
        let change = object(
            vec![(
                "hooks",
                object(
                    vec![("Start", array(vec![(0, Change::Remove)], vec![]))],
                    vec![],
                ),
            )],
            vec![],
        );
        assert_eq!(apply(&added, &change).unwrap(), text);
    }
    assert_eq!(
        apply(
            "{ \"only\" : 1 }",
            &object(vec![("only", Change::Remove)], vec![])
        )
        .unwrap(),
        "{}"
    );
    let pretty = apply("{\n}", &object(vec![], vec![("a", json!([1]))])).unwrap();
    assert_eq!(pretty, "{\n  \"a\": [\n    1\n  ]\n}");
}

/// A replacement equal to the current value keeps its bytes; a different one changes only it.
#[test]
fn replacement_changes_only_a_different_value() {
    let text = "[ {\"b\" : 2 ,\"a\":1}, 7 ]";
    let same = array(vec![(0, Change::Replace(json!({"a": 1, "b": 2})))], vec![]);
    assert_eq!(apply(text, &same).unwrap(), text);
    let other = array(vec![(0, Change::Replace(json!({"a": 3})))], vec![]);
    assert_eq!(apply(text, &other).unwrap(), "[ {\"a\":3}, 7 ]");
    let absent = array(vec![(5, Change::Replace(json!(1)))], vec![]);
    assert!(apply(text, &absent).is_none());
}
