use super::natural_order;

#[test]
fn names_sort_by_the_value_of_their_digit_runs_without_regard_to_case() {
    let mut names = vec!["build-10", "Build-2", "build-02a", "alpha", "build-1"];
    names.sort_by(|left, right| natural_order(left, right));
    assert_eq!(
        names,
        ["alpha", "build-1", "Build-2", "build-02a", "build-10"]
    );
}
