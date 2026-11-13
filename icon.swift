// Draws Notch's app icon: the island on a dark tile, its dot and ring on the left and the
// waveform on the right. Run from examples/notch:
//
//   swift icon.swift && iconutil -c icns icon.iconset && rm -r icon.iconset
import AppKit

func draw(_ px: Int) -> Data {
    let s = CGFloat(px) / 1024
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: px, bitsPerSample: 8,
                               samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                               bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let cg = NSGraphicsContext.current!.cgContext
    cg.scaleBy(x: s, y: s)

    // The tile on macOS's icon grid, with the system's soft drop shadow.
    let tile = NSBezierPath(roundedRect: NSRect(x: 100, y: 100, width: 824, height: 824), xRadius: 185, yRadius: 185)
    NSGraphicsContext.saveGraphicsState()
    let shadow = NSShadow()
    shadow.shadowColor = NSColor.black.withAlphaComponent(0.35)
    shadow.shadowBlurRadius = 28
    shadow.shadowOffset = NSSize(width: 0, height: -12)
    shadow.set()
    NSColor(white: 0.1, alpha: 1).setFill()
    tile.fill()
    NSGraphicsContext.restoreGraphicsState()
    NSGradient(colors: [NSColor(white: 0.17, alpha: 1), NSColor(white: 0.06, alpha: 1)])!.draw(in: tile, angle: -90)
    NSColor(white: 1, alpha: 0.08).setStroke()
    tile.lineWidth = 3
    tile.stroke()

    // The island, black with a hairline so it reads against the tile.
    let island = NSBezierPath(roundedRect: NSRect(x: 212, y: 436, width: 600, height: 152), xRadius: 76, yRadius: 76)
    NSColor.black.setFill()
    island.fill()
    NSColor(white: 1, alpha: 0.1).setStroke()
    island.lineWidth = 3
    island.stroke()

    // The dot in its context ring, as the island shows a finished turn.
    let (cx, cy) = (CGFloat(300), CGFloat(512))
    let green = NSColor(srgbRed: 0.33, green: 0.86, blue: 0.5, alpha: 1)
    green.setFill()
    NSBezierPath(ovalIn: NSRect(x: cx - 22, y: cy - 22, width: 44, height: 44)).fill()
    let track = NSBezierPath()
    track.appendArc(withCenter: NSPoint(x: cx, y: cy), radius: 44, startAngle: 0, endAngle: 360)
    track.lineWidth = 9
    NSColor(white: 1, alpha: 0.16).setStroke()
    track.stroke()
    let arc = NSBezierPath()
    arc.appendArc(withCenter: NSPoint(x: cx, y: cy), radius: 44, startAngle: 90, endAngle: 90 - 250, clockwise: true)
    arc.lineWidth = 9
    arc.lineCapStyle = .round
    NSColor(white: 0.9, alpha: 0.85).setStroke()
    arc.stroke()

    // The waveform.
    for (i, h) in [0.45, 0.8, 1.0, 0.65, 0.38].enumerated() {
        let x = CGFloat(630 + i * 30), len = CGFloat(h) * 84
        let bar = NSBezierPath()
        bar.move(to: NSPoint(x: x, y: cy - len / 2))
        bar.line(to: NSPoint(x: x, y: cy + len / 2))
        bar.lineWidth = 15
        bar.lineCapStyle = .round
        NSColor(srgbRed: 0.45, green: 0.66, blue: 1, alpha: 0.55 + 0.45 * h).setStroke()
        bar.stroke()
    }
    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])!
}

let dir = URL(fileURLWithPath: "icon.iconset")
try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
for size in [16, 32, 128, 256, 512] {
    try! draw(size).write(to: dir.appendingPathComponent("icon_\(size)x\(size).png"))
    try! draw(size * 2).write(to: dir.appendingPathComponent("icon_\(size)x\(size)@2x.png"))
}
