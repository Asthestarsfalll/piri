# Architecture Design

Piri uses a modular, state-driven, and event-oriented architecture designed to provide high-performance, low-latency extensions for the Niri compositor.

## Core Design Principles

### 1. State-Driven & Manager Pattern
Each complex plugin (e.g., Scratchpads, Singleton) follows the **State-Manager** pattern:
- **State Struct**: Aggregates static configuration (from TOML) and runtime state (e.g., `window_id`, visibility, last focused window).
- **Manager Struct**: Maintains a `HashMap<String, State>` and provides atomic operational methods.
- **Plugin Wrapper**: Implements the `Plugin` trait, mapping IPC requests and Niri events to specific Manager actions.

### 2. Resource Assurance Mechanism
Plugins employ a "Lazy Initialization" and "Automatic Recovery" strategy. The core method `ensure_window_id` implements a robust resource assurance chain:
1.  **Validity Check**: Verifies if the currently bound `window_id` still exists.
2.  **Automatic Capture**: If the ID is invalid, searches all current windows based on the `app_id` pattern to recapture.
3.  **Automatic Launch**: If not found, executes the configured command and enters a "Wait-Retry" loop until the window appears.
4.  **Initial Setup**: Once a window is acquired, unified actions like floating setup, resizing, and geometric positioning are performed.

### 3. Smart Config Reload
Piri supports lossless hot reloading:
- **Config Merging**: During a reload, the system compares old and new configurations. If an instance (e.g., a Scratchpad) already has an associated `window_id`, it is preserved.
- **Dynamic Protection**: Resources added dynamically via IPC (marked as `is_dynamic`) are preserved even if they are missing from the TOML file.
- **Cache Invalidation**: Regex caches in the `WindowMatcher` are automatically cleared after a reload to ensure new matching rules take effect immediately.

## Core Modules

### Plugin System (`src/plugins/`)
- `mod.rs`: Defines the `Plugin` trait and the unified event/IPC dispatch bus.
- `scratchpads.rs`: Core functionality for managing hidden/visible windows across workspaces and monitors.
- `singleton.rs`: Ensures only one instance of a specific app exists and supports quick toggling.
- `window_rule.rs`: Rule-based automation center for window placement and focus-triggered commands.
- `window_utils.rs`: Geometric calculation center and window matching engine, including a regex cache pool.

### Communication & Event Center
- `niri.rs`: High-performance asynchronous IPC client encapsulating all Niri actions.
- `daemon.rs`: The nervous system of the project, coordinating event dispatching, signal handling, and plugin lifecycles.
- `ipc.rs`: Internal command protocol based on Unix Sockets.

## Performance & Robustness

### Unified De-duplication
To handle bursty duplicate events from Niri (common during window creation), Piri implements a global de-duplication engine based on **Window ID + Timestamp** (with a cooldown of 200ms - 500ms), ensuring side effects like `focus_command` trigger only once.

### Geometry Abstraction
All mathematical calculations regarding screen edges (FromTop, FromLeft, etc.), margins, and percentage sizes are consolidated in `window_utils.rs`. By fetching output context once, round-trip latency with the compositor is minimized.

### Regex Caching
The `WindowMatcherCache` utilizes `Arc<Mutex<HashMap<String, Regex>>>`. In scenarios like `window_rule` where frequent matching is required, it avoids the CPU overhead of recompiling regex patterns.
