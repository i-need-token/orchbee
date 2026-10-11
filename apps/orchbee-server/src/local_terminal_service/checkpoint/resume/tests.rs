use super::*;
#[test]
fn complete_reference_bounds_fit_the_typed_launch() {
    let shell = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
    for value in ["'".repeat(4096), "%".repeat(512), "中".repeat(1365)] {
        let selection = launch(shell, &["pi".into(), "--session".into(), value]).unwrap();
        assert_eq!(
            &selection.arguments()[..4],
            &["-NoLogo", "-NoProfile", "-NoExit", "-EncodedCommand"]
        );
        assert!(selection.arguments()[4].len() <= 30_000);
    }
}
