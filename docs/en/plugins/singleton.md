# Singleton Plugin

The Singleton plugin manages singleton windows - windows that should only have one instance. When you toggle a singleton, if the window already exists, it will focus it; otherwise, it will launch the application. This is useful for applications like browsers, terminals, or other tools where you typically only want one instance running.

## Configuration

Add `[singleton.{name}]` sections to your configuration file to configure singletons. Each singleton requires a unique name.

### Configuration Example

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

- `app_id` (optional): Application ID used to match windows. If not specified, the plugin will automatically extract the app_id from the command by taking the executable name (e.g., "google-chrome-stable" from "/usr/bin/google-chrome-stable" or "google-chrome-stable --some-arg")

## Usage

### Toggle Singleton

```bash
piri singleton {name} toggle
```

Examples:

```bash
# Toggle browser singleton
piri singleton browser toggle

# Toggle terminal singleton
piri singleton term toggle
```

## How It Works

1. **First Toggle**: When executing `piri singleton {name} toggle` for the first time, the plugin:
   - Checks if a window matching the pattern already exists
   - If found, focuses it and registers the window ID
   - If not found, launches the application using the configured command
   - Waits for the window to appear (up to 5 seconds)
   - Once the window appears, focuses it and registers the window ID

2. **Subsequent Toggles**: 
   - If the registered window still exists, focuses it immediately
   - If the window was closed, removes it from the registry and searches for existing windows matching the pattern
   - If no matching window is found, launches the application again

3. **Window Matching**: 
   - Uses the `app_id` from configuration if specified
   - Otherwise, automatically extracts the app_id from the command (executable name without path)
   - Matches windows by their `app_id` property

## Features

- ✅ **Smart Window Detection**: Automatically detects existing windows before launching new instances
- ✅ **Automatic App ID Extraction**: If `app_id` is not specified, automatically extracts it from the command
- ✅ **Window Registry**: Tracks singleton windows by ID for fast lookup
- ✅ **Focus Management**: Automatically focuses existing windows instead of creating duplicates
- ✅ **Robust Window Matching**: Uses pattern matching to find windows even if they weren't launched by the plugin

## Use Cases

- **Browser Management**: Ensure only one browser window is open at a time
- **Terminal Management**: Keep a single terminal instance accessible via toggle
- **Application Launchers**: Quick access to frequently used applications with single-instance guarantee
- **Resource Management**: Prevent multiple instances of resource-intensive applications

## Notes

1. **Window Matching**: The plugin uses `app_id` to match windows. Make sure your application sets the correct `app_id` property, or specify it explicitly in the configuration
2. **Command Extraction**: If `app_id` is not specified, the plugin extracts it from the command by:
   - Taking the first word (before any whitespace)
   - Removing the path (taking only the executable name)
   - For example: "/usr/bin/google-chrome-stable" → "google-chrome-stable"
3. **Window Registration**: The plugin maintains a registry of singleton windows by ID. If a window is closed externally, the plugin will detect this on the next toggle and remove it from the registry
4. **Timeout**: The plugin waits up to 5 seconds (50 attempts × 100ms) for a window to appear after launching an application. If no window appears within this time, the toggle operation completes without error, but no window will be focused

