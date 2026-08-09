#ifndef ORCHBEE_TERMINAL_HOST_DARWIN_CROSS_UTIL_H
#define ORCHBEE_TERMINAL_HOST_DARWIN_CROSS_UTIL_H

#include <sys/ttycom.h>
#include <termios.h>

__BEGIN_DECLS
int openpty(int *, int *, char *, struct termios *, struct winsize *);
__END_DECLS

#endif
