//! The search path a shell launched now on Windows starts with, before its profile runs. The
//! Terminal Host builds every shell's environment from the registry when it launches it
//! (`apps/terminal-host/src/platform/windows_environment.zig`), so this Worker's own `PATH`,
//! inherited when the Session started, can lack a directory an installer added since. This module
//! repeats the Host's projection for `PATH` alone: the Machine `Path`, then the User `Path`, each
//! expanded when stored as `REG_EXPAND_SZ`, joined by `;`. A directory stored there, such as a
//! version manager's shim directory, is part of it; a directory the shell's profile adds once it
//! runs, such as a version manager's activation, is not.
//!
//! A `%NAME%` reference expands, in order: `%PATH%` in the User value to the projected Machine
//! value; a reference to a value already being expanded, or nested 128 deep, stays literal; an
//! inherited `ORCHBEE_` value; a registry value, User over Machine and expanded in turn when
//! expandable; one of the runtime values the Host keeps from its inherited environment, which is
//! this Worker's environment unchanged; otherwise it stays literal. The Host also drops repeated
//! directories, which cannot change whether a command is found, so they are kept here. A registry
//! scope that cannot be read, or a projection past the Host's bounds, answers `None`: the Host
//! then refuses the launch, so no shell would find anything.
use std::{collections::HashMap, ffi::OsString};
use winsafe::{self as w, co};

const MACHINE_KEY: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
const USER_KEY: &str = "Environment";
const PRIVATE_PREFIX: &str = "ORCHBEE_";
// The Host's projection bounds (`windows_environment_limits.zig`).
const MAXIMUM_VALUES: usize = 256;
const MAXIMUM_NAME_BYTES: usize = 3 * 32 * 1024;
const MAXIMUM_VALUE_BYTES: usize = 256 * 1024;
const MAXIMUM_SNAPSHOT_BYTES: usize = 512 * 1024;
const MAXIMUM_EXPANDED_BYTES: usize = 256 * 1024;
const MAXIMUM_WORK: usize = 64 * 1024;
const MAXIMUM_DEPTH: usize = 128;

/// Values an interactive logon synthesizes outside the two registry keys, which the Host keeps
/// from its inherited environment (`windows_runtime_inherited_names`).
const RUNTIME_NAMES: [&str; 38] = [
    "ALLUSERSPROFILE",
    "APPDATA",
    "CLIENTNAME",
    "CommonProgramFiles",
    "CommonProgramFiles(Arm)",
    "CommonProgramFiles(x86)",
    "CommonProgramW6432",
    "COMPUTERNAME",
    "ComSpec",
    "HOMEDRIVE",
    "HOMEPATH",
    "HOMESHARE",
    "LOCALAPPDATA",
    "LOGONSERVER",
    "NUMBER_OF_PROCESSORS",
    "OS",
    "PROCESSOR_ARCHITECTURE",
    "PROCESSOR_ARCHITEW6432",
    "PROCESSOR_IDENTIFIER",
    "PROCESSOR_LEVEL",
    "PROCESSOR_REVISION",
    "ProgramData",
    "ProgramFiles",
    "ProgramFiles(Arm)",
    "ProgramFiles(x86)",
    "ProgramW6432",
    "PUBLIC",
    "SESSIONNAME",
    "SystemDrive",
    "SystemRoot",
    "TEMP",
    "TMP",
    "USERDOMAIN",
    "USERDOMAIN_ROAMINGPROFILE",
    "USERDNSDOMAIN",
    "USERNAME",
    "USERPROFILE",
    "windir",
];

struct Value {
    text: String,
    expandable: bool,
}

/// The `PATH` a shell launched now would start with, or `None` when the Host would refuse the
/// launch.
pub(super) fn current() -> Option<OsString> {
    let machine = read_scope(&w::HKEY::LOCAL_MACHINE, MACHINE_KEY)?;
    let user = read_scope(&w::HKEY::CURRENT_USER, USER_KEY)?;
    if machine.count + user.count > MAXIMUM_VALUES {
        return None;
    }
    let mut snapshot = 0_usize;
    let mut registry = HashMap::new();
    let mut machine_path = None;
    let mut user_path = None;
    for (scope, is_machine) in [(machine.values, true), (user.values, false)] {
        for (name, value) in scope {
            if name.len() > MAXIMUM_NAME_BYTES || value.text.len() > MAXIMUM_VALUE_BYTES {
                return None;
            }
            snapshot += name.len() + value.text.len() + 2;
            if snapshot > MAXIMUM_SNAPSHOT_BYTES {
                return None;
            }
            let key = fold(&name);
            // HKLM commonly records USERNAME=SYSTEM, which is not the interactive identity.
            if is_machine && key == "USERNAME" {
                continue;
            }
            if key == "PATH" {
                *(if is_machine {
                    &mut machine_path
                } else {
                    &mut user_path
                }) = Some(value);
                continue;
            }
            registry.insert(key, value);
        }
    }
    // A name or value that is not Unicode cannot be named by a registry reference.
    let inherited: HashMap<String, String> = std::env::vars_os()
        .filter_map(|(name, value)| Some((fold(name.to_str()?), value.into_string().ok()?)))
        .filter(|(key, _)| {
            key.starts_with(PRIVATE_PREFIX) || RUNTIME_NAMES.iter().any(|name| fold(name) == *key)
        })
        .collect();
    let mut projection = Projection {
        registry: &registry,
        inherited: &inherited,
        work: 0,
    };
    let machine = match machine_path {
        Some(value) => Some(projection.project(&value, None)?),
        None => None,
    };
    let user = match user_path {
        Some(value) => Some(projection.project(&value, machine.as_deref())?),
        None => None,
    };
    let path = match (machine, user) {
        (Some(machine), Some(user)) => format!("{machine};{user}"),
        (Some(path), None) | (None, Some(path)) => path,
        (None, None) => String::new(),
    };
    (path.len() <= MAXIMUM_EXPANDED_BYTES).then(|| path.into())
}

struct Scope {
    /// Every value the key holds, of any type, as the Host's bound counts them.
    count: usize,
    values: Vec<(String, Value)>,
}

/// A missing key holds no values; any other failure leaves the scope unreadable.
fn read_scope(root: &w::HKEY, sub_key: &str) -> Option<Scope> {
    let key = match root.RegOpenKeyEx(Some(sub_key), co::REG_OPTION::default(), co::KEY::READ) {
        Ok(key) => key,
        Err(error) if error == co::ERROR::FILE_NOT_FOUND || error == co::ERROR::PATH_NOT_FOUND => {
            return Some(Scope {
                count: 0,
                values: Vec::new(),
            });
        }
        Err(_) => return None,
    };
    let mut scope = Scope {
        count: 0,
        values: Vec::new(),
    };
    for entry in key.RegEnumValue().ok()? {
        let (name, kind) = entry.ok()?;
        scope.count += 1;
        if scope.count > MAXIMUM_VALUES {
            return None;
        }
        // The unnamed value is key metadata, and a name with `=` cannot be a variable.
        if name.is_empty() || name.contains('=') {
            continue;
        }
        let value = match (kind, key.RegQueryValueEx(Some(&name)).ok()?) {
            (co::REG::SZ, w::RegistryValue::Sz(text)) => Value {
                text,
                expandable: false,
            },
            (co::REG::EXPAND_SZ, w::RegistryValue::ExpandSz(text)) => Value {
                text,
                expandable: true,
            },
            _ => continue,
        };
        scope.values.push((name, value));
    }
    Some(scope)
}

struct Projection<'a> {
    /// Registry values by folded name, User over Machine; `PATH` is held apart.
    registry: &'a HashMap<String, Value>,
    /// Runtime and `ORCHBEE_` values of this Worker's environment, by folded name.
    inherited: &'a HashMap<String, String>,
    work: usize,
}

impl Projection<'_> {
    fn project(&mut self, path: &Value, machine_path: Option<&str>) -> Option<String> {
        if !path.expandable {
            return Some(path.text.clone());
        }
        self.expand("PATH", &path.text, &mut Vec::new(), machine_path)
    }

    fn expand(
        &mut self,
        key: &str,
        value: &str,
        active: &mut Vec<String>,
        machine_path: Option<&str>,
    ) -> Option<String> {
        self.consume()?;
        active.push(key.to_owned());
        let mut expanded = String::new();
        let mut rest = value;
        while let Some(open) = rest.find('%') {
            let after = &rest[open + 1..];
            let Some(close) = after.find('%') else {
                break;
            };
            append(&mut expanded, &rest[..open])?;
            self.consume()?;
            let token = fold(&after[..close]);
            let original = &rest[open..open + close + 2];
            rest = &after[close + 1..];
            if let Some(machine) = machine_path.filter(|_| token == "PATH") {
                append(&mut expanded, machine)?;
            } else if active.contains(&token) || active.len() >= MAXIMUM_DEPTH {
                append(&mut expanded, original)?;
            } else if let Some(private) = self
                .inherited
                .get(&token)
                .filter(|_| token.starts_with(PRIVATE_PREFIX))
            {
                append(&mut expanded, private)?;
            } else if let Some(dependency) = self.registry.get(&token) {
                if dependency.expandable {
                    let nested = self.expand(&token, &dependency.text, active, machine_path)?;
                    append(&mut expanded, &nested)?;
                } else {
                    append(&mut expanded, &dependency.text)?;
                }
            } else if let Some(runtime) = self.inherited.get(&token) {
                append(&mut expanded, runtime)?;
            } else {
                append(&mut expanded, original)?;
            }
        }
        // An unmatched `%` and everything after it stay literal.
        append(&mut expanded, rest)?;
        active.pop();
        Some(expanded)
    }

    fn consume(&mut self) -> Option<()> {
        self.work += 1;
        (self.work <= MAXIMUM_WORK).then_some(())
    }
}

fn append(expanded: &mut String, text: &str) -> Option<()> {
    if expanded.len() + text.len() > MAXIMUM_EXPANDED_BYTES {
        return None;
    }
    expanded.push_str(text);
    Some(())
}

/// Windows variable names compare without regard to case.
fn fold(name: &str) -> String {
    name.to_uppercase()
}
