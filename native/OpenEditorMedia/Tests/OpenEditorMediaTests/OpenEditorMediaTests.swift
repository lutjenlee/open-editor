import Testing
@testable import OpenEditorMedia

@Test @MainActor func createsAndReleasesAPlayer() {
    let handle = playerCreate()
    #expect(handle != nil)
    playerRelease(handle)
}
