FROM debian:bookworm

ENV container=docker

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y systemd systemd-sysv openssh-server sudo ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /run/sshd /root/.ssh

RUN echo 'root:test-root-password' | chpasswd

COPY entrypoint.sh /entrypoint.sh
RUN chmod 0755 /entrypoint.sh

STOPSIGNAL SIGRTMIN+3
ENTRYPOINT ["/entrypoint.sh"]
