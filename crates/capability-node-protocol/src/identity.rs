use crate::error::ProtocolError;

const IDENTITY_BYTES: usize = 16;

macro_rules! define_identity {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; IDENTITY_BYTES]);

        impl $name {
            pub fn from_bytes(bytes: [u8; IDENTITY_BYTES]) -> Result<Self, ProtocolError> {
                if bytes.iter().all(|byte| *byte == 0) {
                    return Err(ProtocolError::InvalidIdentity);
                }
                Ok(Self(bytes))
            }

            #[must_use]
            pub const fn as_bytes(self) -> [u8; IDENTITY_BYTES] {
                self.0
            }
        }
    };
}

define_identity!(TerminalId);
define_identity!(OperationId);
define_identity!(WorkerId);
define_identity!(LeaseId);
define_identity!(HostCreationId);
define_identity!(HostEpoch);
define_identity!(HostTerminalId);
