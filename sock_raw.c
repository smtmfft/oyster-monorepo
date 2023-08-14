#include <errno.h>
#include <netinet/in.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

int main() {
  int raw_socket = socket(AF_INET, SOCK_RAW, IPPROTO_TCP);
  if (raw_socket < 0) {
    printf("failed to create socket\n");
    return -1;
  }

  int res = setsockopt(raw_socket, SOL_SOCKET, SO_BINDTODEVICE, "lo", 2);
  if (res < 0) {
    printf("bind error: %d, %s\n", res, strerror(errno));
    return -1;
  }

  struct msghdr message_header;
  memset(&message_header, 0, sizeof(message_header));

  uint8_t *buf = aligned_alloc(4, 1600);
  struct iovec iov;
  iov.iov_base = buf;
  iov.iov_len = 10000;
  message_header.msg_iov = &iov;
  message_header.msg_iovlen = 1;

  uint8_t control[10000];
  message_header.msg_control = &control;
  message_header.msg_controllen = 10000;

  while (1) {
    ssize_t res = recvmsg(raw_socket, &message_header, 0);

    if (res < 0) {
      printf("recvmsg error: %ld\n", res);
      break;
    }

    if (res == 0) {
      printf("recvmsg exit\n");
      break;
    }

    printf("recvmsg %ld, ", res);
    for (ssize_t i = 0; i < res; i++) {
      printf("%02x", buf[i]);
    }
    printf("\n");
  }

  close(raw_socket);
  printf("done\n");

  return 0;
}
