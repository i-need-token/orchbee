//! Bounded NDJSON commands and an invocation-owned cancellable Windows stdin reader.
use base64::Engine;
use orchbee_client_server_protocol::Geometry;
use orchbee_local_ipc::ChildOutputReader;
use serde_core::{
    Deserialize, Deserializer,
    de::{self, MapAccess, Visitor},
};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};
use std::{
    io::{self, Read},
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Child, Command as ProcessCommand, Stdio},
};

pub(super) const MAX_INPUT: usize = 256 * 1024;
const MAX_LINE: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Direction {
    Up,
    Down,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum ScrollSource {
    #[default]
    Wheel,
    PageKey,
}
#[derive(Debug, Eq, PartialEq)]
pub(super) enum Command {
    Input(Vec<u8>),
    Resize(Geometry),
    Scroll(Scroll),
    Release,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Scroll {
    pub(super) direction: Direction,
    pub(super) lines: u16,
    pub(super) source: ScrollSource,
    pub(super) column: Option<u16>,
    pub(super) row: Option<u16>,
    pub(super) modifiers: u8,
}
struct Fields(BTreeMap<String, Value>);
impl<'de> Deserialize<'de> for Fields {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Object;
        impl<'de> Visitor<'de> for Object {
            type Value = Fields;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a closed terminal control command")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Fields, M::Error> {
                let mut fields = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, Value>()? {
                    if fields.insert(key, value).is_some() {
                        return Err(de::Error::custom("duplicate command field"));
                    }
                }
                Ok(Fields(fields))
            }
        }
        deserializer.deserialize_map(Object)
    }
}
impl Fields {
    fn string(&mut self, name: &str) -> Result<String, &'static str> {
        match self.0.remove(name) {
            Some(Value::String(value)) => Ok(value),
            _ => Err("invalid-command"),
        }
    }
    fn number(&mut self, name: &str, default: Option<u16>) -> Result<u16, &'static str> {
        match self.0.remove(name) {
            Some(value) => value
                .as_u64()
                .and_then(|n| u16::try_from(n).ok())
                .ok_or("invalid-command"),
            None => default.ok_or("invalid-command"),
        }
    }
    fn coordinate(&mut self, name: &str) -> Result<Option<u16>, &'static str> {
        if !self.0.contains_key(name) {
            Ok(None)
        } else {
            self.number(name, None).map(Some)
        }
    }
}
pub(super) fn parse(line: &[u8]) -> Result<Command, &'static str> {
    if line.len() > MAX_LINE {
        return Err("record-too-large");
    }
    let mut fields: Fields = serde_json::from_slice(line).map_err(|_| "invalid-command")?;
    let command = match fields.string("type")?.as_str() {
        "terminal.input" => {
            if fields.0.contains_key("text") == fields.0.contains_key("bytes") {
                return Err("input-source-conflict");
            }
            let bytes = if fields.0.contains_key("text") {
                fields.string("text")?.into_bytes()
            } else {
                base64::engine::general_purpose::STANDARD
                    .decode(fields.string("bytes")?)
                    .map_err(|_| "invalid-base64")?
            };
            if bytes.len() > MAX_INPUT {
                return Err("input-too-large");
            }
            Command::Input(bytes)
        }
        "terminal.resize" => Command::Resize(
            Geometry::new(
                fields.number("cols", None)?,
                fields.number("rows", None)?,
                fields.number("cell_width_px", Some(0))?,
                fields.number("cell_height_px", Some(0))?,
            )
            .map_err(|_| "invalid-geometry")?,
        ),
        "terminal.scroll" => {
            let direction = match fields.string("direction")?.as_str() {
                "up" => Direction::Up,
                "down" => Direction::Down,
                _ => return Err("invalid-scroll"),
            };
            let lines = fields.number("lines", None)?;
            let source = if fields.0.contains_key("source") {
                match fields.string("source")?.as_str() {
                    "wheel" => ScrollSource::Wheel,
                    "page_key" => ScrollSource::PageKey,
                    _ => return Err("invalid-scroll"),
                }
            } else {
                ScrollSource::Wheel
            };
            let column = fields.coordinate("column")?;
            let row = fields.coordinate("row")?;
            let modifiers = fields.number("modifiers", Some(0))?;
            if lines == 0
                || u8::try_from(modifiers)
                    .ok()
                    .and_then(crossterm::event::KeyModifiers::from_bits)
                    .is_none()
            {
                return Err("invalid-scroll");
            }
            Command::Scroll(Scroll {
                direction,
                lines,
                source,
                column,
                row,
                modifiers: modifiers as u8,
            })
        }
        "terminal.release" => Command::Release,
        _ => return Err("invalid-command"),
    };
    if !fields.0.is_empty() {
        return Err("invalid-command");
    }
    Ok(command)
}

const READER: &str = r#"$ErrorActionPreference='Stop'; try {
$i=[Console]::OpenStandardInput(); $o=[Console]::OpenStandardOutput(); $b=[byte[]]::new(8192)
while ($true) { $n=$i.Read($b,0,$b.Length); if ($n -eq 0) { exit 0 }; $o.Write($b,0,$n); $o.Flush() }
} catch { exit 1 }"#;

pub(super) enum Poll {
    Pending,
    Line(Vec<u8>),
    Eof,
}
pub(super) struct Reader {
    child: Child,
    output: ChildOutputReader,
    pending: Vec<u8>,
    eof: bool,
    first: bool,
}
impl Reader {
    pub(super) fn start() -> io::Result<Self> {
        Self::with_input(Stdio::inherit())
    }
    pub(super) fn with_input(input: Stdio) -> io::Result<Self> {
        let executable = PathBuf::from(winsafe::GetSystemDirectory().map_err(io::Error::other)?)
            .join("WindowsPowerShell/v1.0/powershell.exe");
        let (output, child_output) = ChildOutputReader::pipe()?;
        let mut command = ProcessCommand::new(executable);
        command
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                READER,
            ])
            .stdin(input)
            .stdout(child_output)
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000);
        let child = command.spawn()?;
        drop(command);
        Ok(Self {
            child,
            output,
            pending: Vec::new(),
            eof: false,
            first: true,
        })
    }
    pub(super) fn poll(&mut self) -> io::Result<Poll> {
        if let Some(line) = self.line()? {
            return Ok(Poll::Line(line));
        }
        if self.eof {
            return self.ending();
        }
        let mut buffer = [0; 8192];
        match self.output.read(&mut buffer) {
            Ok(0) => {
                self.eof = true;
                self.ending()
            }
            Ok(n) => {
                self.pending.extend_from_slice(&buffer[..n]);
                Ok(self.line()?.map_or(Poll::Pending, Poll::Line))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(Poll::Pending),
            Err(e) => Err(e),
        }
    }
    fn line(&mut self) -> io::Result<Option<Vec<u8>>> {
        let end = self.pending.iter().position(|b| *b == b'\n');
        if end.unwrap_or(self.pending.len()) > MAX_LINE {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(end.map(|end| {
            let mut line = self.pending.drain(..=end).collect::<Vec<_>>();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            self.strip_bom(line)
        }))
    }
    fn strip_bom(&mut self, mut line: Vec<u8>) -> Vec<u8> {
        if self.first && line.starts_with(&[0xef, 0xbb, 0xbf]) {
            line.drain(..3);
        }
        self.first = false;
        line
    }
    fn ending(&mut self) -> io::Result<Poll> {
        match self.child.try_wait()? {
            None => Ok(Poll::Pending),
            Some(status) if !status.success() => Err(io::ErrorKind::BrokenPipe.into()),
            Some(_) if self.pending.is_empty() => Ok(Poll::Eof),
            Some(_) => {
                let line = std::mem::take(&mut self.pending);
                Ok(Poll::Line(self.strip_bom(line)))
            }
        }
    }
}
impl Drop for Reader {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchbee_local_ipc::ChildInputWriter;
    use std::{
        io::Write,
        thread,
        time::{Duration, Instant},
    };

    #[test]
    fn commands_are_closed_bounded_and_preserve_opaque_input() {
        assert_eq!(
            parse(br#"{"type":"terminal.input","bytes":"AP8bDQ=="}"#),
            Ok(Command::Input(vec![0, 255, 27, 13]))
        );
        assert_eq!(
            parse(br#"{"type":"terminal.input","text":""}"#),
            Ok(Command::Input(Vec::new()))
        );
        assert!(matches!(
            parse(br#"{"type":"terminal.resize","cols":120,"rows":40}"#),
            Ok(Command::Resize(_))
        ));
        assert!(matches!(parse(br#"{"type":"terminal.scroll","direction":"up","lines":65535,"column":2,"modifiers":63}"#),Ok(Command::Scroll(_))));
        for bad in [
            r#"{"type":"terminal.release","extra":1}"#,
            r#"{"type":"terminal.release","type":"terminal.release"}"#,
            r#"{"type":"terminal.input"}"#,
            r#"{"type":"terminal.input","text":null}"#,
            r#"{"type":"terminal.input","text":"a","bytes":"YQ=="}"#,
            r#"{"type":"terminal.input","bytes":"!"}"#,
            r#"{"type":"terminal.resize","cols":4096,"rows":4096}"#,
            r#"{"type":"terminal.resize","cols":80,"rows":24,"cell_width_px":8}"#,
            r#"{"type":"terminal.scroll","direction":"down","lines":0}"#,
            r#"{"type":"terminal.scroll","direction":"down","lines":1,"modifiers":64}"#,
            r#"{"type":"terminal.scroll","direction":"down","lines":1,"source":"unknown"}"#,
        ] {
            assert!(parse(bad.as_bytes()).is_err(), "{bad}");
        }
        for count in [MAX_INPUT, MAX_INPUT + 1] {
            let line = serde_json::to_vec(
                &serde_json::json!({"type":"terminal.input","text":"x".repeat(count)}),
            )
            .unwrap();
            assert_eq!(parse(&line).is_ok(), count == MAX_INPUT);
        }
        assert!(parse(&vec![b' '; MAX_LINE + 1]).is_err());
        assert!(parse(b"\xff").is_err());
    }
    #[test]
    fn input_schema_and_parser_agree_on_public_records_and_reject_unknown_shape() {
        let schema: Value = serde_json::from_str(include_str!(
            "../schemas/orchbee.terminal.control.input.json"
        ))
        .unwrap();
        let validator = jsonschema::draft202012::new(&schema).unwrap();
        for (record, accepted) in [
            (
                serde_json::json!({"type":"terminal.input","text":"hello\n世界"}),
                true,
            ),
            (
                serde_json::json!({"type":"terminal.input","bytes":"AP8bDQ=="}),
                true,
            ),
            (
                serde_json::json!({"type":"terminal.input","text":"a","bytes":"YQ=="}),
                false,
            ),
            (
                serde_json::json!({"type":"terminal.resize","cols":80,"rows":24,"cell_width_px":8,"cell_height_px":16}),
                true,
            ),
            (
                serde_json::json!({"type":"terminal.scroll","direction":"down","lines":3,"modifiers":63}),
                true,
            ),
            (
                serde_json::json!({"type":"terminal.scroll","direction":"down","lines":3,"modifiers":0.5}),
                false,
            ),
            (
                serde_json::json!({"type":"terminal.scroll","direction":"down","lines":3,"modifiers":"1"}),
                false,
            ),
            (serde_json::json!({"type":"terminal.release"}), true),
            (
                serde_json::json!({"type":"terminal.release","extra":null}),
                false,
            ),
        ] {
            assert_eq!(validator.is_valid(&record), accepted, "schema {record}");
            assert_eq!(
                parse(&serde_json::to_vec(&record).unwrap()).is_ok(),
                accepted,
                "parser {record}"
            );
        }
    }
    #[test]
    fn reader_preserves_fragmented_lines_bom_eof_and_reaps_stalled_input() {
        let (mut input, child_input) = ChildInputWriter::pipe().unwrap();
        let mut reader = Reader::with_input(child_input).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let records = thread::scope(|scope| {
            scope.spawn(move || {
                for part in [
                    b"\xef\xbb\xbf{\"type\":\"terminal.release\"}\r".as_slice(),
                    b"\n\nlast",
                ] {
                    input.write_all(part).unwrap();
                    thread::sleep(Duration::from_millis(20));
                }
                drop(input);
            });
            let mut records = Vec::new();
            loop {
                assert!(Instant::now() < deadline, "reader deadline");
                match reader.poll().unwrap() {
                    Poll::Line(line) => records.push(line),
                    Poll::Pending => thread::sleep(Duration::from_millis(2)),
                    Poll::Eof => break,
                }
            }
            records
        });
        assert_eq!(
            records,
            vec![
                br#"{"type":"terminal.release"}"#.to_vec(),
                Vec::new(),
                b"last".to_vec()
            ]
        );
        drop(reader);
        let (_held, child_input) = ChildInputWriter::pipe().unwrap();
        let mut reader = Reader::with_input(child_input).unwrap();
        assert!(matches!(reader.poll().unwrap(), Poll::Pending));
        let started = Instant::now();
        drop(reader);
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
