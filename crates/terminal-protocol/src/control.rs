use std::{collections::HashSet, fmt, marker::PhantomData};

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, DeserializeOwned, MapAccess, SeqAccess, Visitor},
};

use crate::{
    ClientId, HostEpoch, MessageRole, ProtocolError, ProtocolErrorCode, ProtocolVersion,
    limits::{
        MAXIMUM_CONTROL_JSON_BYTES, MAXIMUM_HOST_VERSION_BYTES, MAXIMUM_INPUT_BYTES,
        MAXIMUM_OPTIONAL_FEATURE_NAME_BYTES, MAXIMUM_OPTIONAL_FEATURES, MAXIMUM_ROW_WINDOW_BYTES,
        MAXIMUM_TERMINAL_FRAME_BYTES, MAXIMUM_TYPED_FAILURE_CODE_BYTES,
        MAXIMUM_TYPED_FAILURE_MESSAGE_BYTES,
    },
};

const HELLO_KIND: u16 = 1;
const HELLO_ACCEPTED_KIND: u16 = 2;
const PROTOCOL_REJECTED_KIND: u16 = 3;
const PING_KIND: u16 = 4;
const TYPED_FAILURE_KIND: u16 = 5;
const MAXIMUM_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const DUPLICATE_MEMBER_MARKER: &str = "orchbee-duplicate-json-member";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hello {
    pub client_id: ClientId,
    pub frame_bytes: u32,
    pub input_bytes: u32,
    pub protocol: ProtocolVersion,
    pub row_window_bytes: u32,
    pub supported_optional_features: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HelloAccepted {
    pub frame_bytes: u32,
    pub host_epoch: HostEpoch,
    pub host_version: String,
    pub input_bytes: u32,
    pub protocol: ProtocolVersion,
    pub row_window_bytes: u32,
    pub supported_optional_features: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolRejectionReason {
    VersionMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolRejected {
    pub client: ProtocolVersion,
    pub host: ProtocolVersion,
    pub reason: ProtocolRejectionReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedFailure {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConnectionControl {
    Hello(Hello),
    HelloAccepted(HelloAccepted),
    ProtocolRejected(ProtocolRejected),
    PingRequest,
    PingResponse,
    TypedFailure(TypedFailure),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedConnectionControl {
    pub kind: u16,
    pub payload: Vec<u8>,
    pub role: MessageRole,
}

pub fn decode_connection_control(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<ConnectionControl, ProtocolError> {
    preflight_json(payload)?;
    match (kind, role) {
        (HELLO_KIND, MessageRole::Request) => {
            decode_wire::<HelloWire>(payload).and_then(hello_from_wire)
        }
        (HELLO_ACCEPTED_KIND, MessageRole::Response) => {
            decode_wire::<HelloAcceptedWire>(payload).and_then(hello_accepted_from_wire)
        }
        (PROTOCOL_REJECTED_KIND, MessageRole::Response) => {
            decode_wire::<ProtocolRejectedWire>(payload).and_then(protocol_rejected_from_wire)
        }
        (PING_KIND, MessageRole::Request) => {
            decode_wire::<EmptyControlWire>(payload)?;
            Ok(ConnectionControl::PingRequest)
        }
        (PING_KIND, MessageRole::Response) => {
            decode_wire::<EmptyControlWire>(payload)?;
            Ok(ConnectionControl::PingResponse)
        }
        (TYPED_FAILURE_KIND, MessageRole::Response) => {
            decode_wire::<TypedFailureWire>(payload).and_then(typed_failure_from_wire)
        }
        _ => Err(error(
            ProtocolErrorCode::InvalidField,
            "message kind and role do not select a connection-control payload",
        )),
    }
}

pub fn encode_connection_control(
    value: &ConnectionControl,
) -> Result<EncodedConnectionControl, ProtocolError> {
    let (kind, role, payload) = match value {
        ConnectionControl::Hello(hello) => (
            HELLO_KIND,
            MessageRole::Request,
            serialize_wire(&hello_to_wire(hello)?)?,
        ),
        ConnectionControl::HelloAccepted(accepted) => (
            HELLO_ACCEPTED_KIND,
            MessageRole::Response,
            serialize_wire(&hello_accepted_to_wire(accepted)?)?,
        ),
        ConnectionControl::ProtocolRejected(rejected) => (
            PROTOCOL_REJECTED_KIND,
            MessageRole::Response,
            serialize_wire(&protocol_rejected_to_wire(rejected))?,
        ),
        ConnectionControl::PingRequest => (
            PING_KIND,
            MessageRole::Request,
            serialize_wire(&EmptyControlWire {})?,
        ),
        ConnectionControl::PingResponse => (
            PING_KIND,
            MessageRole::Response,
            serialize_wire(&EmptyControlWire {})?,
        ),
        ConnectionControl::TypedFailure(failure) => (
            TYPED_FAILURE_KIND,
            MessageRole::Response,
            serialize_wire(&typed_failure_to_wire(failure)?)?,
        ),
    };
    Ok(EncodedConnectionControl {
        kind,
        payload,
        role,
    })
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HelloWire {
    client_id: String,
    frame_bytes: SafeInteger,
    input_bytes: SafeInteger,
    protocol: ProtocolVersionWire,
    row_window_bytes: SafeInteger,
    supported_optional_features: Vec<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HelloAcceptedWire {
    frame_bytes: SafeInteger,
    host_epoch: String,
    host_version: String,
    input_bytes: SafeInteger,
    protocol: ProtocolVersionWire,
    row_window_bytes: SafeInteger,
    supported_optional_features: Vec<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProtocolRejectedWire {
    client: ProtocolVersionWire,
    host: ProtocolVersionWire,
    reason: ProtocolRejectionReasonWire,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ProtocolRejectionReasonWire {
    VersionMismatch,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TypedFailureWire {
    code: String,
    message: String,
    retryable: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EmptyControlWire {}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProtocolVersionWire {
    major: SafeInteger,
    minor: SafeInteger,
    patch: SafeInteger,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SafeInteger(u64);

impl SafeInteger {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SafeSignedInteger(i64);

impl SafeSignedInteger {
    pub(crate) const fn new(value: i64) -> Self {
        Self(value)
    }

    pub(crate) const fn get(self) -> i64 {
        self.0
    }
}

#[derive(Default)]
pub(crate) enum RequiredNullable<Value> {
    #[default]
    Missing,
    Null,
    Present(Value),
}

impl<Value> RequiredNullable<Value> {
    pub(crate) fn from_option(value: Option<Value>) -> Self {
        match value {
            Some(value) => Self::Present(value),
            None => Self::Null,
        }
    }

    pub(crate) fn into_option(self) -> Result<Option<Value>, ProtocolError> {
        match self {
            Self::Missing => Err(error(
                ProtocolErrorCode::InvalidField,
                "required nullable control member is missing",
            )),
            Self::Null => Ok(None),
            Self::Present(value) => Ok(Some(value)),
        }
    }
}

impl<Value> Serialize for RequiredNullable<Value>
where
    Value: Serialize,
{
    fn serialize<SerializerType>(
        &self,
        serializer: SerializerType,
    ) -> Result<SerializerType::Ok, SerializerType::Error>
    where
        SerializerType: Serializer,
    {
        match self {
            Self::Missing => Err(serde::ser::Error::custom(
                "required nullable control member is missing",
            )),
            Self::Null => serializer.serialize_none(),
            Self::Present(value) => serializer.serialize_some(value),
        }
    }
}

impl<'de, Value> Deserialize<'de> for RequiredNullable<Value>
where
    Value: Deserialize<'de>,
{
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        deserializer.deserialize_option(RequiredNullableVisitor(PhantomData))
    }
}

struct RequiredNullableVisitor<Value>(PhantomData<Value>);

impl<'de, Value> Visitor<'de> for RequiredNullableVisitor<Value>
where
    Value: Deserialize<'de>,
{
    type Value = RequiredNullable<Value>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an explicit null or value")
    }

    fn visit_none<ErrorType>(self) -> Result<Self::Value, ErrorType> {
        Ok(RequiredNullable::Null)
    }

    fn visit_unit<ErrorType>(self) -> Result<Self::Value, ErrorType> {
        Ok(RequiredNullable::Null)
    }

    fn visit_some<DeserializerType>(
        self,
        deserializer: DeserializerType,
    ) -> Result<Self::Value, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        Value::deserialize(deserializer).map(RequiredNullable::Present)
    }
}

impl Serialize for SafeInteger {
    fn serialize<SerializerType>(
        &self,
        serializer: SerializerType,
    ) -> Result<SerializerType::Ok, SerializerType::Error>
    where
        SerializerType: Serializer,
    {
        serializer.serialize_u64(self.0)
    }
}

impl<'de> Deserialize<'de> for SafeInteger {
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        deserializer.deserialize_any(SafeIntegerVisitor)
    }
}

impl Serialize for SafeSignedInteger {
    fn serialize<SerializerType>(
        &self,
        serializer: SerializerType,
    ) -> Result<SerializerType::Ok, SerializerType::Error>
    where
        SerializerType: Serializer,
    {
        serializer.serialize_i64(self.0)
    }
}

impl<'de> Deserialize<'de> for SafeSignedInteger {
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        deserializer.deserialize_any(SafeSignedIntegerVisitor)
    }
}

struct SafeSignedIntegerVisitor;

impl Visitor<'_> for SafeSignedIntegerVisitor {
    type Value = SafeSignedInteger;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a finite signed binary64 safe integer")
    }

    fn visit_i64<ErrorType>(self, value: i64) -> Result<Self::Value, ErrorType>
    where
        ErrorType: de::Error,
    {
        if value.unsigned_abs() > MAXIMUM_SAFE_INTEGER {
            return Err(ErrorType::custom(
                "number exceeds the binary64 safe-integer range",
            ));
        }
        Ok(SafeSignedInteger(value))
    }

    fn visit_u64<ErrorType>(self, value: u64) -> Result<Self::Value, ErrorType>
    where
        ErrorType: de::Error,
    {
        if value > MAXIMUM_SAFE_INTEGER {
            return Err(ErrorType::custom(
                "number exceeds the binary64 safe-integer range",
            ));
        }
        let value = i64::try_from(value)
            .map_err(|_error| ErrorType::custom("number exceeds the signed integer range"))?;
        Ok(SafeSignedInteger(value))
    }

    fn visit_f64<ErrorType>(self, value: f64) -> Result<Self::Value, ErrorType>
    where
        ErrorType: de::Error,
    {
        if !value.is_finite()
            || value.trunc() != value
            || value < -(MAXIMUM_SAFE_INTEGER as f64)
            || value > MAXIMUM_SAFE_INTEGER as f64
        {
            return Err(ErrorType::custom(
                "number must normalize to a signed safe integer",
            ));
        }
        Ok(SafeSignedInteger(value as i64))
    }
}

struct SafeIntegerVisitor;

impl Visitor<'_> for SafeIntegerVisitor {
    type Value = SafeInteger;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a finite nonnegative binary64 safe integer")
    }

    fn visit_u64<ErrorType>(self, value: u64) -> Result<Self::Value, ErrorType>
    where
        ErrorType: de::Error,
    {
        safe_integer_from_u64(value).map_err(ErrorType::custom)
    }

    fn visit_i64<ErrorType>(self, value: i64) -> Result<Self::Value, ErrorType>
    where
        ErrorType: de::Error,
    {
        let value = u64::try_from(value).map_err(|_error| {
            ErrorType::custom("number must normalize to a nonnegative safe integer")
        })?;
        self.visit_u64(value)
    }

    fn visit_f64<ErrorType>(self, value: f64) -> Result<Self::Value, ErrorType>
    where
        ErrorType: de::Error,
    {
        if !value.is_finite()
            || value < 0.0
            || value.trunc() != value
            || value > MAXIMUM_SAFE_INTEGER as f64
        {
            return Err(ErrorType::custom(
                "number must normalize to a nonnegative safe integer",
            ));
        }
        safe_integer_from_u64(value as u64).map_err(ErrorType::custom)
    }
}

fn safe_integer_from_u64(value: u64) -> Result<SafeInteger, &'static str> {
    if value > MAXIMUM_SAFE_INTEGER {
        return Err("number exceeds the binary64 safe-integer range");
    }
    Ok(SafeInteger(value))
}

fn hello_from_wire(wire: HelloWire) -> Result<ConnectionControl, ProtocolError> {
    let value = Hello {
        client_id: parse_client_id(&wire.client_id)?,
        frame_bytes: bounded_nonzero_u32(wire.frame_bytes, MAXIMUM_TERMINAL_FRAME_BYTES)?,
        input_bytes: bounded_nonzero_u32(wire.input_bytes, MAXIMUM_INPUT_BYTES)?,
        protocol: protocol_version_from_wire(wire.protocol)?,
        row_window_bytes: bounded_nonzero_u32(wire.row_window_bytes, MAXIMUM_ROW_WINDOW_BYTES)?,
        supported_optional_features: wire.supported_optional_features,
    };
    validate_feature_names(&value.supported_optional_features)?;
    Ok(ConnectionControl::Hello(value))
}

fn hello_to_wire(value: &Hello) -> Result<HelloWire, ProtocolError> {
    validate_bounded_nonzero_u32(value.frame_bytes, MAXIMUM_TERMINAL_FRAME_BYTES)?;
    validate_bounded_nonzero_u32(value.input_bytes, MAXIMUM_INPUT_BYTES)?;
    validate_bounded_nonzero_u32(value.row_window_bytes, MAXIMUM_ROW_WINDOW_BYTES)?;
    validate_feature_names(&value.supported_optional_features)?;
    Ok(HelloWire {
        client_id: value.client_id.to_text(),
        frame_bytes: SafeInteger(u64::from(value.frame_bytes)),
        input_bytes: SafeInteger(u64::from(value.input_bytes)),
        protocol: protocol_version_to_wire(value.protocol),
        row_window_bytes: SafeInteger(u64::from(value.row_window_bytes)),
        supported_optional_features: value.supported_optional_features.clone(),
    })
}

fn hello_accepted_from_wire(wire: HelloAcceptedWire) -> Result<ConnectionControl, ProtocolError> {
    let value = HelloAccepted {
        frame_bytes: bounded_nonzero_u32(wire.frame_bytes, MAXIMUM_TERMINAL_FRAME_BYTES)?,
        host_epoch: parse_host_epoch(&wire.host_epoch)?,
        host_version: wire.host_version,
        input_bytes: bounded_nonzero_u32(wire.input_bytes, MAXIMUM_INPUT_BYTES)?,
        protocol: protocol_version_from_wire(wire.protocol)?,
        row_window_bytes: bounded_nonzero_u32(wire.row_window_bytes, MAXIMUM_ROW_WINDOW_BYTES)?,
        supported_optional_features: wire.supported_optional_features,
    };
    validate_host_version(&value.host_version)?;
    validate_feature_names(&value.supported_optional_features)?;
    Ok(ConnectionControl::HelloAccepted(value))
}

fn hello_accepted_to_wire(value: &HelloAccepted) -> Result<HelloAcceptedWire, ProtocolError> {
    validate_bounded_nonzero_u32(value.frame_bytes, MAXIMUM_TERMINAL_FRAME_BYTES)?;
    validate_bounded_nonzero_u32(value.input_bytes, MAXIMUM_INPUT_BYTES)?;
    validate_bounded_nonzero_u32(value.row_window_bytes, MAXIMUM_ROW_WINDOW_BYTES)?;
    validate_host_version(&value.host_version)?;
    validate_feature_names(&value.supported_optional_features)?;
    Ok(HelloAcceptedWire {
        frame_bytes: SafeInteger(u64::from(value.frame_bytes)),
        host_epoch: value.host_epoch.to_text(),
        host_version: value.host_version.clone(),
        input_bytes: SafeInteger(u64::from(value.input_bytes)),
        protocol: protocol_version_to_wire(value.protocol),
        row_window_bytes: SafeInteger(u64::from(value.row_window_bytes)),
        supported_optional_features: value.supported_optional_features.clone(),
    })
}

fn protocol_rejected_from_wire(
    wire: ProtocolRejectedWire,
) -> Result<ConnectionControl, ProtocolError> {
    Ok(ConnectionControl::ProtocolRejected(ProtocolRejected {
        client: protocol_version_from_wire(wire.client)?,
        host: protocol_version_from_wire(wire.host)?,
        reason: match wire.reason {
            ProtocolRejectionReasonWire::VersionMismatch => {
                ProtocolRejectionReason::VersionMismatch
            }
        },
    }))
}

fn protocol_rejected_to_wire(value: &ProtocolRejected) -> ProtocolRejectedWire {
    ProtocolRejectedWire {
        client: protocol_version_to_wire(value.client),
        host: protocol_version_to_wire(value.host),
        reason: match value.reason {
            ProtocolRejectionReason::VersionMismatch => {
                ProtocolRejectionReasonWire::VersionMismatch
            }
        },
    }
}

fn typed_failure_from_wire(wire: TypedFailureWire) -> Result<ConnectionControl, ProtocolError> {
    let value = TypedFailure {
        code: wire.code,
        message: wire.message,
        retryable: wire.retryable,
    };
    validate_typed_failure(&value)?;
    Ok(ConnectionControl::TypedFailure(value))
}

fn typed_failure_to_wire(value: &TypedFailure) -> Result<TypedFailureWire, ProtocolError> {
    validate_typed_failure(value)?;
    Ok(TypedFailureWire {
        code: value.code.clone(),
        message: value.message.clone(),
        retryable: value.retryable,
    })
}

fn protocol_version_from_wire(wire: ProtocolVersionWire) -> Result<ProtocolVersion, ProtocolError> {
    Ok(ProtocolVersion {
        major: bounded_u16(wire.major)?,
        minor: bounded_u16(wire.minor)?,
        patch: bounded_u16(wire.patch)?,
    })
}

const fn protocol_version_to_wire(value: ProtocolVersion) -> ProtocolVersionWire {
    ProtocolVersionWire {
        major: SafeInteger(value.major as u64),
        minor: SafeInteger(value.minor as u64),
        patch: SafeInteger(value.patch as u64),
    }
}

fn bounded_u16(value: SafeInteger) -> Result<u16, ProtocolError> {
    u16::try_from(value.0).map_err(|_error| {
        error(
            ProtocolErrorCode::InvalidField,
            "protocol version component exceeds unsigned 16-bit range",
        )
    })
}

fn bounded_nonzero_u32(value: SafeInteger, maximum: usize) -> Result<u32, ProtocolError> {
    let value = u32::try_from(value.0).map_err(|_error| invalid_bounded_size())?;
    validate_bounded_nonzero_u32(value, maximum)?;
    Ok(value)
}

fn validate_bounded_nonzero_u32(value: u32, maximum: usize) -> Result<(), ProtocolError> {
    if value == 0 || usize::try_from(value).map_or(true, |value| value > maximum) {
        return Err(invalid_bounded_size());
    }
    Ok(())
}

fn invalid_bounded_size() -> ProtocolError {
    error(
        ProtocolErrorCode::InvalidField,
        "negotiated size is zero or exceeds its protocol maximum",
    )
}

fn validate_feature_names(features: &[String]) -> Result<(), ProtocolError> {
    if features.len() > MAXIMUM_OPTIONAL_FEATURES
        || features.iter().any(|feature| {
            feature.is_empty() || feature.len() > MAXIMUM_OPTIONAL_FEATURE_NAME_BYTES
        })
    {
        return Err(error(
            ProtocolErrorCode::InvalidField,
            "optional feature collection exceeds its protocol bounds",
        ));
    }
    Ok(())
}

fn validate_typed_failure(value: &TypedFailure) -> Result<(), ProtocolError> {
    if value.code.is_empty() || value.code.len() > MAXIMUM_TYPED_FAILURE_CODE_BYTES {
        return Err(error(
            ProtocolErrorCode::InvalidField,
            "typed failure code exceeds its protocol bounds",
        ));
    }
    if value.message.is_empty() || value.message.len() > MAXIMUM_TYPED_FAILURE_MESSAGE_BYTES {
        return Err(error(
            ProtocolErrorCode::InvalidField,
            "typed failure message exceeds its protocol bounds",
        ));
    }
    Ok(())
}

fn parse_client_id(value: &str) -> Result<ClientId, ProtocolError> {
    ClientId::parse(value).map_err(|_error| invalid_identity())
}

fn parse_host_epoch(value: &str) -> Result<HostEpoch, ProtocolError> {
    HostEpoch::parse(value).map_err(|_error| invalid_identity())
}

fn invalid_identity() -> ProtocolError {
    error(
        ProtocolErrorCode::InvalidIdentifier,
        "control identity is not canonical nonzero 128-bit hexadecimal text",
    )
}

fn validate_host_version(value: &str) -> Result<(), ProtocolError> {
    if value.len() < 5 || value.len() > MAXIMUM_HOST_VERSION_BYTES || !is_semantic_version(value) {
        return Err(error(
            ProtocolErrorCode::InvalidField,
            "Host version is not bounded semantic-version text",
        ));
    }
    Ok(())
}

fn is_semantic_version(value: &str) -> bool {
    let (without_build, build) = match value.split_once('+') {
        Some((left, right)) => (left, Some(right)),
        None => (value, None),
    };
    if build.is_some_and(|value| !valid_identifier_sequence(value)) {
        return false;
    }
    let (core, prerelease) = match without_build.split_once('-') {
        Some((left, right)) => (left, Some(right)),
        None => (without_build, None),
    };
    if prerelease.is_some_and(|value| !valid_identifier_sequence(value)) {
        return false;
    }
    let mut components = core.split('.');
    let valid = components
        .by_ref()
        .take(3)
        .all(valid_numeric_version_component);
    valid && components.next().is_none() && core.matches('.').count() == 2
}

fn valid_numeric_version_component(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (value == "0" || !value.starts_with('0'))
}

fn valid_identifier_sequence(value: &str) -> bool {
    !value.is_empty()
        && value.split('.').all(|identifier| {
            !identifier.is_empty()
                && identifier
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

pub(crate) fn preflight_json(payload: &[u8]) -> Result<(), ProtocolError> {
    if payload.len() > MAXIMUM_CONTROL_JSON_BYTES {
        return Err(error(
            ProtocolErrorCode::LimitExceeded,
            "control JSON payload exceeds its maximum byte length",
        ));
    }
    std::str::from_utf8(payload).map_err(|_error| {
        error(
            ProtocolErrorCode::InvalidUtf8,
            "control JSON payload is not valid UTF-8",
        )
    })?;

    let mut deserializer = serde_json::Deserializer::from_slice(payload);
    DuplicateChecked::deserialize(&mut deserializer).map_err(json_syntax_error)?;
    deserializer.end().map_err(json_syntax_error)
}

pub(crate) fn decode_wire<Value>(payload: &[u8]) -> Result<Value, ProtocolError>
where
    Value: DeserializeOwned,
{
    serde_json::from_slice(payload).map_err(json_refinement_error)
}

pub(crate) fn serialize_wire<Value>(value: &Value) -> Result<Vec<u8>, ProtocolError>
where
    Value: Serialize,
{
    let payload = serde_json::to_vec(value).map_err(|_error| {
        error(
            ProtocolErrorCode::InvalidField,
            "control value cannot be serialized",
        )
    })?;
    if payload.len() > MAXIMUM_CONTROL_JSON_BYTES {
        return Err(error(
            ProtocolErrorCode::LimitExceeded,
            "encoded control JSON exceeds its maximum byte length",
        ));
    }
    Ok(payload)
}

fn json_syntax_error(value: serde_json::Error) -> ProtocolError {
    let detail = value.to_string();
    if detail.contains("number out of range") {
        return error(
            ProtocolErrorCode::InvalidField,
            "JSON number does not refine to a finite binary64 value",
        );
    }
    error(
        ProtocolErrorCode::InvalidJson,
        if detail.contains(DUPLICATE_MEMBER_MARKER) {
            "control JSON contains a duplicate object member"
        } else {
            "control JSON syntax is invalid"
        },
    )
}

fn json_refinement_error(value: serde_json::Error) -> ProtocolError {
    match value.classify() {
        serde_json::error::Category::Data => error(
            ProtocolErrorCode::InvalidField,
            "control JSON does not satisfy the selected message schema",
        ),
        serde_json::error::Category::Io
        | serde_json::error::Category::Syntax
        | serde_json::error::Category::Eof => error(
            ProtocolErrorCode::InvalidJson,
            "control JSON syntax is invalid",
        ),
    }
}

struct DuplicateChecked;

impl<'de> Deserialize<'de> for DuplicateChecked {
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        deserializer.deserialize_any(DuplicateCheckedVisitor)
    }
}

struct DuplicateCheckedVisitor;

impl<'de> Visitor<'de> for DuplicateCheckedVisitor {
    type Value = DuplicateChecked;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a duplicate-free JSON value")
    }

    fn visit_bool<ErrorType>(self, _value: bool) -> Result<Self::Value, ErrorType> {
        Ok(DuplicateChecked)
    }

    fn visit_i64<ErrorType>(self, _value: i64) -> Result<Self::Value, ErrorType> {
        Ok(DuplicateChecked)
    }

    fn visit_u64<ErrorType>(self, _value: u64) -> Result<Self::Value, ErrorType> {
        Ok(DuplicateChecked)
    }

    fn visit_f64<ErrorType>(self, _value: f64) -> Result<Self::Value, ErrorType> {
        Ok(DuplicateChecked)
    }

    fn visit_str<ErrorType>(self, _value: &str) -> Result<Self::Value, ErrorType> {
        Ok(DuplicateChecked)
    }

    fn visit_string<ErrorType>(self, _value: String) -> Result<Self::Value, ErrorType> {
        Ok(DuplicateChecked)
    }

    fn visit_none<ErrorType>(self) -> Result<Self::Value, ErrorType> {
        Ok(DuplicateChecked)
    }

    fn visit_unit<ErrorType>(self) -> Result<Self::Value, ErrorType> {
        Ok(DuplicateChecked)
    }

    fn visit_some<DeserializerType>(
        self,
        deserializer: DeserializerType,
    ) -> Result<Self::Value, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        DuplicateChecked::deserialize(deserializer)
    }

    fn visit_seq<Sequence>(self, mut sequence: Sequence) -> Result<Self::Value, Sequence::Error>
    where
        Sequence: SeqAccess<'de>,
    {
        while sequence.next_element::<DuplicateChecked>()?.is_some() {}
        Ok(DuplicateChecked)
    }

    fn visit_map<Map>(self, mut map: Map) -> Result<Self::Value, Map::Error>
    where
        Map: MapAccess<'de>,
    {
        let mut members = HashSet::new();
        while let Some(member) = map.next_key::<String>()? {
            if !members.insert(member) {
                return Err(de::Error::custom(DUPLICATE_MEMBER_MARKER));
            }
            map.next_value::<DuplicateChecked>()?;
        }
        Ok(DuplicateChecked)
    }
}

const fn error(code: ProtocolErrorCode, detail: &'static str) -> ProtocolError {
    ProtocolError::new(code, detail)
}
