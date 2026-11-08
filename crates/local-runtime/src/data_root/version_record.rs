//! `internal/version.json`: the root's layout, which every process admits before any other read,
//! and the newest Orchbee version that used the root, so a refusal can name it. Cross-runtime
//! contract: search for `data-root-version-record` before changing it; Desktop Main reads and
//! writes the same record.
use super::DataRoot;
use std::{
    cmp::Ordering,
    fs::File,
    io::{self, Read},
    path::Path,
};

/// The layout this build reads and writes.
pub const LAYOUT: u64 = 1;
/// The product version this build records as the root's `newestVersion` and names its upgrade
/// backups after.
pub const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");
const RECORD_LIMIT: usize = 4096;

/// Why this build refuses a data root, found before it reads anything else in the root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayoutRefusal {
    /// The root's layout, higher than [`LAYOUT`], or `None` when `internal/version.json` exists
    /// but cannot be read as a layout record.
    pub layout: Option<u64>,
    /// The newest Orchbee version that used the root, when the record names one.
    pub newest_version: Option<String>,
}

impl LayoutRefusal {
    const UNREADABLE: Self = Self {
        layout: None,
        newest_version: None,
    };
}

impl DataRoot {
    /// Admits this root's layout, reading only `internal/version.json` and creating nothing. A
    /// root without the record is admitted, since nothing newer wrote it; a record whose layout is
    /// higher than [`LAYOUT`], or that cannot be read as a layout record, is refused.
    pub fn admit_layout(&self) -> Result<(), LayoutRefusal> {
        let Some(bytes) =
            read_record(&self.version_file()).map_err(|_| LayoutRefusal::UNREADABLE)?
        else {
            return Ok(());
        };
        VersionRecord::parse(&bytes)
            .ok_or(LayoutRefusal::UNREADABLE)?
            .refusal()
            .map_or(Ok(()), Err)
    }
}

/// The record as this build reads it, with every entry kept so that raising `newestVersion`
/// preserves entries a later release added.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VersionRecord {
    layout: u64,
    newest_version: Option<String>,
    entries: serde_json::Map<String, serde_json::Value>,
}

impl VersionRecord {
    /// A JSON object whose `layout` is a positive integer and whose `newestVersion`, when present,
    /// is a string. Later releases only add entries, so any other entry is kept unread.
    pub(crate) fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > RECORD_LIMIT {
            return None;
        }
        let serde_json::Value::Object(entries) = serde_json::from_slice(bytes).ok()? else {
            return None;
        };
        let layout = entries
            .get("layout")?
            .as_u64()
            .filter(|layout| *layout > 0)?;
        let newest_version = match entries.get("newestVersion") {
            None => None,
            Some(serde_json::Value::String(version)) => Some(version.clone()),
            Some(_) => return None,
        };
        Some(Self {
            layout,
            newest_version,
            entries,
        })
    }

    /// The record this build writes into a root that holds none.
    pub(crate) fn initial() -> Vec<u8> {
        encode(&serde_json::Map::from_iter([
            ("layout".to_owned(), LAYOUT.into()),
            ("newestVersion".to_owned(), PRODUCT_VERSION.into()),
        ]))
    }

    /// The refusal this record calls for when its layout is higher than this build's.
    pub(crate) fn refusal(&self) -> Option<LayoutRefusal> {
        (self.layout > LAYOUT).then(|| LayoutRefusal {
            layout: Some(self.layout),
            newest_version: self.newest_version.clone(),
        })
    }

    /// The record with `newestVersion` raised to `version`, when `version` is newer than the one
    /// it names or it names none; `None` leaves the record as it is.
    pub(crate) fn raised(&self, version: &str) -> Option<Vec<u8>> {
        if self
            .newest_version
            .as_deref()
            .is_some_and(|newest| !newer_product_version(version, newest))
        {
            return None;
        }
        let mut entries = self.entries.clone();
        entries.insert("newestVersion".into(), version.into());
        Some(encode(&entries))
    }
}

fn encode(entries: &serde_json::Map<String, serde_json::Value>) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(entries).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

/// Reads the record without creating anything; Windows refuses an unsafe file as every storage
/// owner refuses one.
pub(crate) fn read_record(path: &Path) -> io::Result<Option<Vec<u8>>> {
    #[cfg(windows)]
    let opened = crate::private_filesystem::open_file(path, false, false);
    #[cfg(not(windows))]
    let opened = File::open(path);
    let file: File = match opened {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(RECORD_LIMIT as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > RECORD_LIMIT {
        return Err(io::ErrorKind::FileTooLarge.into());
    }
    Ok(Some(bytes))
}

/// Whether `candidate` takes precedence over `than` as Semantic Versioning orders versions: the
/// numeric core first, then a release above its pre-releases, then pre-release identifiers, with
/// build metadata ignored. A version either side cannot read is never newer.
pub(crate) fn newer_product_version(candidate: &str, than: &str) -> bool {
    match (Precedence::parse(candidate), Precedence::parse(than)) {
        (Some(candidate), Some(than)) => candidate.cmp(&than).is_gt(),
        _ => false,
    }
}

/// Orders two product versions by [`newer_product_version`]'s precedence; a version that cannot be
/// read orders below every readable one.
#[cfg(windows)]
pub(crate) fn compare_product_versions(left: &str, right: &str) -> Ordering {
    match (Precedence::parse(left), Precedence::parse(right)) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => left.cmp(right),
    }
}

#[derive(Eq, PartialEq)]
struct Precedence<'a> {
    core: [u64; 3],
    pre_release: Vec<&'a str>,
}

impl<'a> Precedence<'a> {
    fn parse(text: &'a str) -> Option<Self> {
        let text = text.split_once('+').map_or(text, |(version, _)| version);
        let (core, pre_release) = text
            .split_once('-')
            .map_or((text, None), |(core, pre_release)| {
                (core, Some(pre_release))
            });
        let mut numbers = core.split('.').map(|part| {
            part.bytes()
                .all(|byte| byte.is_ascii_digit())
                .then(|| part.parse::<u64>().ok())
                .flatten()
        });
        let core = [numbers.next()??, numbers.next()??, numbers.next()??];
        if numbers.next().is_some() {
            return None;
        }
        let pre_release: Vec<&str> =
            pre_release.map_or_else(Vec::new, |text| text.split('.').collect());
        if pre_release.iter().any(|part| part.is_empty()) {
            return None;
        }
        Some(Self { core, pre_release })
    }
}

impl Ord for Precedence<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.core.cmp(&other.core).then_with(|| {
            match (self.pre_release.is_empty(), other.pre_release.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => self
                    .pre_release
                    .iter()
                    .zip(&other.pre_release)
                    .map(
                        |(left, right)| match (left.parse::<u64>(), right.parse::<u64>()) {
                            (Ok(left), Ok(right)) => left.cmp(&right),
                            (Ok(_), Err(_)) => Ordering::Less,
                            (Err(_), Ok(_)) => Ordering::Greater,
                            (Err(_), Err(_)) => left.cmp(right),
                        },
                    )
                    .find(|order| order.is_ne())
                    .unwrap_or_else(|| self.pre_release.len().cmp(&other.pre_release.len())),
            }
        })
    }
}

impl PartialOrd for Precedence<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
