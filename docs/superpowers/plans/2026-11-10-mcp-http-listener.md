# MCP HTTP listener: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `computer-use-linux mcp --listen ADDR:PORT` match the design the maintainer said he would merge in #362, so a client outside a Kubernetes pod can drive the desktop over authenticated HTTP.

**Architecture:** The listener stays in `src/http_listen.rs` and is configured by `src/cli.rs`.
It hands every HTTP session and stateless request a clone of one `ComputerUseLinux`, so the input lock, snapshot cache and other state stay shared, and a flag on that server switches on the HTTP-only rules (required `snapshot_id`, request-scoped post-action trees, no cache clear on a failed read).
Connections go through hyper's HTTP/1 builder so the server can bound header reads.

**Tech Stack:** Rust 2021, tokio, rmcp 3.9.0 (`transport-streamable-http-server`), axum 0.8, hyper 1 and hyper-util 0.1 (already in `Cargo.lock`), sha2 0.11.

**Spec:** `docs/superpowers/specs/2026-11-10-mcp-http-listener-requirements.md` (on `berney/integration`): the maintainer's two comments on #362, verbatim.

## Global Constraints

- Work on `feat/mcp-http-listen` in `.worktrees/mcp-http-listen`, based on upstream `main` at v0.7.49 (`6b288a6`).
- Commits are SSH-signed with upstream's prefixes (`feat:`, `fix:`, `test:`, `docs:`); never `--no-verify` or `--no-gpg-sign`.
- Gates before each commit: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --no-fail-fast`, `python3 scripts/generate_pi_tool_catalog.py --binary target/debug/computer-use-linux --check`, `bash scripts/check_plugins.sh`.
- A new dependency updates `Cargo.lock` by running `cargo check` once from upstream's lock, never `cargo update`; the lock diff against `main` only adds packages (rmcp stays 3.9.0).
- No cargo feature: the listener is always compiled in and off unless `--listen` is passed.
- Stdio behaviour does not change: `snapshot_id` stays optional, a failed tree read still clears the cache, and post-action trees still use the cached scope.
- Plain HTTP only; no TLS code.
- Tokens never appear in tool results, `doctor` output, log lines or errors.
- No test may send input to the developer's desktop: every process a test starts gets the hermetic environment from Task 1.
- Tool schemas do not change except where a task says so; the Pi catalog check must pass without regenerating unless a task changes a schema.
- No pushing or PR until the user asks.

## Review Focus

1. A bind to `0.0.0.0:5200` or `[::]:5200` is not loopback, so it needs `--allowed-host` (Task 2 test).
2. A `Host` header with a port (`desktop.ns.svc:5200`) or in mixed case passes when `--allowed-host desktop.ns.svc` was given (Task 3 test).
3. A token file that is missing, unreadable or empty stops startup with an error naming the variable and path, never the file's content (Task 2 test).
4. `Authorization: Bearer` with no credentials, or with several spaces before them, is handled: no credentials gets 401, extra spaces are accepted (Task 3 test).
5. Over HTTP, calls that do not pick an element from the cached tree still work without `snapshot_id`: an `element_identifier`, a capture-space `x`/`y` click, `press_key` (Task 5 test).

---

### Task 1: Hermetic listener tests, and drop the lock-hold hook

**Files:**

- Modify: `tests/mcp_http_listen.rs` (helpers `spawn`, `bound_port`, `post`, `initialize`; remove `http_sessions_share_input_lock`)
- Modify: `src/server.rs:1334-1340` (remove the `CUL_INPUT_LOCK_HOLD_MS` block in `dispatch_click`)
- Modify: `scripts/check_env_forwarding.py:69` (remove the `CUL_INPUT_LOCK_HOLD_MS` entry; lines 375-378 fail on stale entries)

**Interfaces:**

- Produces: in `tests/mcp_http_listen.rs`, `struct Hermetic { dir: PathBuf }` (a fresh `std::env::temp_dir()` subdirectory, mode `0700`, removed on `Drop`; the crate has no `tempfile` dependency) with `fn new() -> Hermetic`, `fn command(&self) -> std::process::Command` (the binary with `env_clear()` and the environment below) and `fn recorded(&self) -> String` (the stub command log).
  Every later task's integration tests start the server through `Hermetic::command()`.

The environment copies `PointerChildHarness::run` (`src/server.rs:11373-11478`), because `diagnostics.rs:574-603` fills only empty keys from the process tree: `PATH`, `HOME` and `XDG_RUNTIME_DIR` set to the temp dir; `DBUS_SESSION_BUS_ADDRESS=unix:path=<dir>/no-bus`; `DISPLAY=:9876`; `WAYLAND_DISPLAY=no-wayland`; `XDG_SESSION_TYPE=wayland`; `CU_DISABLE_ABS_POINTER=1`; `DESKTOP_SESSION`, `XDG_SESSION_DESKTOP`, `XDG_CURRENT_DESKTOP`, `HYPRLAND_INSTANCE_SIGNATURE` and `XAUTHORITY` set to dummy values; stub `ydotool`, `xdotool`, `wtype`, `systemctl` and `gnome-screenshot` scripts in the dir that append their argv to `<dir>/commands.log` and exit 1.

- [ ] **Step 1:** Add `Hermetic`, switch `spawn` to it, and add `assert_eq!(hermetic.recorded(), "")` at the end of `http_initialize_and_list_tools` and `http_client_disconnect_does_not_wedge`.
- [ ] **Step 2:** Delete `http_sessions_share_input_lock`, the hook in `dispatch_click`, and the `NOT_PASSED` entry.
- [ ] **Step 3:** Run `cargo test --locked --test mcp_http_listen` and the gates.
      Expected: 3 tests pass, `check_plugins.sh` passes, and `grep -rn CUL_INPUT_LOCK_HOLD_MS src tests scripts` prints nothing.
- [ ] **Step 4:** Commit `test: run the listener tests without a desktop session`.

### Task 2: Listener configuration, token source and refusals

**Files:**

- Modify: `src/cli.rs` (`parse_invocation` lines 171-184, `mcp_listen_addr` at 446-458, the `mcp` help entry at 25-30, test `mcp_listen_takes_one_address` at 507)
- Modify: `src/http_listen.rs` (new config and token functions)
- Modify: `src/server.rs:3631-3636` (`serve_mcp` takes the config)
- Modify: `scripts/check_env_forwarding.py:67` (add `COMPUTER_USE_LINUX_MCP_TOKEN_FILE` to `NOT_PASSED`, reason "read only by `mcp --listen`, which an MCP host never starts")
- Test: `src/cli.rs` tests, `src/http_listen.rs` tests, `tests/mcp_http_listen.rs`

**Interfaces:**

- Produces in `src/http_listen.rs`:
  - `pub(crate) struct ListenConfig { pub addr: SocketAddr, pub allowed_hosts: Vec<String>, pub token: String }`
  - `pub(crate) fn load_listen_token() -> anyhow::Result<String>`
  - `pub(crate) fn validate_listen(config: &ListenConfig, shell_enabled: bool) -> anyhow::Result<()>`
- Produces in `src/cli.rs`: `fn mcp_listen_config(positionals: &[String]) -> anyhow::Result<Option<ListenConfig>>`, replacing `mcp_listen_addr`.
- Produces in `src/server.rs`: `pub async fn serve_mcp(listen: Option<crate::http_listen::ListenConfig>) -> Result<()>`.

Parsing: for `mcp` only, `--listen ADDR` and repeatable `--allowed-host NAME` are removed from the options, and the positionals become `[ADDR, NAME...]` in that order after the count check.
`--allowed-host` without `--listen` is the error `'--allowed-host' needs --listen`; a missing value is `'--allowed-host' needs NAME`.
The help entry's arguments become `[--listen ADDR:PORT [--allowed-host NAME]...]`.

`load_listen_token` reads `COMPUTER_USE_LINUX_MCP_TOKEN_FILE` (a path) or `COMPUTER_USE_LINUX_MCP_TOKEN`, trims the result once, and then removes both variables from the process environment so children do not inherit them.
Errors, exactly: both set is `set only one of COMPUTER_USE_LINUX_MCP_TOKEN_FILE and COMPUTER_USE_LINUX_MCP_TOKEN`; neither set, or empty after trimming, is `--listen needs COMPUTER_USE_LINUX_MCP_TOKEN_FILE or COMPUTER_USE_LINUX_MCP_TOKEN`; a read failure is `could not read COMPUTER_USE_LINUX_MCP_TOKEN_FILE ({path}): {io error}`.
`run_from_env` loads the token right after parsing, before `diagnostics::hydrate_session_bus_env()`.

`validate_listen` refuses, exactly: shell enabled is `--listen is refused while COMPUTER_USE_LINUX_ENABLE_SHELL=1, because every HTTP client would get run_shell`; a bind whose IP is not loopback (`0.0.0.0` and `::` included) with no allowed host is `--listen {addr} is not a loopback address; name the hostnames clients use with --allowed-host`.

- [ ] **Step 1: Write the failing tests**
  - `cli.rs` `mcp_listen_takes_one_address`: add `["mcp","--listen","0.0.0.0:5200","--allowed-host","a.svc","--allowed-host","b.svc"]` gives `run mcp ["0.0.0.0:5200", "a.svc", "b.svc"]`, and the two new error strings.
  - `http_listen.rs` `token_file_is_trimmed_and_removed_from_the_environment`: a temp file containing `"secret\n"` loads as `"secret"`, and both variables are unset afterwards.
  - `http_listen.rs` `token_sources_refuse_ambiguity_and_absence`: both set, neither set, whitespace-only and a missing path each give their exact error; no error contains the file's content.
  - `http_listen.rs` `listen_refusals`: shell enabled; `0.0.0.0:5200`, `[::]:5200` and `10.0.0.5:5200` without hosts refused; `127.0.0.1:5200` and `[::1]:5200` without hosts accepted; `0.0.0.0:5200` with one host accepted.
  - `tests/mcp_http_listen.rs` `listen_refuses_shell`: `mcp --listen 127.0.0.1:0` with a token and `COMPUTER_USE_LINUX_ENABLE_SHELL=1` exits non-zero and stderr names the variable.
  - Tests that set environment variables hold `crate::pixelflux::TEST_ENV_LOCK` (`src/pixelflux.rs:509`).
- [ ] **Step 2:** Run them.
      Expected: compile errors or failures for the missing functions.
- [ ] **Step 3:** Implement the three functions, the parser change and `mcp_listen_config`; `serve_mcp` passes the config to `serve_http(server, config) -> anyhow::Result<()>` (Task 4 changes the return type) and no longer reads the token itself.
- [ ] **Step 4:** Run the tests and the gates.
      Expected: pass, including `check_plugins.sh` with the new `NOT_PASSED` entry.
- [ ] **Step 5:** Commit `feat: read the listener token from a file and refuse unsafe listener setups`.

### Task 3: Request checks: token digest, Host and Origin

**Files:**

- Modify: `src/http_listen.rs` (`token_matches` at 14-32, `unauthorized` at 78-80, config built at 48-58)
- Test: `src/http_listen.rs` tests (replace `token_matches_only_exact_bearer`, which asserts the opposite of the spec at line 98), `tests/mcp_http_listen.rs`

**Interfaces:**

- Consumes: `ListenConfig` (Task 2), `Hermetic` (Task 1).
- Produces: `pub(crate) fn token_digest(token: &str) -> [u8; 32]` and `pub(crate) fn bearer_matches(header: Option<&HeaderValue>, expected: &[u8; 32]) -> bool`, replacing `token_matches`.

`bearer_matches` splits the header at the first space, compares the scheme to `bearer` ignoring ASCII case, trims the credentials, refuses empty credentials, and compares the SHA-256 of the credentials to `expected` in constant time over all 32 bytes.
The 401 response carries `WWW-Authenticate: Bearer`.
The rmcp config is `StreamableHttpServerConfig::default()` with `.with_allowed_hosts(["localhost", "127.0.0.1", "::1"] + config.allowed_hosts)` and `.enforce_origin_validation()` with no allowed origins; `json_response` is no longer set, and `legacy_session_mode` keeps its default (`true`) so 2024-11-05 and 2025 clients keep sessions while 2026-07-28 requests run stateless.

- [ ] **Step 1: Write the failing tests**
  - `http_listen.rs` `bearer_matches_any_case_and_only_the_token`: `Bearer secret`, `bearer secret`, `BEARER secret` and `Bearer   secret` match; `Bearer secre`, `Bearer secrets`, `Bearer`, `Bearer `, `Basic secret`, `secret` and a missing header do not.
  - `mcp_http_listen.rs` `unauthorized_says_bearer`: a request with no token gets 401 with `www-authenticate: Bearer`.
  - `mcp_http_listen.rs` `allowed_and_unknown_hosts`: started with `--allowed-host desktop.ns.svc`, `initialize` with `Host: desktop.ns.svc:5200` and with `Host: Desktop.NS.svc` gets 200; `Host: evil.example` gets 403 even with the right token.
  - `mcp_http_listen.rs` `any_origin_is_refused`: `initialize` with the right token and `Origin: http://127.0.0.1` gets 403.
  - `mcp_http_listen.rs` `stateless_request_from_the_newer_protocol`: a `tools/list` POST with no session, header `MCP-Protocol-Version: 2026-07-28` and the token gets 200 with the tool list.
- [ ] **Step 2:** Run them.
      Expected: the case, header, Host and Origin tests fail.
- [ ] **Step 3:** Implement `token_digest`, `bearer_matches`, the 401 header and the rmcp config.
      `serve_http` stores the digest, never the token.
- [ ] **Step 4:** Run the tests and the gates.
      Expected: pass; every new test ends with `assert_eq!(hermetic.recorded(), "")`.
- [ ] **Step 5:** Commit `fix: check the listener token by digest and validate Host and Origin`.

### Task 4: Lifecycle: spill sweep, signals and a header-read timeout

**Files:**

- Modify: `src/http_listen.rs` (`serve_http`, the accept loop)
- Modify: `src/server.rs:3631-3660` (`serve_mcp`, HTTP branch)
- Modify: `Cargo.toml:66-67` (add `hyper = { version = "1.12", default-features = false, features = ["http1", "server"] }` and `hyper-util = { version = "0.1.21", default-features = false, features = ["tokio", "service"] }`, the versions already in `Cargo.lock`)
- Test: `tests/mcp_http_listen.rs`

**Interfaces:**

- Consumes: `ListenConfig` (Task 2).
- Produces: `pub(crate) async fn serve_http(server: ComputerUseLinux, config: ListenConfig) -> anyhow::Result<i32>` returning the signal number that stopped it; `pub(crate) const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(10)`.

`serve_mcp`'s HTTP branch runs `tree_spiller.sweep()` before serving, and after `serve_http` returns it runs `tree_spiller.remove_current()` and exits with `128 + signal`, as the stdio branch does at 3646-3659.
`serve_http` listens for SIGTERM and SIGINT, cancels the rmcp config's `cancellation_token` (a public field; clone it before building the service), stops accepting, and gives open connections up to 5 seconds to finish.
Each accepted connection is served by `hyper::server::conn::http1::Builder` with `hyper_util::rt::TokioTimer` and `header_read_timeout(HEADER_READ_TIMEOUT)`; `axum::serve` sets no timer, so hyper's timeout never fires there.

- [ ] **Step 1: Write the failing tests**
  - `sigterm_stops_the_listener`: start the server, `initialize`, send SIGTERM with `libc::kill`; the process exits within 5 seconds with status code `143`, and the port refuses new connections.
  - `slow_headers_are_dropped`: open a TCP connection and send `POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\n` with no blank line; the server closes it within `HEADER_READ_TIMEOUT + 2s` (the test takes about 10 seconds).
- [ ] **Step 2:** Run them.
      Expected: the SIGTERM test fails on the exit status, the slow-header test times out.
- [ ] **Step 3:** Implement the accept loop, the shutdown path and the `serve_mcp` changes.
- [ ] **Step 4:** Run the tests and the gates.
      Expected: pass; `Cargo.lock` only adds packages compared with `main`.
- [ ] **Step 5:** Commit `fix: give the listener the stdio lifecycle and a header-read timeout`.

### Task 5: Require `snapshot_id` over HTTP

**Files:**

- Modify: `src/server.rs` (`ComputerUseLinux` at 238-263, `check_element_snapshot` at 6917-6930, `get_tree_page` at 486-505, `serve_mcp`)
- Test: `src/server.rs` tests next to `an_index_from_a_replaced_snapshot_is_refused`, `tests/mcp_http_listen.rs`

**Interfaces:**

- Produces: a private field `require_snapshot_ids: bool` on `ComputerUseLinux` (default `false`), set to `true` by `serve_mcp` before it builds the HTTP service factory, so every clone carries it.

When the field is set and a call resolves an element from the cached tree (`names_cached_element` is true), a missing or blank `snapshot_id` is refused before anything else, with the message `{action} was not sent: snapshot_id is required over HTTP for element_index and selector calls, because another client's get_app_state can replace the cached tree. Pass the snapshot_id from your get_app_state result.` `get_tree_page` without a `snapshot_id` returns no nodes with the same reason.
Calls that do not resolve an element from the cache are unchanged.

- [ ] **Step 1: Write the failing tests**
  - `server.rs` `http_clients_must_name_their_snapshot`: one server with `require_snapshot_ids = true` and two clones; clone A stores a snapshot, clone B stores one; A's click by index with A's id is refused as replaced; a click by index with no id, and with `"  "`, is refused with the required message; a click by `element_identifier` with no id and a click with a capture-space `x`/`y` with no id pass the snapshot check.
  - `server.rs` `stdio_snapshot_id_stays_optional`: the same calls with the field unset behave as upstream's `an_index_from_a_replaced_snapshot_is_refused` expects.
  - `server.rs` `http_tree_page_needs_a_snapshot_id`.
  - `mcp_http_listen.rs` `http_index_click_without_snapshot_id_is_refused`: over HTTP, `click {"element_index": 0}` returns the required message and the stub log stays empty.
- [ ] **Step 2:** Run them.
      Expected: the HTTP cases fail.
- [ ] **Step 3:** Add the field, set it in `serve_mcp`, and extend `check_element_snapshot` and `get_tree_page`.
- [ ] **Step 4:** Run the tests and the gates.
      Expected: pass; the Pi catalog is unchanged.
- [ ] **Step 5:** Commit `feat: require snapshot_id for cached-element calls over HTTP`.

### Task 6: Request-scoped post-action trees and failed reads over HTTP

**Files:**

- Modify: `src/server.rs` (`capture_post_action_observation` at 5443-5454 and its cache reads at 5598-5606 and 5619-5625; its callers `finish_action_output`, `finish_set_value_output`, `finish_activate_window_output`, `finish_window_geometry_output`; `commit_cleared_snapshot` at 6891-6894)
- Test: `src/server.rs` tests

**Interfaces:**

- Consumes: `require_snapshot_ids` (Task 5).
- Produces: `capture_post_action_observation` and the four `finish_*` functions take an extra `named_snapshot: Option<&str>`, the `snapshot_id` the action named (or `None` for tools without one); `fn cached_scope_belongs_to_request(&self, cached: &CachedAccessibilitySnapshot, named_snapshot: Option<&str>) -> bool`.

`cached_scope_belongs_to_request` is true when `require_snapshot_ids` is false, or when the trimmed `named_snapshot` equals the cached id and that id is not empty.
Both cache reads in `capture_post_action_observation` (the pid fallback and `CachedTreeWindow`) use the cache only when it returns true, inside the same lock section that reads the cache; otherwise the post-action tree is scoped from the request's own target alone.
`commit_cleared_snapshot` does nothing when `require_snapshot_ids` is set: over HTTP a failed `get_app_state` or post-action read leaves the cached tree for the clients that hold its id, and the failing result carries no `snapshot_id`.
The `perform_action` recovery clear (`clear_snapshot`, ~7684) is unchanged.

- [ ] **Step 1: Write the failing tests**
  - `http_post_action_tree_ignores_another_clients_scope`: client B's snapshot with `window_scope` `"Window 20"` and pid 42 is cached; client A's action names no `snapshot_id`; the post-action tree is not narrowed to `"Window 20"` and records no pid from the cache.
    With the field unset, the same call keeps upstream's behaviour (scope taken from the cache).
  - `http_failed_read_keeps_the_cached_tree`: with the field set, a stored snapshot survives `commit_cleared_snapshot`, and its id still passes `check_element_snapshot`; with the field unset, the id is refused as "no tree is cached now".
- [ ] **Step 2:** Run them.
      Expected: both fail.
- [ ] **Step 3:** Implement `cached_scope_belongs_to_request`, thread `named_snapshot` through the four `finish_*` callers from each tool's params, and gate `commit_cleared_snapshot`.
- [ ] **Step 4:** Run the tests and the gates.
      Expected: pass, including upstream's post-action window tests unchanged.
- [ ] **Step 5:** Commit `fix: scope HTTP post-action trees by the request and keep other clients' trees`.

### Task 7: Indicator agent name per request

**Files:**

- Modify: `src/indicator.rs` (`Indicator::agent` at 315-329)
- Modify: `src/server.rs` (`impl ServerHandler for ComputerUseLinux`, ~3333-3354)
- Test: `src/indicator.rs` tests

**Interfaces:**

- Produces in `src/indicator.rs`: `tokio::task_local! { static REQUEST_AGENT: String; }` and `pub(crate) async fn with_request_agent<F: std::future::Future>(agent: Option<String>, future: F) -> F::Output`, which runs `future` with `REQUEST_AGENT` set when `agent` is `Some`.

`Indicator::agent()` checks, in order: the agent-name override, `REQUEST_AGENT`, `client_name`, the attached peer, `"Agent"`.
The server's `call_tool` reads `context.client_info()` (from `_meta` on stateless requests, the session's `initialize` otherwise), turns it into a display name with `agent_display_name` (`indicator.rs:204`), and runs the tool router's call inside `with_request_agent`.
If `#[tool_handler]` generates `call_tool`, replace it with a hand-written `call_tool` that delegates to the router; the other generated methods stay.

- [ ] **Step 1: Write the failing test** `request_agent_wins_over_the_last_initialize`: with `remember_client("Gamma", None)` set, events made inside `with_request_agent(Some("Alpha"), ..)` and `with_request_agent(Some("Beta"), ..)`, run concurrently, carry `Alpha` and `Beta`; an event outside any scope carries `Gamma`.
- [ ] **Step 2:** Run it.
      Expected: fail (no `with_request_agent`).
- [ ] **Step 3:** Implement the task-local, the lookup order and the `call_tool` wrapper.
- [ ] **Step 4:** Run the test and the gates.
      Expected: pass; upstream indicator tests unchanged.
- [ ] **Step 5:** Commit `fix: name each HTTP client on the indicator from its own request`.

### Task 8: Docs, changelog and binary size

**Files:**

- Modify: `README.md` (line 278 protocol wording; 443 `--listen` paragraph; 502-503 env table; 525 "MCP transport" bullet, which still says `transport-io` only; 555-556 security bullets)
- Modify: `SECURITY.md:13-15` (Scope)
- Modify: `npm/README.md:165` (move the listener bullet out of `## Input & coordinate contracts` into `## Desktop setup & compositor notes` at 134)
- Modify: `skills/computer-use-linux/references/selkies-setup.md` (line 8 "No port is opened"; new `## Serve MCP over HTTP` section after `## Configure the MCP client`), `skills/computer-use-linux/SKILL.md`, then copy both into `plugins/computer-use-linux/skills/`
- Modify: `CHANGELOG.md:8` (`## [Unreleased]`)
- [ ] **Step 1:** README and npm README describe `--listen`, `--allowed-host`, both token variables, the Origin refusal, the HTTP `snapshot_id` requirement, the shell refusal, and "plain HTTP, put TLS in front (mesh, ingress or sidecar)".
  The security bullet and `SECURITY.md` say the binary opens no listener unless `--listen` is passed.
- [ ] **Step 2:** The Selkies guide's new section shows a Kubernetes-shaped start: `mcp --listen 0.0.0.0:5200 --allowed-host <service name>` with `COMPUTER_USE_LINUX_MCP_TOKEN_FILE` from a mounted Secret, and the client config as a URL with the bearer header; line 8 says the stdio setup opens no port.
- [ ] **Step 3:** `CHANGELOG.md` `[Unreleased]` gets one `### Added` entry for the listener, crediting #362.
- [ ] **Step 4:** Build release binaries of `main` and the branch (`cargo build --release --locked --bin computer-use-linux` in each worktree) and record both sizes in bytes for the PR description.
- [ ] **Step 5:** Run the gates, `npx -y agnix .`, and `scripts/check_plugins.sh`.
      Expected: pass, no agnix errors or warnings.
- [ ] **Step 6:** Commit `docs: document the HTTP listener`.
