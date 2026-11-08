//! Rendering bounds and actual interpreter evidence for the literal-word adapter.

#[test]
fn script_carries_the_shared_grammar_within_the_output_bound() {
    let script = super::render("orchbee");
    assert!(script.len() < crate::cli::output::LIMIT);
    assert!(!script.contains("__ORCHBEE_GRAMMAR__"));
    let start =
        script.find("var grammar = (print '").expect("grammar") + "var grammar = (print '".len();
    let end = start + script[start..].find("' | from-json)").expect("literal end");
    let embedded: serde_json::Value =
        serde_json::from_str(&script[start..end].replace("''", "'")).expect("JSON grammar");
    assert_eq!(embedded, super::super::grammar::build());
}

#[cfg(windows)]
#[test]
#[ignore = "requires ORCHBEE_TEST_ELVISH; run explicitly with the Windows Elvish interpreter"]
fn real_elvish_completes_scoped_literal_words_without_runtime() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };

    let executable = std::env::var_os("ORCHBEE_TEST_ELVISH")
        .expect("set ORCHBEE_TEST_ELVISH to an actual Elvish interpreter");
    let cases = super::super::tests::cases::CASES;
    let mut words = cases
        .iter()
        .map(|(line, _, _)| super::super::tests::cases::literal_words(line))
        .collect::<Vec<_>>();
    // Editor-provided words stay data even when they contain executable-looking text.
    words.push(vec![
        "orchbee.exe".into(),
        "tab".into(),
        "create".into(),
        "--exec".into(),
        "$(throw 'completion must not evaluate this')".into(),
        "--a".into(),
    ]);
    let json = serde_json::to_string(&words)
        .expect("fixtures")
        .replace('\'', "''");
    let script = format!(
        "var edit: = (ns [&completion:=(ns [&arg-completer=[&]])])\n{}\nvar cases = (print '{json}' | from-json)\nfor words $cases {{ put [($edit:completion:arg-completer[$words[0]] $@words)] | to-json }}\n",
        super::render("orchbee")
    );
    let mut child = Command::new(executable)
        .args(["-norc", "-c", "eval (slurp)"])
        .env("ORCHBEE_RUNTIME_BASE", "invalid-relative-runtime")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Elvish interpreter");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(script.as_bytes())
        .expect("script");
    let result = child.wait_with_output().expect("interpreter result");
    assert!(
        result.status.success() && result.stderr.is_empty(),
        "{} {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let stdout = String::from_utf8(result.stdout).expect("UTF-8 output");
    let results = stdout
        .lines()
        .map(|line| serde_json::from_str::<Vec<String>>(line).expect("candidate array"))
        .collect::<Vec<_>>();
    assert_eq!(results.len(), words.len());
    for (index, (line, candidate, expected)) in cases.iter().enumerate() {
        let prefix = words[index].last().expect("unfinished word");
        let found =
            candidate.starts_with(prefix) && results[index].iter().any(|value| value == candidate);
        assert_eq!(
            found, *expected,
            "{line}: {candidate}; {:?}",
            results[index]
        );
        let mut unique = results[index].clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), results[index].len(), "duplicate at {line}");
    }
    assert!(
        results
            .last()
            .expect("literal case")
            .contains(&"--arg".to_owned())
    );
}
