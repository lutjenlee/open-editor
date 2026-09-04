import AppKit
import AVFoundation
import QuartzCore
import CoreMedia
import Darwin

private struct WireTime: Codable {
    let value: Int64
    let timescale: Int32

    var cmTime: CMTime { CMTime(value: value, timescale: timescale) }
    var seconds: Double { CMTimeGetSeconds(cmTime) }
}

private struct WireTransform: Codable {
    let x: Double
    let y: Double
    let scale: Double
    let rotation: Double
    let opacity: Double
}

private struct WireAudio: Codable {
    let volume: Double
    let fadeIn: WireTime
    let fadeOut: WireTime
    let ducking: Bool
}

private struct WireClip: Codable {
    let id: String
    let path: String
    let kind: String
    let sourceIn: WireTime
    let sourceOut: WireTime
    let timelineStart: WireTime
    let playbackRate: Double
    let transform: WireTransform
    let audio: WireAudio
}

private struct WireCaptionStyle: Codable {
    let fontSize: Double
    let color: String
    let background: String
    let position: String
}

private struct WireCaption: Codable {
    let start: WireTime
    let end: WireTime
    let text: String
    let style: WireCaptionStyle
}

private struct WireTransition: Codable {
    let fromClipId: String
    let toClipId: String
    let kind: String
    let duration: WireTime
}

private struct WireComposition: Codable {
    let width: Int
    let height: Int
    let frameRate: WireTime
    let clips: [WireClip]
    let captions: [WireCaption]
    let transitions: [WireTransition]
}

private struct BuiltComposition {
    let composition: AVMutableComposition
    let videoComposition: AVMutableVideoComposition?
    let audioMix: AVMutableAudioMix?
}

public typealias ExportCallback = @convention(c) (Bool, UnsafePointer<CChar>?, UnsafeMutableRawPointer?) -> Void

private func color(from hex: String, fallback: NSColor) -> NSColor {
    var text = hex.trimmingCharacters(in: .whitespacesAndNewlines)
    if text.hasPrefix("#") { text.removeFirst() }
    guard text.count == 6 || text.count == 8, let raw = UInt64(text, radix: 16) else { return fallback }
    let hasAlpha = text.count == 8
    let red = CGFloat((raw >> (hasAlpha ? 24 : 16)) & 0xff) / 255
    let green = CGFloat((raw >> (hasAlpha ? 16 : 8)) & 0xff) / 255
    let blue = CGFloat((raw >> (hasAlpha ? 8 : 0)) & 0xff) / 255
    let alpha = hasAlpha ? CGFloat(raw & 0xff) / 255 : 1
    return NSColor(red: red, green: green, blue: blue, alpha: alpha)
}

@MainActor
private func makeTimedLayer(_ layer: CALayer, start: Double, duration: Double, visibleOpacity: Float = 1) {
    layer.opacity = 0
    let animation = CAKeyframeAnimation(keyPath: "opacity")
    animation.values = [0, visibleOpacity, visibleOpacity, 0]
    animation.keyTimes = [0, 0.001, 0.999, 1]
    animation.beginTime = AVCoreAnimationBeginTimeAtZero + max(0, start)
    animation.duration = max(0.001, duration)
    animation.isRemovedOnCompletion = false
    animation.fillMode = .both
    layer.add(animation, forKey: "visibility")
}

@MainActor
private func buildComposition(_ request: WireComposition) async throws -> BuiltComposition {
    guard request.width > 0, request.height > 0, request.frameRate.value > 0,
          request.frameRate.timescale > 0 else {
        throw NSError(domain: "OpenEditorMedia", code: 1, userInfo: [NSLocalizedDescriptionKey: "Invalid sequence settings"])
    }

    let composition = AVMutableComposition()
    var videoLayers: [(WireClip, AVMutableCompositionTrack, AVAssetTrack)] = []
    var imageLayers: [WireClip] = []
    var audioParameters: [AVMutableAudioMixInputParameters] = []

    for clip in request.clips {
        guard clip.playbackRate > 0, clip.sourceOut.seconds > clip.sourceIn.seconds else { continue }
        if clip.kind == "image" {
            imageLayers.append(clip)
            continue
        }
        let asset = AVURLAsset(url: URL(fileURLWithPath: clip.path))
        let range = CMTimeRange(start: clip.sourceIn.cmTime, end: clip.sourceOut.cmTime)
        let sourceDuration = range.duration
        let scaledDuration = CMTimeMultiplyByFloat64(sourceDuration, multiplier: 1 / clip.playbackRate)
        let destination = clip.timelineStart.cmTime

        if clip.kind == "video", let sourceVideo = try await asset.loadTracks(withMediaType: .video).first,
           let targetVideo = composition.addMutableTrack(withMediaType: .video, preferredTrackID: kCMPersistentTrackID_Invalid) {
            try targetVideo.insertTimeRange(range, of: sourceVideo, at: destination)
            if scaledDuration != sourceDuration {
                targetVideo.scaleTimeRange(CMTimeRange(start: destination, duration: sourceDuration), toDuration: scaledDuration)
            }
            videoLayers.append((clip, targetVideo, sourceVideo))
        }

        if let sourceAudio = try await asset.loadTracks(withMediaType: .audio).first,
           let targetAudio = composition.addMutableTrack(withMediaType: .audio, preferredTrackID: kCMPersistentTrackID_Invalid) {
            try targetAudio.insertTimeRange(range, of: sourceAudio, at: destination)
            if scaledDuration != sourceDuration {
                targetAudio.scaleTimeRange(CMTimeRange(start: destination, duration: sourceDuration), toDuration: scaledDuration)
            }
            let parameters = AVMutableAudioMixInputParameters(track: targetAudio)
            let clipRange = CMTimeRange(start: destination, duration: scaledDuration)
            parameters.setVolume(Float(max(0, min(2, clip.audio.volume))), at: destination)
            let fadeIn = min(max(0, clip.audio.fadeIn.seconds), scaledDuration.seconds)
            if fadeIn > 0 {
                parameters.setVolumeRamp(fromStartVolume: 0, toEndVolume: Float(clip.audio.volume), timeRange: CMTimeRange(start: destination, duration: CMTime(seconds: fadeIn, preferredTimescale: 600)))
            }
            let fadeOut = min(max(0, clip.audio.fadeOut.seconds), scaledDuration.seconds)
            if fadeOut > 0 {
                let fadeStart = CMTimeSubtract(CMTimeRangeGetEnd(clipRange), CMTime(seconds: fadeOut, preferredTimescale: 600))
                parameters.setVolumeRamp(fromStartVolume: Float(clip.audio.volume), toEndVolume: 0, timeRange: CMTimeRange(start: fadeStart, duration: CMTime(seconds: fadeOut, preferredTimescale: 600)))
            }
            audioParameters.append(parameters)
        }
    }

    let audioMix: AVMutableAudioMix? = audioParameters.isEmpty ? nil : {
        let mix = AVMutableAudioMix()
        mix.inputParameters = audioParameters
        return mix
    }()

    guard !videoLayers.isEmpty else { return BuiltComposition(composition: composition, videoComposition: nil, audioMix: audioMix) }
    let instruction = AVMutableVideoCompositionInstruction()
    instruction.timeRange = CMTimeRange(start: .zero, duration: composition.duration)
    var instructions: [AVMutableVideoCompositionLayerInstruction] = []

    for (clip, target, source) in videoLayers.reversed() {
        let layer = AVMutableVideoCompositionLayerInstruction(assetTrack: target)
        let naturalSize = try await source.load(.naturalSize)
        let preferred = try await source.load(.preferredTransform)
        let transformed = naturalSize.applying(preferred)
        let sourceWidth = max(1, abs(transformed.width))
        let sourceHeight = max(1, abs(transformed.height))
        let fit = min(CGFloat(request.width) / sourceWidth, CGFloat(request.height) / sourceHeight)
        var transform = preferred.concatenating(CGAffineTransform(scaleX: fit, y: fit))
        let fitted = CGRect(origin: .zero, size: naturalSize).applying(transform)
        transform = transform.concatenating(CGAffineTransform(
            translationX: (CGFloat(request.width) - abs(fitted.width)) / 2 - fitted.minX,
            y: (CGFloat(request.height) - abs(fitted.height)) / 2 - fitted.minY
        ))
        transform = transform.concatenating(CGAffineTransform(translationX: clip.transform.x, y: clip.transform.y))
            .concatenating(CGAffineTransform(rotationAngle: clip.transform.rotation * .pi / 180))
            .concatenating(CGAffineTransform(scaleX: clip.transform.scale, y: clip.transform.scale))
        layer.setTransform(transform, at: clip.timelineStart.cmTime)
        layer.setOpacity(Float(max(0, min(1, clip.transform.opacity))), at: clip.timelineStart.cmTime)
        layer.setOpacity(0, at: CMTimeAdd(clip.timelineStart.cmTime, CMTimeMultiplyByFloat64(CMTimeSubtract(clip.sourceOut.cmTime, clip.sourceIn.cmTime), multiplier: 1 / clip.playbackRate)))

        for transition in request.transitions where transition.kind != "cut" {
            let transitionDuration = transition.duration.cmTime
            if transition.fromClipId == clip.id {
                let end = CMTimeAdd(clip.timelineStart.cmTime, CMTimeMultiplyByFloat64(CMTimeSubtract(clip.sourceOut.cmTime, clip.sourceIn.cmTime), multiplier: 1 / clip.playbackRate))
                layer.setOpacityRamp(fromStartOpacity: Float(clip.transform.opacity), toEndOpacity: 0, timeRange: CMTimeRange(start: CMTimeSubtract(end, transitionDuration), duration: transitionDuration))
            } else if transition.toClipId == clip.id {
                layer.setOpacityRamp(fromStartOpacity: 0, toEndOpacity: Float(clip.transform.opacity), timeRange: CMTimeRange(start: clip.timelineStart.cmTime, duration: transitionDuration))
            }
        }
        instructions.append(layer)
    }
    instruction.layerInstructions = instructions

    let videoComposition = AVMutableVideoComposition()
    videoComposition.renderSize = CGSize(width: request.width, height: request.height)
    videoComposition.frameDuration = CMTime(value: Int64(request.frameRate.timescale), timescale: Int32(request.frameRate.value))
    videoComposition.instructions = [instruction]

    if !request.captions.isEmpty || !imageLayers.isEmpty {
        let parent = CALayer()
        let video = CALayer()
        let bounds = CGRect(x: 0, y: 0, width: request.width, height: request.height)
        parent.frame = bounds
        video.frame = bounds
        parent.addSublayer(video)
        for clip in imageLayers {
            guard let image = NSImage(contentsOfFile: clip.path) else { continue }
            var proposed = CGRect(origin: .zero, size: image.size)
            guard let contents = image.cgImage(forProposedRect: &proposed, context: nil, hints: nil) else { continue }
            let overlay = CALayer()
            overlay.contents = contents
            overlay.contentsGravity = .resizeAspect
            let baseWidth = CGFloat(request.width) * 0.28
            let aspect = max(0.01, image.size.height / max(0.01, image.size.width))
            let baseHeight = baseWidth * aspect
            overlay.bounds = CGRect(x: 0, y: 0, width: baseWidth, height: baseHeight)
            overlay.position = CGPoint(
                x: CGFloat(request.width) / 2 + clip.transform.x,
                y: CGFloat(request.height) / 2 + clip.transform.y
            )
            overlay.setAffineTransform(CGAffineTransform(rotationAngle: clip.transform.rotation * .pi / 180).scaledBy(x: clip.transform.scale, y: clip.transform.scale))
            let sourceDuration = clip.sourceOut.seconds - clip.sourceIn.seconds
            makeTimedLayer(overlay, start: clip.timelineStart.seconds, duration: sourceDuration / clip.playbackRate, visibleOpacity: Float(max(0, min(1, clip.transform.opacity))))
            parent.addSublayer(overlay)
        }
        for caption in request.captions {
            let text = CATextLayer()
            text.string = caption.text
            text.alignmentMode = .center
            text.isWrapped = true
            text.contentsScale = 2
            text.fontSize = CGFloat(caption.style.fontSize)
            text.foregroundColor = color(from: caption.style.color, fallback: .white).cgColor
            text.backgroundColor = color(from: caption.style.background, fallback: .clear).cgColor
            let y: CGFloat = switch caption.style.position {
            case "top": CGFloat(request.height) * 0.78
            case "center": CGFloat(request.height) * 0.48
            default: CGFloat(request.height) * 0.12
            }
            text.frame = CGRect(x: CGFloat(request.width) * 0.08, y: y, width: CGFloat(request.width) * 0.84, height: CGFloat(caption.style.fontSize) * 2.8)
            makeTimedLayer(text, start: caption.start.seconds, duration: caption.end.seconds - caption.start.seconds)
            parent.addSublayer(text)
        }
        videoComposition.animationTool = AVVideoCompositionCoreAnimationTool(postProcessingAsVideoLayer: video, in: parent)
    }
    return BuiltComposition(composition: composition, videoComposition: videoComposition, audioMix: audioMix)
}

@MainActor
private final class ExportController {
    var session: AVAssetExportSession?
    var retainedHandle: UnsafeMutableRawPointer?
    let callback: ExportCallback
    let context: UnsafeMutableRawPointer?
    let outputPath: String

    init(callback: @escaping ExportCallback, context: UnsafeMutableRawPointer?, outputPath: String) {
        self.callback = callback
        self.context = context
        self.outputPath = outputPath
    }

    func start(request: WireComposition) async {
        do {
            let built = try await buildComposition(request)
            guard let session = AVAssetExportSession(asset: built.composition, presetName: AVAssetExportPresetHighestQuality) else {
                finish(success: false, message: "AVFoundation could not create an export session")
                return
            }
            self.session = session
            session.outputURL = URL(fileURLWithPath: outputPath)
            session.outputFileType = .mp4
            session.shouldOptimizeForNetworkUse = true
            session.videoComposition = built.videoComposition
            session.audioMix = built.audioMix
            try? FileManager.default.removeItem(atPath: outputPath)
            session.exportAsynchronously { [weak self] in
                Task { @MainActor in
                    guard let self, let session = self.session else { return }
                    switch session.status {
                    case .completed: self.finish(success: true, message: self.outputPath)
                    case .cancelled: self.finish(success: false, message: "cancelled")
                    default: self.finish(success: false, message: session.error?.localizedDescription ?? "AVFoundation export failed")
                    }
                }
            }
        } catch {
            finish(success: false, message: error.localizedDescription)
        }
    }

    func finish(success: Bool, message: String) {
        message.withCString { callback(success, $0, context) }
        if let retainedHandle {
            self.retainedHandle = nil
            Unmanaged<ExportController>.fromOpaque(retainedHandle).release()
        }
    }
}

@_cdecl("oe_export_start")
@MainActor
public func exportStart(
    _ json: UnsafePointer<CChar>?,
    _ outputPath: UnsafePointer<CChar>?,
    _ callback: ExportCallback?,
    _ context: UnsafeMutableRawPointer?
) -> UnsafeMutableRawPointer? {
    guard let json, let outputPath, let callback,
          let data = String(cString: json).data(using: .utf8),
          let request = try? JSONDecoder().decode(WireComposition.self, from: data) else { return nil }
    let controller = ExportController(callback: callback, context: context, outputPath: String(cString: outputPath))
    let handle = Unmanaged.passRetained(controller).toOpaque()
    controller.retainedHandle = handle
    Task { @MainActor in await controller.start(request: request) }
    return handle
}

@_cdecl("oe_export_cancel")
@MainActor
public func exportCancel(_ handle: UnsafeMutableRawPointer?) {
    guard let handle else { return }
    Unmanaged<ExportController>.fromOpaque(handle).takeUnretainedValue().session?.cancelExport()
}

@MainActor
final class PreviewController {
    let player = AVPlayer()
    let layer = AVPlayerLayer()
    private var requestedTime: CMTime = .zero

    init() {
        layer.player = player
        layer.videoGravity = .resizeAspect
        layer.backgroundColor = NSColor.black.cgColor
    }

    func attach(to view: NSView, topLeftFrame: CGRect) {
        view.wantsLayer = true
        layer.removeFromSuperlayer()
        layer.frame = CGRect(x: topLeftFrame.minX, y: view.bounds.height - topLeftFrame.maxY, width: topLeftFrame.width, height: topLeftFrame.height)
        layer.autoresizingMask = []
        view.layer?.addSublayer(layer)
    }

    func load(path: String) {
        player.replaceCurrentItem(with: AVPlayerItem(url: URL(fileURLWithPath: path)))
    }

    fileprivate func load(request: WireComposition) async throws {
        let shouldResume = player.rate != 0
        player.pause()
        let built = try await buildComposition(request)
        let item = AVPlayerItem(asset: built.composition)
        item.videoComposition = built.videoComposition
        item.audioMix = built.audioMix
        player.replaceCurrentItem(with: item)
        await player.seek(to: requestedTime, toleranceBefore: .zero, toleranceAfter: .zero)
        if shouldResume { player.play() }
    }

    func seek(to time: CMTime) {
        requestedTime = time
        player.seek(to: time, toleranceBefore: .zero, toleranceAfter: .zero)
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
    instance.attach(to: view, topLeftFrame: CGRect(x: x, y: y, width: width, height: height))
    return true
}

@_cdecl("oe_player_set_frame")
@MainActor
public func playerSetFrame(
    _ handle: UnsafeMutableRawPointer?,
    _ x: Double, _ y: Double, _ width: Double, _ height: Double
) -> Bool {
    guard let instance = controller(handle) else { return false }
    guard let parent = instance.layer.superlayer else { return false }
    instance.layer.frame = CGRect(x: x, y: parent.bounds.height - y - height, width: width, height: height)
    return true
}

@_cdecl("oe_player_detach")
@MainActor
public func playerDetach(_ handle: UnsafeMutableRawPointer?) {
    controller(handle)?.layer.removeFromSuperlayer()
}

@_cdecl("oe_player_load_file")
@MainActor
public func playerLoadFile(_ handle: UnsafeMutableRawPointer?, _ path: UnsafePointer<CChar>?) -> Bool {
    guard let path else { return false }
    guard let instance = controller(handle) else { return false }
    instance.load(path: String(cString: path))
    return true
}

@_cdecl("oe_player_load_composition")
@MainActor
public func playerLoadComposition(_ handle: UnsafeMutableRawPointer?, _ json: UnsafePointer<CChar>?) -> Bool {
    guard let json, let instance = controller(handle),
          let data = String(cString: json).data(using: .utf8),
          let request = try? JSONDecoder().decode(WireComposition.self, from: data) else { return false }
    Task { @MainActor in try? await instance.load(request: request) }
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
    controller(handle)?.seek(to: CMTime(value: value, timescale: timescale))
}

@_cdecl("oe_player_current_time")
@MainActor
public func playerCurrentTime(_ handle: UnsafeMutableRawPointer?, _ timescale: Int32) -> Int64 {
    guard timescale > 0, let instance = controller(handle) else { return 0 }
    return CMTimeConvertScale(instance.player.currentTime(), timescale: timescale, method: .roundHalfAwayFromZero).value
}

@_cdecl("oe_player_rate")
@MainActor
public func playerRate(_ handle: UnsafeMutableRawPointer?) -> Double {
    Double(controller(handle)?.player.rate ?? 0)
}

@_cdecl("oe_bookmark_create")
public func bookmarkCreate(_ path: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>? {
    guard let path else { return nil }
    let url = URL(fileURLWithPath: String(cString: path))
    guard let data = try? url.bookmarkData(
        options: .withSecurityScope,
        includingResourceValuesForKeys: nil,
        relativeTo: nil
    ) else { return nil }
    return strdup(data.base64EncodedString())
}

@_cdecl("oe_bookmark_resolve")
public func bookmarkResolve(_ encoded: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>? {
    guard let encoded,
          let data = Data(base64Encoded: String(cString: encoded)) else { return nil }
    var stale = false
    guard let url = try? URL(
        resolvingBookmarkData: data,
        options: [.withSecurityScope, .withoutUI],
        relativeTo: nil,
        bookmarkDataIsStale: &stale
    ), !stale, url.startAccessingSecurityScopedResource() else { return nil }
    return strdup(url.path)
}

@_cdecl("oe_bookmark_release")
public func bookmarkRelease(_ path: UnsafePointer<CChar>?) {
    // Access is process-scoped in V0.1 and ends when the app exits. Project-session
    // handles will balance this call when multi-project windows are introduced.
    _ = path
}

@_cdecl("oe_string_free")
public func stringFree(_ value: UnsafeMutablePointer<CChar>?) {
    free(value)
}
