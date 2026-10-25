# computer-use-linux linuxserver mod

Adds the `computer-use-linux` MCP server to a Selkies desktop. The image is a filesystem overlay: two static binaries and an s6 service. It does not replace the desktop image.

Publish `5200/tcp` on `127.0.0.1`. The listener is plain HTTP and the token is the only check.

## Webtop or Chrome

```yaml
services:
  webtop:
    image: lscr.io/linuxserver/webtop:alpine-kde
    environment:
      PUID: 1000
      PGID: 1000
      TZ: UTC
      DOCKER_MODS: computer-use-linux-mod:dev
      COMPUTER_USE_LINUX_MCP_TOKEN: change-me
    ports:
      - 127.0.0.1:3001:3001
      - 127.0.0.1:5200:5200
```

The same `DOCKER_MODS` line works on `lscr.io/linuxserver/chrome:latest`. `DOCKER_MODS` pulls from a registry. For a local tag, push `computer-use-linux-mod:dev` to a registry on `127.0.0.1` and use that reference.

## burp-selkies

```dockerfile
FROM computer-use-linux-mod:dev AS computer-use-linux-mod
FROM ghcr.io/linuxserver/baseimage-selkies:debiantrixie
COPY --link --from=computer-use-linux-mod / /
```

Run that image with `COMPUTER_USE_LINUX_MCP_TOKEN` set and `-p 127.0.0.1:5200:5200`.

## Environment

| Variable | Required | Default |
| --- | --- | --- |
| `COMPUTER_USE_LINUX_MCP_TOKEN` | yes | unset disables the MCP server |
| `COMPUTER_USE_LINUX_MCP_LISTEN` | no | `0.0.0.0:5200` |
| `COMPUTER_USE_LINUX_INDICATOR` | no | `0`. Set `1` to show the on-screen indicator |

Clients send `Authorization: Bearer <token>` to `http://127.0.0.1:5200/mcp`.
