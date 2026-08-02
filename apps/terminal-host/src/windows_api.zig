const std = @import("std");

const windows = std.os.windows;

pub const HPCON = windows.LPVOID;
pub const ProcThreadAttributeList = ?*anyopaque;

pub const STARTUPINFOEXW = extern struct {
    StartupInfo: windows.STARTUPINFOW,
    lpAttributeList: ProcThreadAttributeList,
};

pub const CREATE_UNICODE_ENVIRONMENT: windows.DWORD = 0x0000_0400;
pub const CREATE_SUSPENDED: windows.DWORD = 0x0000_0004;
pub const EXTENDED_STARTUPINFO_PRESENT: windows.DWORD = 0x0008_0000;
pub const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: windows.DWORD = 0x0002_0016;

pub extern "kernel32" fn CreatePipe(
    read_pipe: *windows.HANDLE,
    write_pipe: *windows.HANDLE,
    attributes: ?*const windows.SECURITY_ATTRIBUTES,
    size: windows.DWORD,
) callconv(.winapi) windows.BOOL;

pub extern "kernel32" fn CreatePseudoConsole(
    size: windows.COORD,
    input: windows.HANDLE,
    output: windows.HANDLE,
    flags: windows.DWORD,
    pseudo_console: *HPCON,
) callconv(.winapi) windows.HRESULT;

pub extern "kernel32" fn ResizePseudoConsole(
    pseudo_console: HPCON,
    size: windows.COORD,
) callconv(.winapi) windows.HRESULT;

pub extern "kernel32" fn ClosePseudoConsole(pseudo_console: HPCON) callconv(.winapi) void;

pub extern "kernel32" fn InitializeProcThreadAttributeList(
    attribute_list: ProcThreadAttributeList,
    attribute_count: windows.DWORD,
    flags: windows.DWORD,
    size: *windows.SIZE_T,
) callconv(.winapi) windows.BOOL;

pub extern "kernel32" fn UpdateProcThreadAttribute(
    attribute_list: ProcThreadAttributeList,
    flags: windows.DWORD,
    attribute: windows.DWORD_PTR,
    value: windows.PVOID,
    value_size: windows.SIZE_T,
    previous_value: ?windows.PVOID,
    return_size: ?*windows.SIZE_T,
) callconv(.winapi) windows.BOOL;

pub extern "kernel32" fn DeleteProcThreadAttributeList(
    attribute_list: ProcThreadAttributeList,
) callconv(.winapi) void;

pub extern "kernel32" fn CreateProcessW(
    application_name: ?windows.LPWSTR,
    command_line: ?windows.LPWSTR,
    process_attributes: ?*windows.SECURITY_ATTRIBUTES,
    thread_attributes: ?*windows.SECURITY_ATTRIBUTES,
    inherit_handles: windows.BOOL,
    creation_flags: windows.DWORD,
    environment: ?*anyopaque,
    current_directory: ?windows.LPWSTR,
    startup_info: *windows.STARTUPINFOW,
    process_information: *windows.PROCESS_INFORMATION,
) callconv(.winapi) windows.BOOL;

pub extern fn ob_windows_terminal_job_create() ?windows.HANDLE;

pub extern fn ob_windows_terminal_job_assign_and_resume(
    job: windows.HANDLE,
    process: windows.HANDLE,
    thread: windows.HANDLE,
) windows.BOOL;

pub extern fn ob_windows_terminal_job_terminate(
    job: windows.HANDLE,
    exit_code: windows.UINT,
) windows.BOOL;

pub extern fn ob_windows_terminal_job_active_processes(
    job: windows.HANDLE,
    active_processes: *windows.DWORD,
) windows.BOOL;
