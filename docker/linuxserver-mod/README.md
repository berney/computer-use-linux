# computer-use-linux linuxserver mod

Adds the `computer-use-linux` MCP server to a linuxserver.io Selkies desktop (webtop, chrome, or anything built on `baseimage-selkies`).
The mod is a single-layer filesystem: two static binaries and an s6 service that starts `computer-use-linux mcp --listen` once the desktop is up.
It does not replace the desktop image.

Publish `5200/tcp` on `127.0.0.1` only.
The listener is plain HTTP and the token is its only check.

## Build

From the repository root:

```sh
docker bake                       # image computer-use-linux-mod:dev
docker bake linuxserver-mod-tar   # docker/linuxserver-mod/computer-use-linux.tar, for sideloading
```

The image is `linux/amd64` only.
Each checkout (worktree) gets its own cargo cache, keyed by its path, so a rebuild that changes no Rust source takes seconds.

## Use it

Pick one of three ways.

### DOCKER_MODS, sideloaded (no registry)

Mount the tarball at `/mods/computer-use-linux.tar` and turn on sideloading:

```yaml
services:
  webtop:
    image: lscr.io/linuxserver/webtop:alpine-kde
    environment:
      DOCKER_MODS: computer-use-linux
      DOCKER_MODS_SIDELOAD: "true"
      COMPUTER_USE_LINUX_MCP_TOKEN: change-me
    volumes:
      - ./docker/linuxserver-mod/computer-use-linux.tar:/mods/computer-use-linux.tar:ro
    ports:
      - 127.0.0.1:3001:3001
      - 127.0.0.1:5200:5200
```

### DOCKER_MODS from a registry

`DOCKER_MODS` downloads only over HTTPS and applies only the first layer of the image, which is why the mod is one layer.
Push the image to a registry you control, then name it:

```sh
docker bake --push --set linuxserver-mod.tags=ghcr.io/<you>/computer-use-linux-mod:latest
```

```yaml
    environment:
      DOCKER_MODS: ghcr.io/<you>/computer-use-linux-mod:latest
```

### Overlay image

Copy the mod into your own image, for example a `baseimage-selkies` app:

```dockerfile
FROM computer-use-linux-mod:dev AS computer-use-linux-mod
FROM ghcr.io/linuxserver/baseimage-selkies:debiantrixie
COPY --link --from=computer-use-linux-mod / /
```

## Environment

| Variable                        | Required | Default                                       |
|---------------------------------|----------|-----------------------------------------------|
| `COMPUTER_USE_LINUX_MCP_TOKEN`  | yes      | unset disables the MCP server                 |
| `COMPUTER_USE_LINUX_MCP_LISTEN` | no       | `0.0.0.0:5200`                                |
| `COMPUTER_USE_LINUX_INDICATOR`  | no       | `0`. Set `1` to show the on-screen indicator  |

Clients send `Authorization: Bearer <token>` to `http://127.0.0.1:5200/mcp`.

## Notes

- Don't set `PUID`/`PGID` to a different value from the one that created an existing `/config` volume; the desktop will not start.
- The mod also sets `PIXELFLUX_CU`, `PIXELFLUX_CU_TOKEN` and the accessibility variables when they are unset, so `PELORUS=true` is not needed.
