import AVFoundation
import AppKit
import CoreAudio
import FlutterMacOS

/// The largest picture played: 8K (7680 by 4320 pixels), no side longer than 8192. A video
/// can claim far more (and a size that is not a number is refused too).
func pictureSizeAllowed(width: Double, height: Double) -> Bool {
  width > 0 && height > 0 && width <= 8192 && height <= 8192 && width * height <= 7680 * 4320
}

/// Videos on macOS with AVFoundation: a video's first picture and length, and players that
/// draw into Flutter textures.
public class FogataVideoPlugin: NSObject, FlutterPlugin {
  private let textures: FlutterTextureRegistry
  private var players: [Int64: VideoPlayer] = [:]

  init(textures: FlutterTextureRegistry) {
    self.textures = textures
  }

  public static func register(with registrar: FlutterPluginRegistrar) {
    let channel = FlutterMethodChannel(
      name: "fogata/video", binaryMessenger: registrar.messenger)
    let plugin = FogataVideoPlugin(textures: registrar.textures)
    registrar.addMethodCallDelegate(plugin, channel: channel)
  }

  public func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    let args = call.arguments as? [String: Any] ?? [:]
    switch call.method {
    case "probe":
      guard let path = args["path"] as? String else {
        result(FlutterError(code: "args", message: "no path", details: nil))
        return
      }
      let maxWidth = args["maxWidth"] as? Int ?? 480
      // At most a few at once, whoever asks: one more is answered at once as unread.
      guard FogataVideoPlugin.probesRunning < FogataVideoPlugin.probesAtOnce,
        FogataVideoPlugin.inFlight < FogataVideoPlugin.mostInFlight
      else {
        result(nil)
        return
      }
      FogataVideoPlugin.probesRunning += 1
      FogataVideoPlugin.inFlight += 1
      // Answered once, on the main thread: by the reading, or as unread once it is far past
      // its time (a read the system never gives back would otherwise hold its place).
      var answered = false
      let answer = { (probe: [String: Any]?) in
        guard !answered else { return }
        answered = true
        FogataVideoPlugin.probesRunning -= 1
        result(probe)
      }
      let heads = FogataVideoPlugin.heads(args["heads"])
      DispatchQueue.global(qos: .userInitiated).async {
        let probe = FogataVideoPlugin.probe(path: path, heads: heads, maxWidth: maxWidth)
        DispatchQueue.main.async {
          FogataVideoPlugin.inFlight -= 1
          answer(probe)
        }
      }
      DispatchQueue.main.asyncAfter(deadline: .now() + FogataVideoPlugin.probeTime + .seconds(5)) {
        answer(nil)
      }
    case "open":
      guard let path = args["path"] as? String else {
        result(FlutterError(code: "args", message: "no path", details: nil))
        return
      }
      let heads = FogataVideoPlugin.heads(args["heads"])
      let output = args["output"] as? String
      // Files that never open (on a network share that hangs) each hold a thread: past so
      // many, no more is tried.
      guard FogataVideoPlugin.inFlight < FogataVideoPlugin.mostInFlight else {
        result(nil)
        return
      }
      FogataVideoPlugin.inFlight += 1
      // The file is opened and checked away from the main thread, which a file that is
      // slow to open (on a network share) would otherwise hold up. Not the kind of video
      // its name says, or not there: no player.
      DispatchQueue.global(qos: .userInitiated).async {
        var opened = OpenedVideo(path: path, heads: heads)
        // A codec the chat does not offer: no player. Its tracks are read here, away from
        // the main thread, within the time a reading gets.
        if let video = opened {
          let asset = video.asset()
          if !FogataVideoPlugin.loaded(
            asset, keys: ["tracks"], by: .now() + FogataVideoPlugin.probeTime,
            cancel: asset.cancelLoading) || !codecsAllowed(asset)
          {
            opened = nil
          }
          withExtendedLifetime(asset) {}
        }
        DispatchQueue.main.async { [self] in
          FogataVideoPlugin.inFlight -= 1
          guard let opened = opened else {
            result(nil)
            return
          }
          let player = VideoPlayer(video: opened, textures: textures)
          // The output chosen in the application's settings, by its Core Audio id; the
          // system's default when there is none or no such output is connected.
          if let output = output, FogataVideoPlugin.outputConnected(uid: output) {
            player.player.audioOutputDeviceUniqueID = output
          }
          let id = textures.register(player)
          player.textureId = id
          players[id] = player
          player.start()
          result(id)
        }
      }
    default:
      guard let id = (args["id"] as? NSNumber)?.int64Value, let player = players[id] else {
        result(FlutterError(code: "player", message: "no such player", details: nil))
        return
      }
      switch call.method {
      case "play":
        player.play()
      case "pause":
        player.player.pause()
      case "seek":
        player.seek(ms: args["ms"] as? Int ?? 0)
      case "volume":
        player.player.volume = Float(args["volume"] as? Double ?? 1)
      case "state":
        result(player.state())
        return
      case "close":
        player.stop()
        textures.unregisterTexture(id)
        players[id] = nil
      default:
        result(FlutterMethodNotImplemented)
        return
      }
      result(nil)
    }
  }

  /// Whether Core Audio has a device with this unique id now.
  static func outputConnected(uid: String) -> Bool {
    var address = AudioObjectPropertyAddress(
      mSelector: kAudioHardwarePropertyTranslateUIDToDevice,
      mScope: kAudioObjectPropertyScopeGlobal,
      mElement: kAudioObjectPropertyElementMain)
    var name = uid as CFString
    var device = AudioDeviceID(kAudioObjectUnknown)
    var size = UInt32(MemoryLayout<AudioDeviceID>.size)
    let status = withUnsafeMutablePointer(to: &name) { qualifier in
      AudioObjectGetPropertyData(
        AudioObjectID(kAudioObjectSystemObject), &address,
        UInt32(MemoryLayout<CFString>.size), qualifier, &size, &device)
    }
    return status == noErr && device != kAudioObjectUnknown
  }

  /// The heads the application gives for a video (see OpenedVideo.swift): a list of maps
  /// of "bytes" and "mask". None when they are missing or not of that shape.
  static func heads(_ value: Any?) -> [VideoHead] {
    guard let list = value as? [Any] else { return [] }
    return list.compactMap { item in
      guard let map = item as? [String: Any],
        let bytes = map["bytes"] as? FlutterStandardTypedData,
        let mask = map["mask"] as? FlutterStandardTypedData
      else { return nil }
      return VideoHead(bytes: [UInt8](bytes.data), mask: [UInt8](mask.data))
    }
  }

  /// Readings of a video's first picture running now, and the most at once; on the main
  /// thread only.
  static var probesRunning = 0
  static let probesAtOnce = 2

  /// Openings and readings whose thread has not come back yet, answered or not, and the
  /// most there may be; on the main thread only.
  static var inFlight = 0
  static let mostInFlight = 8

  /// How long a video's first picture is waited for, from the start of its reading (the
  /// file's tracks too), before the reading is cancelled.
  static let probeTime: DispatchTimeInterval = .seconds(10)

  /// Loads `keys` of `value` apart, waiting no later than `deadline`: whether they loaded.
  /// Reading a file's header can itself hang (on a network share that stopped answering,
  /// say); then the loading is cancelled and its thread let go of.
  static func loaded(
    _ value: AVAsynchronousKeyValueLoading, keys: [String], by deadline: DispatchTime,
    cancel: () -> Void
  ) -> Bool {
    let done = DispatchSemaphore(value: 0)
    value.loadValuesAsynchronously(forKeys: keys) { done.signal() }
    if done.wait(timeout: deadline) == .timedOut {
      cancel()
      _ = done.wait(timeout: .now() + .seconds(2))
      return false
    }
    return keys.allSatisfy { value.statusOfValue(forKey: $0, error: nil) == .loaded }
  }

  /// The picture a reading gives, set on the generator's thread before it signals.
  final class Picture: @unchecked Sendable {
    var image: CGImage?
  }

  /// The first picture as a JPEG at most `maxWidth` wide, the size and the length; nil
  /// when there is no picture to read, or none within [probeTime].
  static func probe(path: String, heads: [VideoHead], maxWidth: Int) -> [String: Any]? {
    let deadline = DispatchTime.now() + probeTime
    guard let opened = OpenedVideo(path: path, heads: heads) else { return nil }
    // Kept until the reading is done: the asset holds it only weakly.
    defer { withExtendedLifetime(opened) {} }
    let asset = opened.asset()
    // Read apart under the same deadline, so that what follows never waits on the file.
    guard
      loaded(asset, keys: ["tracks", "duration"], by: deadline, cancel: asset.cancelLoading),
      codecsAllowed(asset)
    else { return nil }
    guard let track = asset.tracks(withMediaType: .video).first,
      loaded(
        track, keys: ["naturalSize", "preferredTransform"], by: deadline,
        cancel: asset.cancelLoading)
    else { return nil }
    let size = track.naturalSize.applying(track.preferredTransform)
    guard pictureSizeAllowed(width: Double(abs(size.width)), height: Double(abs(size.height)))
    else { return nil }
    let generator = AVAssetImageGenerator(asset: asset)
    generator.appliesPreferredTrackTransform = true
    generator.maximumSize = CGSize(width: maxWidth, height: maxWidth * 4)
    // Asked for apart, so that a reading that takes too long is cancelled for real rather
    // than left holding its thread.
    let picture = Picture()
    let done = DispatchSemaphore(value: 0)
    generator.generateCGImagesAsynchronously(forTimes: [NSValue(time: .zero)]) {
      _, image, _, outcome, _ in
      if outcome == .succeeded { picture.image = image }
      done.signal()
    }
    if done.wait(timeout: deadline) == .timedOut {
      generator.cancelAllCGImageGeneration()
      _ = done.wait(timeout: .now() + .seconds(2))
      return nil
    }
    guard let image = picture.image else { return nil }
    let rep = NSBitmapImageRep(cgImage: image)
    guard let jpeg = rep.representation(using: .jpeg, properties: [.compressionFactor: 0.75])
    else { return nil }
    let seconds = CMTimeGetSeconds(asset.duration)
    return [
      "thumb": FlutterStandardTypedData(bytes: jpeg),
      "width": Int(abs(size.width)),
      "height": Int(abs(size.height)),
      "durationMs": seconds.isFinite ? Int(seconds * 1000) : 0,
    ]
  }
}

/// A video playing: AVPlayer's pictures, taken as they are due, handed to Flutter.
final class VideoPlayer: NSObject, FlutterTexture {
  let player: AVPlayer
  var textureId: Int64 = 0
  private let output: AVPlayerItemVideoOutput
  private weak var textures: FlutterTextureRegistry?
  private var timer: Timer?
  private var latest: CVPixelBuffer?
  private let lock = NSLock()
  private var ended = false
  private var tooLarge = false
  private var observer: NSObjectProtocol?

  /// The file it plays, kept while it plays: its asset holds it only weakly.
  private let video: OpenedVideo

  init(video: OpenedVideo, textures: FlutterTextureRegistry) {
    self.video = video
    let asset = video.asset()
    let item = AVPlayerItem(asset: asset)
    // The output hands over pictures as they are stored; phones store upright videos as
    // sideways pictures and a rotation (the track's preferred transform). A composition
    // made from the asset's own properties turns them as the file says.
    if let track = asset.tracks(withMediaType: .video).first,
      !track.preferredTransform.isIdentity
    {
      item.videoComposition = AVMutableVideoComposition(propertiesOf: asset)
    }
    output = AVPlayerItemVideoOutput(pixelBufferAttributes: [
      kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
      kCVPixelBufferMetalCompatibilityKey as String: true,
    ])
    item.add(output)
    player = AVPlayer(playerItem: item)
    self.textures = textures
    super.init()
    observer = NotificationCenter.default.addObserver(
      forName: .AVPlayerItemDidPlayToEndTime, object: item, queue: .main
    ) { [weak self] _ in self?.ended = true }
  }

  func start() {
    // In the common modes, so pictures keep coming while a menu is open or the window is
    // being resized.
    let timer = Timer(timeInterval: 1.0 / 60, repeats: true) { [weak self] _ in
      self?.takePicture()
    }
    RunLoop.main.add(timer, forMode: .common)
    self.timer = timer
  }

  private func takePicture() {
    let time = output.itemTime(forHostTime: CACurrentMediaTime())
    guard output.hasNewPixelBuffer(forItemTime: time),
      let buffer = output.copyPixelBuffer(forItemTime: time, itemTimeForDisplay: nil)
    else { return }
    guard
      pictureSizeAllowed(
        width: Double(CVPixelBufferGetWidth(buffer)),
        height: Double(CVPixelBufferGetHeight(buffer)))
    else {
      // Too large to play: it stops here for good, and says why.
      tooLarge = true
      ended = true
      player.pause()
      timer?.invalidate()
      timer = nil
      return
    }
    lock.lock()
    latest = buffer
    lock.unlock()
    textures?.textureFrameAvailable(textureId)
  }

  func copyPixelBuffer() -> Unmanaged<CVPixelBuffer>? {
    lock.lock()
    defer { lock.unlock() }
    guard let buffer = latest else { return nil }
    return Unmanaged.passRetained(buffer)
  }

  func play() {
    if tooLarge { return }
    if ended {
      player.seek(to: .zero)
      ended = false
    }
    player.play()
  }

  func seek(ms: Int) {
    ended = false
    player.seek(
      to: CMTime(value: CMTimeValue(ms), timescale: 1000), toleranceBefore: .zero,
      toleranceAfter: .zero)
  }

  func state() -> [String: Any] {
    let item = player.currentItem
    let position = CMTimeGetSeconds(player.currentTime())
    let duration = item.map { CMTimeGetSeconds($0.duration) } ?? 0
    let size = item?.presentationSize ?? .zero
    var state: [String: Any] = [
      "positionMs": position.isFinite ? Int(position * 1000) : 0,
      "durationMs": duration.isFinite ? Int(duration * 1000) : 0,
      "playing": player.rate != 0,
      "ended": ended,
      "width": Int(size.width),
      "height": Int(size.height),
    ]
    if tooLarge {
      state["error"] = "its pictures are too large to play"
    } else if let error = item?.error {
      state["error"] = error.localizedDescription
    }
    return state
  }

  func stop() {
    timer?.invalidate()
    timer = nil
    player.pause()
    if let observer = observer {
      NotificationCenter.default.removeObserver(observer)
    }
    player.replaceCurrentItem(with: nil)
  }
}
