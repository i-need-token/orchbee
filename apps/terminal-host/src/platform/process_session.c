#if defined(__linux__)
#define _POSIX_C_SOURCE 200809L
#endif

#include <errno.h>
#include <limits.h>
#include <signal.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#if defined(__APPLE__)
#include <libproc.h>
#include <sys/proc.h>
#elif defined(__FreeBSD__)
#include <sys/param.h>
#include <sys/sysctl.h>
#include <sys/types.h>
#include <sys/user.h>
#elif defined(__linux__)
#include <dirent.h>
#endif

typedef int (*process_visitor)(pid_t pid, void *context);

#if defined(__APPLE__)
static int visit_processes(process_visitor visitor, void *context) {
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

        int first_error = 0;
        for (int index = 0; index < count; ++index) {
            if (pids[index] > 0 && visitor(pids[index], context) != 0 && first_error == 0) {
                first_error = errno == 0 ? EIO : errno;
            }
        }
        free(pids);
        if (first_error != 0) {
            errno = first_error;
            return -1;
        }
        return 0;
    }
}

static int process_is_live(pid_t pid, int *live) {
    struct proc_bsdinfo information = {0};
    errno = 0;
    const int length = proc_pidinfo(
        pid,
        PROC_PIDTBSDINFO,
        0,
        &information,
        (int)sizeof(information)
    );
    if (length == 0 && (errno == 0 || errno == ESRCH)) {
        *live = 0;
        return 0;
    }
    if (length != (int)sizeof(information)) {
        if (errno == 0) errno = EIO;
        return -1;
    }
    *live = information.pbi_status != SZOMB;
    return 0;
}
#elif defined(__FreeBSD__)
static int visit_processes(process_visitor visitor, void *context) {
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

        int first_error = 0;
        const size_t count = actual_length / sizeof(struct kinfo_proc);
        for (size_t index = 0; index < count; ++index) {
            if (processes[index].ki_pid > 0 &&
                visitor(processes[index].ki_pid, context) != 0 && first_error == 0) {
                first_error = errno == 0 ? EIO : errno;
            }
        }
        free(processes);
        if (first_error != 0) {
            errno = first_error;
            return -1;
        }
        return 0;
    }
}

static int process_is_live(pid_t pid, int *live) {
    const int mib[] = {CTL_KERN, KERN_PROC, KERN_PROC_PID, pid};
    struct kinfo_proc process = {0};
    size_t byte_length = sizeof(process);
    if (sysctl(mib, 4, &process, &byte_length, NULL, 0) != 0) {
        if (errno == ESRCH) {
            *live = 0;
            return 0;
        }
        return -1;
    }
    if (byte_length == 0) {
        *live = 0;
        return 0;
    }
    if (byte_length < sizeof(process)) {
        errno = EIO;
        return -1;
    }
    *live = process.ki_stat != SZOMB;
    return 0;
}
#elif defined(__linux__)
static int visit_processes(process_visitor visitor, void *context) {
    DIR *directory = opendir("/proc");
    if (directory == NULL) {
        return -1;
    }

    int first_error = 0;
    int scan_error = 0;
    for (;;) {
        errno = 0;
        struct dirent *entry = readdir(directory);
        if (entry == NULL) {
            scan_error = errno;
            if (scan_error != 0 && first_error == 0) {
                first_error = scan_error;
            }
            break;
        }
        char *end = NULL;
        errno = 0;
        const long parsed = strtol(entry->d_name, &end, 10);
        if (errno != 0 || end == entry->d_name || *end != '\0' || parsed <= 0 || parsed > INT_MAX) {
            continue;
        }
        if (visitor((pid_t)parsed, context) != 0 && first_error == 0) {
            first_error = errno == 0 ? EIO : errno;
        }
    }
    if (closedir(directory) != 0 && first_error == 0) {
        first_error = errno == 0 ? EIO : errno;
    }
    if (first_error != 0) {
        errno = first_error;
        return -1;
    }
    return 0;
}

static int process_is_live(pid_t pid, int *live) {
    char path[64];
    const int path_length = snprintf(path, sizeof(path), "/proc/%d/stat", (int)pid);
    if (path_length < 0 || (size_t)path_length >= sizeof(path)) {
        errno = EOVERFLOW;
        return -1;
    }
    FILE *file = fopen(path, "r");
    if (file == NULL) {
        if (errno == ENOENT) {
            *live = 0;
            return 0;
        }
        return -1;
    }
    char stat[4096];
    if (fgets(stat, sizeof(stat), file) == NULL) {
        const int read_error = errno;
        fclose(file);
        if (read_error == ENOENT) {
            *live = 0;
            return 0;
        }
        errno = read_error == 0 ? EIO : read_error;
        return -1;
    }
    if (fclose(file) != 0) return -1;
    char *name_end = strrchr(stat, ')');
    if (name_end == NULL || name_end[1] != ' ' || name_end[2] == '\0') {
        errno = EIO;
        return -1;
    }
    const char state = name_end[2];
    *live = state != 'Z' && state != 'X' && state != 'x';
    return 0;
}
#else
#error "Unsupported POSIX platform"
#endif

struct signal_context {
    pid_t session_id;
    int signal_number;
};

static int signal_if_session_member(pid_t pid, void *raw_context) {
    const struct signal_context *context = raw_context;
    errno = 0;
    const pid_t current_session = getsid(pid);
    if (current_session < 0) {
        return errno == ESRCH || errno == EPERM || errno == EACCES ? 0 : -1;
    }
    if (current_session != context->session_id) return 0;
    if (kill(pid, context->signal_number) == 0 || errno == ESRCH) return 0;
    return -1;
}

static int signal_session_members(pid_t session_id, int signal_number) {
    struct signal_context context = {
        .session_id = session_id,
        .signal_number = signal_number,
    };
    return visit_processes(signal_if_session_member, &context);
}

struct live_member_context {
    pid_t session_id;
    uint32_t count;
};

static int count_live_session_member(pid_t pid, void *raw_context) {
    struct live_member_context *context = raw_context;
    errno = 0;
    const pid_t current_session = getsid(pid);
    if (current_session < 0) {
        return errno == ESRCH || errno == EPERM || errno == EACCES ? 0 : -1;
    }
    if (current_session != context->session_id) return 0;
    int live = 0;
    if (process_is_live(pid, &live) != 0) return -1;
    if (!live) return 0;
    if (context->count == UINT32_MAX) {
        errno = EOVERFLOW;
        return -1;
    }
    context->count += 1;
    return 0;
}

int orchbee_signal_process_session(int session_id, int signal_number) {
    if (session_id <= 0) {
        errno = EINVAL;
        return -1;
    }
    return signal_session_members((pid_t)session_id, signal_number);
}

int orchbee_process_session_live_members(int session_id, uint32_t *count) {
    if (session_id <= 0 || count == NULL) {
        errno = EINVAL;
        return -1;
    }
    struct live_member_context context = {
        .session_id = (pid_t)session_id,
        .count = 0,
    };
    if (visit_processes(count_live_session_member, &context) != 0) return -1;
    *count = context.count;
    return 0;
}

int orchbee_observe_process_exit(
    int process_id,
    int *result_kind,
    uint32_t *result_value
) {
    if (process_id <= 0 || result_kind == NULL || result_value == NULL) {
        return EINVAL;
    }
    siginfo_t information = {0};
    while (waitid(P_PID, (id_t)process_id, &information, WEXITED | WNOWAIT) != 0) {
        if (errno == EINTR) continue;
        return errno == 0 ? EIO : errno;
    }
    switch (information.si_code) {
        case CLD_EXITED:
            *result_kind = 1;
            *result_value = (uint32_t)information.si_status;
            break;
        case CLD_KILLED:
        case CLD_DUMPED:
            *result_kind = 2;
            *result_value = (uint32_t)information.si_status;
            break;
        default:
            *result_kind = 3;
            *result_value = (uint32_t)information.si_code;
            break;
    }
    return 0;
}

int orchbee_reap_process_exit(int process_id) {
    if (process_id <= 0) return EINVAL;
    int status = 0;
    while (waitpid((pid_t)process_id, &status, 0) < 0) {
        if (errno == EINTR) continue;
        return errno == 0 ? ECHILD : errno;
    }
    return 0;
}
