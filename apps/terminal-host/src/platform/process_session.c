#if defined(__linux__)
#define _GNU_SOURCE
#endif

#include <errno.h>
#include <limits.h>
#include <signal.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#if defined(__APPLE__)
#include <libproc.h>
#include <mach/mach.h>
#include <mach/task_info.h>
#include <sys/event.h>
#include <sys/proc.h>
#elif defined(__FreeBSD__)
#include <sys/param.h>
#include <sys/procctl.h>
#elif defined(__linux__)
#include <dirent.h>
#include <poll.h>
#include <sys/syscall.h>
#else
#error "Unsupported POSIX platform"
#endif

// A vanished candidate is the only failed membership observation that can be
// skipped. Permission, resource, and every other error leave membership
// unknown, so the scan must fail conservatively instead of omitting a process
// that may still belong to the owned session.
int orchbee_classify_session_membership_error(int observation_error) {
    if (observation_error == ESRCH) return 0;
    errno = observation_error == 0 ? EIO : observation_error;
    return -1;
}

// Apple's __proc_info_extended_id compares this uint64 value with the signed
// int PID version stored by XNU. Match libproc's direct conversion so a PID
// version with bit 31 set is sign-extended instead of zero-extended.
uint64_t orchbee_pidversion_external_id(int pid_version) {
    return (uint64_t)pid_version;
}

// Shared by the FreeBSD native snapshot/reap pass and deterministic tests so
// the fresh-snapshot retry predicate cannot drift from the waitpid target.
int orchbee_freebsd_member_is_targeted_zombie(
    int process_id,
    int subtree,
    int is_direct_child,
    int is_zombie
) {
    return process_id != subtree && is_direct_child != 0 && is_zombie != 0;
}

// Classify one pidfd poll without treating readiness as proof that its numeric
// PID cannot have been reused. A readable retained leader is an exact zombie:
// the Host owns its only wait and keeps it unreaped until identity release. A
// readable non-leader needs a signal-0 probe on the same pidfd, returned as 1.
int orchbee_linux_classify_pidfd_poll(
    int is_retained_leader,
    int poll_result,
    int poll_error,
    int readable,
    int hangup,
    int error_event,
    int invalid,
    int unknown,
    int *live
) {
    if (live == NULL) {
        errno = EINVAL;
        return -1;
    }
    if (poll_result < 0) {
        errno = poll_error == 0 ? EIO : poll_error;
        return -1;
    }
    if (poll_result == 0) {
        if (readable != 0 || hangup != 0 || error_event != 0 ||
            invalid != 0 || unknown != 0) {
            errno = EIO;
            return -1;
        }
        *live = 1;
        return 0;
    }
    if (poll_result != 1) {
        errno = EIO;
        return -1;
    }
    if (invalid != 0) {
        errno = EBADF;
        return -1;
    }
    if (error_event != 0 || unknown != 0) {
        errno = EIO;
        return -1;
    }
    if (hangup != 0) {
        errno = EAGAIN;
        return -1;
    }
    if (readable == 0) {
        errno = EIO;
        return -1;
    }
    if (is_retained_leader != 0) {
        *live = 0;
        return 0;
    }
    return 1;
}

// signal-0 is the exact pidfd linearization point after readable readiness. A
// successful probe denotes the same unreaped zombie and is safely non-live.
// ESRCH means the captured identity was reaped and its PID may now name a
// same-session replacement, so the whole scan must retry rather than return 0.
int orchbee_linux_classify_pidfd_probe(
    int probe_result,
    int probe_error,
    int *live
) {
    if (live == NULL) {
        errno = EINVAL;
        return -1;
    }
    if (probe_result == 0) {
        *live = 0;
        return 0;
    }
    if (probe_result < 0) {
        errno = probe_error == ESRCH ? EAGAIN :
            (probe_error == 0 ? EIO : probe_error);
        return -1;
    }
    errno = EIO;
    return -1;
}

// An actual signal through an invalidated pidfd cannot silently succeed: a
// fresh stable-member scan must decide whether the replacement is still owned.
int orchbee_linux_classify_pidfd_signal(int signal_result, int signal_error) {
    if (signal_result == 0) return 0;
    if (signal_result < 0) {
        errno = signal_error == ESRCH ? EAGAIN :
            (signal_error == 0 ? EIO : signal_error);
        return -1;
    }
    errno = EIO;
    return -1;
}

#if defined(__APPLE__) || defined(__linux__)

struct orchbee_session_member_iterator {
    pid_t session_id;
#if defined(__APPLE__)
    pid_t *pids;
    size_t count;
    size_t index;
#elif defined(__linux__)
    DIR *directory;
#endif
};

struct orchbee_session_member_ref {
#if defined(__APPLE__)
    mach_port_name_t task;
    audit_token_t audit_token;
#elif defined(__linux__)
    int pidfd;
    pid_t pid;
    pid_t session_id;
#endif
};

#if defined(__APPLE__)

// These constants and the exact-ID entry point are private libproc ABI used by
// proc_pidpath_audittoken in Apple's own libproc wrapper. The pinned SDK exports
// the symbol, and PIF_COMPARE_IDVERSION binds the process-status read to the PID
// version captured in TASK_AUDIT_TOKEN rather than to the reusable numeric PID.
enum {
    orchbee_proc_info_call_pidinfo = 0x2,
    orchbee_pif_compare_idversion = 0x01,
};

// libbsm exposes these accessors for audit_token_t. Declare only the two ABI
// functions needed here so Zig's bundled macOS cross headers need not mirror
// the much larger, deprecated audit header surface.
extern pid_t audit_token_to_pid(audit_token_t audit_token);
extern int audit_token_to_pidversion(audit_token_t audit_token);

extern int __proc_info_extended_id(
    int32_t call_number,
    int32_t pid,
    uint32_t flavor,
    uint32_t flags,
    uint64_t external_id,
    uint64_t argument,
    uint64_t buffer,
    int32_t buffer_size
);

// Session membership is independent of credentials. Enumerating only the
// Host's effective or real UID can omit a member after setuid/seteuid and
// falsely prove containment. The reusable PID from this all-process list is
// only a negative prefilter; every possible member still requires a stable
// task/audit-token reference and an authoritative membership recheck.
int orchbee_darwin_process_enumeration_scope(void) {
    return PROC_ALL_PIDS;
}

static int darwin_list_processes(pid_t **pids, size_t *count) {
    errno = 0;
    int byte_capacity = proc_listpids(
        orchbee_darwin_process_enumeration_scope(),
        0,
        NULL,
        0
    );
    if (byte_capacity == 0 && errno != 0) return -1;
    if (byte_capacity < 0 || byte_capacity > INT_MAX - (int)(64 * sizeof(pid_t))) {
        errno = EOVERFLOW;
        return -1;
    }
    if (byte_capacity % (int)sizeof(pid_t) != 0) {
        errno = EIO;
        return -1;
    }
    byte_capacity += (int)(64 * sizeof(pid_t));

    for (;;) {
        pid_t *values = calloc((size_t)byte_capacity / sizeof(pid_t), sizeof(pid_t));
        if (values == NULL) return -1;
        errno = 0;
        const int byte_count = proc_listpids(
            orchbee_darwin_process_enumeration_scope(),
            0,
            values,
            byte_capacity
        );
        if (byte_count == 0 && errno != 0) {
            const int list_error = errno;
            free(values);
            errno = list_error;
            return -1;
        }
        if (byte_count < 0) {
            free(values);
            errno = EIO;
            return -1;
        }
        if (byte_count % (int)sizeof(pid_t) != 0) {
            free(values);
            errno = EIO;
            return -1;
        }
        if (byte_count >= byte_capacity) {
            free(values);
            if (byte_capacity > INT_MAX / 2) {
                errno = EOVERFLOW;
                return -1;
            }
            byte_capacity *= 2;
            continue;
        }
        *pids = values;
        *count = (size_t)byte_count / sizeof(pid_t);
        return 0;
    }
}

static int darwin_read_task_audit_token(
    mach_port_name_t task,
    audit_token_t *audit_token
) {
    mach_msg_type_number_t token_count = TASK_AUDIT_TOKEN_COUNT;
    const kern_return_t result = task_info(
        task,
        TASK_AUDIT_TOKEN,
        (task_info_t)audit_token,
        &token_count
    );
    if (result == KERN_SUCCESS && token_count == TASK_AUDIT_TOKEN_COUNT) return 1;
    if (result == KERN_INVALID_ARGUMENT || result == KERN_TERMINATED) return 0;
    if (result == KERN_RESOURCE_SHORTAGE || result == KERN_NO_SPACE) {
        errno = ENOMEM;
        return -1;
    }
    errno = EIO;
    return -1;
}

// Revalidate both the retained Mach task identity and its exact BSD PID
// version. exec can replace the task and increment p_idversion while keeping
// the same numeric PID and session. A vanished/old reference is therefore a
// retryable observation failure, never evidence that the current PID binding
// is non-live.
static int darwin_read_member_process_info(
    const struct orchbee_session_member_ref *member_ref,
    struct proc_bsdinfo *information
) {
    audit_token_t current_token = {0};
    const int task_result = darwin_read_task_audit_token(
        member_ref->task,
        &current_token
    );
    if (task_result == 0) {
        errno = EAGAIN;
        return -1;
    }
    if (task_result < 0) return -1;
    if (memcmp(&current_token, &member_ref->audit_token, sizeof(current_token)) != 0) {
        errno = EAGAIN;
        return -1;
    }

    const int pid = audit_token_to_pid(member_ref->audit_token);
    const int pid_version = audit_token_to_pidversion(member_ref->audit_token);
    const int length = __proc_info_extended_id(
        orchbee_proc_info_call_pidinfo,
        pid,
        PROC_PIDTBSDINFO,
        orchbee_pif_compare_idversion,
        orchbee_pidversion_external_id(pid_version),
        0,
        (uint64_t)(uintptr_t)information,
        (int32_t)sizeof(*information)
    );
    if (length < 0 && errno == ESRCH) {
        errno = EAGAIN;
        return -1;
    }
    if (length != (int)sizeof(*information)) {
        if (errno == 0) errno = EIO;
        return -1;
    }
    return 0;
}

static int acquire_session_member_ref(
    pid_t pid,
    struct orchbee_session_member_ref **member_ref
) {
    mach_port_name_t task = MACH_PORT_NULL;
    const kern_return_t task_result = task_name_for_pid(mach_task_self(), pid, &task);
    if (task_result != KERN_SUCCESS) {
        if (task_result == KERN_INVALID_ARGUMENT || task_result == KERN_TERMINATED) {
            return 0;
        }
        if (task_result == KERN_RESOURCE_SHORTAGE || task_result == KERN_NO_SPACE) {
            errno = ENOMEM;
            return -1;
        }
        // The owned-session candidate capability is the tracking boundary.
        // Any non-vanishing acquisition failure must stop the operation: skipping
        // an untracked candidate could omit a terminal member from signaling or
        // containment. No numeric-PID fallback is permitted.
        errno = EPERM;
        return -1;
    }

    audit_token_t audit_token = {0};
    const int token_result = darwin_read_task_audit_token(task, &audit_token);
    if (token_result != 1) {
        const int token_error = errno;
        (void)mach_port_deallocate(mach_task_self(), task);
        errno = token_error;
        return token_result;
    }

    struct orchbee_session_member_ref *reference = malloc(sizeof(*reference));
    if (reference == NULL) {
        const int allocation_error = errno;
        (void)mach_port_deallocate(mach_task_self(), task);
        errno = allocation_error;
        return -1;
    }
    reference->task = task;
    reference->audit_token = audit_token;
    *member_ref = reference;
    return 1;
}

static int iterator_next_pid(
    struct orchbee_session_member_iterator *iterator,
    pid_t *pid
) {
    while (iterator->index < iterator->count) {
        const pid_t candidate = iterator->pids[iterator->index];
        iterator->index += 1;
        if (candidate > 0) {
            *pid = candidate;
            return 1;
        }
    }
    return 0;
}

#elif defined(__linux__)

int orchbee_darwin_process_enumeration_scope(void) {
    return 0;
}

static int acquire_session_member_ref(
    pid_t pid,
    struct orchbee_session_member_ref **member_ref
) {
    const long opened = syscall(SYS_pidfd_open, pid, 0U);
    if (opened < 0) return errno == ESRCH ? 0 : -1;
    if (opened > INT_MAX) {
        (void)close((int)opened);
        errno = EOVERFLOW;
        return -1;
    }
    const int pidfd = (int)opened;
    struct orchbee_session_member_ref *reference = malloc(sizeof(*reference));
    if (reference == NULL) {
        const int allocation_error = errno;
        (void)close(pidfd);
        errno = allocation_error;
        return -1;
    }
    reference->pidfd = pidfd;
    reference->pid = pid;
    reference->session_id = 0;
    *member_ref = reference;
    return 1;
}

static int iterator_next_pid(
    struct orchbee_session_member_iterator *iterator,
    pid_t *pid
) {
    for (;;) {
        errno = 0;
        struct dirent *entry = readdir(iterator->directory);
        if (entry == NULL) return errno == 0 ? 0 : -1;
        char *end = NULL;
        errno = 0;
        const long parsed = strtol(entry->d_name, &end, 10);
        if (errno != 0 || end == entry->d_name || *end != '\0' ||
            parsed <= 0 || parsed > INT_MAX) {
            continue;
        }
        *pid = (pid_t)parsed;
        return 1;
    }
}

#endif

void orchbee_session_member_ref_close(void *raw_member_ref) {
    if (raw_member_ref == NULL) return;
    struct orchbee_session_member_ref *member_ref = raw_member_ref;
#if defined(__APPLE__)
    (void)mach_port_deallocate(mach_task_self(), member_ref->task);
#elif defined(__linux__)
    (void)close(member_ref->pidfd);
#endif
    free(member_ref);
}

int orchbee_session_member_iterator_open(int session_id, void **raw_iterator) {
    if (session_id <= 0 || raw_iterator == NULL) {
        errno = EINVAL;
        return -1;
    }
    struct orchbee_session_member_iterator *iterator = calloc(1, sizeof(*iterator));
    if (iterator == NULL) return -1;
    iterator->session_id = (pid_t)session_id;
#if defined(__APPLE__)
    if (darwin_list_processes(&iterator->pids, &iterator->count) != 0) {
        const int list_error = errno;
        free(iterator);
        errno = list_error;
        return -1;
    }
#elif defined(__linux__)
    iterator->directory = opendir("/proc");
    if (iterator->directory == NULL) {
        const int open_error = errno;
        free(iterator);
        errno = open_error;
        return -1;
    }
#endif
    *raw_iterator = iterator;
    return 0;
}

void orchbee_session_member_iterator_close(void *raw_iterator) {
    if (raw_iterator == NULL) return;
    struct orchbee_session_member_iterator *iterator = raw_iterator;
#if defined(__APPLE__)
    free(iterator->pids);
#elif defined(__linux__)
    (void)closedir(iterator->directory);
#endif
    free(iterator);
}

int orchbee_session_member_iterator_next(
    void *raw_iterator,
    void **raw_member_ref
) {
    if (raw_iterator == NULL || raw_member_ref == NULL) {
        errno = EINVAL;
        return -1;
    }
    struct orchbee_session_member_iterator *iterator = raw_iterator;
    *raw_member_ref = NULL;
    for (;;) {
        pid_t pid = 0;
        const int next_result = iterator_next_pid(iterator, &pid);
        if (next_result <= 0) return next_result;

#if defined(__APPLE__)
        // The all-process list includes unrelated sessions whose task-name
        // ports may be inaccessible. Use the reusable PID only as a negative
        // prefilter: a candidate clearly outside the owned session is skipped,
        // while a possible member must still acquire a stable Mach reference
        // before the authoritative membership observation below.
        errno = 0;
        const pid_t prefiltered_session = getsid(pid);
        if (prefiltered_session < 0) {
            const int prefilter_error = errno;
            if (orchbee_classify_session_membership_error(prefilter_error) == 0) {
                continue;
            }
            return -1;
        }
        if (prefiltered_session != iterator->session_id) continue;
#endif

        struct orchbee_session_member_ref *member_ref = NULL;
        const int acquire_result = acquire_session_member_ref(pid, &member_ref);
        if (acquire_result < 0) return -1;
        if (acquire_result == 0) {
#if defined(__APPLE__)
            // The negative prefilter already observed an owned-session
            // candidate. Losing it before stable acquisition may be exec or a
            // same-session PID replacement, so the whole scan must retry.
            errno = EAGAIN;
            return -1;
#else
            continue;
#endif
        }

        errno = 0;
        const pid_t observed_session = getsid(pid);
        if (observed_session < 0) {
            const int membership_error = errno;
            orchbee_session_member_ref_close(member_ref);
            if (orchbee_classify_session_membership_error(membership_error) == 0) {
#if defined(__APPLE__)
                // After a matched prefilter, ESRCH may be exec/task or
                // same-session PID ABA. It cannot become an omitted member.
                errno = EAGAIN;
                return -1;
#else
                continue;
#endif
            }
            return -1;
        }
        if (observed_session != iterator->session_id) {
            orchbee_session_member_ref_close(member_ref);
            continue;
        }

#if defined(__APPLE__)
        struct proc_bsdinfo information = {0};
        if (darwin_read_member_process_info(member_ref, &information) != 0) {
            const int binding_error = errno;
            orchbee_session_member_ref_close(member_ref);
            errno = binding_error;
            return -1;
        }
#elif defined(__linux__)
        member_ref->session_id = iterator->session_id;
#endif

        *raw_member_ref = member_ref;
        return 1;
    }
}

int orchbee_session_member_ref_signal(void *raw_member_ref, int signal_number) {
    if (raw_member_ref == NULL || signal_number <= 0) {
        errno = EINVAL;
        return -1;
    }
    struct orchbee_session_member_ref *member_ref = raw_member_ref;
#if defined(__APPLE__)
    const int signal_error = proc_signal_with_audittoken(
        &member_ref->audit_token,
        signal_number
    );
    if (signal_error == 0) return 0;
    errno = signal_error == ESRCH ? EAGAIN : signal_error;
    return -1;
#elif defined(__linux__)
    long result;
    do {
        errno = 0;
        result = syscall(
            SYS_pidfd_send_signal,
            member_ref->pidfd,
            signal_number,
            NULL,
            0U
        );
    } while (result < 0 && errno == EINTR);
    const int signal_error = result < 0 ? errno : 0;
    return orchbee_linux_classify_pidfd_signal((int)result, signal_error);
#endif
}

int orchbee_session_member_ref_is_live(void *raw_member_ref, int *live) {
    if (raw_member_ref == NULL || live == NULL) {
        errno = EINVAL;
        return -1;
    }
    struct orchbee_session_member_ref *member_ref = raw_member_ref;
#if defined(__APPLE__)
    struct proc_bsdinfo information = {0};
    if (darwin_read_member_process_info(member_ref, &information) != 0) return -1;
    *live = information.pbi_status != SZOMB;
    return 0;
#elif defined(__linux__)
    struct pollfd descriptor = {
        .fd = member_ref->pidfd,
        .events = POLLIN | POLLRDNORM,
        .revents = 0,
    };
    int result;
    do {
        result = poll(&descriptor, 1, 0);
    } while (result < 0 && errno == EINTR);
    const int poll_error = result < 0 ? errno : 0;
    const short known_events = POLLIN | POLLRDNORM | POLLHUP | POLLERR | POLLNVAL;
    const int action = orchbee_linux_classify_pidfd_poll(
        member_ref->pid == member_ref->session_id,
        result,
        poll_error,
        (descriptor.revents & (POLLIN | POLLRDNORM)) != 0,
        (descriptor.revents & POLLHUP) != 0,
        (descriptor.revents & POLLERR) != 0,
        (descriptor.revents & POLLNVAL) != 0,
        (descriptor.revents & (short)~known_events) != 0,
        live
    );
    if (action <= 0) return action;

    long probe_result;
    do {
        errno = 0;
        probe_result = syscall(
            SYS_pidfd_send_signal,
            member_ref->pidfd,
            0,
            NULL,
            0U
        );
    } while (probe_result < 0 && errno == EINTR);
    const int probe_error = probe_result < 0 ? errno : 0;
    return orchbee_linux_classify_pidfd_probe(
        (int)probe_result,
        probe_error,
        live
    );
#endif
}

#else

int orchbee_darwin_process_enumeration_scope(void) {
    return 0;
}

int orchbee_session_member_iterator_open(int session_id, void **raw_iterator) {
    (void)session_id;
    (void)raw_iterator;
    errno = ENOTSUP;
    return -1;
}

void orchbee_session_member_iterator_close(void *raw_iterator) {
    (void)raw_iterator;
}

int orchbee_session_member_iterator_next(
    void *raw_iterator,
    void **raw_member_ref
) {
    (void)raw_iterator;
    (void)raw_member_ref;
    errno = ENOTSUP;
    return -1;
}

void orchbee_session_member_ref_close(void *raw_member_ref) {
    (void)raw_member_ref;
}

int orchbee_session_member_ref_signal(void *raw_member_ref, int signal_number) {
    (void)raw_member_ref;
    (void)signal_number;
    errno = ENOTSUP;
    return -1;
}

int orchbee_session_member_ref_is_live(void *raw_member_ref, int *live) {
    (void)raw_member_ref;
    (void)live;
    errno = ENOTSUP;
    return -1;
}

#endif

#if defined(__FreeBSD__)

static int freebsd_reaper_is_owned(void) {
    struct procctl_reaper_status status = {0};
    if (procctl(P_PID, (id_t)getpid(), PROC_REAP_STATUS, &status) != 0) return -1;
    return (status.rs_flags & REAPER_STATUS_OWNED) != 0 &&
        status.rs_reaper == getpid();
}

int orchbee_prepare_process_tracking(void) {
    const int owned = freebsd_reaper_is_owned();
    if (owned < 0) return -1;
    if (owned != 0) return 0;
    if (procctl(P_PID, (id_t)getpid(), PROC_REAP_ACQUIRE, NULL) != 0 &&
        errno != EBUSY) {
        return -1;
    }
    const int acquired = freebsd_reaper_is_owned();
    if (acquired > 0) return 0;
    if (acquired == 0) errno = EPERM;
    return -1;
}

struct freebsd_reaper_snapshot {
    struct procctl_reaper_pidinfo *members;
    size_t capacity;
};

static void freebsd_snapshot_close(struct freebsd_reaper_snapshot *snapshot) {
    free(snapshot->members);
    snapshot->members = NULL;
    snapshot->capacity = 0;
}

static int freebsd_reaper_snapshot(struct freebsd_reaper_snapshot *snapshot) {
    struct procctl_reaper_status status = {0};
    if (procctl(P_PID, (id_t)getpid(), PROC_REAP_STATUS, &status) != 0) return -1;
    if ((status.rs_flags & REAPER_STATUS_OWNED) == 0 ||
        status.rs_reaper != getpid()) {
        errno = EPERM;
        return -1;
    }
    size_t capacity = (size_t)status.rs_descendants + 16;
    if (capacity < 16 || capacity > UINT_MAX) {
        errno = EOVERFLOW;
        return -1;
    }

    for (;;) {
        struct procctl_reaper_pidinfo *members = calloc(capacity, sizeof(*members));
        if (members == NULL) return -1;
        struct procctl_reaper_pids request = {
            .rp_count = (u_int)capacity,
            .rp_pids = members,
        };
        if (procctl(P_PID, (id_t)getpid(), PROC_REAP_GETPIDS, &request) != 0) {
            const int query_error = errno;
            free(members);
            errno = query_error;
            return -1;
        }
        size_t valid_count = 0;
        while (valid_count < capacity &&
            (members[valid_count].pi_flags & REAPER_PIDINFO_VALID) != 0) {
            valid_count += 1;
        }
        if (valid_count < capacity) {
            snapshot->members = members;
            snapshot->capacity = valid_count;
            return 0;
        }
        free(members);
        if (capacity > UINT_MAX / 2) {
            errno = EOVERFLOW;
            return -1;
        }
        capacity *= 2;
    }
}

static int freebsd_reap_subtree_zombies(
    const struct freebsd_reaper_snapshot *snapshot,
    pid_t subtree
) {
    for (size_t index = 0; index < snapshot->capacity; ++index) {
        const struct procctl_reaper_pidinfo *member = &snapshot->members[index];
        if (member->pi_subtree != subtree ||
            !orchbee_freebsd_member_is_targeted_zombie(
                member->pi_pid,
                subtree,
                (member->pi_flags & REAPER_PIDINFO_CHILD) != 0,
                (member->pi_flags & REAPER_PIDINFO_ZOMBIE) != 0
            )) {
            continue;
        }
        pid_t reaped;
        do {
            reaped = waitpid(member->pi_pid, NULL, WNOHANG);
        } while (reaped < 0 && errno == EINTR);
        if (reaped != member->pi_pid) {
            if (reaped == 0 || errno == 0) errno = EAGAIN;
            return -1;
        }
    }
    return 0;
}

int orchbee_signal_process_session(int session_id, int signal_number) {
    if (session_id <= 0 || signal_number <= 0) {
        errno = EINVAL;
        return -1;
    }
    if (orchbee_prepare_process_tracking() != 0) return -1;
    struct procctl_reaper_kill request = {
        .rk_sig = signal_number,
        .rk_flags = REAPER_KILL_SUBTREE,
        .rk_subtree = (pid_t)session_id,
        .rk_killed = 0,
        .rk_fpid = -1,
    };
    if (procctl(P_PID, (id_t)getpid(), PROC_REAP_KILL, &request) != 0) {
        return errno == ESRCH ? 0 : -1;
    }
    // FreeBSD can complete PROC_REAP_KILL while reporting a partial signaling
    // failure through rk_fpid. Preserve that as a typed error so force KILL
    // enters the full-subtree CONT compensation path.
    if (request.rk_fpid != -1) {
        errno = EPERM;
        return -1;
    }
    return 0;
}

int orchbee_process_session_member_pass(
    int session_id,
    uint32_t *live_members,
    int *targeted_zombies_remain
) {
    if (session_id <= 0 || live_members == NULL ||
        targeted_zombies_remain == NULL) {
        errno = EINVAL;
        return -1;
    }
    if (orchbee_prepare_process_tracking() != 0) return -1;

    struct freebsd_reaper_snapshot before = {0};
    if (freebsd_reaper_snapshot(&before) != 0) return -1;
    const int reap_result = freebsd_reap_subtree_zombies(
        &before,
        (pid_t)session_id
    );
    freebsd_snapshot_close(&before);
    if (reap_result != 0) return -1;

    struct freebsd_reaper_snapshot after = {0};
    if (freebsd_reaper_snapshot(&after) != 0) return -1;
    uint32_t live_count = 0;
    int zombies_remain = 0;
    for (size_t index = 0; index < after.capacity; ++index) {
        const struct procctl_reaper_pidinfo *member = &after.members[index];
        if (member->pi_subtree != (pid_t)session_id) continue;
        if ((member->pi_flags & REAPER_PIDINFO_ZOMBIE) != 0) {
            if (orchbee_freebsd_member_is_targeted_zombie(
                member->pi_pid,
                (pid_t)session_id,
                (member->pi_flags & REAPER_PIDINFO_CHILD) != 0,
                1
            )) zombies_remain = 1;
        } else {
            if (live_count == UINT32_MAX) {
                freebsd_snapshot_close(&after);
                errno = EOVERFLOW;
                return -1;
            }
            live_count += 1;
        }
    }
    freebsd_snapshot_close(&after);
    *live_members = live_count;
    *targeted_zombies_remain = zombies_remain;
    return 0;
}

#else

int orchbee_prepare_process_tracking(void) {
    return 0;
}

int orchbee_signal_process_session(int session_id, int signal_number) {
    (void)session_id;
    (void)signal_number;
    errno = ENOTSUP;
    return -1;
}

int orchbee_process_session_member_pass(
    int session_id,
    uint32_t *live_members,
    int *targeted_zombies_remain
) {
    (void)session_id;
    (void)live_members;
    (void)targeted_zombies_remain;
    errno = ENOTSUP;
    return -1;
}

#endif

int orchbee_try_observe_process_exit_now(
    int process_id,
    int *result_kind,
    uint32_t *result_value
);

#if defined(__APPLE__)

// Darwin's waitid may report CLD_STOPPED even when the caller requests only
// WEXITED. Waiting for NOTE_EXIT first prevents a force-termination SIGSTOP
// from being mistaken for child completion while retaining the zombie for the
// later containment-confirmed reap. EVFILT_PROC registration may return ESRCH
// once XNU has drained the process reference but before the child is waitable.
// A successful registration can also lose NOTE_EXIT across XNU process-reference
// drain races. The audit timeout closes the kqueue before observing again so
// its retained process reference cannot itself prevent proc_refdrain from
// making the child waitable. A still-running child is then re-registered. The
// caller follows a witnessed exit, an already-waitable child, and registration
// ESRCH with the same bounded non-blocking waitid observation loop.
enum {
    darwin_wait_ready = 0,
    darwin_wait_audit_timeout = 1,
};

static int darwin_wait_for_process_exit(pid_t process_id) {
    const int queue = kqueue();
    if (queue < 0) return -1;

    struct kevent change;
    EV_SET(
        &change,
        (uintptr_t)process_id,
        EVFILT_PROC,
        EV_ADD | EV_ENABLE | EV_ONESHOT,
        NOTE_EXIT,
        0,
        NULL
    );
    int result;
    do {
        result = kevent(queue, &change, 1, NULL, 0, NULL);
    } while (result < 0 && errno == EINTR);
    if (result < 0) {
        const int registration_error = errno;
        (void)close(queue);
        if (registration_error == ESRCH) return 0;
        errno = registration_error == 0 ? EIO : registration_error;
        return -1;
    }

    for (;;) {
        int immediate_kind = 0;
        uint32_t immediate_value = 0;
        const int immediate_error = orchbee_try_observe_process_exit_now(
            process_id,
            &immediate_kind,
            &immediate_value
        );
        if (immediate_error == 0) {
            (void)close(queue);
            return 0;
        }
        if (immediate_error != EAGAIN) {
            (void)close(queue);
            errno = immediate_error;
            return -1;
        }

        const struct timespec audit_interval = {
            .tv_sec = 1,
            .tv_nsec = 0,
        };
        struct kevent event = {0};
        result = kevent(queue, NULL, 0, &event, 1, &audit_interval);
        if (result < 0 && errno == EINTR) continue;
        if (result < 0) {
            const int wait_error = errno;
            (void)close(queue);
            errno = wait_error == 0 ? EIO : wait_error;
            return -1;
        }
        if (result == 0) {
            (void)close(queue);
            return darwin_wait_audit_timeout;
        }
        if (result != 1) {
            (void)close(queue);
            errno = EIO;
            return -1;
        }
        if ((event.flags & EV_ERROR) != 0) {
            (void)close(queue);
            errno = event.data == 0 ? EIO : (int)event.data;
            return -1;
        }
        if ((event.fflags & NOTE_EXIT) == 0) {
            (void)close(queue);
            errno = EIO;
            return -1;
        }
        (void)close(queue);
        return darwin_wait_ready;
    }
}

#endif

int orchbee_try_observe_process_exit_now(
    int process_id,
    int *result_kind,
    uint32_t *result_value
) {
    if (process_id <= 0 || result_kind == NULL || result_value == NULL) {
        return EINVAL;
    }
    siginfo_t information = {0};
    while (waitid(
        P_PID,
        (id_t)process_id,
        &information,
        WEXITED | WNOWAIT | WNOHANG
    ) != 0) {
        if (errno == EINTR) continue;
        return errno == 0 ? EIO : errno;
    }
    if (information.si_pid == 0) return EAGAIN;
    switch (information.si_code) {
        case CLD_EXITED:
            *result_kind = 1;
            *result_value = (uint32_t)information.si_status;
            return 0;
        case CLD_KILLED:
        case CLD_DUMPED:
            *result_kind = 2;
            *result_value = (uint32_t)information.si_status;
            return 0;
        default:
            // Darwin can surface CLD_STOPPED despite WEXITED. A non-exit
            // status is never completion and remains retryable.
            return EAGAIN;
    }
}

int orchbee_observe_process_exit(
    int process_id,
    int *result_kind,
    uint32_t *result_value
) {
    if (process_id <= 0 || result_kind == NULL || result_value == NULL) {
        return EINVAL;
    }
#if defined(__APPLE__)
    // EVFILT_PROC registration can race proc_refdrain before waitid considers
    // the child waitable. Never fall back to blocking waitid. A witnessed exit
    // or registration ESRCH gets the full fixed retry bound. An audit timeout
    // closes the kqueue first, gives proc_refdrain one millisecond to advance,
    // and re-registers if the child is still running.
    enum {
        darwin_waitid_retry_limit = 1000,
        darwin_audit_retry_limit = 2,
    };
    const struct timespec retry_delay = {
        .tv_sec = 0,
        .tv_nsec = 1000000,
    };
    for (;;) {
        const int wait_result = darwin_wait_for_process_exit((pid_t)process_id);
        if (wait_result < 0) return errno == 0 ? EIO : errno;
        const size_t retry_limit = wait_result == darwin_wait_audit_timeout ?
            darwin_audit_retry_limit : darwin_waitid_retry_limit;
        for (size_t attempt = 0; attempt < retry_limit; ++attempt) {
            const int observation_error = orchbee_try_observe_process_exit_now(
                process_id,
                result_kind,
                result_value
            );
            if (observation_error == 0) return 0;
            if (observation_error != EAGAIN) return observation_error;
            if (attempt + 1 < retry_limit) {
                struct timespec remaining = retry_delay;
                while (nanosleep(&remaining, &remaining) != 0 && errno == EINTR) {}
            }
        }
        if (wait_result != darwin_wait_audit_timeout) return EAGAIN;
    }
#else
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
            return EIO;
    }
    return 0;
#endif
}

int orchbee_reap_process_exit(int process_id) {
    if (process_id <= 0) return EINVAL;
    int status = 0;
    pid_t reaped;
    do {
        reaped = waitpid((pid_t)process_id, &status, WNOHANG);
    } while (reaped < 0 && errno == EINTR);
    if (reaped == 0) return EAGAIN;
    if (reaped < 0) {
        return errno == 0 ? ECHILD : errno;
    }
    if (reaped != (pid_t)process_id) return ECHILD;
    return 0;
}
