#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>

int main() {
    printf("Creating socket...\n");
    fflush(stdout);
    
    int sockfd = socket(AF_INET, SOCK_STREAM, 0);
    printf("Socket: %d\n", sockfd);
    fflush(stdout);
    
    if (sockfd < 0) {
        perror("socket");
        return 1;
    }
    
    printf("Converting IP...\n");
    fflush(stdout);
    
    uint32_t addr = inet_addr("54.204.147.107");
    printf("Addr: %u (0x%08x)\n", addr, addr);
    fflush(stdout);
    
    printf("Connecting...\n");
    fflush(stdout);
    
    struct sockaddr_in sa;
    memset(&sa, 0, sizeof(sa));
    sa.sin_family = AF_INET;
    sa.sin_port = htons(80);
    sa.sin_addr.s_addr = addr;
    
    int result = connect(sockfd, (struct sockaddr*)&sa, sizeof(sa));
    printf("Connect: %d\n", result);
    fflush(stdout);
    
    if (result < 0) {
        perror("connect");
        close(sockfd);
        return 1;
    }
    
    printf("Sending...\n");
    fflush(stdout);
    
    const char* msg = "GET /get HTTP/1.1\r\nHost: httpbin.org\r\n\r\n";
    int sent = send(sockfd, msg, strlen(msg), 0);
    printf("Sent: %d\n", sent);
    fflush(stdout);
    
    printf("Receiving...\n");
    fflush(stdout);
    
    char buf[4096];
    int received = recv(sockfd, buf, sizeof(buf), 0);
    printf("Received: %d\n", received);
    fflush(stdout);
    
    if (received > 0) {
        buf[received] = '\0';
        printf("Response:\n%s\n", buf);
    }
    
    close(sockfd);
    printf("Done!\n");
    
    return 0;
}
