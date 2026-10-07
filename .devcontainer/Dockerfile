FROM mcr.microsoft.com/devcontainers/rust:2-1-trixie

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        openssh-client \
        iputils-ping \
        dnsutils \
        netcat-openbsd \
        mosquitto-clients \
    && rm -rf /var/lib/apt/lists/*