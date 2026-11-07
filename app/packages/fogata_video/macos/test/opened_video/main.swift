// Checks OpenedVideo.swift on its own, without Flutter: a video is read only when it
// begins as its name says, from the file opened then, whatever is put at its path later.
//   run_tests.sh [clip.mp4] (in the folder above)
// The clip is the integration tests' (480 by 270, four seconds).
import AVFoundation
import Foundation

var failures = 0

func check(_ ok: Bool, _ what: String) {
  print("\(ok ? "ok  " : "FAIL"): \(what)")
  if !ok { failures += 1 }
}

let mp4 = [VideoHead(bytes: [0, 0, 0, 0] + Array("ftyp".utf8), mask: [0, 0, 0, 0, 255, 255, 255, 255])]
let mkv = [VideoHead(bytes: [0x1A, 0x45, 0xDF, 0xA3], mask: [255, 255, 255, 255])]

/// The first picture's size and the length of `video`, read as the plugin reads them.
func read(_ video: OpenedVideo) -> (CGSize, Double)? {
  let asset = video.asset()
  let done = DispatchSemaphore(value: 0)
  asset.loadValuesAsynchronously(forKeys: ["tracks", "duration"]) { done.signal() }
  guard done.wait(timeout: .now() + 10) == .success,
    asset.statusOfValue(forKey: "tracks", error: nil) == .loaded
  else { return nil }
  let generator = AVAssetImageGenerator(asset: asset)
  var image: CGImage?
  generator.generateCGImagesAsynchronously(forTimes: [NSValue(time: .zero)]) { _, got, _, _, _ in
    image = got
    done.signal()
  }
  guard done.wait(timeout: .now() + 10) == .success, let image = image else { return nil }
  return (CGSize(width: image.width, height: image.height), CMTimeGetSeconds(asset.duration))
}

let args = CommandLine.arguments
let clip = args[1]
let scratch = FileManager.default.temporaryDirectory.appendingPathComponent(
  "fogata-opened-video-\(getpid())")
try! FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: scratch) }
let path = scratch.appendingPathComponent("clip.mp4").path
try! FileManager.default.copyItem(atPath: clip, toPath: path)

check(
  VideoHead.matches(mp4, [0, 0, 0, 0x20] + Array("ftypisom".utf8)), "an MP4's head matches")
check(!VideoHead.matches(mp4, [0, 0, 0]), "too few bytes do not")
check(!VideoHead.matches([], [0, 0, 0, 0x20] + Array("ftypisom".utf8)), "nor do no heads")
check(OpenedVideo(path: path, heads: mkv) == nil, "a file that is not what its name says is not opened")
check(OpenedVideo(path: path, heads: []) == nil, "nor one with no heads at all")
check(OpenedVideo(path: scratch.path, heads: mp4) == nil, "nor a folder")
let fifo = scratch.appendingPathComponent("pipe.mp4").path
mkfifo(fifo, 0o600)
let started = Date()
check(OpenedVideo(path: fifo, heads: mp4) == nil, "nor a pipe")
check(Date().timeIntervalSince(started) < 1, "which is not waited on")

if let video = OpenedVideo(path: path, heads: mp4) {
  // Another file put in its place, and the original gone: the one opened is still read.
  let other = scratch.appendingPathComponent("other.mp4").path
  try! Data(repeating: 7, count: 100_000).write(to: URL(fileURLWithPath: other))
  _ = rename(other, path)
  let got = read(video)
  check(got?.0 == CGSize(width: 480, height: 270), "the video opened gives its first picture")
  check((got?.1 ?? 0) > 3.9 && (got?.1 ?? 0) < 4.1, "and its length")
  check(OpenedVideo(path: path, heads: mp4) == nil, "what was put in its place is not opened")
} else {
  check(false, "the clip is opened")
}
/// A request for bytes as AVFoundation makes one, that keeps count of what it was given.
final class FakeLoading: VideoLoading {
  init(from: Int64, length: Int, toEnd: Bool = false) {
    requestedOffset = from
    currentOffset = from
    requestedLength = length
    self.toEnd = toEnd
  }
  var isCancelled = false
  var currentOffset: Int64
  let requestedOffset: Int64
  let requestedLength: Int
  let toEnd: Bool
  var given = 0
  var biggest = 0
  var finished = false
  var error: Error?
  var cancelAfter: Int?
  func respond(with data: Data) {
    given += data.count
    biggest = max(biggest, data.count)
    currentOffset += Int64(data.count)
    if let after = cancelAfter, given >= after { isCancelled = true }
  }
  func finish(_ error: Error?) {
    finished = true
    self.error = error
  }
}

/// The memory this program holds now, in bytes.
func resident() -> Int {
  var info = mach_task_basic_info()
  var count = mach_msg_type_number_t(MemoryLayout<mach_task_basic_info>.size / MemoryLayout<natural_t>.size)
  let status = withUnsafeMutablePointer(to: &info) {
    $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
      task_info(mach_task_self_, task_flavor_t(MACH_TASK_BASIC_INFO), $0, &count)
    }
  }
  return status == KERN_SUCCESS ? Int(info.resident_size) : 0
}

// A video of 2 GB: the clip, then 2 GB that are never read for its pictures.
let big = scratch.appendingPathComponent("big.mp4").path
try! FileManager.default.copyItem(atPath: clip, toPath: big)
let clipSize = (try! FileManager.default.attributesOfItem(atPath: clip)[.size] as! NSNumber).int64Value
truncate(big, clipSize + 2 * 1024 * 1024 * 1024)
if let video = OpenedVideo(path: big, heads: mp4) {
  // Asked for all of it, one answer gives at most a few megabytes, and ends.
  let all = FakeLoading(from: 0, length: Int.max, toEnd: true)
  video.answer(all)
  check(all.finished && all.error == nil, "a request for all of it is answered and ended")
  check(all.given == OpenedVideo.most, "with at most \(OpenedVideo.most) bytes")
  check(all.biggest <= 256 * 1024, "a piece at a time")
  // A range is answered as asked, from where it stands.
  let range = FakeLoading(from: 1000, length: 5000)
  video.answer(range)
  check(range.given == 5000 && range.finished, "a range is answered as asked")
  // A seek far into the file: from there.
  let far = FakeLoading(from: clipSize + 1_000_000_000, length: 100_000)
  video.answer(far)
  check(far.given == 100_000 && far.error == nil, "a seek far in is answered from there")
  // Cancelled part way: it stops, and is not ended.
  let cancelled = FakeLoading(from: 0, length: Int.max, toEnd: true)
  cancelled.cancelAfter = 512 * 1024
  video.answer(cancelled)
  check(cancelled.given == 512 * 1024 && !cancelled.finished, "a cancelled request stops at once")
  // Played as AVFoundation reads it, the memory stays small.
  let before = resident()
  let got = read(video)
  let grew = resident() - before
  print("      2 GB video read, memory grew by \(grew / 1024 / 1024) MB")
  check(got?.0 == CGSize(width: 480, height: 270), "a 2 GB video gives its first picture")
  check(grew < 200 * 1024 * 1024, "without holding the file in memory")
  // Made shorter than it was: what is missing is an error, not made up.
  truncate(big, 4096)
  let short = FakeLoading(from: 0, length: 100_000)
  video.answer(short)
  check(short.finished && short.error != nil, "a file cut short ends with an error")
} else {
  check(false, "the 2 GB video is opened")
}

// Small answers: AVFoundation asks again for the rest, and seeks, as it reads.
OpenedVideo.most = 16 * 1024
if let video = OpenedVideo(path: clip, heads: mp4) {
  let got = read(video)
  check(got?.0 == CGSize(width: 480, height: 270), "answers of 16 KB still give the picture")
  let asset = video.asset()
  let generator = AVAssetImageGenerator(asset: asset)
  let done = DispatchSemaphore(value: 0)
  var late: CGImage?
  generator.generateCGImagesAsynchronously(forTimes: [NSValue(time: CMTime(value: 3, timescale: 1))]) {
    _, image, _, _, _ in
    late = image
    done.signal()
  }
  _ = done.wait(timeout: .now() + 10)
  check(late != nil, "and a picture three seconds in")
  withExtendedLifetime(asset) {}
}

// Only the codecs the chat offers are read: the clip's are, ProRes is not.
func loadedAsset(_ video: OpenedVideo) -> AVURLAsset {
  let asset = video.asset()
  let done = DispatchSemaphore(value: 0)
  asset.loadValuesAsynchronously(forKeys: ["tracks"]) { done.signal() }
  _ = done.wait(timeout: .now() + 10)
  return asset
}
check(fourcc("av01") == 0x6176_3031, "a four-character code is read as the system's")
if let video = OpenedVideo(path: clip, heads: mp4) {
  let asset = loadedAsset(video)
  check(codecsAllowed(asset), "the clip's H.264 is read")
  withExtendedLifetime(asset) {}
}
let prores = scratch.appendingPathComponent("prores.mov")
if let writer = try? AVAssetWriter(outputURL: prores, fileType: .mov) {
  let input = AVAssetWriterInput(
    mediaType: .video,
    outputSettings: [
      AVVideoCodecKey: AVVideoCodecType.proRes422, AVVideoWidthKey: 64, AVVideoHeightKey: 64,
    ])
  let frames = AVAssetWriterInputPixelBufferAdaptor(
    assetWriterInput: input, sourcePixelBufferAttributes: nil)
  writer.add(input)
  writer.startWriting()
  writer.startSession(atSourceTime: .zero)
  for i in 0..<2 {
    var buffer: CVPixelBuffer?
    CVPixelBufferCreate(nil, 64, 64, kCVPixelFormatType_32BGRA, nil, &buffer)
    while !input.isReadyForMoreMediaData { usleep(1000) }
    frames.append(buffer!, withPresentationTime: CMTime(value: CMTimeValue(i), timescale: 10))
  }
  input.markAsFinished()
  let done = DispatchSemaphore(value: 0)
  writer.finishWriting { done.signal() }
  _ = done.wait(timeout: .now() + 10)
  let quicktime = [VideoHead(bytes: [0, 0, 0, 0] + Array("ftyp".utf8), mask: [0, 0, 0, 0, 255, 255, 255, 255]),
                   VideoHead(bytes: [0, 0, 0, 0] + Array("wide".utf8), mask: [0, 0, 0, 0, 255, 255, 255, 255])]
  if let video = OpenedVideo(path: prores.path, heads: quicktime) {
    let asset = loadedAsset(video)
    check(!asset.tracks.isEmpty && !codecsAllowed(asset), "ProRes, which the chat does not offer, is not")
    withExtendedLifetime(asset) {}
  } else {
    check(false, "the ProRes movie is opened")
  }
}

print(failures == 0 ? "all passed" : "\(failures) failed")
exit(failures == 0 ? 0 : 1)
