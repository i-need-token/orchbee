//! A ZIP archive with stored entries, which every supported operating system opens without another
//! tool. Entries are not compressed, so no compression dependency is needed; a bundle is bounded
//! by the seven daily logs.
use std::io;

const LOCAL_HEADER: u32 = 0x0403_4b50;
const CENTRAL_HEADER: u32 = 0x0201_4b50;
const END_OF_DIRECTORY: u32 = 0x0605_4b50;
/// Version 2.0, the first that defines folders and UTF-8 names.
const VERSION: u16 = 20;
/// General purpose flag 11: names are UTF-8.
const UTF8_NAMES: u16 = 0x0800;

/// The archive holding `entries` in order, each dated at `unix_seconds` (UTC). An entry or archive
/// past the format's 32-bit limits is refused.
pub fn diagnostic_archive(entries: &[(String, Vec<u8>)], unix_seconds: u64) -> io::Result<Vec<u8>> {
    let too_large = || io::Error::from(io::ErrorKind::InvalidInput);
    let (date, time) = dos_date_time(unix_seconds);
    let mut archive = Vec::new();
    let mut directory = Vec::new();
    for (name, data) in entries {
        let offset = u32::try_from(archive.len()).map_err(|_| too_large())?;
        let size = u32::try_from(data.len()).map_err(|_| too_large())?;
        let name_length = u16::try_from(name.len()).map_err(|_| too_large())?;
        let checksum = crc32(data);
        let mut common = Vec::new();
        for value in [VERSION, UTF8_NAMES, 0, time, date] {
            common.extend_from_slice(&value.to_le_bytes());
        }
        for value in [checksum, size, size] {
            common.extend_from_slice(&value.to_le_bytes());
        }
        common.extend_from_slice(&name_length.to_le_bytes());
        common.extend_from_slice(&0u16.to_le_bytes());

        archive.extend_from_slice(&LOCAL_HEADER.to_le_bytes());
        archive.extend_from_slice(&common);
        archive.extend_from_slice(name.as_bytes());
        archive.extend_from_slice(data);

        directory.extend_from_slice(&CENTRAL_HEADER.to_le_bytes());
        directory.extend_from_slice(&VERSION.to_le_bytes());
        directory.extend_from_slice(&common);
        for value in [0u16, 0, 0] {
            directory.extend_from_slice(&value.to_le_bytes());
        }
        directory.extend_from_slice(&0u32.to_le_bytes());
        directory.extend_from_slice(&offset.to_le_bytes());
        directory.extend_from_slice(name.as_bytes());
    }
    let count = u16::try_from(entries.len()).map_err(|_| too_large())?;
    let directory_offset = u32::try_from(archive.len()).map_err(|_| too_large())?;
    let directory_size = u32::try_from(directory.len()).map_err(|_| too_large())?;
    archive.extend_from_slice(&directory);
    archive.extend_from_slice(&END_OF_DIRECTORY.to_le_bytes());
    for value in [0u16, 0, count, count] {
        archive.extend_from_slice(&value.to_le_bytes());
    }
    archive.extend_from_slice(&directory_size.to_le_bytes());
    archive.extend_from_slice(&directory_offset.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    u32::try_from(archive.len()).map_err(|_| too_large())?;
    Ok(archive)
}

/// The MS-DOS date and time fields, clamped to the format's 1980 epoch.
fn dos_date_time(unix_seconds: u64) -> (u16, u16) {
    let days = i64::try_from(unix_seconds / 86_400).unwrap_or(0);
    let (year, month, day) = crate::diagnostic_log::civil_from_days(days);
    if year < 1980 {
        return (1 << 5 | 1, 0);
    }
    let seconds = unix_seconds % 86_400;
    let date = ((year - 1980).min(127) << 9 | month << 5 | day) as u16;
    let time = ((seconds / 3600) << 11 | (seconds / 60 % 60) << 5 | ((seconds % 60) / 2)) as u16;
    (date, time)
}

/// CRC-32 (IEEE 802.3), as ZIP records it.
pub(super) fn crc32(data: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                crc >> 1 ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}
