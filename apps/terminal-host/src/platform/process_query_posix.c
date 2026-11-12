#if defined(__linux__)
#define _GNU_SOURCE
#endif

#include <errno.h>
#include <limits.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <time.h>
#include <unistd.h>

#if defined(__APPLE__)
#include <libproc.h>
#include <sys/param.h>
#include <sys/proc.h>
#include <sys/proc_info.h>
#include <sys/sysctl.h>
#elif defined(__linux__)
#include <dirent.h>
#include <fcntl.h>
#else
#error "terminal process samples are implemented for Linux and macOS only"
#endif

enum {
    OB_PROCESS_LIMIT = 256,
    OB_PATH_BYTES = 4096,
    /* Three UTF-8 bytes per UTF-16 unit at most: longer argv text cannot fit the
     * 4096-unit text bound, so it is unavailable rather than truncated. */
    OB_ARGUMENT_BYTES = 3 * 4096,
};
#define OB_ABSENT 0xffffffffu

/* This fixed ABI is mirrored and asserted in process_query_posix.zig. */
struct ob_posix_process_facts {
    /* Linux: clock ticks since boot. macOS: microseconds since the Unix epoch. */
    uint64_t start;
    uint64_t terminal;
    uint32_t parent_pid;
    /* 0: no controlling terminal; 1: `terminal` names it. */
    uint32_t terminal_present;
    uint32_t image_length;
    uint32_t arguments_length;
    uint32_t directory_length;
    uint32_t reserved;
    char image[OB_PATH_BYTES];
    /* Each argument followed by one NUL. */
    char arguments[OB_ARGUMENT_BYTES];
    char directory[OB_PATH_BYTES];
};
_Static_assert(offsetof(struct ob_posix_process_facts, image) == 40, "process facts ABI");
_Static_assert(offsetof(struct ob_posix_process_facts, arguments) == 4136, "process facts ABI");
_Static_assert(offsetof(struct ob_posix_process_facts, directory) == 16424, "process facts ABI");
_Static_assert(sizeof(struct ob_posix_process_facts) == 20520, "process facts size");

enum { OB_FACTS_GONE = 0, OB_FACTS_READ = 1, OB_FACTS_UNAVAILABLE = 2 };

static void reset_facts(struct ob_posix_process_facts *facts) {
    facts->start = 0;
    facts->terminal = 0;
    facts->parent_pid = 0;
    facts->terminal_present = 0;
    facts->image_length = OB_ABSENT;
    facts->arguments_length = OB_ABSENT;
    facts->directory_length = OB_ABSENT;
    facts->reserved = 0;
}

/* A program that execs between the two image reads may pair its old arguments
 * with its new image. The later image wins and the arguments become unavailable,
 * never a mixed or reconstructed command. */
static void settle_image(struct ob_posix_process_facts *facts, const char *later, uint32_t later_length) {
    if (later_length == facts->image_length &&
            (later_length == OB_ABSENT || memcmp(later, facts->image, later_length) == 0)) {
        return;
    }
    facts->image_length = later_length;
    if (later_length != OB_ABSENT) memcpy(facts->image, later, later_length);
    facts->arguments_length = OB_ABSENT;
}

/* Session membership is the containment domain the Host already owns: the root
 * leads a new session, and only a member that calls setsid leaves it. The PID
 * scan is a reusable-ID prefilter; each candidate is confirmed again when its
 * facts are read. A vanished candidate is skipped; any other membership error
 * leaves membership unknown and fails the sample. Return 1 with every candidate,
 * 2 when they exceed the row bound, or 0 when the scan is unavailable. */
static int add_candidate(pid_t session, pid_t pid, int32_t *pids, uint32_t *count) {
    errno = 0;
    const pid_t observed = getsid(pid);
    if (observed < 0) return errno == ESRCH ? 1 : 0;
    if (observed != session) return 1;
    if (*count == OB_PROCESS_LIMIT) return 2;
    pids[*count] = (int32_t)pid;
    *count += 1;
    return 1;
}

#if defined(__linux__)

/* A /proc mounted for another PID namespace would name other processes than
 * getsid and the Host's own PIDs do. */
static int proc_matches_namespace(void) {
    char link[32] = {0};
    const ssize_t length = readlink("/proc/self", link, sizeof(link) - 1);
    if (length <= 0) return 0;
    char *end = NULL;
    errno = 0;
    const long value = strtol(link, &end, 10);
    return errno == 0 && end == link + length && value == (long)getpid();
}

int ob_posix_session_members(int session, int32_t *pids, uint32_t *count) {
    *count = 0;
    if (session <= 0 || !proc_matches_namespace()) return 0;
    DIR *directory = opendir("/proc");
    if (directory == NULL) return 0;
    int result = 1;
    for (;;) {
        errno = 0;
        const struct dirent *entry = readdir(directory);
        if (entry == NULL) {
            if (errno != 0) result = 0;
            break;
        }
        char *end = NULL;
        errno = 0;
        const long parsed = strtol(entry->d_name, &end, 10);
        if (errno != 0 || end == entry->d_name || *end != '\0' || parsed <= 0 || parsed > INT_MAX) continue;
        result = add_candidate((pid_t)session, (pid_t)parsed, pids, count);
        if (result != 1) break;
    }
    (void)closedir(directory);
    return result;
}

struct linux_stat {
    char state;
    long long parent_pid;
    long long session;
    long long terminal;
    unsigned long long start;
};

/* Read through the member's /proc directory descriptor: it stays bound to that
 * process, so a reaped member reads as gone rather than as a PID's next owner.
 * The command name is free text and may hold spaces and parentheses; the fields
 * follow its last closing parenthesis. Return 1 read, 0 gone, -1 with errno. */
static int linux_read_stat(int directory, struct linux_stat *stat) {
    const int file = openat(directory, "stat", O_RDONLY | O_CLOEXEC);
    if (file < 0) return errno == ENOENT || errno == ESRCH ? 0 : -1;
    char text[2048];
    size_t length = 0;
    for (;;) {
        const ssize_t read_length = read(file, text + length, sizeof(text) - 1 - length);
        if (read_length < 0 && errno == EINTR) continue;
        if (read_length < 0) {
            const int read_error = errno;
            (void)close(file);
            if (read_error == ESRCH) return 0;
            errno = read_error;
            return -1;
        }
        if (read_length == 0) break;
        length += (size_t)read_length;
        if (length == sizeof(text) - 1) break;
    }
    (void)close(file);
    text[length] = '\0';
    const char *close_paren = strrchr(text, ')');
    if (close_paren == NULL) {
        errno = EIO;
        return -1;
    }
    /* state ppid pgrp session tty_nr, 14 skipped fields, starttime (field 22). */
    const int fields = sscanf(close_paren + 1,
        " %c %lld %*s %lld %lld %*s %*s %*s %*s %*s %*s %*s %*s %*s %*s %*s %*s %*s %*s %llu",
        &stat->state, &stat->parent_pid, &stat->session, &stat->terminal, &stat->start);
    if (fields != 5) {
        errno = EIO;
        return -1;
    }
    return 1;
}

static uint32_t linux_read_link(int directory, const char *name, char *out) {
    const ssize_t length = readlinkat(directory, name, out, OB_PATH_BYTES);
    if (length <= 0 || length >= OB_PATH_BYTES) return OB_ABSENT;
    return (uint32_t)length;
}

/* The kernel copies at most a bounded prefix; text that fills the buffer is
 * oversize and therefore unavailable. */
static uint32_t linux_read_arguments(int directory, char *out) {
    const int file = openat(directory, "cmdline", O_RDONLY | O_CLOEXEC);
    if (file < 0) return OB_ABSENT;
    size_t length = 0;
    for (;;) {
        char overflow = 0;
        char *target = length < OB_ARGUMENT_BYTES ? out + length : &overflow;
        const size_t room = length < OB_ARGUMENT_BYTES ? OB_ARGUMENT_BYTES - length : 1;
        const ssize_t read_length = read(file, target, room);
        if (read_length < 0 && errno == EINTR) continue;
        if (read_length < 0 || (read_length > 0 && length == OB_ARGUMENT_BYTES)) {
            (void)close(file);
            return OB_ABSENT;
        }
        if (read_length == 0) break;
        length += (size_t)read_length;
    }
    (void)close(file);
    return (uint32_t)length;
}

static int live_member(const struct linux_stat *stat, pid_t session) {
    return stat->session == session && stat->state != 'Z' && stat->state != 'X';
}

/* Return 1 with the facts of a live member, 0 when the PID is gone or no longer
 * a member, or 2 when a member's identity cannot be read (for example under
 * hidepid). The identity comes from stat, which is world-readable without
 * hidepid; the image, arguments, and directory need ptrace read access, and a
 * member that denies the image is an opaque row. A final stat through the same
 * descriptor drops a member that exited while it was read. */
int ob_posix_process_facts(int session, int pid, struct ob_posix_process_facts *facts, char *scratch, size_t scratch_bytes) {
    (void)scratch;
    (void)scratch_bytes;
    reset_facts(facts);
    char path[32];
    (void)snprintf(path, sizeof(path), "/proc/%d", pid);
    const int directory = open(path, O_RDONLY | O_DIRECTORY | O_CLOEXEC);
    if (directory < 0) return errno == ENOENT || errno == ESRCH ? OB_FACTS_GONE : OB_FACTS_UNAVAILABLE;
    struct linux_stat first = {0};
    int status = linux_read_stat(directory, &first);
    if (status <= 0 || !live_member(&first, (pid_t)session)) {
        (void)close(directory);
        return status < 0 ? OB_FACTS_UNAVAILABLE : OB_FACTS_GONE;
    }
    facts->start = first.start;
    facts->parent_pid = first.parent_pid > 0 && first.parent_pid <= INT_MAX ? (uint32_t)first.parent_pid : 0;
    facts->terminal_present = first.terminal != 0;
    facts->terminal = (uint64_t)first.terminal;
    facts->image_length = linux_read_link(directory, "exe", facts->image);
    if (facts->image_length != OB_ABSENT) {
        facts->arguments_length = linux_read_arguments(directory, facts->arguments);
        facts->directory_length = linux_read_link(directory, "cwd", facts->directory);
        char later[OB_PATH_BYTES];
        settle_image(facts, later, linux_read_link(directory, "exe", later));
    }
    struct linux_stat last = {0};
    status = linux_read_stat(directory, &last);
    (void)close(directory);
    if (status < 0) return OB_FACTS_UNAVAILABLE;
    if (status == 0 || !live_member(&last, (pid_t)session) || last.start != first.start) return OB_FACTS_GONE;
    return OB_FACTS_READ;
}

size_t ob_posix_argument_scratch_bytes(void) {
    return 0;
}

/* Linux records process start in clock ticks since boot (CLOCK_BOOTTIME). The
 * wall-clock time of boot is the realtime reading minus the boottime reading;
 * reading realtime first errs early, never late, and the closest of three pairs
 * keeps a preempted pair from widening that error. */
int ob_posix_boot_time_ns(int64_t *nanoseconds) {
    int64_t best = INT64_MIN;
    for (int attempt = 0; attempt < 3; ++attempt) {
        struct timespec real = {0}, boot = {0};
        if (clock_gettime(CLOCK_REALTIME, &real) != 0 || clock_gettime(CLOCK_BOOTTIME, &boot) != 0) return -1;
        const int64_t value = ((int64_t)real.tv_sec - (int64_t)boot.tv_sec) * 1000000000 +
            ((int64_t)real.tv_nsec - (int64_t)boot.tv_nsec);
        if (value > best) best = value;
    }
    *nanoseconds = best;
    return 0;
}

long ob_posix_clock_ticks_per_second(void) {
    return sysconf(_SC_CLK_TCK);
}

#elif defined(__APPLE__)

int ob_posix_session_members(int session, int32_t *pids, uint32_t *count) {
    *count = 0;
    if (session <= 0) return 0;
    const int byte_capacity = proc_listpids(PROC_ALL_PIDS, 0, NULL, 0);
    if (byte_capacity <= 0 || byte_capacity > INT_MAX - (int)(64 * sizeof(pid_t))) return 0;
    const int capacity = byte_capacity + (int)(64 * sizeof(pid_t));
    pid_t *all = calloc((size_t)capacity / sizeof(pid_t), sizeof(pid_t));
    if (all == NULL) return 0;
    const int byte_count = proc_listpids(PROC_ALL_PIDS, 0, all, capacity);
    /* A full buffer may have dropped processes started since the size query. */
    if (byte_count <= 0 || byte_count >= capacity || byte_count % (int)sizeof(pid_t) != 0) {
        free(all);
        return 0;
    }
    int result = 1;
    for (size_t index = 0; index < (size_t)byte_count / sizeof(pid_t) && result == 1; ++index) {
        if (all[index] > 0) result = add_candidate((pid_t)session, all[index], pids, count);
    }
    free(all);
    return result;
}

/* Public kinfo_proc facts are readable for every user's process. Return 1 read,
 * 0 when no live process has the PID, or -1. */
static int darwin_kinfo(pid_t pid, struct kinfo_proc *info) {
    int name[4] = {CTL_KERN, KERN_PROC, KERN_PROC_PID, pid};
    size_t size = sizeof(*info);
    memset(info, 0, sizeof(*info));
    if (sysctl(name, 4, info, &size, NULL, 0) != 0) return errno == ESRCH ? 0 : -1;
    if (size == 0) return 0;
    if (size != sizeof(*info) || info->kp_proc.p_pid != pid) {
        errno = EIO;
        return -1;
    }
    return info->kp_proc.p_stat == SZOMB ? 0 : 1;
}

static uint32_t darwin_image(pid_t pid, char *out) {
    char path[PROC_PIDPATHINFO_MAXSIZE];
    const int length = proc_pidpath(pid, path, sizeof(path));
    if (length <= 0 || length >= OB_PATH_BYTES) return OB_ABSENT;
    memcpy(out, path, (size_t)length);
    return (uint32_t)length;
}

/* KERN_PROCARGS2 holds argc, the exec path, NUL padding, then argv. It needs the
 * member's own user (or root); otherwise the arguments are unavailable. A
 * malformed or oversize area is unavailable too. */
static uint32_t darwin_arguments(pid_t pid, char *scratch, size_t scratch_bytes, char *out) {
    if (scratch == NULL || scratch_bytes <= sizeof(int)) return OB_ABSENT;
    int name[3] = {CTL_KERN, KERN_PROCARGS2, pid};
    size_t size = scratch_bytes;
    if (sysctl(name, 3, scratch, &size, NULL, 0) != 0 || size <= sizeof(int) || size > scratch_bytes) return OB_ABSENT;
    int argc = 0;
    memcpy(&argc, scratch, sizeof(argc));
    if (argc < 0) return OB_ABSENT;
    const char *cursor = scratch + sizeof(argc);
    const char *end = scratch + size;
    while (cursor < end && *cursor != '\0') ++cursor;
    while (cursor < end && *cursor == '\0') ++cursor;
    size_t length = 0;
    for (int index = 0; index < argc; ++index) {
        const char *terminator = memchr(cursor, '\0', (size_t)(end - cursor));
        if (terminator == NULL) return OB_ABSENT;
        const size_t argument = (size_t)(terminator - cursor) + 1;
        if (argument > OB_ARGUMENT_BYTES - length) return OB_ABSENT;
        memcpy(out + length, cursor, argument);
        length += argument;
        cursor = terminator + 1;
    }
    return (uint32_t)length;
}

static uint32_t darwin_directory(pid_t pid, char *out) {
    struct proc_vnodepathinfo information;
    memset(&information, 0, sizeof(information));
    if (proc_pidinfo(pid, PROC_PIDVNODEPATHINFO, 0, &information, (int)sizeof(information)) != (int)sizeof(information)) {
        return OB_ABSENT;
    }
    const char *path = information.pvi_cdir.vip_path;
    const size_t length = strnlen(path, sizeof(information.pvi_cdir.vip_path));
    if (length == 0 || length >= sizeof(information.pvi_cdir.vip_path) || length >= OB_PATH_BYTES) return OB_ABSENT;
    memcpy(out, path, length);
    return (uint32_t)length;
}

/* Return 1 with the facts of a live member, 0 when the PID is gone, reused, or
 * no longer a member, or 2 on an observation failure. The start time read before
 * and after every other fact binds them to one process: a reused PID has a later
 * start. The image comes from proc_pidpath, which any user may read; arguments
 * and directory need the member's own user. A member whose image cannot be read
 * is an opaque row. */
int ob_posix_process_facts(int session, int pid, struct ob_posix_process_facts *facts, char *scratch, size_t scratch_bytes) {
    reset_facts(facts);
    struct kinfo_proc first;
    int status = darwin_kinfo((pid_t)pid, &first);
    if (status <= 0) return status < 0 ? OB_FACTS_UNAVAILABLE : OB_FACTS_GONE;
    errno = 0;
    const pid_t observed = getsid((pid_t)pid);
    if (observed < 0) return errno == ESRCH ? OB_FACTS_GONE : OB_FACTS_UNAVAILABLE;
    if (observed != (pid_t)session) return OB_FACTS_GONE;
    const struct timeval started = first.kp_proc.p_starttime;
    if (started.tv_sec < 0 || started.tv_usec < 0 || started.tv_usec >= 1000000) return OB_FACTS_UNAVAILABLE;
    facts->start = (uint64_t)started.tv_sec * 1000000u + (uint64_t)started.tv_usec;
    facts->parent_pid = first.kp_eproc.e_ppid > 0 ? (uint32_t)first.kp_eproc.e_ppid : 0;
    facts->terminal_present = first.kp_eproc.e_tdev != NODEV;
    facts->terminal = (uint64_t)(uint32_t)first.kp_eproc.e_tdev;
    facts->image_length = darwin_image((pid_t)pid, facts->image);
    if (facts->image_length != OB_ABSENT) {
        facts->arguments_length = darwin_arguments((pid_t)pid, scratch, scratch_bytes, facts->arguments);
        facts->directory_length = darwin_directory((pid_t)pid, facts->directory);
        char later[OB_PATH_BYTES];
        settle_image(facts, later, darwin_image((pid_t)pid, later));
    }
    struct kinfo_proc last;
    status = darwin_kinfo((pid_t)pid, &last);
    if (status < 0) return OB_FACTS_UNAVAILABLE;
    if (status == 0 || last.kp_proc.p_starttime.tv_sec != started.tv_sec ||
            last.kp_proc.p_starttime.tv_usec != started.tv_usec) {
        return OB_FACTS_GONE;
    }
    return OB_FACTS_READ;
}

/* One reusable KERN_PROCARGS2 buffer per sample, sized like ps sizes it. */
size_t ob_posix_argument_scratch_bytes(void) {
    int name[2] = {CTL_KERN, KERN_ARGMAX};
    int maximum = 0;
    size_t size = sizeof(maximum);
    if (sysctl(name, 2, &maximum, &size, NULL, 0) != 0 || maximum <= 0 || maximum > (16 << 20)) return 0;
    return (size_t)maximum;
}

int ob_posix_boot_time_ns(int64_t *nanoseconds) {
    *nanoseconds = 0;
    errno = ENOTSUP;
    return -1;
}

long ob_posix_clock_ticks_per_second(void) {
    return 0;
}

#endif
