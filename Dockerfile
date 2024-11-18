FROM ubuntu:latest
WORKDIR /app
RUN apt-get update -y && \
    apt-get install -y wget rustup build-essential ruby ruby-toml-rb mingw-w64 libiconv-hook-dev libc6-dev && \
    rm -rf /var/lib/apt/lists /var/cache/apt/archives
RUN wget --quiet -O /tmp/zig.tar.xz https://ziglang.org/download/0.13.0/zig-linux-x86_64-0.13.0.tar.xz && \
    mkdir -p /usr/local/zig && \
    tar xf /tmp/zig.tar.xz -C /usr/local/zig --strip-components=1 && \
    ln -s /usr/local/zig/zig /usr/local/bin/zig
COPY MacOSX.sdk .
RUN rustup default stable
RUN rustup target add \
    x86_64-pc-windows-gnu aarch64-pc-windows-gnullvm \
    x86_64-apple-darwin aarch64-apple-darwin \
    x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
RUN cargo install --locked cargo-zigbuild
COPY . .
