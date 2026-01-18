# Singleton Plugin

The Singleton plugin manages singleton windows - windows that should only have one instance. When toggling, if the window exists it focuses it, otherwise it launches the application.

## Configuration

Use the `[singleton.{name}]` format to configure singletons:

```toml
[piri.plugins]
singleton = true

[singleton.browser]
command = "google-chrome-stable"

[singleton.term]
command = "GTK_IM_MODULE=wayland ghostty --class=singleton.term"
app_id = "singleton.term"

[singleton.editor]
command = "code"
app_id = "code"
on_created_command = "notify-send 'Editor opened'"
```

### Configuration Parameters

- `command` (required): Full command string to launch the application, can include environment variables and arguments
- `app_id` (optional): Application ID used to match windows (supports regular expressions). If not specified, the plugin automatically extracts it from the command (executable name)
- `on_created_command` (optional): Command to execute after the window is created. This command is only executed when a new window is created, not when an existing window is focused

> **Note**: `app_id` uses regular expression matching. If `app_id` contains special characters (such as `.`, `*`, etc.), they need to be escaped. For example: `app_id = "float\\.dropterm"`

> **Reference**: For detailed information about the window matching mechanism, see [Window Matching Mechanism](../window_matching.md)

## Usage

```bash
# Toggle singleton (focus if exists, launch if not)
piri singleton {name} toggle

# Examples
piri singleton browser toggle
piri singleton term toggle
```

## How It Works

1. **First Toggle**: Checks if a matching window exists, if found focuses and registers it, otherwise launches the application and waits for the window to appear
2. **Window Creation**: When a new window is created (not found existing), after the window appears, if `on_created_command` is configured, it will be executed
3. **Subsequent Toggles**: If the registered window still exists, focuses it, otherwise searches for matching windows or relaunches (and executes `on_created_command` again if configured)
4. **Window Matching**: Uses the configured `app_id` or extracts `app_id` from the command

## Features

- ✅ **Smart Detection**: Automatically detects existing windows to avoid duplicate launches
- ✅ **Auto Extraction**: Automatically extracts `app_id` from command if not specified
- ✅ **Window Registry**: Tracks singleton windows by ID for fast lookup
- ✅ **Robust Matching**: Can match windows even if they weren't launched by the plugin

## Use Cases

- Applications that typically only need one instance (browser, terminal, etc.)
- Quick access to frequently used applications with single-instance guarantee
- Prevent multiple instances of resource-intensive applications

## Notes

1. **Window Matching**: Make sure your application sets the correct `app_id` property, or specify it explicitly in the configuration
2. **app_id Extraction**: Extracts executable name from the first word of the command (removing path), e.g., `/usr/bin/google-chrome-stable` → `google-chrome-stable`
3. **Timeout**: Waits up to 5 seconds for a window to appear after launching an application, times out without error but no window will be focused
4. **on_created_command**: The `on_created_command` is only executed when a new window is created. It will not be executed when focusing an existing window. If the window is closed and later reopened, the command will be executed again
