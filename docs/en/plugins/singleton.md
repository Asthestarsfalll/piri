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
```

### Configuration Parameters

- `command` (required): Full command string to launch the application, can include environment variables and arguments
- `app_id` (optional): Application ID used to match windows. If not specified, the plugin automatically extracts it from the command (executable name)

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
2. **Subsequent Toggles**: If the registered window still exists, focuses it, otherwise searches for matching windows or relaunches
3. **Window Matching**: Uses the configured `app_id` or extracts `app_id` from the command

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
