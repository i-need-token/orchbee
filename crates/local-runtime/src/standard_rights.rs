//! The standard-rights gate and relaunch of a process that Win32-OpenSSH may start with an elevated
//! administrator token. The gate reads this process's own token; a process that fails it relaunches
//! itself with a derived standard token through the private native adapter and relays the child's
//! exit status, or refuses. The README's standard-rights section owns the contract.
use std::{ffi::OsStr, io};

use winsafe::{HACCESSTOKEN, HPROCESS, TokenInfo, co, prelude::Handle};

use crate::standard_rights_native::{self as native, Derived};

/// The integrity RID of Medium, `S-1-16-8192`.
const MEDIUM_RID: u32 = 0x2000;

/// A token's integrity level as the gate judges it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntegrityLevel {
    /// Below Medium, as an AppContainer or a sandbox gives.
    Low,
    Medium,
    /// Above Medium, as an elevated administrator token has.
    High,
}

/// How a token holds BUILTIN Administrators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Administrators {
    /// The group is enabled, so access checks grant what it is granted.
    Enabled,
    /// The group is present for deny entries only, as UAC leaves a filtered token.
    DenyOnly,
    /// The token does not list the group.
    Absent,
}

/// The rights the gate reads from this process's own token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenRights {
    pub level: IntegrityLevel,
    pub administrators: Administrators,
    /// `TokenIsElevated`, recorded only; the gate never decides on it.
    pub elevated: bool,
}

impl TokenRights {
    /// Whether the token passes the gate: integrity at most Medium and BUILTIN Administrators not
    /// enabled.
    pub fn is_standard(self) -> bool {
        self.level != IntegrityLevel::High && self.administrators != Administrators::Enabled
    }
}

/// Reads the gate's view of this process's own token.
pub fn token_rights() -> io::Result<TokenRights> {
    let token = HPROCESS::GetCurrentProcess()
        .OpenProcessToken(co::TOKEN::QUERY)
        .map_err(system)?;
    let integrity = match token
        .GetTokenInformation(co::TOKEN_INFORMATION_CLASS::IntegrityLevel)
        .map_err(system)?
    {
        TokenInfo::IntegrityLevel(label) => label
            .Label
            .Sid()
            .and_then(|sid| sid.SubAuthority().last().map(|rid| rid.raw())),
        _ => None,
    }
    .ok_or_else(unexpected)?;
    let level = match integrity {
        rid if rid > MEDIUM_RID => IntegrityLevel::High,
        MEDIUM_RID => IntegrityLevel::Medium,
        _ => IntegrityLevel::Low,
    };

    let builtin = winsafe::CreateWellKnownSid(co::WELL_KNOWN_SID_TYPE::BuiltinAdministrators, None)
        .map_err(system)?;
    let administrators = if HACCESSTOKEN::NULL
        .CheckTokenMembership(&builtin)
        .map_err(system)?
    {
        Administrators::Enabled
    } else {
        let listed = match token
            .GetTokenInformation(co::TOKEN_INFORMATION_CLASS::Groups)
            .map_err(system)?
        {
            TokenInfo::Groups(groups) => groups
                .Groups()
                .iter()
                .any(|group| group.Sid().is_some_and(|sid| *sid == *builtin)),
            _ => return Err(unexpected()),
        };
        if listed {
            Administrators::DenyOnly
        } else {
            Administrators::Absent
        }
    };

    let elevated = match token
        .GetTokenInformation(co::TOKEN_INFORMATION_CLASS::Elevation)
        .map_err(system)?
    {
        TokenInfo::Elevation(elevation) => elevation.TokenIsElevated(),
        _ => return Err(unexpected()),
    };
    Ok(TokenRights {
        level,
        administrators,
        elevated,
    })
}

/// Why a relaunch with standard rights refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelaunchFailure {
    /// A call deriving the standard token failed, or the gate could not read this token.
    Derive,
    /// The derived token kept an integrity above Medium or an enabled administrator group.
    StillElevated,
    /// The child could not be started or waited for.
    Start,
}

/// Relaunches this executable as `"<executable>" <argument>` with a standard token derived from
/// this process's own, hands it this process's standard handles, waits for it, and returns its
/// exit status. The child stays in this process's Job. A failure starts nothing elevated.
pub fn relaunch_with_standard_rights(argument: &OsStr) -> Result<u32, RelaunchFailure> {
    let token = match native::derive_standard_token() {
        Ok(Derived::Standard(token)) => token,
        Ok(Derived::StillElevated) => return Err(RelaunchFailure::StillElevated),
        Err(_) => return Err(RelaunchFailure::Derive),
    };
    let start = |_: io::Error| RelaunchFailure::Start;
    let executable = std::env::current_exe().map_err(start)?;
    let stdio = native::take_standard_handles().map_err(start)?;
    native::spawn(&token, &executable, argument, stdio)
        .map_err(start)?
        .wait()
        .map_err(start)
}

fn system(error: co::ERROR) -> io::Error {
    io::Error::from_raw_os_error(error.raw() as i32)
}

fn unexpected() -> io::Error {
    io::ErrorKind::InvalidData.into()
}
