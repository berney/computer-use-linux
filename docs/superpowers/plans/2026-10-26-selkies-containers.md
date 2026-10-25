# computer-use-linux in Selkies containers: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make one static `computer-use-linux` binary, started by s6 inside any Selkies Wayland image, serve MCP over an authenticated HTTP port with working screenshots, clicks, keys and text on KWin and labwc, without Pelorus.

**Architecture:** Six independent branches off upstream `main`, each a candidate upstream PR except the packaging branch, merged in order into `berney/integration`.
Input and screenshots inside Selkies go through pixelflux's loopback computer-use server; windows on labwc come from its IPC socket; the transport gains a streamable-HTTP listener with a bearer token; packaging is a `FROM scratch` linuxserver docker mod.

**Tech Stack:** Rust 2021 (tokio current-thread, rmcp 3.5.1, zbus, atspi, wayland-client and wayland-protocols-wlr), s6-overlay v3, Docker Buildx bake.

**Spec:** `docs/superpowers/specs/2026-10-26-selkies-containers-design.md` (on `berney/integration`).

## Global Constraints

- Each branch starts from `main` (`7f6867e` or later) in its own worktree `.worktrees/<leaf>` of `~/co/git/agent-sh/computer-use-linux`, for example `git worktree add -b fix/kde-terminal-paste .worktrees/kde-terminal-paste main`.
- One branch at a time, in task order; a fresh implementer subagent per task and a reviewer subagent after each task.
- After each branch is finished: `echo "<branch>: <one-line result>" | notify`.
- Commits are SSH-signed (repo config already set); never `--no-verify` or `--no-gpg-sign`.
- Commit prefixes on the six feature branches follow upstream: `fix:`, `feat:`, `test:`, `docs:`; the integration branch uses `[area] Summary`.
- Gates per branch before it is done: `cargo fmt --all -- --check`, `cargo check --locked --all-targets`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --no-fail-fast`; plus `scripts/mcp_safety_check.py` and `scripts/check_plugins.sh` when tool definitions or plugin files change.
  New dependencies may update `Cargo.lock`; use `cargo check` without `--locked` once to write it, then the gates with `--locked`.
- User-facing changes update `README.md`, `npm/README.md` and `skills/computer-use-linux/SKILL.md`, then copy the skill into `plugins/computer-use-linux/skills/` (enforced by `scripts/check_plugins.sh`).
- New and changed MCP tools keep their safety annotations.
- Test first: write the failing test, watch it fail, then implement.
- Live tests use a static build made with `docker run --rm -v "$PWD":/src:ro,Z -v /tmp/cul-musl:/out:Z -w /src docker.io/library/rust:alpine sh -c 'apk add -q musl-dev && cargo build -q --release --locked --target-dir /out --bins'`, copied into the container with `docker cp` and run as `abc` with the session environment read from the compositor process (`/proc/$(pgrep -u abc -x kwin_wayland || pgrep -u abc -x labwc)/environ`: `DBUS_SESSION_BUS_ADDRESS`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, `DISPLAY`, `XDG_SESSION_TYPE`, `XDG_CURRENT_DESKTOP`).
- Live containers: the existing `webtop` (`lscr.io/linuxserver/webtop:alpine-kde`, port 3101) may be recreated; throwaway ones publish only on `127.0.0.1` and are removed afterwards; leave the `pelorus` container (3003) alone.
  Scale 2 is the default when viewed from a HiDPI browser; scale 1 needs `-e SELKIES_SCALING_DPI=96`.
- No pushing, PRs or image publishing.

## Review Focus

1. Secrets in output: `PIXELFLUX_CU_TOKEN` and `COMPUTER_USE_LINUX_MCP_TOKEN` must never appear in a tool result, `doctor` output, log line or error, including errors from failed HTTP requests (Task 3.1, Task 4.1).
2. Display scale changing mid-session (the viewing browser resizes or changes device pixel ratio): a coordinate or element click after the change must use the new geometry or refuse, never land at a stale position (Task 3.2).
3. pixelflux configured but not answering (`PIXELFLUX_CU` set, server not started or wrong token): probe failure falls through to the existing backend chain; a failure after input was sent returns an error without replaying through another backend, matching upstream's existing rule (Task 3.1, Task 3.2).
4. Two MCP clients acting at once over HTTP: actions stay serialised by the existing input lock, and one client disconnecting mid-action does not wedge the other (Task 4.1).
5. Text with newlines, tabs and non-ASCII characters through pixelflux `type`: delivered exactly, or refused before anything is sent (Task 3.2).

---

## Branch 1: `fix/kde-terminal-paste`

### Task 1.1: Keep the terminal paste chord for non-text focus

**Files:**

- Modify: `src/server.rs` `kde_clipboard_shortcut_for_focus` (around line 7183).
- Test: `src/server.rs` test module, next to `kde_clipboard_focus_fallback_uses_the_role_enum_not_the_name` (around line 9551).

**Interfaces:**

- Consumes: `FocusedElementSummary { role, name, editable, states, is_terminal }` (`src/atspi_tree.rs:444`), `focused_element_is_editable(&FocusedElementSummary) -> bool` (`src/server.rs` around 7558), test helper `focused(role: &str, is_terminal: bool)`.
- Produces: unchanged signature `fn kde_clipboard_shortcut_for_focus(terminal: Option<TerminalPasteShortcut>, focused_element: Option<&FocusedElementSummary>) -> KdeClipboardPasteShortcut`.
- [ ] **Step 1: Check Konsole's search field live.**
  In the webtop, open Konsole's Find bar (Ctrl+Shift+F), then run `computer-use-linux state konsole` and record the focused element's `role`, `editable` and `states`.
  Expected: a `text` (or `entry`) role; if it is something else, add that role to the list in Step 4 and note it in the commit body.
- [ ] **Step 2: Write failing tests** `kde_terminal_keeps_chord_for_non_text_focus`:
  - `kde_clipboard_shortcut_for_focus(Some(CtrlShiftV), Some(&focused("filler", false)))` == `KdeClipboardPasteShortcut::CtrlShiftV`;
  - same with `focused("panel", false)` == `CtrlShiftV`;
  - `Some(ShiftInsert)` with `focused("filler", false)` == `ShiftInsert`;
  - `Some(CtrlShiftV)` with `focused("text", false)` == `Standard`;
  - `Some(CtrlShiftV)` with a `filler` whose `editable` is true == `Standard`.
- [ ] **Step 3: Run** `cargo test --locked kde_terminal_keeps_chord_for_non_text_focus`; expected FAIL on the `filler` and `panel` cases.
- [ ] **Step 4: Implement:** return `Standard` only when the element is not a terminal and either `focused_element_is_editable(element)` or its role is one of `text`, `entry`, `password text`, `combo box`; otherwise fall through to the terminal chord.
- [ ] **Step 5: Run** `cargo test --locked kde_clipboard`; expected all pass, including the existing fallback tests.
- [ ] **Step 6: Live check** in the webtop: `type_text` `echo cul-ok` into Konsole, `press_key Return`; Konsole's AT-SPI text contains `cul-ok`, and the Find bar still pastes with `type_text`.
- [ ] **Step 7: Gates, then commit** `fix: keep the terminal paste chord when Konsole focus is not a text field`, and notify.

## Branch 2: `fix/kde-nested-scale`

### Task 2.1: Reproduce on current main

**Files:** none changed.

- [ ] **Step 1:** Build `main` statically, start the webtop without `SELKIES_SCALING_DPI` and view it from a HiDPI browser so the log shows `scale 2.00`.
- [ ] **Step 2:** Through MCP (stdio over `docker exec -i`), take a `screenshot`, then `click` the Dolphin taskbar icon at its screenshot coordinates; approve the Remote Control prompt once through Pelorus or the web view.
  Expected on a broken build: `could not be mapped safely to the complete shared desktop`.
  If it opens Dolphin, upstream `#291` or later fixed it: stop the branch, record the evidence in the integration branch's spec under Current state, notify, and go to Branch 3.
- [ ] **Step 3:** Record `kscreen-doctor -j` output, the portal stream `position` and `size` (add a temporary debug print or read `doctor` output), and KWin's `desktopGeometry` from `computer-use-linux windows --verbose` or the KWin script output, as test fixtures.

### Task 2.2: Take the KDE logical layout from KWin

**Files:**

- Modify: `src/remote_desktop.rs` `logical_desktop_layout` (around line 227), KDE branch.
- Modify (if per-output data is needed): `src/windowing/backends/kwin.rs` to expose per-output logical rectangles from `KwinWindowSnapshot.outputs`.
- Test: `src/remote_desktop.rs` tests near `capture_points_scale_to_portal_logical_stream_space`; `src/windowing/backends/kwin.rs` tests near the `parse_kwin_logical_desktop_rect` test.

**Interfaces:**

- Consumes: `kwin::logical_desktop_rect() -> Result<(i32, i32, i32, i32)>`, `parse_kscreen_monitor_layout(&[u8]) -> Option<Vec<LogicalMonitor>>`, `map_capture_point_to_stream_layout(...)`.
- Produces: `pub(crate) fn kwin_output_layout(json: &str) -> Result<Vec<LogicalMonitor>>` in `kwin.rs` (logical `x, y, width, height` per enabled output, `scale` from KWin's output scale), used by `logical_desktop_layout` in preference to `kscreen-doctor` when KWin answers; `kscreen-doctor` stays the fallback.
- [ ] **Step 1: Failing test** `kwin_layout_maps_nested_scale_two_capture`: streams `[(0,0) 1432x768]`, layout from `kwin_output_layout` on a fixture with one output at logical `0,0 1432x768` scale 2, capture 2864x1536; `map_capture_point_to_stream_layout(.., 386, 1474, 2864, 1536)` == `Some((193, 737))`.
- [ ] **Step 2: Failing test** `kwin_output_layout_parses_outputs`: the Task 2.1 fixture parses to one `LogicalMonitor { x: 0, y: 0, width: 1432, height: 768, scale: 2.0 }`; a two-output fixture parses to two monitors in KWin's order; a fixture without outputs is an `Err`.
- [ ] **Step 3: Run both;** expected FAIL (function missing).
- [ ] **Step 4: Implement** `kwin_output_layout` and use it in `logical_desktop_layout`'s KDE branch.
  The existing kscreen tests (`parses_kscreen_physical_sizes_in_logical_coordinates`, `kscreen_layout_excludes_mirrored_outputs`) stay unchanged and passing.
- [ ] **Step 5: Run** `cargo test --locked remote_desktop kwin`; expected PASS.
- [ ] **Step 6: Live check:** repeat Task 2.1 Step 2; Dolphin opens at scale 2, and at scale 1 (`SELKIES_SCALING_DPI=96`) too.
- [ ] **Step 7: Gates, commit** `fix: take the KDE logical layout from KWin for portal pointer mapping`, notify.

## Branch 3: `feat/pixelflux-backend`

### Task 3.0: Smoke-test pixelflux on labwc

**Files:** none changed.

- [ ] **Step 1:** Start `lscr.io/linuxserver/chrome:latest` with `-e PELORUS=true -e PIXELFLUX_WAYLAND=true -p 127.0.0.1:3201:3001`; read `PIXELFLUX_CU_TOKEN` from `/run/s6/container_environment/`.
- [ ] **Step 2:** Inside the container, `curl` `POST http://127.0.0.1:5000/computer-use` with the bearer token: `{"action":"screenshot"}` returns PNG data; `{"action":"left_click","coordinate":[x,y]}` on the address bar, then `{"action":"type","text":"example.com\n"}` navigates (check with the AT-SPI tree or Pelorus `/api/windows` title).
  Record the exact response JSON shapes as fixtures for Task 3.1.
  If any action fails, stop the branch and report with the evidence.
- [ ] **Step 3:** Repeat one screenshot with no browser viewing the desktop, after a click that changes the screen; record whether the frame is fresh.

### Task 3.1: pixelflux client

**Files:**

- Create: `src/pixelflux.rs` (client, config, request types).
- Modify: `src/lib.rs` (module declaration).
- Test: `src/pixelflux.rs` test module with a local fake HTTP server on `127.0.0.1:0` (tokio `TcpListener`).

**Interfaces:**

- Produces:
  - `pub(crate) struct PixelfluxConfig { port: u16, token: String }` with `pub(crate) fn from_env() -> Option<PixelfluxConfig>` reading `PIXELFLUX_CU` and `PIXELFLUX_CU_TOKEN`; `None` unless both are set and non-empty; `PIXELFLUX_CU` accepts a bare port or `127.0.0.1:<port>`/`[::1]:<port>`/`localhost:<port>` only, anything else is `None` with a warning that names the variable, not its value.
  - `pub(crate) enum PixelfluxAction { Screenshot, Click { x: i32, y: i32, button: PointerButton, count: u8 }, MouseMove { x, y }, MouseDown { x, y }, MouseUp { x, y }, Drag { from: (i32, i32), to: (i32, i32) }, Scroll { x: i32, y: i32, direction: ScrollDirection, amount: u32 }, Key { chord: String }, Type { text: String } }`, mapped to the wire actions observed in Task 3.0 (`left_click`, `right_click`, `middle_click`, `double_click`, `triple_click`, `mouse_move`, `left_mouse_down`, `left_mouse_up`, `left_click_drag`, `scroll`, `key`, `type`, `screenshot`).
  - `pub(crate) async fn send(config: &PixelfluxConfig, action: &PixelfluxAction) -> Result<PixelfluxReply>`; `PixelfluxReply { data: Option<Vec<u8>> }` (decoded screenshot bytes).
  - `pub(crate) async fn probe(config: &PixelfluxConfig) -> Result<()>` (a `cursor_position` request with a 1 s timeout).
- HTTP: hand-written HTTP/1.1 over `tokio::net::TcpStream` (loopback only, `Connection: close`, `Content-Length` body, 5 s timeout, response body capped at 64 MiB), so no new dependency; my call, because the server is loopback-only and fixed.
- [ ] **Step 1: Failing tests:**
  - `from_env_requires_port_and_token` (both missing, one missing, empty token → `None`; `5000` and `127.0.0.1:5000` → `Some`; `10.0.0.1:5000` → `None`);
  - `send_builds_click_request` (fake server sees `POST /computer-use`, `Authorization: Bearer secret`, body `{"action":"left_click","coordinate":[10,20]}`);
  - `send_decodes_screenshot` (fake returns the Task 3.0 fixture; bytes are a PNG of the stated size);
  - `errors_never_contain_token` (fake returns 401, then closes mid-body; the error `to_string()` and `{:#}` do not contain `secret`);
  - `type_preserves_text` (body for `"a\tb\nä€😉"` round-trips through `serde_json` unchanged).
- [ ] **Step 2: Run** `cargo test --locked pixelflux`; expected FAIL (module missing).
- [ ] **Step 3: Implement** `src/pixelflux.rs`.
- [ ] **Step 4: Run;** expected PASS.
- [ ] **Step 5: Commit** `feat: add a pixelflux computer-use client`.

### Task 3.2: Route screenshots and input through pixelflux

**Files:**

- Modify: `src/screenshot.rs` (`ScreenshotBackend` gains `Pixelflux`, `parse("pixelflux")`, `capture_screenshot_raw` tries it first when `PixelfluxConfig::from_env()` is `Some`).
- Modify: `src/server.rs` action paths: `click` (around 834), `scroll` (around 1375), `drag` (around 1739), `press_key` (around 1873), `type_text` (around 2038); a pixelflux arm before the abs-pointer, portal, xdotool, ydotool, wtype and KDE clipboard arms.
- Modify: `src/diagnostics.rs` (`doctor` input section reports `pixelflux` with `ok` from `probe`, detail naming the port only).
- Test: `src/server.rs` and `src/screenshot.rs` test modules.

**Interfaces:**

- Consumes: Task 3.1 `PixelfluxConfig`, `PixelfluxAction`, `send`, `probe`; `cache_desktop_size` (`src/server.rs` around 4513); `logical_desktop_layout()` from `src/remote_desktop.rs` for the scale (KWin from Branch 2 once merged, `kscreen-doctor` before that).
- Produces:
  - `fn pixelflux_point_for_element(bounds: (i32, i32, i32, i32), capture: (u32, u32), layout: &[LogicalMonitor]) -> Option<(i32, i32)>` (bounds are logical `x, y, width, height` from AT-SPI) in `src/server.rs`: centre of the element's logical bounds scaled by `capture / logical`, `None` when the scale is not uniform or the layout is empty.
  - On labwc, the layout comes from `wlr-output-management` via `pub(crate) async fn wlr_output_layout() -> Option<Vec<LogicalMonitor>>` in a new `src/wlr_outputs.rs` (bind `zwlr_output_manager_v1`, read each head's logical position, mode size and scale, done on the `done` event, 2 s timeout), added as a branch of `logical_desktop_layout` when `XDG_CURRENT_DESKTOP` or the compositor is labwc (wlroots).
- [ ] **Step 1: Failing tests:**
  - `screenshot_prefers_pixelflux_when_configured` (with a fake pixelflux server and env set, `capture_screenshot_raw` returns `source == "pixelflux"` and the fixture size);
  - `forced_backend_parses_pixelflux`;
  - `pixelflux_point_for_element_scales` (bounds logical `(166,714,54,46)`, capture 2864x1536, layout one monitor 1432x768 → `Some((386, 1474))`; capture equal to logical → unscaled centre; mismatched aspect → `None`);
  - `pixelflux_click_uses_new_geometry_after_resize` (cached capture 2864x1536, then a new screenshot at 1428x1536 with layout 714x768: the next element click maps with the new values);
  - `pixelflux_failure_after_send_is_not_replayed` (fake accepts the connection then returns 500: the tool result is an error, and no portal or ydotool call is recorded by the existing command-recording test hooks);
  - `pixelflux_probe_failure_falls_through` (env set, nothing listening: `click` takes the next existing backend, as in the current tests);
  - `wlr_output_layout` parsing of a recorded head set, if the protocol handling is split into a pure function.
- [ ] **Step 2: Run;** expected FAIL.
- [ ] **Step 3: Implement** the screenshot arm, the five action arms, the scale helper, `wlr_outputs.rs` and the `doctor` entry.
  Keys map from the server's chord syntax (`Ctrl+L`, `Return`) to pixelflux `key` text as observed in Task 3.0.
- [ ] **Step 4: Run** the new tests and `cargo test --locked`; expected PASS.
- [ ] **Step 5: Live checks,** with no browser viewing, in the webtop (KWin) and the chrome image (labwc), at scale 1 and scale 2:
  no consent dialog appears; a coordinate click on a taskbar icon (webtop) or Chrome's address bar works; an element `click` by name works; `type_text` delivers `echo cul-ok` into Konsole and `example.com` into Chrome's address bar exactly; a screenshot after an action shows its result (or the stale-frame limitation is recorded in README with Task 3.0 Step 3's evidence).
- [ ] **Step 6:** Document `PIXELFLUX_CU`, `PIXELFLUX_CU_TOKEN`, `COMPUTER_USE_LINUX_SCREENSHOT_BACKEND=pixelflux` and the backend order in `README.md` (Environment variables, Architecture), `npm/README.md` and the skill.
- [ ] **Step 7: Gates, commit** `feat: drive Selkies desktops through pixelflux computer use`, notify.

## Branch 4: `feat/mcp-http-listen`

### Task 4.1: Streamable HTTP listener with a bearer token

**Files:**

- Modify: `Cargo.toml` (rmcp features add `transport-streamable-http-server`; add the HTTP server crate rmcp's tower service needs, `axum` with default features off plus `http1` and `tokio`, or `hyper` and `hyper-util`, whichever is smaller in `Cargo.lock`).
- Modify: `src/cli.rs` (`mcp` accepts `--listen <addr:port>`), `src/server.rs` `serve_mcp` (around 2634).
- Create: `src/http_listen.rs` (listener, auth layer).
- Test: `tests/mcp_http_listen.rs` (integration test driving the built binary), unit tests in `src/http_listen.rs`.

**Interfaces:**

- Consumes: `ComputerUseLinux` (`Clone`, all state `Arc`-shared), `rmcp::transport::streamable_http_server::{StreamableHttpService, StreamableHttpServerConfig}`, `rmcp::transport::streamable_http_server::session::local::LocalSessionManager`.
- Produces:
  - `pub async fn serve_mcp(listen: Option<SocketAddr>) -> Result<()>`: `None` keeps today's stdio behaviour exactly.
  - `pub(crate) async fn serve_http(server: ComputerUseLinux, addr: SocketAddr, token: String) -> Result<()>` serving MCP at path `/mcp`.
  - `fn token_matches(header: Option<&HeaderValue>, token: &str) -> bool`: `Bearer <token>` only, constant-time comparison.
  - Env: `COMPUTER_USE_LINUX_MCP_TOKEN` (required with `--listen`).
- [ ] **Step 1: Failing tests:**
  - `token_matches_only_exact_bearer` (exact → true; missing, `Basic …`, wrong token, token plus suffix, lowercase `bearer` → as decided, default false);
  - `listen_requires_token` (binary with `mcp --listen 127.0.0.1:0` and no or empty `COMPUTER_USE_LINUX_MCP_TOKEN` exits non-zero, stderr names the variable);
  - `http_initialize_and_list_tools` (binary with a token on a free port: `initialize` plus `tools/list` succeed with `Authorization: Bearer t`; without it or with `Bearer x`, HTTP 401 and the body does not contain `t`'s value);
  - `http_sessions_share_input_lock` (two sessions issue a mutating call each; the calls do not overlap, checked with the existing input-lock test hook or timing on a fake slow action);
  - `http_client_disconnect_does_not_wedge` (session A starts a slow call and drops the connection; session B's next call completes).
- [ ] **Step 2: Run;** expected FAIL.
- [ ] **Step 3: Implement.**
  Each HTTP session gets `server.clone()`; the indicator name follows the most recent session's `clientInfo`.
  Bind failure, missing token and invalid address exit non-zero with a message; the token is never logged.
- [ ] **Step 4: Run;** expected PASS, and the existing stdio protocol tests (around `src/server.rs:8266`) unchanged.
- [ ] **Step 5: Live check:** in the webtop, `computer-use-linux mcp --listen 0.0.0.0:5200` with a token, container published `-p 127.0.0.1:5200:5200`; from the host, an MCP client (omp `.omp/mcp.json` streamable-http entry with the header, or a short `curl` sequence) lists tools and runs `get_app_state`.
- [ ] **Step 6:** Document `--listen` and `COMPUTER_USE_LINUX_MCP_TOKEN` in README (CLI list, Generic MCP client, Environment variables, safety contract note that the port is plain HTTP and belongs on loopback or a private network), `npm/README.md`, the skill.
- [ ] **Step 7: Gates (including `scripts/mcp_safety_check.py`), commit** `feat: serve MCP over streamable HTTP with a bearer token`, notify.

## Branch 5: `feat/labwc-windows`

### Task 5.1: labwc window backend

**Files:**

- Create: `src/windowing/backends/labwc.rs`.
- Modify: `src/windowing/backends/mod.rs`, `src/windowing/registry.rs` (`BackendKind::Labwc`, `BACKEND_ORDER` before `X11`, `DESCRIPTORS` entry, `LABWC_BACKEND` re-export, `WINDOW_PERMISSION_HINT` text, match arms in `list_windows_for`, `activate_window`, `focused_window_for_backend`, `probe_backends`), `src/diagnostics.rs` windowing text (around 896).
- Test: `src/windowing/backends/labwc.rs` test module.

**Interfaces:**

- Consumes: `WindowInfo`, `WindowBounds` (`src/windowing/types.rs`).
- Produces:
  - `pub(crate) const LABWC_BACKEND: &str = "labwc";`
  - `pub(crate) async fn list_windows() -> Result<Vec<WindowInfo>>` (`GET_WINDOWS`), `pub(crate) async fn focused_window() -> Result<Option<WindowInfo>>` (`GET_FOCUSED_WINDOW`), `pub(crate) async fn activate(window: &WindowInfo) -> Result<()>`.
  - `fn parse_windows(json: &str) -> Result<Vec<WindowInfo>>`, `fn parse_window(json: &str) -> Result<Option<WindowInfo>>`.
  - Socket `$XDG_RUNTIME_DIR/labwc.sock`, one command plus `\n` per connection, read to EOF, 2 s timeout, reply capped at 4 MiB; `{"error": …}` replies become `Err` with the message.
  - `window_id` is a stable hash of `pid` and `app_id` (labwc gives no id), my call; `hidden = minimized`; `backend = "labwc"`; `client_type = None`.
- [ ] **Step 1: Failing tests:** `parses_labwc_windows` (the spec's `GET_WINDOWS` fixture → one window, title `New Tab - Google Chrome`, app_id `google-chrome`, pid 510, bounds `0,0 1024x768`, focused, not hidden); `labwc_error_reply_is_err` (`{"error":"unknown command"}`); `empty_focused_reply_is_none` (`{}` and `null`); `window_id_is_stable`.
- [ ] **Step 2: Run;** expected FAIL.
- [ ] **Step 3: Implement** listing and focus over the socket; `activate` binds `zwlr_foreign_toplevel_manager_v1` (from `wayland-protocols-wlr`, `wlr-foreign-toplevel-management-unstable-v1`), matches the toplevel by `app_id` and title, calls `activate` with the first seat, and returns an "activation unsupported" error when the global is absent.
- [ ] **Step 4: Run;** expected PASS.
- [ ] **Step 5: Live check** in the chrome image: `computer-use-linux windows` lists Chrome with backend `labwc`; with two Chrome windows open, `activate_window` on the background one makes `GET_FOCUSED_WINDOW` return it.
- [ ] **Step 6: Commit** `feat: list and focus labwc windows over its IPC socket`.

### Task 5.2: AT-SPI enabled by environment counts in `doctor`

**Files:**

- Modify: `src/diagnostics.rs` (`can_build_accessibility_tree` around 1116 and the blocker around 993).
- Test: `src/diagnostics.rs` test module (helper `accessibility_report` around 1848).

**Interfaces:**

- Produces: `fn toolkit_bridge_forced_by_env(vars: impl Fn(&str) -> Option<String>) -> bool`: true when `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1` or `GTK_MODULES` contains `atk-bridge`; read from the server's environment and, when set, the compositor process's environment.
- [ ] **Step 1: Failing test** `bus_reachable_and_forced_bridge_is_enabled`: bus ok, `IsEnabled` false, toolkit setting false, forced bridge true → `can_build_accessibility_tree` true and no "disabled" blocker; bus not ok → still false.
- [ ] **Step 2: Run;** FAIL.
      **Step 3: Implement.**
      **Step 4: Run;** PASS.
- [ ] **Step 5: Live:** in the chrome image, `doctor` shows `can_query_windows: true`, `can_build_accessibility_tree: true`, no AT-SPI blocker.
- [ ] **Step 6:** README support-matrix row for labwc; gates; commit `fix: count toolkit bridges forced by environment as AT-SPI enabled`; notify.

## Branch 6: `feat/linuxserver-mod`

### Task 6.1: Mod image

**Files:**

- Create: `docker/linuxserver-mod/Dockerfile`, `docker-bake.hcl` (target `linuxserver-mod`, local tag `computer-use-linux-mod:dev`).
- Create under `docker/linuxserver-mod/root/`:
  - `etc/s6-overlay/s6-rc.d/init-mod-computer-use-linux/{type,up,run}` (oneshot) and `dependencies.d/init-selkies-config`, plus a `contents.d` entry in `init-mods-end` or `user/contents.d`, following the docker-mods template;
  - `etc/s6-overlay/s6-rc.d/svc-mod-computer-use-linux/{type,run}` (longrun), `dependencies.d/init-mod-computer-use-linux`, `dependencies.d/svc-de`, and `user/contents.d/svc-mod-computer-use-linux`.
- Create: `docker/linuxserver-mod/README.md`.

**Interfaces:**

- Consumes: the `computer-use-linux` binary with `mcp --listen` (Branch 4), the pixelflux backend (Branch 3), labwc windows (Branch 5), `computer-use-linux-indicator`.
- Produces: an image whose filesystem is exactly:
  - `/usr/local/bin/computer-use-linux`, `/usr/local/bin/computer-use-linux-indicator` (static musl);
  - the s6 files above.
- Init behaviour, each only when the variable is unset: write `PIXELFLUX_CU=5000`, a 64-hex `PIXELFLUX_CU_TOKEN` (`openssl rand -hex 32`, as `init-selkies-config` does), `QT_ACCESSIBILITY=1`, `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1`, `GTK_MODULES=gail:atk-bridge`, `COMPUTER_USE_LINUX_INDICATOR=0` into `/run/s6/container_environment/`; if `COMPUTER_USE_LINUX_MCP_TOKEN` is unset or empty, log `computer-use-linux: COMPUTER_USE_LINUX_MCP_TOKEN is not set; MCP server disabled` and mark the service down.
- Service behaviour: wait for `kwin_wayland` or `labwc` owned by `abc`; import `DBUS_SESSION_BUS_ADDRESS`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, `DISPLAY` from its `/proc/<pid>/environ`; set `org.a11y.Status IsEnabled true` on that bus; on labwc start `/usr/libexec/at-spi2-registryd` if no registry owns `org.a11y.atspi.Registry`; `exec s6-setuidgid abc computer-use-linux mcp --listen "${COMPUTER_USE_LINUX_MCP_LISTEN:-0.0.0.0:5200}"`; a watcher ends the service when the compositor pid exits, so s6 restarts it against the new session.
- Dockerfile shape: `# syntax=docker/dockerfile:1`; stage `build` `FROM docker.io/library/rust:alpine` with `RUN <<-EOF` (tab-indented) `apk add musl-dev` and `cargo build --release --locked --bin computer-use-linux --bin computer-use-linux-indicator` using cache mounts for the registry and target; final `FROM scratch` with `COPY --link` of the binaries and `root/`.
- [ ] **Step 1:** Write `docker/linuxserver-mod/README.md` with the two usage forms (compose `DOCKER_MODS` for webtop and chrome; `COPY --link --from=computer-use-linux-mod:dev / /` for burp-selkies), the environment table (`COMPUTER_USE_LINUX_MCP_TOKEN` required, `COMPUTER_USE_LINUX_MCP_LISTEN` default `0.0.0.0:5200`, `COMPUTER_USE_LINUX_INDICATOR=1` opt-in), and ports published on `127.0.0.1` in every example.
- [ ] **Step 2:** Write the Dockerfile, bake target and s6 files; `docker bake linuxserver-mod` succeeds and `docker create` plus `docker export` of the image lists exactly the produced paths.
- [ ] **Step 3: Live check, without `PELORUS=true`:**
  - stock webtop and stock chrome with `DOCKER_MODS` pointing at the local image (push to a local registry such as `registry:2` on `127.0.0.1` if `DOCKER_MODS` needs a registry), and burp-selkies rebuilt with `COPY --link --from`;
  - each with `-p 127.0.0.1:5200:5200` and a token: `tools/list` and `get_app_state` from the host, then the Task 3.2 Step 5 and Task 5.1 Step 5 checks;
  - a container with no `COMPUTER_USE_LINUX_MCP_TOKEN` logs the disabled message and nothing listens on 5200;
  - with `COMPUTER_USE_LINUX_INDICATOR=1` a click shows the overlay in the web view and a screenshot does not contain it; without it, no `computer-use-linux-indicator` process exists;
  - restarting the desktop (`kill` the compositor) brings the service back on the new session bus.
  - `doctor` from the static binary runs without crashing and reports the GNOME keymap preflight (libxkbcommon) as unavailable.
- [ ] **Step 4:** Commit `[docker] Add a linuxserver docker mod for computer-use-linux` (fork-only branch, Berne's style), notify.

## Integration: `berney/integration`

### Task 7.1: Merge and verify

- [ ] **Step 1:** In `.worktrees/integration`, merge the six branches in order with `git merge --no-ff` (signed merge commits); resolve conflicts, chiefly `src/server.rs` action paths and `README.md`.
- [ ] **Step 2:** Full `CONTRIBUTING.md` gate list passes.
- [ ] **Step 3:** `docker bake linuxserver-mod` from this branch; repeat Task 6.1 Step 3.
- [ ] **Step 4:** Update the spec's Current state table with the new results, commit `[docs] Record Selkies support results`, notify `berney/integration: all branches merged and verified`.
