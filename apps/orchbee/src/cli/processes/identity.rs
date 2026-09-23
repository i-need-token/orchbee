//! Explicit executable and runtime entrypoint recognition, never argument search.

pub(in crate::cli) fn provider(label: &str) -> Option<&'static str> {
    let name = basename(label);
    match name.as_str() {
        "pi" => Some("pi"),
        "claude" | "claude-code" => Some("claude-code"),
        "codex" => Some("codex"),
        "gemini" => Some("gemini"),
        "cursor" | "cursor-agent" => Some("cursor"),
        "devin" | "devin-cli" | "devin cli" => Some("devin"),
        "agy" | "antigravity" | "antigravity-cli" => Some("antigravity"),
        "cline" | ".cline" => Some("cline"),
        "omp" => Some("omp"),
        "mastracode" | "mastra-code" | "mastra code" => Some("mastracode"),
        "opencode" | "opencode2" | "open-code" => Some("opencode"),
        "copilot" | "github-copilot" | "ghcs" => Some("github-copilot"),
        "kimi" | "kimi-code" | "kimi code" => Some("kimi"),
        "kiro" | "kiro-cli" => Some("kiro"),
        "droid" => Some("droid"),
        "amp" | "amp-local" => Some("amp"),
        "grok" | "grok-build" => Some("grok"),
        "hermes" | "hermes-agent" => Some("hermes"),
        "kilo" | "kilo-code" | "kilo code" => Some("kilo"),
        "qodercli" | "qoderclicn" | "qoder" | "qodercn" => Some("qodercli"),
        "qwen" | "qwen-code" | "qwen code" => Some("qwen-code"),
        "letta" | "letta-code" | "letta code" => Some("letta"),
        "maki" => Some("maki"),
        "muse" | "muse-code" | "muse-cli" => Some("muse"),
        value
            if value
                .strip_prefix("muse-bin-")
                .is_some_and(|v| v.starts_with(|c: char| c.is_ascii_digit())) =>
        {
            Some("muse")
        }
        _ => None,
    }
}

pub(super) fn identify(image: Option<&str>, argv: Option<&[String]>) -> Option<&'static str> {
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

fn basename(path: &str) -> String {
    let value = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    for suffix in [".exe", ".com", ".cmd", ".ps1", ".js", ".mjs", ".cjs", ".py"] {
        if let Some(stem) = value.strip_suffix(suffix) {
            return stem.to_owned();
        }
    }
    value
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
