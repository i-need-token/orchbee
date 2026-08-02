#if defined(__linux__)
#define _POSIX_C_SOURCE 200809L
#endif

#include <errno.h>
#include <limits.h>
#include <signal.h>
#include <stdint.h>
#include <stdlib.h>
#include <sys/types.h>
#include <unistd.h>

#if defined(__APPLE__)
#include <libproc.h>
#elif defined(__FreeBSD__)
#include <sys/param.h>
#include <sys/sysctl.h>
#include <sys/types.h>
#include <sys/user.h>
#elif defined(__linux__)
#include <dirent.h>
#endif

static int signal_if_session_member(pid_t pid, pid_t session_id, int signal_number) {
    errno = 0;
    const pid_t current_session = getsid(pid);
    if (current_session < 0) {
        return errno == ESRCH ? 0 : -1;
    }
    if (current_session != session_id) {
        return 0;
    }
    if (kill(pid, signal_number) == 0 || errno == ESRCH) {
        return 0;
    }
    return -1;
}

#if defined(__APPLE__)
static int signal_session_members(pid_t session_id, int signal_number) {
    int capacity = proc_listallpids(NULL, 0);
    if (capacity < 0) {
        return -1;
    }
    capacity += 64;

    for (;;) {
        if (capacity <= 0 || capacity > INT_MAX / (int)sizeof(pid_t)) {
            errno = EOVERFLOW;
            return -1;
        }
        pid_t *pids = malloc((size_t)capacity * sizeof(pid_t));
        if (pids == NULL) {
            return -1;
        }
        const int count = proc_listallpids(pids, capacity * (int)sizeof(pid_t));
        if (count < 0) {
            free(pids);
            return -1;
        }
        if (count >= capacity) {
            free(pids);
            if (capacity > INT_MAX / 2) {
                errno = EOVERFLOW;
                return -1;
            }
            capacity *= 2;
            continue;
        }

        int failed = 0;
        for (int index = 0; index < count; ++index) {
            if (pids[index] > 0 &&
                signal_if_session_member(pids[index], session_id, signal_number) != 0) {
                failed = -1;
            }
        }
        free(pids);
        return failed;
    }
}
#elif defined(__FreeBSD__)
static int signal_session_members(pid_t session_id, int signal_number) {
    const int mib[] = {CTL_KERN, KERN_PROC, KERN_PROC_ALL, 0};
    size_t byte_length = 0;
    if (sysctl(mib, 4, NULL, &byte_length, NULL, 0) != 0) {
        return -1;
    }

    for (;;) {
        const size_t spare = byte_length / 8 + sizeof(struct kinfo_proc) * 16;
        if (byte_length > SIZE_MAX - spare) {
            errno = EOVERFLOW;
            return -1;
        }
        byte_length += spare;
        struct kinfo_proc *processes = malloc(byte_length);
        if (processes == NULL) {
            return -1;
        }
        size_t actual_length = byte_length;
        if (sysctl(mib, 4, processes, &actual_length, NULL, 0) != 0) {
            const int query_error = errno;
            free(processes);
            if (query_error == ENOMEM) {
                if (byte_length > SIZE_MAX / 2) {
                    errno = EOVERFLOW;
                    return -1;
                }
                byte_length *= 2;
                continue;
            }
            errno = query_error;
            return -1;
        }

        int failed = 0;
        const size_t count = actual_length / sizeof(struct kinfo_proc);
        for (size_t index = 0; index < count; ++index) {
            if (processes[index].ki_pid > 0 &&
                signal_if_session_member(processes[index].ki_pid, session_id, signal_number) != 0) {
                failed = -1;
            }
        }
        free(processes);
        return failed;
    }
}
#elif defined(__linux__)
static int signal_session_members(pid_t session_id, int signal_number) {
    DIR *directory = opendir("/proc");
    if (directory == NULL) {
        return -1;
    }

    int failed = 0;
    int scan_error = 0;
    for (;;) {
        errno = 0;
        struct dirent *entry = readdir(directory);
        if (entry == NULL) {
            scan_error = errno;
            if (scan_error != 0) {
                failed = -1;
            }
            break;
        }
        char *end = NULL;
        errno = 0;
        const long parsed = strtol(entry->d_name, &end, 10);
        if (errno != 0 || end == entry->d_name || *end != '\0' || parsed <= 0 || parsed > INT_MAX) {
            continue;
        }
        if (signal_if_session_member((pid_t)parsed, session_id, signal_number) != 0) {
            failed = -1;
        }
    }
    if (closedir(directory) != 0) {
        failed = -1;
    }
    if (failed != 0 && scan_error != 0) {
        errno = scan_error;
    }
    return failed;
}
#else
#error "Unsupported POSIX platform"
#endif

int orchbee_signal_process_session(int session_id, int signal_number) {
    if (session_id <= 0) {
        errno = EINVAL;
        return -1;
    }
    return signal_session_members((pid_t)session_id, signal_number);
}
