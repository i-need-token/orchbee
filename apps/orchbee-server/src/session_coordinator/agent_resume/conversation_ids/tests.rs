use super::{binding, command_line_session, resume_arguments};
use orchbee_server::{AgentResumeProvider, RetainedAgentSession};

const UUID: &str = "0f8fad5b-d9cb-469f-a165-70867728950e";
const OTHER_UUID: &str = "c56a4180-65aa-42ec-a945-5fd21dec0538";
const OPENCODE_ID: &str = "ses_0123456789abCdEfGhIjKlMnOp";

/// One valid id of `provider`, and a second one, distinct from it.
fn ids(provider: AgentResumeProvider) -> [&'static str; 2] {
    use AgentResumeProvider as P;
    match provider {
        P::OpenCode | P::Kilo | P::Mimo => [OPENCODE_ID, "ses_abcdef012345ZyXwVuTsRqPoNm"],
        P::Kimi => [
            "session_0f8fad5b-d9cb-469f-a165-70867728950e",
            "session_c56a4180-65aa-42ec-a945-5fd21dec0538",
        ],
        P::Letta => [
            "conv-0f8fad5b-d9cb-469f-a165-70867728950e",
            "conv-c56a4180-65aa-42ec-a945-5fd21dec0538",
        ],
        P::Devin => ["a1b2c3d4-e5f6", "devin-7788"],
        P::Hermes => ["20261111_120000_a1b2c3", "20261111_130000_d4e5f6"],
        P::Cline => ["1730000000000_a1b2c", "1730000000001_d3e4f"],
        _ => [UUID, OTHER_UUID],
    }
}

/// An id of another provider's format, invalid for `provider`.
fn foreign(provider: AgentResumeProvider) -> &'static str {
    [OPENCODE_ID, UUID]
        .into_iter()
        .find(|id| RetainedAgentSession::new(provider, id).is_none())
        .unwrap()
}

fn read(provider: AgentResumeProvider, line: &str) -> Option<String> {
    let [id, other] = ids(provider);
    let arguments: Vec<String> = line
        .split(' ')
        .filter(|token| !token.is_empty())
        .map(|token| {
            token
                .replace("{id}", id)
                .replace("{other}", other)
                .replace("{foreign}", foreign(provider))
        })
        .collect();
    command_line_session(provider, &arguments).map(|session| session.session_id().to_owned())
}

/// The in-step rule: the tokens a resume types read back as exactly the id they resume.
#[test]
fn every_typed_resume_reads_back_its_own_id() {
    let letta_default = "default:agent-0f8fad5b-d9cb-469f-a165-70867728950e";
    let sessions = AgentResumeProvider::ALL
        .into_iter()
        .map(|provider| RetainedAgentSession::new(provider, ids(provider)[0]).unwrap())
        .chain(RetainedAgentSession::new(
            AgentResumeProvider::Letta,
            letta_default,
        ));
    for session in sessions {
        let typed = resume_arguments(&session);
        assert_eq!(
            command_line_session(session.provider(), &typed),
            Some(session.clone()),
            "{typed:?}"
        );
    }
}

/// Every option of every provider row in both written forms, and the look-alikes it rejects.
#[test]
fn every_binding_option_reads_one_valid_id_and_rejects_look_alikes() {
    for provider in AgentResumeProvider::ALL {
        let [id, _] = ids(provider);
        let row = binding(provider);
        let expect = |line: String, want: Option<&str>| {
            assert_eq!(
                read(provider, &line),
                want.map(str::to_owned),
                "{provider:?}: {line}"
            );
        };
        for option in row.options {
            let long = option.starts_with("--");
            expect(format!("{option} {{id}}"), Some(id));
            expect(format!("{option}={{id}}"), long.then_some(id));
            expect(format!("{option}x {{id}}"), None);
            expect(format!("{option} {{foreign}}"), None);
            expect(format!("{option} {{id}} {option} {{other}}"), None);
            expect(format!("{option} {{id}} {option}={{id}}"), None);
            expect(format!("-- {option} {{id}}"), None);
            expect(format!("{option} -x {{id}}"), None);
            for flag in row.voiding {
                expect(format!("{flag} {option} {{id}}"), None);
                expect(format!("{option} {{id}} {flag}"), None);
            }
        }
        expect(String::new(), None);
        expect("{id}".to_owned(), None);
    }
}

#[test]
fn provider_specific_forms() {
    use AgentResumeProvider as P;
    let letta_default = "default:agent-0f8fad5b-d9cb-469f-a165-70867728950e";
    let cases: &[(P, &str, Option<&str>)] = &[
        // Codex binds the positional after a first positional `resume`.
        (P::Codex, "resume {id}", Some(UUID)),
        (P::Codex, "--search -m gpt-5 resume {id}", Some(UUID)),
        (P::Codex, "resume {id} continue the work", Some(UUID)),
        (P::Codex, "-m resume {id}", None),
        (P::Codex, "resume --last", None),
        (P::Codex, "resume --last {id}", None),
        (P::Codex, "fork {id}", None),
        (P::Codex, "review resume {id}", None),
        (P::Codex, "resume -- {id}", None),
        (P::Codex, "resume {foreign}", None),
        // A value of another option is never an id.
        (P::ClaudeCode, "--append-system-prompt {id}", None),
        (P::ClaudeCode, "--model opus --resume {id}", Some(UUID)),
        (P::Grok, "-s {id} --fork-session", None),
        // Letta completes a bound default conversation with exactly one agent.
        (
            P::Letta,
            "--conversation default --agent agent-0f8fad5b-d9cb-469f-a165-70867728950e",
            Some(letta_default),
        ),
        (
            P::Letta,
            "-a agent-0f8fad5b-d9cb-469f-a165-70867728950e -C default",
            Some(letta_default),
        ),
        (P::Letta, "--conversation default", None),
        (
            P::Letta,
            "--conversation default --agent agent-0f8fad5b-d9cb-469f-a165-70867728950e -a agent-c56a4180-65aa-42ec-a945-5fd21dec0538",
            None,
        ),
        (
            P::Letta,
            "--agent agent-0f8fad5b-d9cb-469f-a165-70867728950e",
            None,
        ),
    ];
    for (provider, line, want) in cases {
        assert_eq!(
            read(*provider, line).as_deref(),
            *want,
            "{provider:?}: {line}"
        );
    }
}
