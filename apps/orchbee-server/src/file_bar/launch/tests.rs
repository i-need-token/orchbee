use super::{Endpoint, Judgement, judge, this_build};
use orchbee_files_protocol::{
    BuildIdentity, LinkHello, LinkMessage, LinkSide, LinkToken, read_link_message,
    write_link_message,
};
use orchbee_local_ipc::LocalStream;
use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

#[test]
fn only_the_start_token_from_this_build_is_admitted() {
    let build = this_build().expect("this build names itself");
    let token = LinkToken::from_bytes([7; 32]);
    let hello = |build: &BuildIdentity, token: [u8; 32]| LinkHello {
        build: build.clone(),
        token: LinkToken::from_bytes(token),
    };
    let other = BuildIdentity::new("0.0.0-other", None, build.environment(), [0; 32])
        .expect("a valid build");
    assert_eq!(
        judge(&hello(&build, [7; 32]), &token, &build),
        Judgement::Admit
    );
    assert_eq!(
        judge(&hello(&other, [7; 32]), &token, &build),
        Judgement::Refuse
    );
    // A wrong token learns nothing, whatever build it names.
    assert_eq!(
        judge(&hello(&build, [8; 32]), &token, &build),
        Judgement::Ignore
    );
    assert_eq!(
        judge(&hello(&other, [8; 32]), &token, &build),
        Judgement::Ignore
    );
}

/// A fresh endpoint path of this test process.
fn endpoint_path() -> PathBuf {
    let mut suffix = [0; 4];
    getrandom::fill(&mut suffix).expect("randomness");
    let name = format!(
        "obft-{}-{:08x}",
        std::process::id(),
        u32::from_be_bytes(suffix)
    );
    if cfg!(windows) {
        PathBuf::from(format!(r"\\.\pipe\{name}"))
    } else {
        std::env::temp_dir().join(format!("{name}.sock"))
    }
}

/// Dials `path` and sends a hello that presents `token`.
fn dial(path: &PathBuf, build: &BuildIdentity, token: [u8; 32]) -> LocalStream {
    let mut stream = LocalStream::connect(path).expect("the endpoint listens");
    let hello = LinkMessage::Hello(LinkHello {
        build: build.clone(),
        token: LinkToken::from_bytes(token),
    });
    write_link_message(&mut stream, &hello).expect("the hello is sent");
    stream
}

#[test]
fn the_endpoint_answers_nothing_to_a_stranger_and_still_admits_the_start_after_it() {
    let path = endpoint_path();
    let endpoint = Endpoint::bind(&path).expect("the endpoint binds");
    let build = this_build().expect("this build names itself");
    let process = thread::spawn({
        let path = path.clone();
        move || {
            let mut stranger = dial(&path, &build, [8; 32]);
            let answer = read_link_message(&mut stranger, LinkSide::Server);
            let mut files = dial(&path, &build, [7; 32]);
            let admitted = read_link_message(&mut files, LinkSide::Server).expect("an answer");
            (answer.is_err(), admitted)
        }
    });
    let admitted = endpoint.admit(
        &LinkToken::from_bytes([7; 32]),
        Instant::now() + Duration::from_secs(10),
    );
    assert!(admitted.is_ok());
    let (stranger_dropped, answer) = process.join().expect("the process side ends");
    assert!(stranger_dropped, "the stranger's hello gets no answer");
    assert_eq!(answer, LinkMessage::Admitted);
}
