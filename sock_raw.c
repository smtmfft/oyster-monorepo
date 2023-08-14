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
  iov.iov_len = 1600;
  message_header.msg_iov = &iov;
  message_header.msg_iovlen = 1;

  uint8_t control[100];
  message_header.msg_control = &control;
  message_header.msg_controllen = 100;

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

    // get src and dst addr
    // NOTE: buf is aligned to 4 byte boundary, can directly cast and read
    uint32_t src_addr = ntohl(*(uint32_t *)(buf + 12));
    uint32_t dst_addr = ntohl(*(uint32_t *)(buf + 16));

    // ignore packets not originating from 127.0.0.1
    if (src_addr != 0x7f000001) {
      continue;
    }

    // https://en.wikipedia.org/wiki/Reserved_IP_addresses
    // ignore packets sent to
    // 0.0.0.0/8
    if ((dst_addr & 0xff000000) == 0x00000000 ||
        // 10.0.0.0/8
        (dst_addr & 0xff000000) == 0x0a000000 ||
        // 100.64.0.0/10
        (dst_addr & 0xffc00000) == 0x64400000 ||
        // 127.0.0.0/8
        (dst_addr & 0xff000000) == 0x7f000000 ||
        // 169.254.0.0/16
        (dst_addr & 0xffff0000) == 0xa9fe0000 ||
        // 172.16.0.0/12
        (dst_addr & 0xfff00000) == 0xac100000 ||
        // 192.0.0.0/24
        (dst_addr & 0xffffff00) == 0xc0000000 ||
        // 192.0.2.0/24
        (dst_addr & 0xffffff00) == 0xc0000200 ||
        // 192.88.99.0/24
        (dst_addr & 0xffffff00) == 0xc0586300 ||
        // 192.168.0.0/16
        (dst_addr & 0xffff0000) == 0xc0a80000 ||
        // 198.18.0.0/15
        (dst_addr & 0xfffe0000) == 0xc6120000 ||
        // 198.51.100.0/24
        (dst_addr & 0xffffff00) == 0xc6336400 ||
        // 203.0.113.0/24
        (dst_addr & 0xffffff00) == 0xcb007100 ||
        // 224.0.0.0/4
        (dst_addr & 0xf0000000) == 0xe0000000 ||
        // 233.252.0.0/24
        (dst_addr & 0xffffff00) == 0xe9fc0000 ||
        // 240.0.0.0/4
        (dst_addr & 0xf0000000) == 0xf0000000 ||
        // 255.255.255.255/32
        (dst_addr & 0xffffffff) == 0xffffffff) {
      continue;
    }

    printf("recvmsg %ld, ", res);
    for (ssize_t i = 0; i < res; i++) {
      printf("%02x", buf[i]);
    }
    printf("\n");
  }

  close(raw_socket);
  free(buf);
  printf("done\n");

  return 0;
}
