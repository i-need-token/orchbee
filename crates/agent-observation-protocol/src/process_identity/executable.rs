//! Explicit executable and runtime entrypoint recognition, never argument search.

use super::{native_agent_basename as basename, native_agent_provider as provider};

/// Recognizes an executable or supported runtime entrypoint; arguments are never searched for names.
pub fn identify_agent_process(
    image: Option<&str>,
    argv: Option<&[String]>,
) -> Option<&'static str> {
    let image = image?;
    if let Some(direct) = provider(image) {
        return interactive(direct, argv.and_then(|a| a.get(1..)));
    }
    let runtime = basename(image);
    let argv = argv?;
    // A forged argv[0] must not change the actual executable's identity.
    if basename(argv.first()?) != runtime {
        return None;
    }
    let script = entrypoint(&runtime, argv)?;
    let identified = provider(&argv[script]).or_else(|| package(&argv[script]))?;
    interactive(identified, argv.get(script + 1..))
}

fn entrypoint(runtime: &str, argv: &[String]) -> Option<usize> {
    let python = runtime
        .strip_prefix("python")
        .is_some_and(|v| v.chars().all(|c| c.is_ascii_digit() || c == '.'));
    if !matches!(
        runtime,
        "node" | "bun" | "sh" | "bash" | "zsh" | "fish" | "pwsh" | "powershell"
    ) && !python
    {
        return None;
    }
    let powershell = matches!(runtime, "pwsh" | "powershell");
    let mut index = 1;
    while let Some(arg) = argv.get(index) {
        let lower = arg.to_ascii_lowercase();
        if powershell {
            match lower.as_str() {
                "-file" | "-f" => return (index + 1 < argv.len()).then_some(index + 1),
                "-noprofile" | "-nologo" | "-noninteractive" => {}
                "-executionpolicy" | "-windowstyle" | "-workingdirectory" => {
                    argv.get(index + 1)?;
                    index += 1;
                }
                _ => return None,
            }
        } else {
            match arg.as_str() {
                "--" => return (index + 1 < argv.len()).then_some(index + 1),
                "-r"
                | "--require"
                | "--loader"
                | "--import"
                | "--experimental-loader"
                | "-W"
                | "-X" => {
                    argv.get(index + 1)?;
                    index += 1;
                }
                "--no-warnings"
                | "--enable-source-maps"
                | "--experimental-strip-types"
                | "--inspect"
                | "--inspect-brk"
                | "-u"
                | "-B"
                | "-E"
                | "-s" => {}
                value if value.starts_with("--inspect=") || value.starts_with("--inspect-brk=") => {
                }
                value if value.starts_with('-') => return None,
                _ => return Some(index),
            }
        }
        index += 1;
    }
    None
}

fn package(path: &str) -> Option<&'static str> {
    let path = path.replace('\\', "/").to_ascii_lowercase();
    [
        (
            "node_modules/@anthropic-ai/claude-code/cli.js",
            "claude-code",
        ),
        ("node_modules/@google/gemini-cli/dist/index.js", "gemini"),
        ("node_modules/@github/copilot/index.js", "github-copilot"),
        (
            "node_modules/@qwen-code/qwen-code/dist/index.js",
            "qwen-code",
        ),
        (
            "node_modules/@earendil-works/pi-coding-agent/dist/cli.js",
            "pi",
        ),
        (
            "node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js",
            "pi",
        ),
        (
            "node_modules/@mariozechner/pi-coding-agent/dist/cli.js",
            "pi",
        ),
        ("node_modules/@moonshot-ai/kimi-code/dist/main.mjs", "kimi"),
        ("node_modules/mastracode/dist/cli.js", "mastracode"),
        ("node_modules/@letta-ai/letta-code/letta.js", "letta"),
    ]
    .into_iter()
    .find_map(|(suffix, provider)| {
        (path == suffix
            || path
                .strip_suffix(suffix)
                .is_some_and(|prefix| prefix.ends_with('/')))
        .then_some(provider)
    })
}

fn interactive(provider: &'static str, args: Option<&[String]>) -> Option<&'static str> {
    if provider == "letta"
        && args.is_some_and(|args| {
            args.iter().any(|arg| {
                matches!(
                    arg.split('=').next().unwrap_or(""),
                    "-p" | "--print"
                        | "--prompt"
                        | "--json"
                        | "--stream-json"
                        | "--run"
                        | "--output-format"
                        | "--input-format"
                )
            })
        })
    {
        return None;
    }
    Some(provider)
}
