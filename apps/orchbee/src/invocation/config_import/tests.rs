use super::*;
use crate::invocation::{CommandPath, HelpTopic, parse_request};

fn parse(args: &[&str]) -> Result<Request, ParseFailure> {
    parse_request(args.iter().map(std::ffi::OsString::from))
}

#[test]
fn import_requires_both_explicit_files_and_rejects_runtime_or_extra_options() {
    for invalid in [
        vec!["config", "import-herdr"],
        vec!["config", "import-herdr", "--input-file", "source.toml"],
        vec!["config", "import-herdr", "--output-file", "new.toml"],
        vec![
            "config",
            "import-herdr",
            "--input-file=-",
            "--output-file=new.toml",
        ],
        vec![
            "config",
            "import-herdr",
            "--input-file=C:/file:stream",
            "--output-file=new.toml",
        ],
        vec![
            "config",
            "import-herdr",
            "--input-file=source.toml",
            "--output-file=new.toml",
            "extra",
        ],
        vec![
            "config",
            "import-herdr",
            "--input-file=source.toml",
            "--output-file=new.toml",
            "--confirm",
        ],
        vec![
            "--site",
            "named",
            "config",
            "import-herdr",
            "--input-file=source.toml",
            "--output-file=new.toml",
        ],
        vec![
            "--machine",
            "saved",
            "config",
            "import-herdr",
            "--input-file=source.toml",
            "--output-file=new.toml",
        ],
        vec![
            "config",
            "import-herdr",
            "--input-file=source.toml",
            "--output-file=new.toml",
            "--dry-run",
            "--dry-run",
        ],
        vec![
            "config",
            "import-herdr",
            "--input-file=source.toml",
            "--output-file=new.toml",
            "--input-file=second.toml",
        ],
    ] {
        assert!(parse(&invalid).is_err(), "{invalid:?}");
    }
    assert_eq!(
        parse(&[
            "config",
            "import-herdr",
            "--input-file=source.toml",
            "--output-file",
            "new.toml",
            "--dry-run",
            "--format=json"
        ])
        .unwrap(),
        Request::ConfigImport {
            command: Command {
                input_file: "source.toml".into(),
                output_file: "new.toml".into(),
                dry_run: true
            },
            format: OutputFormat::Json,
        }
    );
    assert_eq!(
        parse(&["config", "import-herdr", "--help"]).unwrap(),
        Request::Help(HelpTopic::Command(CommandPath::ConfigImportHerdr))
    );
    assert!(parse(&["config", "check", "--dry-run"]).is_err());
}
