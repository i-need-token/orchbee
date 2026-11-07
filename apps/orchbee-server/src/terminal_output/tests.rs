use super::*;

#[derive(Default)]
struct Memory(Mutex<BTreeMap<u64, Vec<u8>>>);

impl TerminalOutputStore for Memory {
    fn tokens(&self) -> io::Result<Vec<u64>> {
        Ok(self.0.lock().unwrap().keys().copied().collect())
    }
    fn read(&self, token: u64) -> io::Result<Option<Vec<u8>>> {
        Ok(self.0.lock().unwrap().get(&token).cloned())
    }
    fn write(&self, token: u64, payload: &[u8]) -> io::Result<()> {
        self.0.lock().unwrap().insert(token, payload.to_vec());
        Ok(())
    }
    fn remove(&self, token: u64) -> io::Result<()> {
        self.0.lock().unwrap().remove(&token);
        Ok(())
    }
}

fn binding(terminal: &str) -> PresentationBinding {
    serde_json::from_value(serde_json::json!({
        "workerId": "11111111111111111111111111111111",
        "terminalId": terminal,
    }))
    .unwrap()
}

fn presentation(id: &str) -> TerminalPresentationId {
    TerminalPresentationId::parse(&id.repeat(32)).unwrap()
}

fn kept(lines: &[String]) -> (Arc<Memory>, TerminalOutput, u64) {
    let store = Arc::new(Memory::default());
    let output = TerminalOutput::new(Arc::clone(&store) as Arc<dyn TerminalOutputStore>);
    let binding = binding("22222222222222222222222222222222");
    output
        .record(&binding, &presentation("a"), (1, 1), lines, 0)
        .unwrap();
    let [(_, token)] = output.tokens_for(&[binding])[..] else {
        panic!("one token");
    };
    (store, output, token)
}

#[test]
fn the_newest_lines_within_both_caps_are_kept() {
    let lines: Vec<String> = (0..2500).map(|line| format!("line {line}")).collect();
    let (_, output, token) = kept(&lines);
    let (content, _) = output.restored(&presentation("a"), token).unwrap();
    let content = String::from_utf8(content).unwrap();
    assert_eq!(content.split("\r\n").count(), usize::from(LINES));
    assert!(content.starts_with("line 500\r\n") && content.ends_with("line 2499"));

    let wide: Vec<String> = (0..1500).map(|line| format!("{line:>1000}")).collect();
    let (_, output, token) = kept(&wide);
    let (content, _) = output.restored(&presentation("a"), token).unwrap();
    assert!(content.len() <= MAXIMUM_OUTPUT_BYTES);
    assert!(content.ends_with(b"1499"));
    let seed = seed(&content, &"x".repeat(200));
    assert!(seed.len() <= 1024 * 1024);
}

#[test]
fn only_the_named_presentation_replays_and_a_foreign_file_is_removed() {
    let (store, output, token) = kept(&["kept".to_owned()]);
    assert!(output.restored(&presentation("b"), token).is_none());
    assert!(store.0.lock().unwrap().is_empty());

    let (store, output, token) = kept(&["kept".to_owned()]);
    output.after_publish(&[token]).unwrap();
    assert_eq!(store.tokens().unwrap(), [token]);
    output.after_publish(&[]).unwrap();
    assert!(store.tokens().unwrap().is_empty());
}
