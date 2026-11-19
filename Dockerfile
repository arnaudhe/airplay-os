# Buildroot build environment (native arm64 on Apple Silicon).
FROM debian:bookworm
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential gcc g++ make patch perl python3 file wget ca-certificates \
    cpio unzip rsync bc git libncurses-dev bzip2 xz-utils gzip diffutils \
    findutils sed bash locales && \
    sed -i 's/# en_US.UTF-8/en_US.UTF-8/' /etc/locale.gen && locale-gen && \
    rm -rf /var/lib/apt/lists/*
ENV LANG=en_US.UTF-8
RUN useradd -m br
USER br
WORKDIR /work
