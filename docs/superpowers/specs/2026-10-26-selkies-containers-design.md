# computer-use-linux inside Selkies containers

## Intent

Berne wants computer use of GUI apps that run inside linuxserver Selkies containers, driven by an MCP client (omp, Claude Code) from outside the container.
Today that needs Pelorus, which has no tests, no licence, a scaling bug and an unauthenticated REST API.
computer-use-linux is the better base: richer AT-SPI support (text, actions, values, editable text), safety checks, 546 tests and an MIT licence.
It cannot yet do the job inside these containers, because of the gaps found on 2026-10-25 (listed under Current state).

The end state is one static binary, added to any Selkies image, started by s6 at boot, listening on a published port, with no Pelorus running.

## Goals

- One static x86_64 musl binary of `computer-use-linux` that runs unchanged in Alpine, Debian and Arch Selkies images.
- It works in both kinds of Selkies image:
  - webtop KDE (`lscr.io/linuxserver/webtop:alpine-kde`), KWin on Wayland;
  - single-app images on labwc and Wayland, for example `lscr.io/linuxserver/chrome` and Berne's `burp-selkies` (`~/co/berney/burp-selkies`, built `FROM ghcr.io/linuxserver/baseimage-selkies:debiantrixie` with `PIXELFLUX_WAYLAND=true`).
- It starts unattended: no consent dialog or other human step at boot or on the first action.
- An MCP client outside the container connects over the network to a port published by Docker Compose.
- Screenshots, coordinate clicks, element clicks, keys and typed text all work, at display scale 1 and scale 2.
- Each fix or feature is its own branch off upstream `main`, so each can become a separate upstream PR; one integration branch merges them all for Berne's own use.

## Non-goals

- Pelorus's built-in LLM agent and chat UI.
  The MCP client is the agent.
- A Pelorus-compatible REST API.
- aarch64 builds (upstream's existing release builds stay as they are).
- Opening upstream PRs, pushing, or publishing images.
  Those stay with Berne.
- X11-mode Selkies images (`PIXELFLUX_WAYLAND` unset); Wayland only for now.

## Anti-goals

- Pelorus still has to run for any of this to work.
- The listening port accepts requests without a token.
- Anything that makes a webtop agent's input reach the host desktop (for example passing `/dev/uinput` into the container; see the 2026-10-25 finding that the nested KWin never reads `/dev/input`).
- A branch meant for upstream that mixes two unrelated changes, so upstream cannot take one without the other.

## Current state (evidence from 2026-10-25)

Upstream `main` at `7f6867e` (`fix: map AT-SPI logical pointer points on KWin (#291)`).
The tests below used a static build of the clone at that time; whether it already included `#291` is unknown.

| Area                          | webtop KDE (KWin)                                                                                                                                                                 | chrome (labwc)                                                   |
|-------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------|
| AT-SPI tree, `perform_action` | Works (KCalc `7×6=42` by pressing buttons)                                                                                                                                        | Tree works (283 nodes); `doctor` wrongly reports AT-SPI disabled |
| Window list                   | Works (KWin scripting)                                                                                                                                                            | Fails: `backend: unavailable`, no labwc backend                  |
| Screenshots                   | Portal; one-time KDE "Allow Apps to Take Screenshots?" prompt, can be pre-seeded in the permission store                                                                          | No route: no portal, not GNOME, not X11                          |
| Coordinate clicks             | Portal; prompt per process (persistable with `COMPUTER_USE_LINUX_PERSIST_REMOTE_DESKTOP=1`), then refused: "could not be mapped safely to the complete shared desktop" at scale 2 | No route: no uinput, no portal                                   |
| Keys                          | Portal; prompt per process (persistable)                                                                                                                                          | `wtype` or `xdotool` only; untested                              |
| Typed text                    | KDE clipboard paste: works in KCalc; in Konsole sends Ctrl+V, which readline treats as quoted-insert (`^M^M`, text lost)                                                          | Untested                                                         |
| Transport                     | stdio only (`server.serve(rmcp::transport::stdio())`, `src/server.rs`)                                                                                                            | same                                                             |

Relevant facts about the containers:

- pixelflux, the Selkies display server, has its own computer-use HTTP server: `POST /computer-use` on `127.0.0.1:$PIXELFLUX_CU`, bearer token `$PIXELFLUX_CU_TOKEN`, actions `screenshot`, `left_click`, `right_click`, `double_click`, `middle_click`, `triple_click`, `mouse_move`, `left_mouse_down`, `left_mouse_up`, `left_click_drag`, `scroll`, `key`, `hold_key`, `type`, `cursor_position`, `wait`, `zoom` (docs: `~/co/git/linuxserver/docker-documentation/docs/selkies/components/pixelflux.md`; request shape mirrored by Pelorus `src/rest.py` `ControlRequest`).
  It injects input inside the compositor, so there is no consent prompt, and its coordinates are physical screenshot pixels.
- `PELORUS=true` makes `init-selkies-config` write `PIXELFLUX_CU=5000` and a random `PIXELFLUX_CU_TOKEN` into `/run/s6/container_environment/`.
- `PELORUS=true` also gates the accessibility setup in `/defaults/startwm_wayland.sh`: it exports `QT_ACCESSIBILITY=1`, `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1`, `GTK_MODULES=gail:atk-bridge`, sets `org.a11y.Status IsEnabled` over D-Bus, and (on labwc) starts `at-spi2-registryd`.
- The desktop runs inside `dbus-run-session`, so its session bus address is random (`/tmp/dbus-XXXXXXXX`) and private to the processes started by `startwm_wayland.sh`.
  An s6 service started outside it has to discover the address, `WAYLAND_DISPLAY` and `XDG_RUNTIME_DIR` (`/config/.XDG`), for example from the compositor process's `/proc/<pid>/environ`.
- labwc in the baseimage always opens `$XDG_RUNTIME_DIR/labwc.sock`: one newline-terminated command per connection, JSON reply, then close.
  Only `GET_WINDOWS`, `GET_WINDOW_BY_PID <pid>` and `GET_FOCUSED_WINDOW` exist; there is no focus command.
  Each window has `pid, x, y, width, height, title, app_id, minimized, fullscreen, maximized, tiled, focused`.
- Display scale comes from the viewing browser's device pixel ratio unless `SELKIES_SCALING_DPI=96` pins it to 1.
  At scale 2 on KDE, `kscreen-doctor -j` reported `2864x1536, scale 1` while KWin laid windows out on a 1432x768 grid.
- The s6 service names in the images include `svc-de` (starts the desktop), `svc-selkies`, `svc-dbus` and `init-selkies-config`.

## The branches

All branches start from upstream `main`.
Work happens in `.worktrees/<branch-leaf>` with a fresh subagent per branch and a reviewer subagent after it, one branch at a time, in this order.
After each branch is finished, run `echo "<branch>: <one-line result>" | notify`.

### 1. `fix/kde-terminal-paste`

Problem: on KDE, `type_text` into Konsole loses the text.
`kde_clipboard_shortcut_for_focus` (`src/server.rs`) switches a terminal back to plain Ctrl+V whenever the focused AT-SPI element is not a terminal; Konsole reports focus on a `filler`.

Change: switch to Ctrl+V only when the focused element is a concrete text-entry element: editable (`FocusedElementSummary.editable`) or a text-entry role (`text`, `entry`, `password text`, `combo box`), and not a terminal.
A non-editable container such as a `filler` or `panel` keeps the terminal chord.
Before writing the rule, check live what Konsole's search field reports over AT-SPI, so the rule keeps it on Ctrl+V; the existing test that `focused("text", false)` gives `Standard` stays valid under this rule.

Acceptance:

- Unit tests: Konsole window with focused `filler` (not editable) → Ctrl+Shift+V; focused `text` (editable or not) → Ctrl+V; no AT-SPI answer → terminal chord (existing tests kept).
- Live: in the webtop, `type_text` of `echo cul-ok` then `press_key Return` into Konsole leaves `cul-ok` in Konsole's AT-SPI text.

### 2. `fix/kde-nested-scale`

Problem: on KDE at scale 2, portal coordinate clicks are refused because the stream and monitor layouts disagree.
`map_capture_point_to_stream_layout` (`src/remote_desktop.rs`) compares the portal stream rectangles with `logical_desktop_layout()`, which on KDE comes from `kscreen-doctor -j`.

First step: reproduce on current `main`; if `#291` already fixed it, drop the branch and say so.

Change: when KWin is the compositor, derive the logical layout (and so the scale) from KWin, which the KWin window backend already queries (`parse_kwin_logical_desktop_rect`), instead of trusting `kscreen-doctor` alone.
Keep the existing refusal when sources disagree in a way that cannot be resolved.

Acceptance:

- Unit test with the observed values: stream 1432x768, `kscreen-doctor` 2864x1536 scale 1, KWin logical 1432x768, capture 2864x1536 → the point maps instead of returning `None`.
- Live: in the webtop at scale 2 (default `SELKIES_SCALING_DPI`), a coordinate `click` on the Dolphin taskbar icon, taken from a fresh screenshot, opens Dolphin.

### 3. `feat/pixelflux-backend`

Problem: in Selkies containers the portal needs consent dialogs (KDE) or does not exist (labwc), so unattended screenshots and pointer input are impossible.

Change: a new input and screenshot backend that talks to pixelflux's computer-use server.

- Enabled when `PIXELFLUX_CU` and `PIXELFLUX_CU_TOKEN` are both set; then it is preferred over the portal, uinput, ydotool, wtype and the KDE clipboard route for screenshots, coordinate pointer actions, keys and literal text.
- Every request carries `Authorization: Bearer $PIXELFLUX_CU_TOKEN`; the token never appears in tool output, logs or errors.
- Only loopback addresses are accepted for `PIXELFLUX_CU` (a bare port means `127.0.0.1`).
- Typed text uses pixelflux `type`, which sidesteps the clipboard-paste problem entirely.
- Coordinates sent to pixelflux are physical screenshot pixels; element clicks convert AT-SPI bounds to physical pixels using the display scale (screenshot size against the compositor's logical layout), and refuse when the scale cannot be established, in keeping with upstream's existing refusal behaviour.
  KWin's logical layout comes from the KWin backend; on labwc it comes from the `wlr-output-management` protocol (labwc supports it; `wlr-randr` ships in the images), my call.
  Element clicks on labwc at scale 2 are in scope.
- Pixelflux's server was exercised through Pelorus on 2026-10-25 on KWin (webtop `41cb5ac5-ls149`, pixelflux 2.2.1): clicks, keys, typing into Konsole and screenshots worked, and screenshots were sometimes a stale frame.
  On labwc only Pelorus's window list was seen, so the first step on this branch is a manual smoke test of pixelflux `screenshot`, `left_click` and `type` in the chrome image; if it fails there, stop and report.
- `doctor` reports the backend and whether its server answered.

Acceptance:

- Unit tests for request building (action names, coordinate fields, header present) against a local fake HTTP server; token absent from every returned message.
- Live, in the webtop and in the chrome image, with no browser viewing the desktop:
  - no consent dialog appears at any point;
  - a coordinate click on a taskbar icon (webtop) or on Chrome's address bar (chrome) works at scale 1 and at scale 2;
  - `type_text` into Konsole and into Chrome's address bar delivers the exact text;
  - a screenshot taken after an action shows that action's result.
    If pixelflux returns stale frames when nobody is viewing, record it as a known limitation with the evidence rather than working around it silently.

### 4. `feat/mcp-http-listen`

Problem: stdio only; an MCP client outside the container cannot connect.

Change: `computer-use-linux mcp --listen <addr:port>` serves the same MCP server over rmcp's streamable HTTP server transport (`transport-streamable-http-server` feature of rmcp 3.5.1).

- Refuses to start unless `COMPUTER_USE_LINUX_MCP_TOKEN` is set and non-empty; every request needs `Authorization: Bearer <token>`, compared in constant time; wrong or missing token gets 401 and no tool runs.
- Plain HTTP; TLS is out of scope (see Transport security decision).
- stdio stays the default when `--listen` is absent.
- One server state shared by all HTTP sessions, so the existing input lock still serialises desktop actions.
- The on-screen indicator takes its agent name from the most recent session's `clientInfo`, my call.

Acceptance:

- Integration test: start with a token, `initialize` and `tools/list` succeed with the token, fail with 401 without it or with a wrong one.
- Test: missing or empty `COMPUTER_USE_LINUX_MCP_TOKEN` with `--listen` exits non-zero with a clear message.
- Live: from the host, an MCP client lists tools and runs `get_app_state` against the published port.

### 5. `feat/labwc-windows`

Problem: on labwc, `list_windows`, `focused_window` and window targeting fail.

Change:

- A labwc window backend in the registry using `$XDG_RUNTIME_DIR/labwc.sock`: `GET_WINDOWS` for listing, `GET_FOCUSED_WINDOW` for focus state, with timeouts like the other backends.
- Activation through the `wlr-foreign-toplevel-management` Wayland protocol (labwc supports it; the crate already depends on `wayland-protocols-wlr`); if that proves unavailable, listing still works and activation reports unsupported.
- `doctor` stops reporting AT-SPI as disabled when the accessibility bus is reachable and the toolkit bridges are forced on by environment, which is how the baseimage enables it.

Acceptance:

- Unit tests parse the observed `GET_WINDOWS` JSON (`[{"pid":510,"x":0,"y":0,"width":1024,"height":768,"title":"New Tab - Google Chrome","app_id":"google-chrome","minimized":false,"fullscreen":false,"maximized":true,"tiled":false,"focused":true}]`) and the `{"error":"unknown command"}` reply.
- Live in the chrome image: `list_windows` returns the Chrome window with backend `labwc`; `doctor` shows `can_query_windows: true` and no AT-SPI blocker.

### 6. `feat/linuxserver-mod`

Packaging only; it lives in Berne's fork and is not expected to go upstream.

- A Dockerfile (heredoc `RUN <<-EOF` style) that builds the static musl binary from the repo and produces a `FROM scratch` image laid out from `/`, so the same image works both as a linuxserver docker mod (`DOCKER_MODS=…`) and as `COPY --link --from=<image> / /` in a Dockerfile.
- A `docker bake` target for it.
- Contents:
  - the binary at `/usr/local/bin/computer-use-linux`;
  - a oneshot init (s6-rc, before `svc-de`) that, unless already set, writes `PIXELFLUX_CU=5000` and a random `PIXELFLUX_CU_TOKEN`, plus the accessibility variables (`QT_ACCESSIBILITY`, `QT_LINUX_ACCESSIBILITY_ALWAYS_ON`, `GTK_MODULES`), into `/run/s6/container_environment/`, so no `PELORUS=true` is needed;
  - a longrun `svc-computer-use-linux` that waits for the compositor, takes the session bus address, `WAYLAND_DISPLAY` and `XDG_RUNTIME_DIR` from the compositor process's environment, makes sure the accessibility bus is enabled (and the registry running on labwc), then `exec`s `computer-use-linux mcp --listen` as `abc`; it exits when the session goes away so s6 restarts it.
- Configuration through environment: `COMPUTER_USE_LINUX_MCP_TOKEN` (required), `COMPUTER_USE_LINUX_MCP_LISTEN` (default `0.0.0.0:5200`, my call).
- The on-screen indicator is off by default in the mod and opt-in with `COMPUTER_USE_LINUX_INDICATOR=1` in the container's environment (Berne's request); the service passes the variable through, and the mod ships the `computer-use-linux-indicator` binary (static, next to the main one) so opting in works.
  Acceptance: with `COMPUTER_USE_LINUX_INDICATOR=1`, a click shows the overlay in the webtop (KWin) and the chrome image (labwc) as seen through the Selkies web view, and screenshots still do not contain it; without the variable, no indicator process starts.
- A README section with a compose example for a stock image (`DOCKER_MODS`) and a Dockerfile example for burp-selkies.

Acceptance:

- Live, without `PELORUS=true`: stock webtop with `DOCKER_MODS`, stock chrome with `DOCKER_MODS`, and burp-selkies rebuilt with `COPY --from`, each with the port published, all pass `tools/list` and a `get_app_state` from the host, and the build-3 and build-5 live checks.
- A container without `COMPUTER_USE_LINUX_MCP_TOKEN` logs why the service is not starting, and the port is closed.

### `berney/integration`

Starts from `main`, holds this document, and merges the six branches in order.
After the last merge, the full gate list in `CONTRIBUTING.md` passes, and the mod image built from this branch passes branch 6's acceptance checks.

## Constraints

- Every commit is SSH-signed; never `--no-verify` or `--no-gpg-sign`.
- Each branch passes upstream's gates before it counts as done: `cargo fmt --all -- --check`, `cargo check --locked --all-targets`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --no-fail-fast`, plus `scripts/mcp_safety_check.py` and `scripts/check_plugins.sh` when tools or plugin files change.
- Test first for new logic.
- New or changed MCP tools keep their safety annotations; user-facing changes update `README.md`, `npm/README.md` and `skills/computer-use-linux/SKILL.md` as `CONTRIBUTING.md` asks.
- Live tests run in throwaway containers published on `127.0.0.1` only, removed afterwards.
  The existing `webtop` (3101) and `pelorus` (3003) containers may be recreated for tests; the `pelorus` container's open windows show it is in use, so leave it alone.
- Pushing, PRs and publishing images stay with Berne.

## Decisions I made (not asked of Berne)

- Branch names and order as above.
- Commits on the upstream-bound branches use upstream's prefixes (`fix:`, `feat:`), because they are meant to become upstream PRs; this departs from Berne's usual `[area] Summary` style.
- The mod image name and registry are left to Berne at publish time; the bake target uses a local tag.
- Default listen address `0.0.0.0:5200`, token variable `COMPUTER_USE_LINUX_MCP_TOKEN`.
- The labwc AT-SPI `doctor` fix rides on the labwc branch rather than getting its own.
- Gates run with the host's cargo, as upstream documents, rather than a new dev container in the fork.
- The static musl build loses the GNOME-only libxkbcommon keymap preflight (`src/keyboard_keymap.rs`, loaded with `dlopen`, which a static musl binary cannot do); that is acceptable because no target here is GNOME, and the binary must report it as unavailable rather than crash.
  The static build must otherwise pass the live checks in Alpine (webtop) and Debian (chrome, burp-selkies) images.

## Left to the builder

- Internal module layout, type names and error wording.
- How the pixelflux backend finds the display scale, within the rule that it refuses when unsure.
- Exact s6-rc dependency wiring, as long as the service starts after the desktop and restarts with it.

## Transport security decision

- Plain HTTP with the bearer token, as in branch 4; the port is published only on loopback, a private network or the tailnet, and the README examples publish it on `127.0.0.1`.
- TLS through the selkies nginx on 3001 (a `/mcp` route, optional basic auth) is a possible later branch, not part of this work.
