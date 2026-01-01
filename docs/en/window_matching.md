# Window Matching Mechanism

Piri uses a unified window matching mechanism that supports matching windows by `app_id` and `title` using regular expressions. Multiple plugins (such as `window_rule`, `singleton`, `scratchpads`) use this mechanism to find and match windows.

## Supported Matching Methods

### 1. Regular Expression Matching

All window matching is based on **Regular Expressions (Regex)**, supporting full regex syntax.

### 2. Matching Fields

- **`app_id`**: Window application ID (optional)
- **`title`**: Window title (optional)

**Note**: At least one of `app_id` or `title` must be specified. If both are specified, either match works (OR logic).

## Matching Logic

1. **Single Field Matching**: If only `app_id` or `title` is specified, that field must match
2. **Multiple Field Matching**: If both `app_id` and `title` are specified, either match works (OR logic)
3. **Regular Expressions**: Uses Rust's `regex` crate, supporting full regular expression syntax

## Usage Examples

### Basic Matching

```toml
# Exact match for app_id
app_id = "code"

# Match app_id containing specific string
app_id = ".*chrome.*"

# Match app_id starting with specific string
app_id = "^firefox"

# Match exact app_id (using anchors)
app_id = "^code$"
```

### Title Matching

```toml
# Match title containing "Chrome"
title = ".*Chrome.*"

# Match title starting with "VS Code"
title = "^VS Code"

# Match title containing numbers
title = ".*\\d+.*"
```

### Combined Matching

```toml
# Either app_id or title match works
app_id = "code"
title = ".*VS Code.*"
```

## Usage in Plugins

### Window Rule Plugin

```toml
[[window_rule]]
app_id = ".*firefox.*"
open_on_workspace = "2"

[[window_rule]]
title = ".*Chrome.*"
open_on_workspace = "3"
focus_command = "notify-send 'Focusing on Chrome'"
```

### Singleton Plugin

```toml
[singleton.browser]
command = "google-chrome-stable"
app_id = "google-chrome"  # Uses regex matching
```

### Scratchpads Plugin

```toml
[scratchpads.term]
direction = "fromRight"
command = "ghostty"
app_id = "float\\.dropterm"  # Uses regex matching, note escaped dot
```

## Regular Expression Syntax Reference

### Common Patterns

| Pattern | Description | Example |
|---------|-------------|---------|
| `.` | Match any character | `"c.ode"` matches `"code"`, `"cade"` |
| `.*` | Match any characters (zero or more) | `".*chrome.*"` matches strings containing `chrome` |
| `^` | Match start of string | `"^firefox"` matches strings starting with `firefox` |
| `$` | Match end of string | `"code$"` matches strings ending with `code` |
| `[abc]` | Match any character in set | `"[abc]ode"` matches `"aode"`, `"bode"`, `"code"` |
| `[0-9]` | Match digits | `"[0-9]+"` matches one or more digits |
| `\d` | Match digits (equivalent to `[0-9]`) | `"\d+"` matches one or more digits |
| `\w` | Match word characters (letters, digits, underscore) | `"\w+"` matches words |
| `+` | One or more | `"[0-9]+"` matches one or more digits |
| `*` | Zero or more | `".*"` matches any string |
| `?` | Zero or one | `"colou?r"` matches `"color"` or `"colour"` |
| `\|` | Or | `"firefox\|chrome"` matches `"firefox"` or `"chrome"` |

### Escaping Special Characters

If you need to match special characters (such as `.`, `*`, `+`, `?`, `[`, `]`, `(`, `)`, `{`, `}`, `^`, `$`, `|`, `\`) in your pattern, you need to escape them with a backslash:

```toml
# Match app_id containing a dot
app_id = "float\\.dropterm"

# Match title containing parentheses
title = ".*\\(.*\\).*"
```

## Performance Optimization

1. **Regex Caching**: Compiled regular expressions are cached to avoid repeated compilation
2. **Simple Patterns First**: Using simple and clear patterns provides better performance
3. **Avoid Over-complexity**: Overly complex regular expressions may affect performance

## Best Practices

1. **Exact Matching**: If you know the exact `app_id`, use `^app_id$` for exact matching
2. **Partial Matching**: Use `.*pattern.*` for partial matching
3. **Escape Special Characters**: If `app_id` or `title` contains regex special characters, remember to escape them
4. **Test Patterns**: Before configuring, use online regex testing tools to verify patterns are correct

## Debugging Tips

If window matching doesn't work, you can:

1. **Check Logs**: View piri's log output to understand the matching process
2. **Verify app_id/title**: Use `niri-ipc` tool to view actual window `app_id` and `title`
3. **Test Regex**: Use online tools to test if the regular expression is correct
4. **Simplify Patterns**: Start with simple patterns (like exact matching) to verify basic functionality, then gradually make them more complex

## Example Configurations

### Match Multiple Browsers

```toml
[[window_rule]]
app_id = ".*(firefox|chrome|chromium).*"
open_on_workspace = "browser"
```

### Match Development Tools

```toml
[[window_rule]]
app_id = ".*(code|vscode|idea).*"
open_on_workspace = "dev"
```

### Match Terminals

```toml
[[window_rule]]
app_id = ".*(term|terminal|ghostty|alacritty).*"
open_on_workspace = "1"
```

### Match Specific Windows by Title

```toml
[[window_rule]]
title = ".*GitHub.*"
open_on_workspace = "dev"
```

