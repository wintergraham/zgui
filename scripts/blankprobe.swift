// blankprobe PID WARMUP SECONDS RATE: how often the vlist scene's window
// shows unfilled rows. After WARMUP seconds it captures the window of PID
// RATE times a second for SECONDS (ScreenCaptureKit, at the window's points)
// and, in each capture, scans columns inside every row's left padding, where
// a drawn row always shows its own background (vlist::ROW_COLORS): a run
// down a column closer to the window's background (vlist::BACKGROUND) than
// to either row colour, longer than BLANK points, is a place no row was
// drawn. Prints one JSON line: captures, captures with a blank run, and the
// longest run. It needs Screen Recording permission, which it only checks
// (CGPreflightScreenCaptureAccess): it never asks.
//
//   swiftc -O scripts/blankprobe.swift -o /tmp/zgx-blankprobe
import AppKit
import CoreGraphics
import ScreenCaptureKit

let args = CommandLine.arguments
guard args.count == 5, let pid = pid_t(args[1]), let warmup = Double(args[2]),
      let seconds = Double(args[3]), let rate = Double(args[4]) else {
    FileHandle.standardError.write("usage: blankprobe PID WARMUP SECONDS RATE\n".data(using: .utf8)!)
    exit(2)
}
// A connection to the window server (CoreGraphics needs one), as an app
// that never shows itself: no Dock icon, never active.
let app = NSApplication.shared
app.setActivationPolicy(.prohibited)
guard CGPreflightScreenCaptureAccess() else {
    print("{\"error\":\"no screen recording permission\"}")
    exit(1)
}
// vlist's colours (comparisons/workload: vlist::BACKGROUND, ROW_COLORS).
let background: [Double] = [0x0e, 0x12, 0x18]
let rows: [[Double]] = [[0x16, 0x1c, 0x26], [0x19, 0x20, 0x2b]]
// Columns, in points from the window's left: inside the rows' padding (16).
let columns: [Double] = [4, 8, 12]
// A blank run: longer than this many points (rows are at least 50 tall,
// so a gap between drawn rows is never this long).
let blank = 24.0

func distance(_ a: [Double], _ b: [Double]) -> Double {
    // A plain loop: the closure form's operator overloads timed out the type
    // checker ("unable to type-check this expression in reasonable time").
    var sum = 0.0
    for (x, y) in zip(a, b) {
        sum += (x - y) * (x - y)
    }
    return sum
}

func longestBlank(_ image: CGImage, scale: Double) -> Double {
    guard let data = image.dataProvider?.data, let bytes = CFDataGetBytePtr(data) else { return 0 }
    let perRow = image.bytesPerRow, perPixel = image.bitsPerPixel / 8
    let bgra = image.bitmapInfo.contains(.byteOrder32Little)
    var longest = 0.0
    for column in columns {
        let x = Int(column * scale)
        guard x < image.width else { continue }
        var run = 0
        for y in 0..<image.height {
            let p = bytes + y * perRow + x * perPixel
            let rgb: [Double] = bgra ? [Double(p[2]), Double(p[1]), Double(p[0])] : [Double(p[0]), Double(p[1]), Double(p[2])]
            let toBackground = distance(rgb, background)
            if toBackground < distance(rgb, rows[0]) && toBackground < distance(rgb, rows[1]) {
                run += 1
                longest = max(longest, Double(run) / scale)
            } else {
                run = 0
            }
        }
    }
    return longest
}

Task {
    try await Task.sleep(nanoseconds: UInt64(warmup * 1e9))
    let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
    guard let window = content.windows.first(where: { $0.owningApplication?.processID == pid && $0.frame.width > 100 }) else {
        print("{\"error\":\"no window for pid \(pid)\"}")
        exit(1)
    }
    let filter = SCContentFilter(desktopIndependentWindow: window)
    let config = SCStreamConfiguration()
    let scale = 1.0
    config.width = Int(window.frame.width * scale)
    config.height = Int(window.frame.height * scale)
    config.showsCursor = false
    config.colorSpaceName = CGColorSpace.sRGB
    var captures = 0, blanks = 0, longest = 0.0
    let end = Date().addingTimeInterval(seconds)
    while Date() < end {
        let started = Date()
        if let image = try? await SCScreenshotManager.captureImage(contentFilter: filter, configuration: config) {
            captures += 1
            let run = longestBlank(image, scale: scale)
            longest = max(longest, run)
            if run > blank { blanks += 1 }
        }
        let wait = 1 / rate - Date().timeIntervalSince(started)
        if wait > 0 { try await Task.sleep(nanoseconds: UInt64(wait * 1e9)) }
    }
    print(String(format: "{\"captures\":%d,\"blank_captures\":%d,\"blank_fraction\":%.4f,\"longest_blank_points\":%.1f}",
                 captures, blanks, captures > 0 ? Double(blanks) / Double(captures) : 0, longest))
    exit(0)
}
RunLoop.main.run()
