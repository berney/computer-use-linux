# syntax=docker/dockerfile:1

FROM docker.io/library/rust:alpine AS build
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    <<-EOF
	apk add --no-cache musl-dev
	cargo build --release --locked --bin computer-use-linux --bin computer-use-linux-indicator
	mkdir -p /out
	cp target/release/computer-use-linux target/release/computer-use-linux-indicator /out/
EOF

FROM scratch
COPY --link --from=build /out/computer-use-linux /usr/local/bin/computer-use-linux
COPY --link --from=build /out/computer-use-linux-indicator /usr/local/bin/computer-use-linux-indicator
COPY --link docker/linuxserver-mod/root/ /
