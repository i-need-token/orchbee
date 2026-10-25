#[test]
fn exact_name_query_keeps_a_native_directory_failure() {
    // The running executable is a real file, so it cannot be this child's directory.
    // Exercise the native query boundary without changing public pipe-name admission.
    let executable = std::env::current_exe().unwrap();
    let query = executable.join("child");
    let error = super::exact_name_exists(query.to_str().unwrap()).unwrap_err();
    assert_eq!(error.raw_os_error(), Some(267)); // ERROR_DIRECTORY from FindFirstFileW.

    // A missing name in the existing executable directory follows the ordinary
    // ERROR_FILE_NOT_FOUND path; the named-pipe behavior suite covers NO_MORE_FILES.
    let missing =
        executable.with_file_name(format!("orchbee-name-query-{}-missing", std::process::id()));
    assert!(!super::exact_name_exists(missing.to_str().unwrap()).unwrap());
}
