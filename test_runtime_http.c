#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>

/* Copy the runtime functions exactly as they are */
int64_t sys_socket(int domain, int type, int protocol) {
  return socket(domain, type, protocol);
}

uint32_t sys_inet_addr(const char* cp) {
  /* inet_addr already returns network byte order, use directly */
  return inet_addr(cp);
}

int64_t sys_connect(int64_t sockfd, uint32_t addr, uint16_t port) {
  struct sockaddr_in sa;
  memset(&sa, 0, sizeof(sa));
  sa.sin_family = AF_INET;
  sa.sin_port = htons(port);
  /* addr is already in network byte order from inet_addr */
  sa.sin_addr.s_addr = addr;
  
  return connect(sockfd, (struct sockaddr*)&sa, sizeof(sa));
}

int64_t sys_send(int sockfd, const char* buf, size_t len, int flags) {
  return send(sockfd, buf, len, flags);
}

int64_t sys_recv(int sockfd, char* buf, size_t len, int flags) {
  return recv(sockfd, buf, len, flags);
}

int64_t sys_close(int fd) {
  return close(fd);
}

int main() {
    printf("Testing runtime wrappers...\n");
    fflush(stdout);
    
    int sockfd = sys_socket(AF_INET, SOCK_STREAM, 0);
    printf("Socket: %d\n", sockfd);
    fflush(stdout);
    
    uint32_t addr = sys_inet_addr("52.204.75.48");
    printf("Addr: %u (0x%08x)\n", addr, addr);
    printf("Expected: 1804848182 (0x6b93cc36) for 52.204.75.48 in network order\n");
    fflush(stdout);
    
    printf("Connecting to 52.204.75.48:80...\n");
    fflush(stdout);
    
    int result = sys_connect(sockfd, addr, 80);
    printf("Connect: %d\n", result);
    fflush(stdout);
    
    if (result < 0) {
        perror("connect");
        close(sockfd);
        return 1;
    }
    
    const char* msg = "GET /get HTTP/1.1\r\nHost: httpbin.org\r\n\r\n";
    int sent = sys_send(sockfd, msg, strlen(msg), 0);
    printf("Sent: %d\n", sent);
    fflush(stdout);
    
    char buf[4096];
    int received = sys_recv(sockfd, buf, sizeof(buf), 0);
    printf("Received: %d\n", received);
    fflush(stdout);
    
    if (received > 0) {
        buf[received] = '\0';
        printf("Response:\n%.500s...\n", buf);
    }
    
    sys_close(sockfd);
    printf("Done!\n");
    
    return 0;
}
