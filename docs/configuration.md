# Configuration

Folio reads an optional TOML configuration file:

```text
~/Library/Application Support/com.folio.reader/config.toml
```

The file is created with commented examples on first launch. Open it from
**Folio → Configuration…** or from **Open configuration** at the bottom of the
theme picker. Edit and restart Folio to apply changes.

Every key is optional. Uncomment a key to **pin** it; a pinned value overrides
the in-app setting on launch. Keys that are absent or commented out leave the
in-app setting in control.

```toml
# theme = "blend"        # built-in or custom theme id
# matchSystem = true     # follow the light/dark counterpart
# textSize = 1.0         # 0.85 to 1.5
# showOutline = true     # show the contents rail on launch
```

## Keys

| Key | Type | Meaning |
| --- | --- | --- |
| `theme` | string | A built-in theme id (`blend`, `blend-dark`, `editorial`, `editorial-dark`, `studio`, `studio-dark`) or an installed custom theme id. |
| `matchSystem` | boolean | When `true`, use the theme's paired counterpart for the system light/dark appearance. When `false`, use the named theme exactly. |
| `textSize` | number | Reading scale from `0.85` to `1.5`. `1.0` is the theme's natural size. |
| `showOutline` | boolean | Whether the contents rail starts visible on desktop. On narrow windows the rail remains a closed drawer. |

## Examples

Use Editorial on a light page, regardless of the system appearance:

```toml
theme = "editorial"
matchSystem = false
textSize = 1.05
```

Follow the system with a custom theme:

```toml
theme = "my-paper"
matchSystem = true
```

Start with a wider reading page and the rail tucked away:

```toml
textSize = 1.1
showOutline = false
```

## Precedence and the in-app controls

The theme picker, **Match system**, and text-size commands still work. If a key
is pinned in `config.toml`, it wins on the next launch; the in-app change is
kept as your local preference but does not overwrite the file. Remove or
comment the key to let the in-app preference take over again.

For example, with `theme = "editorial"` pinned:

- Choosing Studio in the picker changes the current session.
- Restarting Folio returns to Editorial.
- Commenting out `theme` and restarting uses the last Studio preference.

`showOutline` only controls the initial desktop state. Once the app is open,
**⌘B** and the rail toggle stay in control.

## Validation

The file is parsed as TOML before the window is shown. Folio never executes
configuration:

- Unknown keys, malformed TOML, an unknown theme id, or a `textSize` outside
  `0.85`–`1.5` are reported in a toast.
- Invalid individual values are ignored; the rest of the app still launches.
- An invalid file falls back to defaults instead of blocking startup.
- Theme ids are resolved against the theme catalog. A custom theme must load
  successfully before it can be pinned here.

## Why not write the file from the UI?

Keeping the configuration file read-only from the app preserves comments and
formatting. The in-app controls remain the everyday path; `config.toml` is the
pinned, durable override for people and for generated setups.
