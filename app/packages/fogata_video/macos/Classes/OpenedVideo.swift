import AVFoundation
import Foundation
import UniformTypeIdentifiers

/// One way a video may begin, as the application says for its name (`videoHeads` in
/// lib/fogata_video.dart): `bytes` where `mask` has bits set, anything elsewhere.
struct VideoHead {
  let bytes: [UInt8]
  let mask: [UInt8]

  /// The most bytes of a file's start that are read to check it.
  static let most = 16

  /// Whether `start`, a file's first bytes, is one of `heads`. None is a match for no head
  /// at all, nor for a head longer than what was read.
  static func matches(_ heads: [VideoHead], _ start: [UInt8]) -> Bool {
    heads.contains { head in
      !head.bytes.isEmpty && head.mask.count == head.bytes.count
        && head.bytes.count <= start.count && head.bytes.count <= most
        && head.bytes.indices.allSatisfy {
          start[$0] & head.mask[$0] == head.bytes[$0] & head.mask[$0]
        }
    }
  }
}

/// Whether every track of `asset` (its tracks loaded) is in a codec the chat offers: VP8,
/// VP9, AV1, H.264, H.265, MPEG-4 Part 2 or Motion JPEG pictures; AAC, MP3, Opus or PCM
/// sound. As on Linux and Windows, no other decoder of the system reads a received file:
/// decoders are the most error-prone code it reaches. Other kinds of track are not decoded.
func codecsAllowed(_ asset: AVAsset) -> Bool {
  let video: Set<FourCharCode> = [
    kCMVideoCodecType_VP9, fourcc("vp08"), fourcc("av01"), kCMVideoCodecType_H264,
    kCMVideoCodecType_HEVC, fourcc("hev1"), kCMVideoCodecType_MPEG4Video,
    kCMVideoCodecType_JPEG, kCMVideoCodecType_JPEG_OpenDML,
  ]
  let audio: Set<FourCharCode> = [
    kAudioFormatMPEG4AAC, kAudioFormatMPEG4AAC_HE, kAudioFormatMPEG4AAC_HE_V2,
    kAudioFormatMPEGLayer3, kAudioFormatOpus, kAudioFormatLinearPCM,
  ]
  let tracks = asset.tracks
  guard !tracks.isEmpty else { return false }
  for track in tracks {
    let allowed: Set<FourCharCode>
    switch track.mediaType {
    case .video: allowed = video
    case .audio: allowed = audio
    default: continue
    }
    for description in track.formatDescriptions {
      let subtype = CMFormatDescriptionGetMediaSubType(description as! CMFormatDescription)
      if !allowed.contains(subtype) { return false }
    }
  }
  return true
}

/// A four-character code from its text.
func fourcc(_ text: String) -> FourCharCode {
  text.utf8.reduce(0) { ($0 << 8) | FourCharCode($1) }
}

/// A video file opened once, its first bytes checked, that AVFoundation then reads through
/// this (a resource loader for an address of its own) from that same open file: whatever is
/// at its path later, it is never read.
final class OpenedVideo: NSObject, AVAssetResourceLoaderDelegate {
  private let fd: Int32
  private let length: Int64
  private let type: String
  /// Where AVFoundation asks for the bytes; each answer is read apart ([serve]).
  let queue = DispatchQueue(label: "fogata.video.loader")

  /// The address AVFoundation is given: ours, with the file's own extension, which says its
  /// kind.
  let url: URL

  /// Opens the regular file at `path`, read only, if it begins as one of `heads`; nil
  /// otherwise (and nothing kept open). A pipe or a device is not read at all: opening it
  /// does not wait for a writer.
  init?(path: String, heads: [VideoHead]) {
    let fd = open(path, O_RDONLY | O_CLOEXEC | O_NOCTTY | O_NONBLOCK)
    if fd < 0 { return nil }
    var info = stat()
    var head = [UInt8](repeating: 0, count: VideoHead.most)
    var got = -1
    if fstat(fd, &info) == 0, info.st_mode & S_IFMT == S_IFREG, fcntl(fd, F_SETFL, 0) == 0 {
      repeat {
        got = head.withUnsafeMutableBytes { pread(fd, $0.baseAddress, $0.count, 0) }
      } while got < 0 && errno == EINTR
    }
    guard got >= 0, VideoHead.matches(heads, Array(head.prefix(got))) else {
      close(fd)
      return nil
    }
    self.fd = fd
    length = Int64(info.st_size)
    let ext = (path as NSString).pathExtension.lowercased()
    type = UTType(filenameExtension: ext)?.identifier ?? UTType.movie.identifier
    url = URL(string: "fogata-video://file/video.\(ext.isEmpty ? "mp4" : ext)")!
    super.init()
  }

  deinit {
    close(fd)
  }

  /// An asset that reads this file, following no reference it holds to other files or to
  /// addresses on the network. The asset holds this only weakly: whoever reads it keeps
  /// this as long as the asset.
  func asset() -> AVURLAsset {
    let asset = AVURLAsset(
      url: url,
      options: [
        AVURLAssetReferenceRestrictionsKey: AVAssetReferenceRestrictions.forbidAll.rawValue
      ])
    asset.resourceLoader.setDelegate(self, queue: queue)
    return asset
  }

  func resourceLoader(
    _ resourceLoader: AVAssetResourceLoader,
    shouldWaitForLoadingOfRequestedResource request: AVAssetResourceLoadingRequest
  ) -> Bool {
    guard request.request.url == url else { return false }
    if let info = request.contentInformationRequest {
      info.contentType = type
      info.contentLength = length
      info.isByteRangeAccessSupported = true
    }
    guard let data = request.dataRequest else {
      request.finishLoading()
      return true
    }
    serve(Loading(request, data))
    return true
  }

  /// The most bytes one answer gives: AVFoundation asks again for what it still wants, as
  /// it plays, so a whole video (or what it asks for "to the end") never sits in memory.
  static var most = 2 * 1024 * 1024

  /// Answers `request` on a queue of its own, apart from the others: one that waits on a
  /// slow file holds up no other, and a cancelled one stops at its next piece.
  func serve(_ request: VideoLoading) {
    DispatchQueue(label: "fogata.video.read").async { [self] in answer(request) }
  }

  /// Hands up to [most] bytes from where `request` is to it, a piece at a time, then ends
  /// it; with an error when the file cannot be read, or is shorter than it was.
  func answer(_ request: VideoLoading) {
    let from = request.currentOffset
    var end = request.toEnd ? length : request.requestedOffset + Int64(request.requestedLength)
    end = min(end, length, from + Int64(OpenedVideo.most))
    let piece = 256 * 1024
    var buffer = [UInt8](repeating: 0, count: piece)
    var at = from
    while at < end {
      if request.isCancelled { return }
      let want = Int(min(Int64(piece), end - at))
      let got = buffer.withUnsafeMutableBytes { pread(fd, $0.baseAddress, want, off_t(at)) }
      if got < 0 {
        if errno == EINTR { continue }
        request.finish(NSError(domain: NSPOSIXErrorDomain, code: Int(errno)))
        return
      }
      // The file is shorter than it was: what is not there is not made up.
      if got == 0 {
        request.finish(NSError(domain: NSPOSIXErrorDomain, code: Int(EIO)))
        return
      }
      request.respond(with: Data(buffer[0..<got]))
      at += Int64(got)
    }
    if !request.isCancelled { request.finish(nil) }
  }
}

/// What the loader answers: one of AVFoundation's requests for bytes, or a test's own.
protocol VideoLoading: AnyObject {
  var isCancelled: Bool { get }
  /// Where the next bytes go from.
  var currentOffset: Int64 { get }
  var requestedOffset: Int64 { get }
  var requestedLength: Int { get }
  /// Asked for all there is from its offset on.
  var toEnd: Bool { get }
  func respond(with data: Data)
  /// Ends it, with an error or none.
  func finish(_ error: Error?)
}

/// AVFoundation's request as a [VideoLoading].
final class Loading: VideoLoading {
  private let request: AVAssetResourceLoadingRequest
  private let data: AVAssetResourceLoadingDataRequest

  init(_ request: AVAssetResourceLoadingRequest, _ data: AVAssetResourceLoadingDataRequest) {
    self.request = request
    self.data = data
  }

  var isCancelled: Bool { request.isCancelled }
  var currentOffset: Int64 { data.currentOffset }
  var requestedOffset: Int64 { data.requestedOffset }
  var requestedLength: Int { data.requestedLength }
  var toEnd: Bool { data.requestsAllDataToEndOfResource }
  func respond(with bytes: Data) { data.respond(with: bytes) }
  func finish(_ error: Error?) {
    if let error = error {
      request.finishLoading(with: error)
    } else {
      request.finishLoading()
    }
  }
}
