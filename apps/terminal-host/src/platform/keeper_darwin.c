// The Keeper's macOS system calls, shared with the Host's Keeper link: descriptor passing over a
// Unix stream socket, where one frame's bytes leave with the descriptors its message carries by
// SCM_RIGHTS and a read returns the descriptors that arrived with its bytes; the peer's process id;
// a process's start time; a PTY master's slave name; and the descriptor limit.

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/resource.h>
#include <sys/socket.h>
#include <sys/sysctl.h>
#include <sys/ttycom.h>
#include <sys/types.h>
#include <sys/un.h>
#include <unistd.h>

#if !defined(__APPLE__)
#error "descriptor passing for the Keeper is implemented for macOS only"
#endif

enum {
    // The most descriptors one message carries, as the Keeper contract bounds it.
    OB_DESCRIPTORS_PER_MESSAGE = 64,
};

// A write to a closed peer fails with EPIPE instead of raising SIGPIPE.
#define OB_SEND_FLAGS (MSG_NOSIGNAL | MSG_DONTWAIT)

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
        received = recvmsg(socket_fd, &header, MSG_DONTWAIT);
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
    pid_t peer = 0;
    socklen_t length = sizeof(peer);
    if (getsockopt(socket_fd, SOL_LOCAL, LOCAL_PEERPID, &peer, &length) != 0) return errno;
    if (length != sizeof(peer) || peer <= 0) return EPROTO;
    *pid = (int32_t)peer;
    return 0;
}

// A process's start time in microseconds since 1970, as `kinfo_proc` records it. Answers 0, ESRCH
// when no such process exists, or another errno.
int orchbee_process_start_microseconds(int32_t pid, uint64_t *start) {
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

// The name of the slave of the PTY master `master`, NUL-terminated in `name`, which holds 128
// bytes. Fails when `master` is not a PTY master. Answers 0 or an errno.
int orchbee_pty_slave_name(int master, char *name) {
    char buffer[128];
    memset(buffer, 0, sizeof(buffer));
    if (ioctl(master, TIOCPTYGNAME, buffer) != 0) return errno;
    buffer[sizeof(buffer) - 1] = 0;
    memcpy(name, buffer, sizeof(buffer));
    return 0;
}

// Raises the soft descriptor limit to the hard limit, at most OPEN_MAX, since the Keeper's
// connections, their wake queues, and the masters it holds exceed the default of 256.
void orchbee_raise_descriptor_limit(void) {
    struct rlimit limit;
    if (getrlimit(RLIMIT_NOFILE, &limit) != 0) return;
    rlim_t wanted = limit.rlim_max;
    if (wanted == RLIM_INFINITY || wanted > OPEN_MAX) wanted = OPEN_MAX;
    if (wanted <= limit.rlim_cur) return;
    limit.rlim_cur = wanted;
    (void)setrlimit(RLIMIT_NOFILE, &limit);
}
