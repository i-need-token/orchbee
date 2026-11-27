//! A helper reaches its process's relay: both ends prove their user, the prompt arrives with the
//! operation's token and its kind, the answer goes back to the helper, a prompt the engine drops
//! fails the helper, an editor's file that does not resolve fails it before any prompt, and a
//! closed relay admits no helper.

use super::{
    super::{
        helper::ask,
        relay::{self, Answer, Asked, Relay},
    },
    fixture::Fixture,
};
use orchbee_files_protocol::PromptKind;
use std::{sync::mpsc, thread, time::Duration};

#[test]
fn a_helper_gets_the_answer_its_prompt_was_given() {
    let (forward, asks) = mpsc::channel();
    let relay = Relay::bind(move |ask| {
        let _ = forward.send(ask);
    })
    .unwrap();
    let endpoint = relay.endpoint().to_owned();
    let token = [7; 32];
    let helper = |asked: Asked, text: &str| {
        let endpoint = endpoint.clone();
        let request = relay::encode_request(asked, &token, text);
        thread::spawn(move || ask(&endpoint, &request))
    };

    let asking = helper(Asked::Askpass, "Password for 'https://example.invalid': ");
    let prompt = asks.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(
        (prompt.token, prompt.kind, prompt.path.as_deref()),
        (token, PromptKind::Password, None)
    );
    prompt.answer.send(Answer::Value("secret".into())).unwrap();
    assert_eq!(
        asking.join().unwrap().unwrap(),
        Answer::Value("secret".into())
    );

    let fixture = Fixture::new();
    fixture.write("COMMIT_EDITMSG", "");
    let file = fixture.path("COMMIT_EDITMSG");
    let editing = helper(Asked::Editor, &file);
    let prompt = asks.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(
        (prompt.kind, prompt.path.as_deref()),
        (PromptKind::Editor, Some(file.as_str()))
    );
    drop(prompt);
    assert_eq!(editing.join().unwrap().unwrap(), Answer::Cancelled);

    let missing = helper(Asked::Editor, &fixture.path("missing"));
    assert_eq!(missing.join().unwrap().unwrap(), Answer::Cancelled);
    assert!(asks.try_recv().is_err());

    drop(relay);
    let request = relay::encode_request(Asked::Askpass, &token, "Username: ");
    assert!(ask(&endpoint, &request).is_err());
}
