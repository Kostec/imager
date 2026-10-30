# Tools of the e2e run. QEMU 6.2 from Ubuntu 22.04 has the orangepi-pc machine.
FROM ubuntu:22.04
RUN apt-get update -qq \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
      ca-certificates e2fsprogs fdisk openssh-client openssl python3 qemu-system-arm sshpass \
 && rm -rf /var/lib/apt/lists/*
