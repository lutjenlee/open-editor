import AppKit
import AVFoundation
import CoreMedia

@MainActor
final class PreviewController {
    let player = AVPlayer()
    let layer = AVPlayerLayer()

    init() {
        layer.player = player
        layer.videoGravity = .resizeAspect
        layer.backgroundColor = NSColor.black.cgColor
    }

    func attach(to view: NSView, frame: CGRect) {
        view.wantsLayer = true
        layer.removeFromSuperlayer()
        layer.frame = frame
        layer.autoresizingMask = [.layerWidthSizable, .layerHeightSizable]
        view.layer?.addSublayer(layer)
    }

    func load(path: String) {
        player.replaceCurrentItem(with: AVPlayerItem(url: URL(fileURLWithPath: path)))
    }
}

@MainActor
private func controller(_ handle: UnsafeMutableRawPointer?) -> PreviewController? {
    guard let handle else { return nil }
    return Unmanaged<PreviewController>.fromOpaque(handle).takeUnretainedValue()
}

@_cdecl("oe_player_create")
@MainActor
public func playerCreate() -> UnsafeMutableRawPointer? {
    Unmanaged.passRetained(PreviewController()).toOpaque()
}

@_cdecl("oe_player_release")
@MainActor
public func playerRelease(_ handle: UnsafeMutableRawPointer?) {
    guard let handle else { return }
    let instance = Unmanaged<PreviewController>.fromOpaque(handle).takeRetainedValue()
    instance.player.pause()
    instance.layer.removeFromSuperlayer()
}

@_cdecl("oe_player_attach")
@MainActor
public func playerAttach(
    _ handle: UnsafeMutableRawPointer?,
    _ viewPointer: UnsafeMutableRawPointer?,
    _ x: Double, _ y: Double, _ width: Double, _ height: Double
) -> Bool {
    guard let viewPointer else { return false }
    guard let instance = controller(handle) else { return false }
    let view = Unmanaged<NSView>.fromOpaque(viewPointer).takeUnretainedValue()
    instance.attach(to: view, frame: CGRect(x: x, y: y, width: width, height: height))
    return true
}

@_cdecl("oe_player_set_frame")
@MainActor
public func playerSetFrame(
    _ handle: UnsafeMutableRawPointer?,
    _ x: Double, _ y: Double, _ width: Double, _ height: Double
) -> Bool {
    guard let instance = controller(handle) else { return false }
    instance.layer.frame = CGRect(x: x, y: y, width: width, height: height)
    return true
}

@_cdecl("oe_player_load_file")
@MainActor
public func playerLoadFile(_ handle: UnsafeMutableRawPointer?, _ path: UnsafePointer<CChar>?) -> Bool {
    guard let path else { return false }
    guard let instance = controller(handle) else { return false }
    instance.load(path: String(cString: path))
    return true
}

@_cdecl("oe_player_play")
@MainActor
public func playerPlay(_ handle: UnsafeMutableRawPointer?) {
    controller(handle)?.player.play()
}

@_cdecl("oe_player_pause")
@MainActor
public func playerPause(_ handle: UnsafeMutableRawPointer?) {
    controller(handle)?.player.pause()
}

@_cdecl("oe_player_seek")
@MainActor
public func playerSeek(_ handle: UnsafeMutableRawPointer?, _ value: Int64, _ timescale: Int32) {
    guard timescale > 0 else { return }
    controller(handle)?.player.seek(
        to: CMTime(value: value, timescale: timescale),
        toleranceBefore: .zero,
        toleranceAfter: .zero
    )
}
