import Testing
import Foundation
@testable import OpenEditorMedia

@Test @MainActor func createsAndReleasesAPlayer() {
    let handle = playerCreate()
    #expect(handle != nil)
    playerRelease(handle)
}

@Test func roundTripsSecurityScopedBookmark() throws {
    let file = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try Data("media".utf8).write(to: file)
    let bookmark = file.path.withCString { bookmarkCreate($0) }
    #expect(bookmark != nil)
    let resolved = bookmarkResolve(bookmark)
    #expect(resolved.map { String(cString: $0) } == file.path)
    if let resolved { bookmarkRelease(resolved); stringFree(resolved) }
    stringFree(bookmark)
    try? FileManager.default.removeItem(at: file)
}
