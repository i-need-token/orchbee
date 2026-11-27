//! Differential evidence for the OBFB record codec. Every single-byte mutation, truncation and
//! insertion of each valid corpus record, at every place a record travels, is either refused by
//! both the decoder and the check or decoded into a record that encodes back to exactly those
//! bytes, so no record has a second spelling and the check never disagrees with the decoder. The
//! bounds hold at their edges in both directions.

use orchbee_files_protocol::{
    Chunk, ChunkEnd, FilesRecord, LineEnding, MAXIMUM_DATA_BYTES, MAXIMUM_REPLY_BYTES,
    MAXIMUM_REQUEST_BYTES, Put, PutEnd, RecordError, RecordPlace, TransferId, check_files_record,
    decode_files_record, encode_files_record,
};

use crate::support::{bytes, corpus};

const PLACES: [RecordPlace; 4] = [
    RecordPlace::RequestOpen,
    RecordPlace::EventsOpen,
    RecordPlace::ResultFrame,
    RecordPlace::NoticeFrame,
];
const BOUNDARY_BYTES: [u8; 6] = [0x00, 0x01, 0x7f, 0x80, 0xfe, 0xff];

fn agree(record: &[u8], origin: &str) {
    for place in PLACES {
        match decode_files_record(record, place) {
            Ok(decoded) => {
                assert_eq!(
                    encode_files_record(&decoded).as_deref(),
                    Ok(record),
                    "{origin} at {place:?} decoded into another spelling"
                );
                assert_eq!(check_files_record(record, place), Ok(()), "{origin}");
            }
            Err(error) => assert_eq!(
                check_files_record(record, place),
                Err(error),
                "{origin} at {place:?}"
            ),
        }
    }
}

#[test]
fn every_single_byte_mutation_is_refused_or_canonical() {
    for case in corpus().records.valid {
        let record = bytes(&case.hex);
        for position in 0..record.len() {
            let flips = (0..8).map(|bit| record[position] ^ (1 << bit));
            for value in flips.chain(BOUNDARY_BYTES) {
                let mut mutated = record.clone();
                mutated[position] = value;
                agree(
                    &mutated,
                    &format!("{} with byte {position} = {value:#04x}", case.name),
                );
            }
            let mut inserted = record.clone();
            inserted.insert(position, 0);
            agree(
                &inserted,
                &format!("{} with a byte inserted at {position}", case.name),
            );
            let mut deleted = record.clone();
            deleted.remove(position);
            agree(&deleted, &format!("{} without byte {position}", case.name));
            agree(
                &record[..position],
                &format!("{} truncated to {position}", case.name),
            );
        }
    }
}

fn transfer() -> TransferId {
    TransferId::new(1).unwrap()
}

fn json(length: usize) -> Vec<u8> {
    let mut json = vec![b' '; length];
    json[0] = b'{';
    json[length - 1] = b'}';
    json
}

fn final_chunk(bytes: Vec<u8>) -> FilesRecord {
    FilesRecord::Chunk(Chunk {
        transfer: transfer(),
        offset: 0,
        end: Some(ChunkEnd {
            line_ending: LineEnding::Lf,
            changed: false,
            digest: Some([7; 32]),
        }),
        bytes,
    })
}

#[test]
fn bounds_admit_their_edge_and_refuse_one_byte_more() {
    let put = |bytes| {
        FilesRecord::Put(Put {
            transfer: transfer(),
            offset: 0,
            end: PutEnd::Commit,
            bytes,
        })
    };
    let cases = [
        (
            FilesRecord::Request(json(MAXIMUM_REQUEST_BYTES)),
            FilesRecord::Request(json(MAXIMUM_REQUEST_BYTES + 1)),
            RecordPlace::RequestOpen,
        ),
        (
            FilesRecord::Result(json(MAXIMUM_REPLY_BYTES)),
            FilesRecord::Result(json(MAXIMUM_REPLY_BYTES + 1)),
            RecordPlace::ResultFrame,
        ),
        (
            FilesRecord::Notice(json(MAXIMUM_REPLY_BYTES)),
            FilesRecord::Notice(json(MAXIMUM_REPLY_BYTES + 1)),
            RecordPlace::NoticeFrame,
        ),
        (
            put(vec![0; MAXIMUM_DATA_BYTES]),
            put(vec![0; MAXIMUM_DATA_BYTES + 1]),
            RecordPlace::RequestOpen,
        ),
        (
            final_chunk(vec![0; MAXIMUM_DATA_BYTES]),
            final_chunk(vec![0; MAXIMUM_DATA_BYTES + 1]),
            RecordPlace::ResultFrame,
        ),
    ];
    for (edge, beyond, place) in cases {
        let encoded = encode_files_record(&edge).unwrap();
        assert_eq!(decode_files_record(&encoded, place), Ok(edge));
        assert_eq!(
            encode_files_record(&beyond),
            Err(RecordError::InvalidLength)
        );
        // The same bytes with a header that admits them still exceed the kind's bound.
        let mut oversized = encoded.clone();
        oversized.push(0);
        let length = u32::try_from(oversized.len() - 12).unwrap();
        oversized[8..12].copy_from_slice(&length.to_be_bytes());
        assert_eq!(
            decode_files_record(&oversized, place),
            Err(RecordError::InvalidLength)
        );
    }
}
