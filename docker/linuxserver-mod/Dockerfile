# syntax=docker/dockerfile:1

FROM docker.io/library/rust:alpine AS build
# docker bake sets one id per checkout; cargo trusts mtimes, which only stay ordered within one checkout.
ARG CARGO_CACHE_ID=shared
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cul-target-${CARGO_CACHE_ID},target=/src/target \
    <<-EOF
	apk add --no-cache musl-dev
	# A shared cache can hold a newer binary built from other sources.
	if [ "${CARGO_CACHE_ID}" = shared ]; then cargo clean --release -p computer-use-linux; fi
	cargo build --release --locked --bin computer-use-linux --bin computer-use-linux-indicator
	mkdir -p /out/usr/local/bin
	cp target/release/computer-use-linux target/release/computer-use-linux-indicator /out/usr/local/bin/
	cp -a docker/linuxserver-mod/root/. /out/
EOF

# One layer: DOCKER_MODS applies only the first layer of a mod image.
FROM scratch
COPY --link --from=build /out/ /
