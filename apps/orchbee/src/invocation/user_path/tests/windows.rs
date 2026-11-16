//! Windows-native non-Unicode environment evidence; lexical path cases stay shared.
use super::{OsString, PathFailure, normalize};
use std::os::windows::ffi::OsStringExt;

#[test]
fn non_unicode_home_directory_is_rejected_before_cwd_access() {
    assert_eq!(
        normalize(
            "~",
            |_| Some(OsString::from_wide(&[0xd800])),
            || panic!("unused cwd")
        ),
        Err(PathFailure::Context("USERPROFILE"))
    );
}
