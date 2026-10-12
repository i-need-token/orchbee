use super::*;

#[test]
fn every_admitted_root_command_has_exactly_one_group() {
    for path in help_paths().iter().filter(|path| path.words().len() == 1) {
        let count = ROOT_GROUPS
            .iter()
            .flat_map(|(_, names)| *names)
            .filter(|name| **name == path.words()[0])
            .count();
        assert_eq!(count, 1, "{}", path.words()[0]);
    }
}
