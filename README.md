# rustyborders

`rustyborders` draws configurable borders around macOS windows. It is a Rust port of
[JankyBorders](https://github.com/FelixKratz/JankyBorders), using the same general
SkyLight/CoreGraphics approach while wrapping the macOS private APIs in Rust.

It tracks visible application windows, detects the active window, and draws a border
overlay using SkyLight window creation plus CoreGraphics drawing. By default inactive
borders are transparent, matching JankyBorders' visible behavior of showing a border
around the active window.

## Usage

Run from the repository:

```sh
cargo run -- width=6.0 active_color=0xff00ff00
```

That starts `rustyborders` with a 6px green active-window border.

If an instance is already running, a second invocation sends the new settings to
the running process over a Mach IPC port:

```sh
cargo run -- width=5.0 active_color=0xffff0000
```

Useful options currently include:

- `width=6.0`
- `active_color=0xff00ff00`
- `inactive_color=0x00000000`
- `background_color=0x00000000`
- `order=above` or `order=below`
- `style=round`, `style=square`, or `style=uniform`
- `hidpi=on` or `hidpi=off`
- `ax_focus=on` or `ax_focus=off`
- `blacklist=AppName,OtherApp`
- `whitelist=AppName,OtherApp`
- `apply-to=<window_id>`

Enable debug logging with:

```sh
RUSTYBORDERS_LOG=1 cargo run -- width=6.0 active_color=0xff00ff00
```

## How It Works

The code is organized around a few core modules:

- `src/app.rs` owns process-level state, settings, startup, IPC dispatch, and
  active-window focus updates.
- `src/windows.rs` discovers suitable windows through SkyLight queries, tracks
  spaces, window levels, app filters, and corner radii.
- `src/border.rs` owns each target window's border overlay and redraws it when
  the target moves, resizes, focuses, hides, or changes settings.
- `src/drawing.rs` contains the CoreGraphics path and color drawing helpers.
- `src/events.rs` registers SkyLight notifications and translates window events
  into app updates.
- `src/ipc.rs` implements the Mach message path used to update a running
  instance.
- `src/sys/*` contains the low-level FFI declarations and ABI-sensitive structs.

The compositor path uses `SLSNewWindowWithOpaqueShapeAndContext` to create a
fullscreen overlay window. Borders are drawn in absolute display coordinates into
that overlay. This differs slightly from JankyBorders' managed-window path
because managed border windows did not visibly composite in this port during
testing.

Corner radii are read through the private
`SLSWindowIteratorGetCornerRadii` symbol when available. The current drawing path
uses the first returned radius and draws a uniform rounded rectangle, mirroring
JankyBorders' scalar radius behavior.

## Development

Run the normal verification gate with:

```sh
cargo fmt --all
cargo test
cargo clippy --all-targets -- -D warnings
cargo build
```

Because this project calls private macOS APIs directly, the FFI boundary is kept
explicit and covered with a few ABI layout tests for hand-written CoreGraphics and
Mach structs.
