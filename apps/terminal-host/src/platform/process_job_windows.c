#define WIN32_LEAN_AND_MEAN
#include <windows.h>

HANDLE ob_windows_terminal_job_create(void) {
    HANDLE job = CreateJobObjectW(NULL, NULL);
    if (job == NULL) {
        return NULL;
    }

    JOBOBJECT_EXTENDED_LIMIT_INFORMATION limits = {0};
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if (!SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &limits,
            sizeof(limits)
        )) {
        const DWORD error = GetLastError();
        CloseHandle(job);
        SetLastError(error);
        return NULL;
    }
    return job;
}

BOOL ob_windows_terminal_job_assign_and_resume(
    HANDLE job,
    HANDLE process,
    HANDLE thread
) {
    BOOL assigned = FALSE;
    if (!IsProcessInJob(process, job, &assigned) ||
        (!assigned && !AssignProcessToJobObject(job, process))) {
        const DWORD error = GetLastError();
        if (TerminateProcess(process, 1)) {
            (void)WaitForSingleObject(process, INFINITE);
        }
        SetLastError(error);
        return FALSE;
    }
    if (ResumeThread(thread) == (DWORD)-1) {
        const DWORD error = GetLastError();
        if (TerminateProcess(process, 1)) {
            (void)WaitForSingleObject(process, INFINITE);
        }
        SetLastError(error);
        return FALSE;
    }
    return TRUE;
}

BOOL ob_windows_terminal_job_terminate(HANDLE job, UINT exit_code) {
    return TerminateJobObject(job, exit_code);
}

/* Clear kill-on-close so closing the last Job handle leaves the remaining
 * members running outside Terminal ownership. Every other limit is kept. */
BOOL ob_windows_terminal_job_release_members(HANDLE job) {
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION limits = {0};
    if (!QueryInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &limits,
            sizeof(limits),
            NULL
        )) {
        return FALSE;
    }
    limits.BasicLimitInformation.LimitFlags &= ~(DWORD)JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    return SetInformationJobObject(
        job,
        JobObjectExtendedLimitInformation,
        &limits,
        sizeof(limits)
    );
}

BOOL ob_windows_terminal_job_active_processes(HANDLE job, DWORD *active_processes) {
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION accounting = {0};
    if (!QueryInformationJobObject(
            job,
            JobObjectBasicAccountingInformation,
            &accounting,
            sizeof(accounting),
            NULL
        )) {
        return FALSE;
    }
    *active_processes = accounting.ActiveProcesses;
    return TRUE;
}
