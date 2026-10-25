#[test]
fn exact_name_query_keeps_a_native_directory_failure() {
    // The running executable is a real file, so it cannot be this child's directory.
    // Exercise the native query boundary without changing public pipe-name admission.
    let query = std::env::current_exe().unwrap().join("child");
    let error = super::exact_name_exists(query.to_str().unwrap()).unwrap_err();
    assert_eq!(error.raw_os_error(), Some(267)); // ERROR_DIRECTORY from FindFirstFileW.
}
