// The Keeper's macOS and Linux system calls, shared with the Host's Keeper link: descriptor passing
// over a Unix stream socket, where one frame's bytes leave with the descriptors its message carries
// by SCM_RIGHTS and a read returns the descriptors that arrived with its bytes; the peer's process
// id; a process's start; a PTY master's slave name; a pipe end's direction; the descriptor limit;
// and the channel and child of an update's check run.

#if defined(__linux__)
#define _GNU_SOURCE
#endif

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <spawn.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/resource.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/un.h>
#include <unistd.h>

#if defined(__APPLE__)
#include <sys/sysctl.h>
#include <sys/ttycom.h>
#elif !defined(__linux__)
#error "descriptor passing for the Keeper is implemented for macOS and Linux only"
#endif

extern char **environ;

enum {
    // The most descriptors one message carries, as the Keeper contract bounds it.
    OB_DESCRIPTORS_PER_MESSAGE = 64,
};

// A write to a closed peer fails with EPIPE instead of raising SIGPIPE.
#define OB_SEND_FLAGS (MSG_NOSIGNAL | MSG_DONTWAIT)

// Linux marks each descriptor that arrives close-on-exec as it arrives, so no child another thread
// starts meanwhile inherits it; macOS marks it once the read returns.
#if defined(__linux__)
#define OB_RECEIVE_FLAGS (MSG_DONTWAIT | MSG_CMSG_CLOEXEC)
#else
#define OB_RECEIVE_FLAGS MSG_DONTWAIT
#endif

// Sends up to `length` bytes, with `count` descriptors attached to the first byte when `count` is
// not zero. Answers 0 and the bytes sent, or an errno; EAGAIN when the socket takes nothing now.
int orchbee_send_with_descriptors(
    int socket_fd,
    const void *bytes,
    size_t length,
    const int *descriptors,
    size_t count,
    size_t *sent) {
    *sent = 0;
    if (length == 0 || count > OB_DESCRIPTORS_PER_MESSAGE) return EINVAL;
    struct iovec vector = {.iov_base = (void *)bytes, .iov_len = length};
    union {
        char buffer[CMSG_SPACE(sizeof(int) * OB_DESCRIPTORS_PER_MESSAGE)];
        struct cmsghdr align;
    } control;
    struct msghdr header;
    memset(&header, 0, sizeof(header));
    header.msg_iov = &vector;
    header.msg_iovlen = 1;
    if (count > 0) {
        memset(&control, 0, sizeof(control));
        header.msg_control = control.buffer;
        header.msg_controllen = (socklen_t)CMSG_SPACE(sizeof(int) * count);
        struct cmsghdr *message = CMSG_FIRSTHDR(&header);
        message->cmsg_level = SOL_SOCKET;
        message->cmsg_type = SCM_RIGHTS;
        message->cmsg_len = (socklen_t)CMSG_LEN(sizeof(int) * count);
        memcpy(CMSG_DATA(message), descriptors, sizeof(int) * count);
    }
    for (;;) {
        const ssize_t written = sendmsg(socket_fd, &header, OB_SEND_FLAGS);
        if (written >= 0) {
            *sent = (size_t)written;
            return 0;
        }
        if (errno != EINTR) return errno;
    }
}

// Reads up to `capacity` bytes and every descriptor that arrived with them, each marked
// close-on-exec, into `descriptors`, which holds `descriptor_capacity`. Answers 0 with the byte and
// descriptor counts and whether the kernel truncated the control data, which closes what did not
// fit; or an errno. A zero byte count is the peer's close.
int orchbee_receive_with_descriptors(
    int socket_fd,
    void *bytes,
    size_t capacity,
    int *descriptors,
    size_t descriptor_capacity,
    size_t *read_length,
    size_t *descriptor_count,
    int *truncated) {
    *read_length = 0;
    *descriptor_count = 0;
    *truncated = 0;
    if (capacity == 0) return EINVAL;
    struct iovec vector = {.iov_base = bytes, .iov_len = capacity};
    union {
        char buffer[CMSG_SPACE(sizeof(int) * OB_DESCRIPTORS_PER_MESSAGE)];
        struct cmsghdr align;
    } control;
    memset(&control, 0, sizeof(control));
    struct msghdr header;
    memset(&header, 0, sizeof(header));
    header.msg_iov = &vector;
    header.msg_iovlen = 1;
    header.msg_control = control.buffer;
    header.msg_controllen = sizeof(control.buffer);
    ssize_t received;
    for (;;) {
        received = recvmsg(socket_fd, &header, OB_RECEIVE_FLAGS);
        if (received >= 0) break;
        if (errno != EINTR) return errno;
    }
    if ((header.msg_flags & MSG_CTRUNC) != 0) *truncated = 1;
    for (struct cmsghdr *message = CMSG_FIRSTHDR(&header); message != NULL;
         message = CMSG_NXTHDR(&header, message)) {
        if (message->cmsg_level != SOL_SOCKET || message->cmsg_type != SCM_RIGHTS) continue;
        const size_t payload = message->cmsg_len - CMSG_LEN(0);
        const size_t count = payload / sizeof(int);
        const unsigned char *data = CMSG_DATA(message);
        for (size_t index = 0; index < count; index += 1) {
            int descriptor;
            memcpy(&descriptor, data + index * sizeof(int), sizeof(int));
            (void)fcntl(descriptor, F_SETFD, FD_CLOEXEC);
            if (*descriptor_count < descriptor_capacity) {
                descriptors[*descriptor_count] = descriptor;
                *descriptor_count += 1;
            } else {
                close(descriptor);
                *truncated = 1;
            }
        }
    }
    *read_length = (size_t)received;
    return 0;
}

// The process id of the socket's peer when it connected. Answers 0 or an errno.
int orchbee_peer_process_id(int socket_fd, int32_t *pid) {
#if defined(__APPLE__)
    pid_t peer = 0;
    socklen_t length = sizeof(peer);
    if (getsockopt(socket_fd, SOL_LOCAL, LOCAL_PEERPID, &peer, &length) != 0) return errno;
    if (length != sizeof(peer) || peer <= 0) return EPROTO;
#else
    struct ucred credentials;
    socklen_t length = sizeof(credentials);
    if (getsockopt(socket_fd, SOL_SOCKET, SO_PEERCRED, &credentials, &length) != 0) return errno;
    const pid_t peer = credentials.pid;
    if (length != sizeof(credentials) || peer <= 0) return EPROTO;
#endif
    *pid = (int32_t)peer;
    return 0;
}

#if defined(__APPLE__)
// A process's start: its start time in microseconds since 1970, as `kinfo_proc` records it. Answers
// 0, ESRCH when no such process exists, or another errno.
int orchbee_process_start(int32_t pid, uint64_t *start) {
    if (pid <= 0) return EINVAL;
    int name[4] = {CTL_KERN, KERN_PROC, KERN_PROC_PID, pid};
    struct kinfo_proc info;
    size_t length = sizeof(info);
    memset(&info, 0, sizeof(info));
    if (sysctl(name, 4, &info, &length, NULL, 0) != 0) return errno;
    if (length == 0 || info.kp_proc.p_pid != pid) return ESRCH;
    const struct timeval started = info.kp_proc.p_starttime;
    if (started.tv_sec <= 0 || started.tv_usec < 0 || started.tv_usec >= 1000000) return EPROTO;
    *start = (uint64_t)started.tv_sec * 1000000u + (uint64_t)started.tv_usec;
    return 0;
}
#else
// Whether `byte` is ASCII white space as Rust's `u8::is_ascii_whitespace` reads it.
static int ascii_white_space(unsigned char byte) {
    return byte == ' ' || byte == '\t' || byte == '\n' || byte == '\f' || byte == '\r';
}

// This boot's tag: 23 bits of the FNV-1a hash of /proc/sys/kernel/random/boot_id without the white
// space around it, with bit 23 set. The local runtime computes the same tag, so the start this file
// reads equals the one a Worker registers. Answers 0 or an errno.
static int boot_tag(uint64_t *tag) {
    static uint64_t cached = 0;
    if (cached != 0) {
        *tag = cached;
        return 0;
    }
    const int descriptor = open("/proc/sys/kernel/random/boot_id", O_RDONLY | O_CLOEXEC);
    if (descriptor < 0) return errno;
    unsigned char bytes[128];
    ssize_t length;
    do {
        length = read(descriptor, bytes, sizeof(bytes));
    } while (length < 0 && errno == EINTR);
    const int failure = errno;
    close(descriptor);
    if (length < 0) return failure;
    size_t first = 0;
    size_t end = (size_t)length;
    while (first < end && ascii_white_space(bytes[first])) first += 1;
    while (end > first && ascii_white_space(bytes[end - 1])) end -= 1;
    uint64_t hash = 0xcbf29ce484222325u;
    for (size_t index = first; index < end; index += 1) {
        hash = (hash ^ bytes[index]) * 0x100000001b3u;
    }
    cached = (hash & 0x7fffffu) | 0x800000u;
    *tag = cached;
    return 0;
}

// A process's start: its start in clock ticks since boot, field 22 of /proc/<pid>/stat, in the low
// 40 bits under this boot's tag, as the local runtime registers it. A process that ended and waits
// to be reaped keeps its start, as on macOS. Answers 0, ESRCH when no such process exists, or
// another errno.
int orchbee_process_start(int32_t pid, uint64_t *start) {
    if (pid <= 0) return EINVAL;
    char path[32];
    snprintf(path, sizeof(path), "/proc/%d/stat", (int)pid);
    const int descriptor = open(path, O_RDONLY | O_CLOEXEC);
    if (descriptor < 0) return errno == ENOENT ? ESRCH : errno;
    char stat_line[1024];
    ssize_t length;
    do {
        length = read(descriptor, stat_line, sizeof(stat_line) - 1);
    } while (length < 0 && errno == EINTR);
    const int failure = errno;
    close(descriptor);
    if (length < 0) return failure;
    stat_line[length] = 0;
    // The command name in parentheses may hold any byte, so the fields after it start past the last
    // `)`, each after one space: the state is field 3 and the start field 22.
    const char *field = strrchr(stat_line, ')');
    if (field == NULL) return EPROTO;
    for (int number = 2; number < 22; number += 1) {
        field = strchr(field + 1, ' ');
        if (field == NULL) return EPROTO;
    }
    char *end = NULL;
    errno = 0;
    const unsigned long long ticks = strtoull(field + 1, &end, 10);
    if (errno != 0 || end == field + 1 || ticks >= (1ull << 40)) return EPROTO;
    uint64_t tag = 0;
    const int tagged = boot_tag(&tag);
    if (tagged != 0) return tagged;
    *start = (tag << 40) | (uint64_t)ticks;
    return 0;
}
#endif

// The name of the slave of the PTY master `master`, NUL-terminated in `name`, which holds 128
// bytes. Fails when `master` is not a PTY master. Answers 0 or an errno.
int orchbee_pty_slave_name(int master, char *name) {
    char buffer[128];
    memset(buffer, 0, sizeof(buffer));
#if defined(__APPLE__)
    if (ioctl(master, TIOCPTYGNAME, buffer) != 0) return errno;
#else
    unsigned int number = 0;
    if (ioctl(master, TIOCGPTN, &number) != 0) return errno;
    snprintf(buffer, sizeof(buffer), "/dev/pts/%u", number);
#endif
    buffer[sizeof(buffer) - 1] = 0;
    memcpy(name, buffer, sizeof(buffer));
    return 0;
}

// Answers 1 when `descriptor` is a pipe end open only for writing when `write` is not zero, or only
// for reading when it is zero, and 0 otherwise.
int orchbee_pipe_end(int descriptor, int write) {
    struct stat info;
    if (fstat(descriptor, &info) != 0 || !S_ISFIFO(info.st_mode)) return 0;
    int flags = fcntl(descriptor, F_GETFL);
    if (flags < 0) return 0;
    return (flags & O_ACCMODE) == (write ? O_WRONLY : O_RDONLY);
}

// Connects a close-on-exec Unix stream socket to `path`, which writes never turn into SIGPIPE.
// Answers 0 with the socket in `socket_out`, or an errno.
int orchbee_connect_local(const char *path, int *socket_out) {
    struct sockaddr_un address;
    memset(&address, 0, sizeof(address));
    const size_t length = strlen(path);
    if (length == 0 || length >= sizeof(address.sun_path)) return ENAMETOOLONG;
    address.sun_family = AF_UNIX;
    memcpy(address.sun_path, path, length);
#if defined(__APPLE__)
    const int socket_fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (socket_fd < 0) return errno;
    const int enabled = 1;
    if (fcntl(socket_fd, F_SETFD, FD_CLOEXEC) != 0 ||
        setsockopt(socket_fd, SOL_SOCKET, SO_NOSIGPIPE, &enabled, sizeof(enabled)) != 0) {
        const int failure = errno;
        close(socket_fd);
        return failure;
    }
#else
    // Linux has no option against SIGPIPE; every write to this socket sends with MSG_NOSIGNAL.
    const int socket_fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
    if (socket_fd < 0) return errno;
#endif
    int connected;
    do {
        connected = connect(socket_fd, (const struct sockaddr *)&address, sizeof(address));
    } while (connected != 0 && errno == EINTR);
    if (connected != 0) {
        const int failure = errno;
        close(socket_fd);
        return failure;
    }
    *socket_out = socket_fd;
    return 0;
}

// Raises the soft descriptor limit to the hard limit, at most OPEN_MAX on macOS and 2^20 on Linux,
// since the Keeper's connections, their wakes, and the masters it holds exceed macOS's default of
// 256 and can exceed Linux's 1024.
void orchbee_raise_descriptor_limit(void) {
#if defined(__APPLE__)
    const rlim_t ceiling = OPEN_MAX;
#else
    const rlim_t ceiling = (rlim_t)1 << 20;
#endif
    struct rlimit limit;
    if (getrlimit(RLIMIT_NOFILE, &limit) != 0) return;
    rlim_t wanted = limit.rlim_max;
    if (wanted == RLIM_INFINITY || wanted > ceiling) wanted = ceiling;
    if (wanted <= limit.rlim_cur) return;
    limit.rlim_cur = wanted;
    (void)setrlimit(RLIMIT_NOFILE, &limit);
}

// The channel that carries a copy of the state to an update's check run: a read end for the child's
// standard input and a non-blocking write end, both close-on-exec, whose writes to a child that
// stopped reading fail with EPIPE instead of raising SIGPIPE. A pipe on macOS, which turns SIGPIPE
// off per descriptor; a socket pair on Linux, whose writes send with MSG_NOSIGNAL. Answers 0 or an
// errno.
int orchbee_check_channel(int *read_end, int *write_end) {
    int ends[2];
#if defined(__APPLE__)
    if (pipe(ends) != 0) return errno;
    if (fcntl(ends[0], F_SETFD, FD_CLOEXEC) != 0 || fcntl(ends[1], F_SETFD, FD_CLOEXEC) != 0 ||
        fcntl(ends[1], F_SETNOSIGPIPE, 1) != 0) {
        const int failure = errno;
        close(ends[0]);
        close(ends[1]);
        return failure;
    }
#else
    if (socketpair(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0, ends) != 0) return errno;
#endif
    const int flags = fcntl(ends[1], F_GETFL);
    if (flags < 0 || fcntl(ends[1], F_SETFL, flags | O_NONBLOCK) != 0) {
        const int failure = errno;
        close(ends[0]);
        close(ends[1]);
        return failure;
    }
    *read_end = ends[0];
    *write_end = ends[1];
    return 0;
}

// Writes up to `length` bytes to a check channel's write end without waiting. Answers 0 and the
// bytes written, or an errno; EAGAIN when the channel takes nothing now.
int orchbee_check_write(int write_end, const void *bytes, size_t length, size_t *written) {
    *written = 0;
    for (;;) {
#if defined(__APPLE__)
        const ssize_t count = write(write_end, bytes, length);
#else
        const ssize_t count = send(write_end, bytes, length, MSG_NOSIGNAL);
#endif
        if (count >= 0) {
            *written = (size_t)count;
            return 0;
        }
        if (errno != EINTR) return errno;
    }
}

// Starts `program` as `keep --check`, with `input` as its standard input and /dev/null as its
// output and errors, and no other descriptor of this process: macOS closes every other one as it
// starts the child, and every descriptor the Keeper holds on Linux closes on exec. Answers 0 with
// the child's id in `pid`, or an errno.
int orchbee_spawn_check(const char *program, int input, int32_t *pid) {
    posix_spawn_file_actions_t actions;
    int failure = posix_spawn_file_actions_init(&actions);
    if (failure != 0) return failure;
    posix_spawnattr_t attributes;
    failure = posix_spawnattr_init(&attributes);
    if (failure != 0) {
        posix_spawn_file_actions_destroy(&actions);
        return failure;
    }
#if defined(__APPLE__)
    failure = posix_spawnattr_setflags(&attributes, POSIX_SPAWN_CLOEXEC_DEFAULT);
#endif
    if (failure == 0) failure = posix_spawn_file_actions_adddup2(&actions, input, 0);
    if (failure == 0) failure = posix_spawn_file_actions_addopen(&actions, 1, "/dev/null", O_RDONLY, 0);
    if (failure == 0) failure = posix_spawn_file_actions_addopen(&actions, 2, "/dev/null", O_RDONLY, 0);
    if (failure == 0) {
        char *const argv[] = {(char *)program, "keep", "--check", NULL};
        pid_t child = 0;
        failure = posix_spawn(&child, program, &actions, &attributes, argv, environ);
        if (failure == 0) *pid = (int32_t)child;
    }
    posix_spawnattr_destroy(&attributes);
    posix_spawn_file_actions_destroy(&actions);
    return failure;
}
