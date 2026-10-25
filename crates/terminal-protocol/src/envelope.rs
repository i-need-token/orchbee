use crate::{
    CorrelationId, HostEpoch, MessageRole, PayloadEncoding, ProtocolError, ProtocolErrorCode,
    ProtocolVersion,
    limits::{MAXIMUM_TRANSPORT_RECORD_BYTES, TRANSPORT_ENVELOPE_HEADER_BYTES},
    message::message_contract,
};

const TRANSPORT_ENVELOPE_MAGIC: [u8; 4] = *crate::wire_constants::TRANSPORT_MAGIC;
const OPTIONAL_MESSAGE_FLAG: u16 = 1;
const RESPONSE_FLAG: u16 = 1 << 1;
const EVENT_FLAG: u16 = 1 << 2;
const KNOWN_FLAG_MASK: u16 = OPTIONAL_MESSAGE_FLAG | RESPONSE_FLAG | EVENT_FLAG;
const OPTIONAL_FLAG_PARTITION_MASK: u16 = 0x00ff;
const REQUIRED_FLAG_PARTITION_MASK: u16 = 0xff00;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Envelope<'payload> {
    pub correlation_id: Option<CorrelationId>,
    pub host_epoch: Option<HostEpoch>,
    pub kind: u16,
    pub optional_message: bool,
    pub payload: &'payload [u8],
    pub payload_encoding: PayloadEncoding,
    pub protocol: ProtocolVersion,
    pub role: MessageRole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedEnvelope<'record> {
    pub correlation_id: Option<CorrelationId>,
    pub host_epoch: Option<HostEpoch>,
    pub ignored_optional_flags: u8,
    pub kind: u16,
    pub known_message: bool,
    pub optional_message: bool,
    pub payload: &'record [u8],
    pub payload_encoding: PayloadEncoding,
    pub protocol: ProtocolVersion,
    pub role: MessageRole,
}

pub fn decode_transport_envelope(record: &[u8]) -> Result<DecodedEnvelope<'_>, ProtocolError> {
    if record.len() < TRANSPORT_ENVELOPE_HEADER_BYTES {
        return Err(error(
            ProtocolErrorCode::Truncated,
            "transport envelope is shorter than its fixed header",
        ));
    }
    if record.len() > MAXIMUM_TRANSPORT_RECORD_BYTES {
        return Err(error(
            ProtocolErrorCode::LimitExceeded,
            "transport record exceeds its maximum byte length",
        ));
    }
    if read_array::<4>(record, 0)? != TRANSPORT_ENVELOPE_MAGIC {
        return Err(error(
            ProtocolErrorCode::InvalidMagic,
            "transport envelope magic is not OBTP",
        ));
    }

    let header_length = usize::from(read_u16(record, 4)?);
    let flags = read_u16(record, 6)?;
    let total_length = usize::try_from(read_u32(record, 8)?).map_err(|_error| {
        error(
            ProtocolErrorCode::ArithmeticOverflow,
            "transport record length cannot be represented on this target",
        )
    })?;
    let payload_length = usize::try_from(read_u32(record, 12)?).map_err(|_error| {
        error(
            ProtocolErrorCode::ArithmeticOverflow,
            "transport payload length cannot be represented on this target",
        )
    })?;
    let protocol = ProtocolVersion {
        major: read_u16(record, 16)?,
        minor: read_u16(record, 18)?,
        patch: read_u16(record, 56)?,
    };
    let kind = read_u16(record, 20)?;
    let encoding_code = read_u8(record, 22)?;
    let reserved_byte = read_u8(record, 23)?;
    let correlation_bytes = read_array::<16>(record, 24)?;
    let host_epoch_bytes = read_array::<16>(record, 40)?;
    let reserved = read_array::<6>(record, 58)?;

    if header_length != TRANSPORT_ENVELOPE_HEADER_BYTES {
        return Err(error(
            ProtocolErrorCode::UnsupportedVersion,
            "transport envelope header length is unsupported",
        ));
    }
    if total_length != record.len() || payload_length != record.len().saturating_sub(header_length)
    {
        return Err(error(
            ProtocolErrorCode::LengthMismatch,
            "transport envelope length fields do not match the record",
        ));
    }
    if reserved_byte != 0 || reserved.iter().any(|byte| *byte != 0) {
        return Err(error(
            ProtocolErrorCode::UnknownRequiredFlag,
            "transport envelope reserved bytes must be zero",
        ));
    }
    if flags & REQUIRED_FLAG_PARTITION_MASK != 0 {
        return Err(error(
            ProtocolErrorCode::UnknownRequiredFlag,
            "transport envelope contains an unknown required flag",
        ));
    }

    let event = flags & EVENT_FLAG != 0;
    let response = flags & RESPONSE_FLAG != 0;
    if event && response {
        return Err(error(
            ProtocolErrorCode::InvalidField,
            "transport envelope cannot be both an event and a response",
        ));
    }
    let role = if event {
        MessageRole::Event
    } else if response {
        MessageRole::Response
    } else {
        MessageRole::Request
    };
    let optional_message = flags & OPTIONAL_MESSAGE_FLAG != 0;
    let known_message = message_contract(kind).is_some();
    if !known_message && !optional_message {
        return Err(error(
            ProtocolErrorCode::UnknownRequiredMessage,
            "transport envelope contains an unknown required message kind",
        ));
    }

    let payload_encoding = PayloadEncoding::from_code(encoding_code).ok_or_else(|| {
        error(
            ProtocolErrorCode::InvalidField,
            "transport envelope contains an unknown payload encoding",
        )
    })?;
    if payload_length > payload_encoding.maximum_payload_bytes() {
        return Err(error(
            ProtocolErrorCode::LimitExceeded,
            "transport payload exceeds its encoding-specific limit",
        ));
    }
    validate_message_contract(kind, optional_message, role, payload_encoding)?;

    let correlation_id = decode_correlation_id(correlation_bytes)?;
    let host_epoch = decode_host_epoch(host_epoch_bytes)?;
    let payload = record.get(header_length..).ok_or_else(truncated)?;

    Ok(DecodedEnvelope {
        correlation_id,
        host_epoch,
        ignored_optional_flags: (flags & OPTIONAL_FLAG_PARTITION_MASK & !KNOWN_FLAG_MASK) as u8,
        kind,
        known_message,
        optional_message,
        payload,
        payload_encoding,
        protocol,
        role,
    })
}

pub fn encode_transport_envelope(envelope: &Envelope<'_>) -> Result<Vec<u8>, ProtocolError> {
    validate_message_contract(
        envelope.kind,
        envelope.optional_message,
        envelope.role,
        envelope.payload_encoding,
    )?;
    if envelope.payload.len() > envelope.payload_encoding.maximum_payload_bytes() {
        return Err(error(
            ProtocolErrorCode::LimitExceeded,
            "transport payload exceeds its encoding-specific limit",
        ));
    }
    let total_length = TRANSPORT_ENVELOPE_HEADER_BYTES
        .checked_add(envelope.payload.len())
        .ok_or_else(|| {
            error(
                ProtocolErrorCode::ArithmeticOverflow,
                "transport record length overflowed",
            )
        })?;
    if total_length > MAXIMUM_TRANSPORT_RECORD_BYTES {
        return Err(error(
            ProtocolErrorCode::LimitExceeded,
            "transport record exceeds its maximum byte length",
        ));
    }
    let total_length_u32 = u32::try_from(total_length).map_err(|_error| {
        error(
            ProtocolErrorCode::ArithmeticOverflow,
            "transport record length exceeds the wire integer",
        )
    })?;
    let payload_length_u32 = u32::try_from(envelope.payload.len()).map_err(|_error| {
        error(
            ProtocolErrorCode::ArithmeticOverflow,
            "transport payload length exceeds the wire integer",
        )
    })?;

    let mut record = vec![0; total_length];
    write_bytes(&mut record, 0, &TRANSPORT_ENVELOPE_MAGIC)?;
    write_u16(&mut record, 4, TRANSPORT_ENVELOPE_HEADER_BYTES as u16)?;
    write_u16(&mut record, 6, encode_flags(envelope))?;
    write_u32(&mut record, 8, total_length_u32)?;
    write_u32(&mut record, 12, payload_length_u32)?;
    write_u16(&mut record, 16, envelope.protocol.major)?;
    write_u16(&mut record, 18, envelope.protocol.minor)?;
    write_u16(&mut record, 20, envelope.kind)?;
    write_u8(&mut record, 22, envelope.payload_encoding.code())?;
    write_optional_identity(
        &mut record,
        24,
        envelope.correlation_id.map(|identity| *identity.as_bytes()),
    )?;
    write_optional_identity(
        &mut record,
        40,
        envelope.host_epoch.map(|identity| *identity.as_bytes()),
    )?;
    write_u16(&mut record, 56, envelope.protocol.patch)?;
    write_bytes(
        &mut record,
        TRANSPORT_ENVELOPE_HEADER_BYTES,
        envelope.payload,
    )?;
    Ok(record)
}

fn validate_message_contract(
    kind: u16,
    optional_message: bool,
    role: MessageRole,
    payload_encoding: PayloadEncoding,
) -> Result<(), ProtocolError> {
    let Some(contract) = message_contract(kind) else {
        if optional_message {
            return Ok(());
        }
        return Err(error(
            ProtocolErrorCode::UnknownRequiredMessage,
            "transport envelope contains an unknown required message kind",
        ));
    };
    let expected = contract.payload_encoding(role).ok_or_else(|| {
        error(
            ProtocolErrorCode::InvalidField,
            "message kind is illegal for the envelope role",
        )
    })?;
    if payload_encoding != expected {
        return Err(error(
            ProtocolErrorCode::InvalidField,
            "message kind is paired with the wrong payload encoding",
        ));
    }
    Ok(())
}

fn encode_flags(envelope: &Envelope<'_>) -> u16 {
    let mut flags = if envelope.optional_message {
        OPTIONAL_MESSAGE_FLAG
    } else {
        0
    };
    flags |= match envelope.role {
        MessageRole::Request => 0,
        MessageRole::Response => RESPONSE_FLAG,
        MessageRole::Event => EVENT_FLAG,
    };
    flags
}

fn decode_correlation_id(bytes: [u8; 16]) -> Result<Option<CorrelationId>, ProtocolError> {
    if bytes.iter().all(|byte| *byte == 0) {
        return Ok(None);
    }
    CorrelationId::from_bytes(bytes)
        .map(Some)
        .map_err(|_error| {
            error(
                ProtocolErrorCode::InvalidIdentifier,
                "correlation identity is invalid",
            )
        })
}

fn decode_host_epoch(bytes: [u8; 16]) -> Result<Option<HostEpoch>, ProtocolError> {
    if bytes.iter().all(|byte| *byte == 0) {
        return Ok(None);
    }
    HostEpoch::from_bytes(bytes).map(Some).map_err(|_error| {
        error(
            ProtocolErrorCode::InvalidIdentifier,
            "Host epoch identity is invalid",
        )
    })
}

fn read_u8(record: &[u8], offset: usize) -> Result<u8, ProtocolError> {
    record.get(offset).copied().ok_or_else(truncated)
}

fn read_u16(record: &[u8], offset: usize) -> Result<u16, ProtocolError> {
    Ok(u16::from_le_bytes(read_array(record, offset)?))
}

fn read_u32(record: &[u8], offset: usize) -> Result<u32, ProtocolError> {
    Ok(u32::from_le_bytes(read_array(record, offset)?))
}

fn read_array<const LENGTH: usize>(
    record: &[u8],
    offset: usize,
) -> Result<[u8; LENGTH], ProtocolError> {
    record
        .get(offset..offset.saturating_add(LENGTH))
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(truncated)
}

fn write_u8(record: &mut [u8], offset: usize, value: u8) -> Result<(), ProtocolError> {
    let target = record.get_mut(offset).ok_or_else(truncated)?;
    *target = value;
    Ok(())
}

fn write_u16(record: &mut [u8], offset: usize, value: u16) -> Result<(), ProtocolError> {
    write_bytes(record, offset, &value.to_le_bytes())
}

fn write_u32(record: &mut [u8], offset: usize, value: u32) -> Result<(), ProtocolError> {
    write_bytes(record, offset, &value.to_le_bytes())
}

fn write_optional_identity(
    record: &mut [u8],
    offset: usize,
    identity: Option<[u8; 16]>,
) -> Result<(), ProtocolError> {
    if let Some(bytes) = identity {
        write_bytes(record, offset, &bytes)?;
    }
    Ok(())
}

fn write_bytes(record: &mut [u8], offset: usize, value: &[u8]) -> Result<(), ProtocolError> {
    let target = record
        .get_mut(offset..offset.saturating_add(value.len()))
        .ok_or_else(truncated)?;
    target.copy_from_slice(value);
    Ok(())
}

const fn error(code: ProtocolErrorCode, detail: &'static str) -> ProtocolError {
    ProtocolError::new(code, detail)
}

const fn truncated() -> ProtocolError {
    error(
        ProtocolErrorCode::Truncated,
        "transport envelope ended before a required field",
    )
}
