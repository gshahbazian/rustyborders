# rustyborders

A Rust port of JankyBorders that draws borders around macOS windows using
private SkyLight/CoreGraphics APIs. See `README.md` for architecture details.

## Verification gate

Run before committing:

```sh
cargo fmt --all
cargo test
cargo clippy --all-targets -- -D warnings
cargo build
```

## Seeing real-world results

Because this program draws into real on-screen windows via private APIs, unit
tests can't tell you whether a border actually looks right. To see the real
output, run the visual-verification harness:

```sh
./scripts/screenshot.sh
```

It builds the release binary, opens two subject windows side by side (the
right-hand one made frontmost), runs rustyborders against them, captures the
**whole display**, and writes `border-screenshot.png` (gitignored) for you to
open and inspect. Capturing the full display lets you confirm the border is
drawn only around the frontmost/active window and that inactive windows stay
untouched.

Pass any border arguments through to rustyborders to exercise specific settings:

```sh
./scripts/screenshot.sh width=10 'active_color=oklch(84% 0.32 150 / 1)'
```

Notes:
- Requires a logged-in GUI session and Screen Recording permission for the
  terminal app (granted once in System Settings → Privacy & Security → Screen &
  System Audio Recording; the terminal must be relaunched after granting). This
  cannot run in headless CI.
- `scripts/subject_window.swift` is the controlled subject; it needs the Swift
  toolchain (`/usr/bin/swift`).
- After any change to drawing, color parsing, or focus detection, run the script
  and view `border-screenshot.png` to confirm the change looks correct.
