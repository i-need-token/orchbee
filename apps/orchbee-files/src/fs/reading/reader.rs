//! One read's open file and how it serves chunks. A text read decodes forward from its first byte
//! and serves its UTF-8 output in order, each chunk exactly the length asked except the last, so a
//! Client can ask for the next offsets before the answers arrive. A byte read serves file bytes at
//! any offset. The final chunk reports whether the file's modification time or size moved while it
//! was read and, for text, its line ending and the SHA-256 of the file bytes it consumed.

use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
};

use orchbee_files_protocol::{ChunkEnd, LineEnding, ReadMode};
use sha2::{Digest, Sha256};

use crate::fs::{
    detection::{DETECTION_BYTES, detect},
    encoding::{self, Decoder, Encoding},
    metadata::milliseconds,
    os_error::{FsError, FsErrorCode},
};

/// How many file bytes a text read takes from the file at a time.
const BLOCK_BYTES: usize = 64 * 1024;

/// What a read is asked to do.
#[derive(Clone, Debug)]
pub(crate) struct ReadRequest {
    /// Decode text or serve bytes.
    pub(crate) mode: ReadMode,
    /// The encoding to decode without a byte order mark; `None` is UTF-8.
    pub(crate) encoding: Option<String>,
    /// Decode a binary file as text.
    pub(crate) force_text: bool,
    /// The largest content the Client accepts, in bytes.
    pub(crate) limit: u64,
}

/// What the read found before its first chunk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReadFacts {
    /// The size in bytes when the read opened the file.
    pub(crate) size: u64,
    /// The modification time in milliseconds since the Unix epoch.
    pub(crate) mtime_ms: i64,
    /// The encoding text chunks decode from; `None` for a byte read or an undecoded binary file.
    pub(crate) encoding: Option<&'static Encoding>,
    /// The content starts with a byte order mark, which chunks omit.
    pub(crate) bom: bool,
    /// A zero byte that no UTF-16 pattern explains; without `force_text` no chunk follows.
    pub(crate) binary: bool,
}

/// One served piece of a read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Piece {
    pub(crate) offset: u64,
    pub(crate) bytes: Vec<u8>,
    /// Present on the final piece only.
    pub(crate) end: Option<ChunkEnd>,
}

pub(crate) enum Reader {
    Text(Box<TextReader>),
    Bytes(ByteReader),
    /// A binary file read without `force_text`.
    Binary,
}

/// The file's size and modification time, which tell whether it moved during the read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Facts {
    size: u64,
    mtime_ms: i64,
}

impl Facts {
    fn of(metadata: &fs::Metadata) -> Self {
        Self {
            size: metadata.len(),
            mtime_ms: milliseconds(metadata.modified()),
        }
    }
}

/// The file a read holds open, and what tells whether it moved.
struct Source {
    file: File,
    path: PathBuf,
    opened: Facts,
    limit: u64,
}

impl Source {
    /// The file's size or modification time moved since it was opened, through the open handle
    /// or under its path, which another program may have replaced.
    fn moved(&self) -> bool {
        let now =
            |metadata: io::Result<fs::Metadata>| metadata.ok().map(|metadata| Facts::of(&metadata));
        now(self.file.metadata()) != Some(self.opened)
            || now(fs::metadata(&self.path)) != Some(self.opened)
    }
}

/// Opens `path` for `request` and detects what its content is.
///
/// # Errors
///
/// `is-directory`, `too-large` when the file exceeds the limit, `failed` for what is neither file
/// nor directory, such as a FIFO a read would wait on, or the system's error.
pub(crate) fn open(path: &Path, request: &ReadRequest) -> Result<(ReadFacts, Reader), FsError> {
    let error = |error: io::Error| FsError::io(&error);
    let found = fs::metadata(path).map_err(error)?;
    if found.is_dir() {
        return Err(FsError::new(FsErrorCode::IsDirectory));
    }
    if !found.is_file() {
        return Err(FsError::new(FsErrorCode::Failed));
    }
    let mut file = File::open(path).map_err(error)?;
    let opened = Facts::of(&file.metadata().map_err(error)?);
    if opened.size > request.limit {
        return Err(FsError::new(FsErrorCode::TooLarge));
    }
    let mut facts = ReadFacts {
        size: opened.size,
        mtime_ms: opened.mtime_ms,
        encoding: None,
        bom: false,
        binary: false,
    };
    let source = |file| Source {
        file,
        path: path.to_path_buf(),
        opened,
        limit: request.limit,
    };
    if request.mode == ReadMode::Bytes {
        return Ok((facts, Reader::Bytes(ByteReader(source(file)))));
    }
    let head = read_head(&mut file).map_err(error)?;
    let detection = detect(&head);
    if detection.binary && !request.force_text {
        facts.binary = true;
        return Ok((facts, Reader::Binary));
    }
    let encoding = detection.encoding.unwrap_or_else(|| {
        request
            .encoding
            .as_deref()
            .map_or(&encoding::UTF8, encoding::find)
    });
    facts.encoding = Some(encoding);
    facts.bom = detection.bom > 0;
    facts.binary = detection.binary;
    let reader = TextReader::new(source(file), head, detection.bom, encoding);
    Ok((facts, Reader::Text(Box::new(reader))))
}

/// The file's first [`DETECTION_BYTES`] bytes, or all of a shorter file.
fn read_head(file: &mut File) -> io::Result<Vec<u8>> {
    let mut head = vec![0; DETECTION_BYTES];
    let mut filled = 0;
    while filled < head.len() {
        match file.read(&mut head[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    head.truncate(filled);
    Ok(head)
}

/// A text read: the decoder's output, served forward.
pub(crate) struct TextReader {
    source: Source,
    decoder: Decoder,
    /// Bytes already read that wait for decoding: the detection head, until the first chunk.
    head: Vec<u8>,
    /// The byte order mark's length at the start of `head`.
    bom: usize,
    /// Decoded output not yet served.
    pending: Vec<u8>,
    /// The output offset the next chunk starts at.
    output: u64,
    /// File bytes read so far.
    consumed: u64,
    /// The decoder has seen the end of the file.
    finished: bool,
    digest: Sha256,
    endings: LineEndings,
    end: Option<ChunkEnd>,
}

impl TextReader {
    fn new(source: Source, head: Vec<u8>, bom: usize, encoding: &Encoding) -> Self {
        Self {
            source,
            decoder: Decoder::new(encoding),
            consumed: head.len() as u64,
            head,
            bom,
            pending: Vec::new(),
            output: 0,
            finished: false,
            digest: Sha256::new(),
            endings: LineEndings::default(),
            end: None,
        }
    }

    /// The `length` output bytes at `offset`, fewer only at the end. Once the end is served, the
    /// next offset answers with the same end and no bytes.
    ///
    /// # Errors
    ///
    /// `invalid-request` for any offset but the next, `too-large` once the file grows past the
    /// limit, or the system's error.
    pub(crate) fn chunk(&mut self, offset: u64, length: usize) -> Result<Piece, FsError> {
        if offset != self.output {
            return Err(FsError::new(FsErrorCode::InvalidRequest));
        }
        if let Some(end) = self.end {
            return Ok(Piece {
                offset,
                bytes: Vec::new(),
                end: Some(end),
            });
        }
        while self.pending.len() < length && !self.finished {
            self.fill()?;
        }
        let served = self.pending.len().min(length);
        let bytes: Vec<u8> = self.pending.drain(..served).collect();
        self.output += bytes.len() as u64;
        if self.finished && self.pending.is_empty() {
            self.end = Some(ChunkEnd {
                line_ending: self.endings.finish(),
                changed: self.source.moved(),
                digest: Some(std::mem::take(&mut self.digest).finalize().into()),
            });
        }
        Ok(Piece {
            offset,
            bytes,
            end: self.end,
        })
    }

    /// Decodes the next block of the file, or flushes the decoder at its end.
    fn fill(&mut self) -> Result<(), FsError> {
        let mut text = String::new();
        if self.head.is_empty() {
            let mut block = vec![0; BLOCK_BYTES];
            let read = loop {
                match self.source.file.read(&mut block) {
                    Ok(read) => break read,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(error) => return Err(FsError::io(&error)),
                }
            };
            if read == 0 {
                self.decoder.decode(&[], true, &mut text);
                self.finished = true;
            } else {
                self.consumed += read as u64;
                if self.consumed > self.source.limit {
                    return Err(FsError::new(FsErrorCode::TooLarge));
                }
                self.digest.update(&block[..read]);
                self.decoder.decode(&block[..read], false, &mut text);
            }
        } else {
            let head = std::mem::take(&mut self.head);
            self.digest.update(&head);
            self.decoder.decode(&head[self.bom..], false, &mut text);
        }
        self.endings.count(text.as_bytes());
        self.pending.extend_from_slice(text.as_bytes());
        Ok(())
    }
}

/// Line breaks counted by kind across the output, by VS Code's text buffer rule: CRLF when
/// carriage returns, alone or before a line feed, exceed half of all breaks.
#[derive(Default)]
struct LineEndings {
    cr: u64,
    lf: u64,
    crlf: u64,
    /// The output so far ends with a carriage return whose kind the next byte decides.
    carriage: bool,
}

impl LineEndings {
    fn count(&mut self, text: &[u8]) {
        for &byte in text {
            match byte {
                b'\r' => {
                    self.cr += u64::from(self.carriage);
                    self.carriage = true;
                }
                b'\n' if self.carriage => {
                    self.crlf += 1;
                    self.carriage = false;
                }
                b'\n' => self.lf += 1,
                _ if self.carriage => {
                    self.cr += 1;
                    self.carriage = false;
                }
                _ => {}
            }
        }
    }

    fn finish(&mut self) -> LineEnding {
        self.cr += u64::from(std::mem::take(&mut self.carriage));
        let breaks = self.cr + self.lf + self.crlf;
        if breaks == 0 {
            LineEnding::None
        } else if 2 * (self.cr + self.crlf) > breaks {
            LineEnding::CrLf
        } else {
            LineEnding::Lf
        }
    }
}

/// A byte read: file bytes at any offset, read without moving a shared position.
pub(crate) struct ByteReader(Source);

impl ByteReader {
    /// Up to `length` file bytes at `offset`; the piece that reaches the end of the file is the
    /// final one.
    ///
    /// # Errors
    ///
    /// `too-large` for bytes past the limit, or the system's error.
    pub(crate) fn chunk(&self, offset: u64, length: usize) -> Result<Piece, FsError> {
        let mut bytes = vec![0; length];
        let mut filled = 0;
        while filled < length {
            match read_at(&self.0.file, &mut bytes[filled..], offset + filled as u64) {
                Ok(0) => break,
                Ok(read) => filled += read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(FsError::io(&error)),
            }
        }
        bytes.truncate(filled);
        let reached = offset.saturating_add(filled as u64);
        if reached > self.0.limit {
            return Err(FsError::new(FsErrorCode::TooLarge));
        }
        let at_end = filled < length
            || self
                .0
                .file
                .metadata()
                .map_or(true, |metadata| metadata.len() <= reached);
        Ok(Piece {
            offset,
            bytes,
            end: at_end.then(|| ChunkEnd {
                line_ending: LineEnding::None,
                changed: self.0.moved(),
                digest: None,
            }),
        })
    }
}

#[cfg(unix)]
fn read_at(file: &File, buffer: &mut [u8], offset: u64) -> io::Result<usize> {
    std::os::unix::fs::FileExt::read_at(file, buffer, offset)
}

/// Windows reads at an offset through the handle's position, which no other read relies on.
#[cfg(windows)]
fn read_at(file: &File, buffer: &mut [u8], offset: u64) -> io::Result<usize> {
    std::os::windows::fs::FileExt::seek_read(file, buffer, offset)
}
