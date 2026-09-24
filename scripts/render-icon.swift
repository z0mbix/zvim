// Convert the unmodified upstream mark to a square, transparent macOS icon master.
// Usage: swift scripts/render-icon.swift assets/icons/neovim.png assets/icons/neovim-app.png
import AppKit

let source = NSImage(contentsOfFile: CommandLine.arguments[1])!
let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 1024, pixelsHigh: 1024,
    bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
    colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
NSGraphicsContext.current!.imageInterpolation = .high
let height: CGFloat = 896
let width = height * source.size.width / source.size.height
source.draw(in: NSRect(x: (1024-width)/2, y: (1024-height)/2, width: width, height: height),
    from: .zero, operation: .copy, fraction: 1)
NSGraphicsContext.restoreGraphicsState()
try bitmap.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: CommandLine.arguments[2]))
