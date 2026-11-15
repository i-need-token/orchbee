use std::{ffi::OsStr, io::Read};

use windows_permissions::{
    LocalBox, Sid,
    constants::{AceType, SeObjectType, SecurityInformation},
    wrappers,
};
use winsafe::{TokenInfo, co};

use super::{Derived, Stdio, derive_standard_token, spawn};

const MARKER: &str = "standard-rights-child-ran";
const MEDIUM_RID: u32 = 0x2000;
const GROUP_ENABLED: u32 = 0x4;

/// The groups UAC filters from an elevated token, as the decision lists them.
fn is_listed_administrator_group(sid: &str) -> bool {
    const BUILTIN: [&str; 9] = [
        "S-1-5-32-544",
        "S-1-5-32-547",
        "S-1-5-32-548",
        "S-1-5-32-549",
        "S-1-5-32-550",
        "S-1-5-32-551",
        "S-1-5-32-553",
        "S-1-5-32-556",
        "S-1-5-32-569",
    ];
    const DOMAIN: [&str; 7] = ["498", "512", "516", "518", "519", "520", "521"];
    let parts: Vec<&str> = sid.split('-').collect();
    BUILTIN.contains(&sid)
        || (parts.len() == 8
            && sid.starts_with("S-1-5-21-")
            && parts.last().is_some_and(|rid| DOMAIN.contains(rid)))
}

/// The derived token keeps no administrator right and the child it starts inherits exactly the
/// handle it was given and reports its own exit status.
#[test]
fn a_standard_child_runs_with_reduced_rights_and_relays_its_status() {
    let Derived::Standard(token) = derive_standard_token().expect("derive a standard token") else {
        panic!("the derived token stayed elevated");
    };
    let user = orchbee_local_ipc::current_user_sid().expect("read the user SID");

    let TokenInfo::IntegrityLevel(label) = token
        .0
        .GetTokenInformation(co::TOKEN_INFORMATION_CLASS::IntegrityLevel)
        .expect("read the integrity level")
    else {
        panic!("integrity level information");
    };
    let integrity = label
        .Label
        .Sid()
        .and_then(|sid| sid.SubAuthority().last().map(|rid| rid.raw()))
        .expect("integrity RID");
    assert!(integrity <= MEDIUM_RID, "integrity RID {integrity:#x}");

    let TokenInfo::Groups(groups) = token
        .0
        .GetTokenInformation(co::TOKEN_INFORMATION_CLASS::Groups)
        .expect("read the groups")
    else {
        panic!("group information");
    };
    for group in groups.Groups() {
        let sid = winsafe::ConvertSidToStringSid(group.Sid().expect("group SID")).expect("SID");
        assert!(
            !(is_listed_administrator_group(&sid) && group.Attributes & GROUP_ENABLED != 0),
            "{sid} stayed enabled"
        );
    }

    let kept = [
        co::SE_PRIV::CHANGE_NOTIFY_NAME,
        co::SE_PRIV::SHUTDOWN_NAME,
        co::SE_PRIV::UNDOCK_NAME,
        co::SE_PRIV::INC_WORKING_SET_NAME,
        co::SE_PRIV::TIME_ZONE_NAME,
    ]
    .map(|name| winsafe::LookupPrivilegeValue(None, name).expect("look up a kept privilege"));
    let TokenInfo::Privileges(privileges) = token
        .0
        .GetTokenInformation(co::TOKEN_INFORMATION_CLASS::Privileges)
        .expect("read the privileges")
    else {
        panic!("privilege information");
    };
    for privilege in privileges.Privileges() {
        assert!(kept.contains(&privilege.Luid), "{:?} kept", privilege.Luid);
    }

    let TokenInfo::Owner(owner) = token
        .0
        .GetTokenInformation(co::TOKEN_INFORMATION_CLASS::Owner)
        .expect("read the owner")
    else {
        panic!("owner information");
    };
    let owner = winsafe::ConvertSidToStringSid(owner.Owner().expect("owner SID")).expect("SID");
    assert_eq!(owner, user);

    let (mut reader, writer) = std::io::pipe().expect("create the output pipe");
    let stdio = Stdio {
        input: None,
        output: Some(writer.into()),
        error: None,
    };
    let command = crate::windows_system_program("cmd.exe").expect("locate cmd.exe");
    let arguments = format!("/d /c echo {MARKER}& exit 7");
    let child = spawn(&token, &command, OsStr::new(&arguments), stdio).expect("start the child");

    // The system builds a new process's security from its token's owner and default DACL.
    let descriptor = wrappers::GetSecurityInfo(
        &child.process,
        SeObjectType::SE_KERNEL_OBJECT,
        SecurityInformation::Owner | SecurityInformation::Dacl,
    )
    .expect("read the child's security");
    let user: LocalBox<Sid> = user.parse().expect("parse the user SID");
    let system: LocalBox<Sid> = "S-1-5-18".parse().expect("parse the SYSTEM SID");
    assert!(descriptor.owner() == Some(&*user), "child owner");
    let acl = descriptor.dacl().expect("child DACL");
    let allowed: Vec<&Sid> = (0..acl.len())
        .filter_map(|index| acl.get_ace(index))
        .inspect(|ace| assert!(ace.ace_type() == AceType::ACCESS_ALLOWED_ACE_TYPE))
        .filter_map(|ace| ace.sid())
        .collect();
    assert_eq!(allowed.len(), 2, "child DACL entries");
    assert!(allowed.contains(&&*user) && allowed.contains(&&*system));

    let status = child.wait().expect("wait for the child");
    let mut output = String::new();
    reader
        .read_to_string(&mut output)
        .expect("read the child's output");
    assert!(output.contains(MARKER), "child output {output:?}");
    assert_eq!(status, 7);
}
