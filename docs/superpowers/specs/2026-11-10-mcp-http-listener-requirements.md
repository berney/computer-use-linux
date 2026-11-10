# MCP HTTP listener: maintainer requirements

The listener design the maintainer (Avi Fenesh) said agent-sh would merge, copied verbatim from [agent-sh/computer-use-linux#362](https://github.com/agent-sh/computer-use-linux/issues/362).
Snapshot handles (point 6) landed upstream in #436 and shipped in v0.7.48; the second comment lists what is left for the listener.
The plan is `docs/superpowers/plans/2026-11-10-mcp-http-listener.md`.

## avifenesh, 2026-11-04

Yes, we'll take an opt-in listener.
Thanks for asking first and for the clear write-up.

We ran your branch (68c190a).
It needs some changes before it works for the Kubernetes case and before it's safe to put on a network.

### What we found

- **Non-loopback `Host` gets 403.**
  rmcp's default `allowed_hosts` is localhost, 127.0.0.1 and ::1.
  A request through a Service name or the pod IP gets `403 Host header is not allowed`, even with the right token.
- **Sessions share element indices.**
  `last_snapshot` is one `Arc<Mutex<CachedAccessibilitySnapshot>>` (server.rs:176).
  If client B calls `get_app_state`, client A's `element_index` silently points into B's tree.
  Untargeted element actions don't check whose tree it is.
  A per-session service factory doesn't fix this: stateless requests in the newer protocol get a fresh service per request.
- **Shell tool.**
  With `COMPUTER_USE_LINUX_ENABLE_SHELL=1`, the listener hands out the same power as `pods/exec`.
- **Token handling.**
  - cli.rs trims the token, but http_listen.rs compares it untrimmed, so a Secret with a trailing newline never matches.
  - The comparison returns early on a length mismatch.
  - The scheme check is case-sensitive.
  - A 401 carries no `WWW-Authenticate`.
  - Child processes inherit the token.
- **Origin.**
  A request with a hostile `Origin` gets 200.
  The MCP transport spec requires validating `Origin`.
- **`json_response=true`** does nothing in rmcp's legacy session mode; the response is still `text/event-stream`.
- **Lifecycle.**
  The HTTP path skips the tree spill sweep and SIGTERM/SIGINT cleanup that stdio runs.
  `axum::serve` sets no header-read timeout.

### The design we'd merge

1. `mcp --listen ADDR:PORT`, opt-in, no default address.
   No cargo feature.
   Please put the binary size change in the PR.
2. A repeatable `--allowed-host NAME`, passed to `with_allowed_hosts`.
   Refuse to start on a non-loopback bind that names no host.
3. Origin validation on, with an empty allowlist: any request with an `Origin` header gets 403.
   No browser client needs this server.
4. Token from `COMPUTER_USE_LINUX_MCP_TOKEN_FILE` (a mounted Secret) or `COMPUTER_USE_LINUX_MCP_TOKEN`.
   Trim it once and remove it from the environment so children don't inherit it.
   Compare SHA-256 digests (`sha2` is already a dependency).
   Accept the `Bearer` scheme in any case, and send `WWW-Authenticate: Bearer` with the 401.
5. Refuse `--listen` when `COMPUTER_USE_LINUX_ENABLE_SHELL=1`.
6. Snapshot handles.
   `get_app_state` returns a `snapshot_id`, and element actions and `get_tree_page` take it and fail closed on a mismatch.
   This works the same on stdio.
   If you'd rather not take this part on, say so and we'll do it first as its own PR, then you rebase on it.
7. The input lock, portal sessions, uinput pointer, clipboard lock and diagnostics cache stay global.
   The indicator's agent name comes from each request, not from one last-writer field.
8. Same lifecycle as stdio: spill sweep at start, cleanup on SIGTERM/SIGINT through rmcp's cancellation token, and a header-read timeout.
9. Plain HTTP only.
   TLS sits in front (mesh, ingress, sidecar), and the docs say so.

### Tests we'd want

- A non-loopback `Host` that is allowed, and one that is not.
- A request with an `Origin` header gets 403.
- Token from a file with a trailing newline.
  Wrong-length token.
  Lowercase `bearer`.
- `--listen` with `ENABLE_SHELL=1` refuses to start.
- Two clients: an element index from one client's tree is refused for the other.
- A stateless request from the newer protocol version.
- SIGTERM cleanup.
- The tests must not send real input to the developer's desktop.
  The server picks up the session bus from the process tree even with `env_clear` (diagnostics.rs:458-488), so a `click` in a test can land on a real screen.
  Use the private fixture peers the other input tests use.

### Smaller points

- Drop the `CUL_INPUT_LOCK_HOLD_MS` hook from `dispatch_click`.
- Drop `json_response`, or set `legacy_session_mode` on purpose.
- Rebase onto v0.7.36; the version literal will conflict.
- Docs:
  - README still says "rmcp 2024-11-05".
  - Reword the "No network" bullet and the SECURITY.md scope to "no listener unless `--listen` is passed".
  - Add a listener section to `references/selkies-setup.md`.
  - The npm README bullet landed under the "Input & coordinate contracts" heading.

Open the PR when it's ready.
We'll review it against the list above.

## avifenesh, 2026-11-09

@berney Snapshot handles landed in #436 and shipped in [v0.7.48](https://github.com/agent-sh/computer-use-linux/releases/tag/v0.7.48).
Rebase the listener on it.

What it does:

- `get_app_state`, `include_tree` observations and `get_tree_page` return a `snapshot_id`.
  It is a random prefix per server process plus a serial, for example `3f9a1c2e-12`.
- `click`, `scroll`, `perform_action`, `set_value` and `get_tree_page` take an optional `snapshot_id`.
- The server still caches one tree, and every new tree replaces it.
  When a call picks its element by `element_index` or selector and names a replaced snapshot, it is refused under the input lock, before any window is raised or input is sent.
  `get_tree_page` returns no nodes for a replaced snapshot.
- Without `snapshot_id`, the element resolves in the latest tree, as before.
  An `element_identifier` and a full `x`/`y` point are not checked.

For the listener:

1. Require `snapshot_id` on index and selector calls over HTTP, and refuse an empty one.
   On stdio it stays optional.
   Without that, a client that leaves it out still acts on whatever tree another client cached last.
2. Two clients that alternate `get_app_state` invalidate each other's ids.
   A refused client calls `get_app_state` again.
   That is the fail-closed behaviour we asked for, so no change is needed unless you see it hurt in practice.
3. The post-action tree takes its pid and window scope from whatever tree is cached, which can be another client's.
4. A failed `get_app_state` clears the cached tree for every client.

Points 3 and 4 belong with the listener.
Pass the scope through the request rather than reading it from the cache.
