import Foundation
import XCTest
@testable import OpenEditorMedia

private final class ExportTestContext: @unchecked Sendable {
    var continuation: CheckedContinuation<Bool, Never>?
}

private func exportTestCallback(_ success: Bool, _ message: UnsafePointer<CChar>?, _ context: UnsafeMutableRawPointer?) {
    guard let context else { return }
    let value = Unmanaged<ExportTestContext>.fromOpaque(context).takeUnretainedValue()
    value.continuation?.resume(returning: success)
    value.continuation = nil
}

@MainActor
final class OpenEditorMediaTests: XCTestCase {
    func testCreatesAndReleasesAPlayer() {
        let handle = playerCreate()
        XCTAssertNotNil(handle)
        playerRelease(handle)
    }

    func testLoadsAValidEmptyComposition() {
        let handle = playerCreate()
        let json = #"{"width":1080,"height":1920,"frameRate":{"value":30,"timescale":1},"clips":[],"captions":[],"transitions":[]}"#
        XCTAssertTrue(json.withCString { playerLoadComposition(handle, $0) })
        XCTAssertEqual(playerCurrentTime(handle, 600), 0)
        XCTAssertEqual(playerRate(handle), 0)
        playerRelease(handle)
    }

    func testComposesAndExportsFixtureVideo() async throws {
        let ffmpeg = ["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"].first { FileManager.default.isExecutableFile(atPath: $0) }
        guard let ffmpeg else { throw XCTSkip("FFmpeg is unavailable") }
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let source = root.appendingPathComponent("source.mp4")
        let output = root.appendingPathComponent("output.mp4")
        let process = Process()
        process.executableURL = URL(fileURLWithPath: ffmpeg)
        process.arguments = ["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "color=c=blue:s=320x180:d=1", "-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo", "-shortest", "-c:v", "h264_videotoolbox", "-c:a", "aac", source.path]
        try process.run()
        process.waitUntilExit()
        XCTAssertEqual(process.terminationStatus, 0)
        let json = """
        {"width":180,"height":320,"frameRate":{"value":30,"timescale":1},"clips":[{"id":"clip","path":\(String(reflecting: source.path)),"kind":"video","sourceIn":{"value":0,"timescale":600},"sourceOut":{"value":480,"timescale":600},"timelineStart":{"value":0,"timescale":600},"playbackRate":1,"transform":{"x":0,"y":0,"scale":1,"rotation":0,"opacity":1},"audio":{"volume":1,"fadeIn":{"value":0,"timescale":600},"fadeOut":{"value":0,"timescale":600},"ducking":false}}],"captions":[{"start":{"value":60,"timescale":600},"end":{"value":360,"timescale":600},"text":"Fixture","style":{"fontSize":24,"color":"#ffffff","background":"#000000","position":"bottom"}}],"transitions":[]}
        """
        let context = ExportTestContext()
        let contextPointer = Unmanaged.passUnretained(context).toOpaque()
        let success = await withCheckedContinuation { continuation in
            context.continuation = continuation
            let handle = json.withCString { jsonPointer in
                output.path.withCString { outputPointer in
                    exportStart(jsonPointer, outputPointer, exportTestCallback, contextPointer)
                }
            }
            if handle == nil {
                context.continuation?.resume(returning: false)
                context.continuation = nil
            }
        }
        XCTAssertTrue(success)
        XCTAssertGreaterThan((try? output.resourceValues(forKeys: [.fileSizeKey]).fileSize ?? 0) ?? 0, 0)
    }

    nonisolated func testRoundTripsSecurityScopedBookmark() throws {
        let file = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try Data("media".utf8).write(to: file)
        let bookmark = file.path.withCString { bookmarkCreate($0) }
        XCTAssertNotNil(bookmark)
        let resolved = bookmarkResolve(bookmark)
        let expected = file.resolvingSymlinksInPath().path
        XCTAssertEqual(resolved.map { URL(fileURLWithPath: String(cString: $0)).resolvingSymlinksInPath().path }, expected)
        if let resolved { bookmarkRelease(resolved); stringFree(resolved) }
        stringFree(bookmark)
        try? FileManager.default.removeItem(at: file)
    }
}
