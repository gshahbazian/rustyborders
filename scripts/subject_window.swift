// Deterministic subject windows for rustyborders visual verification.
//
// Opens two titled NSWindows side by side with a solid fill, makes the
// right-hand one key/frontmost, prints "READY", then runs until killed.
//
// The point: rustyborders should draw a border ONLY around the frontmost
// (active) window. With two visible windows we can confirm the inactive one is
// left untouched.

import AppKit

let winW: CGFloat = 520
let winH: CGFloat = 360

let app = NSApplication.shared
app.setActivationPolicy(.regular)

let area = NSScreen.screens.first?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
let y = area.midY - winH / 2
let leftX = area.minX + 100
let rightX = area.maxX - winW - 100

func makeWindow(title: String, x: CGFloat, fill: CGFloat) -> NSWindow {
    let window = NSWindow(
        contentRect: NSRect(x: x, y: y, width: winW, height: winH),
        styleMask: [.titled, .closable],
        backing: .buffered,
        defer: false
    )
    window.title = title
    window.backgroundColor = NSColor(calibratedWhite: fill, alpha: 1.0)
    window.isReleasedWhenClosed = false
    return window
}

let inactiveWindow = makeWindow(title: "rustyborders-inactive", x: leftX, fill: 0.28)
let activeWindow = makeWindow(title: "rustyborders-active", x: rightX, fill: 0.18)

inactiveWindow.orderFront(nil)
// Order the active window last and make it key so it is the frontmost window.
activeWindow.makeKeyAndOrderFront(nil)
app.activate(ignoringOtherApps: true)

print("READY")
fflush(stdout)

app.run()
