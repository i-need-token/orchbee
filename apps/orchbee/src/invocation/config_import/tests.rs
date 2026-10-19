use super::*;
use crate::invocation::{CommandPath, HelpTopic, parse_request};

fn parse(args: &[&str]) -> Result<Request, ParseFailure> {
    parse_request(args.iter().map(std::ffi::OsString::from))
}

#[test]
fn import_defaults_omitted_files_and_rejects_runtime_or_extra_options() {
    for (args, input, output) in [
        (vec!["config", "import-herdr"], None, None),
        (
            vec!["config", "import-herdr", "--input-file", "source.toml"],
            Some("source.toml"),
            None,
        ),
        (
            vec!["config", "import-herdr", "--output-file", "new.toml"],
            None,
            Some("new.toml"),
        ),
    ] {
        assert_eq!(
            parse(&args).unwrap(),
            Request::ConfigImport {
                command: Command {
                    input_file: input.map(str::to_owned),
                    output_file: output.map(str::to_owned),
                    dry_run: false,
                },
                format: OutputFormat::Text,
            },
            "{args:?}"
        );
    }
    for invalid in [
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
            "--session",
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
                input_file: Some("source.toml".into()),
                output_file: Some("new.toml".into()),
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
