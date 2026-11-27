//! One read's source, an open file or content held in memory, and how it serves chunks. A text
//! read decodes forward from its first byte and serves its UTF-8 output in order, each chunk
//! exactly the length asked except the last, so a Client can ask for the next offsets before the
//! answers arrive. A byte read serves source bytes at any offset. The final chunk reports whether
//! the file's modification time or size moved while it was read, which content in memory never
//! does, and, for text, its line ending and the SHA-256 of the source bytes it consumed.

use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::Arc,
};

use orchbee_files_protocol::{ChunkEnd, ErrorCode, FilesError, LineEnding, ReadMode, Signed};
use sha2::{Digest, Sha256};

use crate::{
    entry, error,
    fs::{
        detection::{DETECTION_BYTES, detect},
        encoding::{self, Decoder, Encoding},
    },
};

/// How many source bytes a text read takes at a time.
const BLOCK_BYTES: usize = 64 * 1024;

/// Content another part of the process holds, such as a file's version in Git, read as a file.
pub(crate) type Content = Arc<dyn AsRef<[u8]> + Send + Sync>;

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
    /// The modification time, as entries report it.
    pub(crate) mtime_ms: Signed,
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
    mtime_ms: Signed,
}

impl Facts {
    fn of(metadata: &fs::Metadata) -> Self {
        Self {
            size: metadata.len(),
            mtime_ms: entry::milliseconds(metadata.modified()),
        }
    }
}

/// What a read takes its bytes from, and the largest content its Client accepts.
struct Source {
    body: Body,
    limit: u64,
}

enum Body {
    /// An open file, with what it was when opened, which tells whether it moved.
    File {
        file: File,
        path: PathBuf,
        opened: Facts,
    },
    /// Content in memory, which never moves, and how much of it a text read took.
    Memory { content: Content, taken: usize },
}

impl Source {
    /// The file's size or modification time moved since it was opened, through the open handle
    /// or under its path, which another program may have replaced.
    fn moved(&self) -> bool {
        let Body::File { file, path, opened } = &self.body else {
            return false;
        };
        let now =
            |metadata: io::Result<fs::Metadata>| metadata.ok().map(|metadata| Facts::of(&metadata));
        now(file.metadata()) != Some(*opened) || now(fs::metadata(path)) != Some(*opened)
    }

    /// Reads the next bytes into `buffer`, as [`Read::read`] does.
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match &mut self.body {
            Body::File { file, .. } => file.read(buffer),
            Body::Memory { content, taken } => {
                let read = copy_from(bytes(content), *taken, buffer);
                *taken += read;
                Ok(read)
            }
        }
    }

    /// Reads into `buffer` at `offset` without moving a shared position.
    fn read_at(&self, buffer: &mut [u8], offset: u64) -> io::Result<usize> {
        match &self.body {
            Body::File { file, .. } => read_at(file, buffer, offset),
            Body::Memory { content, .. } => {
                let start = usize::try_from(offset).unwrap_or(usize::MAX);
                Ok(copy_from(bytes(content), start, buffer))
            }
        }
    }

    /// The source's length now, when it can be read.
    fn length(&self) -> Option<u64> {
        match &self.body {
            Body::File { file, .. } => file.metadata().ok().map(|metadata| metadata.len()),
            Body::Memory { content, .. } => Some(bytes(content).len() as u64),
        }
    }
}

fn bytes(content: &Content) -> &[u8] {
    (**content).as_ref()
}

/// Copies into `buffer` as much of `bytes` from `start` on as fits, and returns how much.
fn copy_from(bytes: &[u8], start: usize, buffer: &mut [u8]) -> usize {
    let rest = bytes.get(start..).unwrap_or_default();
    let copied = rest.len().min(buffer.len());
    buffer[..copied].copy_from_slice(&rest[..copied]);
    copied
}

/// Opens `path` for `request` and detects what its content is.
///
/// # Errors
///
/// `is-directory`, `too-large` when the file exceeds the limit, `failed` for what is neither file
/// nor directory, such as a FIFO a read would wait on, or the system's error.
pub(crate) fn open(path: &Path, request: &ReadRequest) -> Result<(ReadFacts, Reader), FilesError> {
    let failed = |failure: io::Error| error::from_io(&failure);
    let found = fs::metadata(path).map_err(failed)?;
    if found.is_dir() {
        return Err(FilesError::new(ErrorCode::IsDirectory));
    }
    if !found.is_file() {
        return Err(FilesError::new(ErrorCode::Failed));
    }
    let file = File::open(path).map_err(failed)?;
    let opened = Facts::of(&file.metadata().map_err(failed)?);
    let body = Body::File {
        file,
        path: path.to_path_buf(),
        opened,
    };
    start(body, opened, request)
}

/// Reads `content`, held in memory, for `request` as a file that never moves and whose
/// modification time reads as 0.
///
/// # Errors
///
/// `too-large` when the content exceeds the limit.
pub(crate) fn open_content(
    content: Content,
    request: &ReadRequest,
) -> Result<(ReadFacts, Reader), FilesError> {
    let facts = Facts {
        size: bytes(&content).len() as u64,
        mtime_ms: Signed::default(),
    };
    start(Body::Memory { content, taken: 0 }, facts, request)
}

/// Detects what `body`, as it was when opened, holds and starts the read `request` asks for.
fn start(
    body: Body,
    opened: Facts,
    request: &ReadRequest,
) -> Result<(ReadFacts, Reader), FilesError> {
    if opened.size > request.limit {
        return Err(FilesError::new(ErrorCode::TooLarge));
    }
    let mut facts = ReadFacts {
        size: opened.size,
        mtime_ms: opened.mtime_ms,
        encoding: None,
        bom: false,
        binary: false,
    };
    let mut source = Source {
        body,
        limit: request.limit,
    };
    if request.mode == ReadMode::Bytes {
        return Ok((facts, Reader::Bytes(ByteReader(source))));
    }
    let head = read_head(&mut source).map_err(|failure| error::from_io(&failure))?;
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
    let reader = TextReader::new(source, head, detection.bom, encoding);
    Ok((facts, Reader::Text(Box::new(reader))))
}

/// The source's first [`DETECTION_BYTES`] bytes, or all of a shorter source.
fn read_head(source: &mut Source) -> io::Result<Vec<u8>> {
    let mut head = vec![0; DETECTION_BYTES];
    let mut filled = 0;
    while filled < head.len() {
        match source.read(&mut head[filled..]) {
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
    pub(crate) fn chunk(&mut self, offset: u64, length: usize) -> Result<Piece, FilesError> {
        if offset != self.output {
            return Err(FilesError::new(ErrorCode::InvalidRequest));
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
    fn fill(&mut self) -> Result<(), FilesError> {
        let mut text = String::new();
        if self.head.is_empty() {
            let mut block = vec![0; BLOCK_BYTES];
            let read = loop {
                match self.source.read(&mut block) {
                    Ok(read) => break read,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(failure) => return Err(error::from_io(&failure)),
                }
            };
            if read == 0 {
                self.decoder.decode(&[], true, &mut text);
                self.finished = true;
            } else {
                self.consumed += read as u64;
                if self.consumed > self.source.limit {
                    return Err(FilesError::new(ErrorCode::TooLarge));
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
    pub(crate) fn chunk(&self, offset: u64, length: usize) -> Result<Piece, FilesError> {
        let mut bytes = vec![0; length];
        let mut filled = 0;
        while filled < length {
            match self.0.read_at(&mut bytes[filled..], offset + filled as u64) {
                Ok(0) => break,
                Ok(read) => filled += read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(failure) => return Err(error::from_io(&failure)),
            }
        }
        bytes.truncate(filled);
        let reached = offset.saturating_add(filled as u64);
        if reached > self.0.limit {
            return Err(FilesError::new(ErrorCode::TooLarge));
        }
        let at_end = filled < length || self.0.length().is_none_or(|length| length <= reached);
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
