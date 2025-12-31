# Plugin System

Piri supports a plugin system that allows you to extend functionality. Plugins run automatically in daemon mode.

## Plugin Control

You can control which plugins are enabled or disabled in the configuration file:

```toml
[piri.plugins]
scratchpads = true
empty = true
window_rule = true
autofill = true
```

**Default Behavior**:
- If not explicitly specified, plugins are **disabled** by default (`false`)
- You must explicitly set `scratchpads = true`, `empty = true`, `window_rule = true`, or `autofill = true` to enable plugins
- Exception: `window_rule` plugin is enabled by default if window rules are configured (unless explicitly set to `false`)
