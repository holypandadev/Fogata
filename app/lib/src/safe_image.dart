import 'dart:async';
import 'dart:ui' as ui;

import 'package:flutter/foundation.dart';
import 'package:flutter/painting.dart';

/// Most pixels of a small picture someone else sent (a profile picture, a banner, a group's
/// picture). The app makes them at most 900 pixels wide.
const maxSmallPicturePixels = 2048 * 2048;

/// Most pixels of a picture shared in a room: a 48-megapixel photo fits. A JPEG or WebP
/// shown smaller is decoded at a fraction of its size; other kinds are decoded whole, at 4
/// bytes a pixel, however small they are shown, and are held to [maxWholePicturePixels].
const maxPicturePixels = 50 * 1000 * 1000;

/// Most pixels of a picture decoded whole (PNG, GIF, BMP...): 64 MB once decoded. Such a
/// picture can be a few kilobytes of one colour, so a room could hold many.
const maxWholePicturePixels = 16 * 1000 * 1000;

/// Most pixels of pictures decoded whole at the same time; one larger than this waits for
/// the others and then goes alone. Pictures scrolled into view at once take turns instead
/// of all being held decoded together.
const decodingPixels = 32 * 1000 * 1000;

/// Most pixels of a picture with more than one frame (a GIF or WebP animation). Each frame
/// is decoded at the picture's full size, however small it is drawn, while the one before
/// it is kept, and again every time the animation comes round: a large one would hold
/// hundreds of megabytes and keep a processor busy for as long as it shows.
const maxAnimationPixels = maxSmallPicturePixels;

/// Whether the header of an encoded picture promises at most [maxPixels], or at most
/// [maxAnimationPixels] for an animation. A file of a few kilobytes can ask for gigabytes
/// once decoded, so the size is read before anything is.
Future<bool> fitsToDecode(Uint8List bytes, int maxPixels) async {
  final buffer = await ui.ImmutableBuffer.fromUint8List(bytes);
  try {
    return await _pixelsToDecode(bytes, buffer, maxPixels) > 0;
  } finally {
    buffer.dispose();
  }
}

/// [fitsToDecode] for [bytes] already in [buffer], which stays: how many pixels the picture
/// has, or 0 when it is too large or cannot be read.
Future<int> _pixelsToDecode(
  Uint8List bytes,
  ui.ImmutableBuffer buffer,
  int maxPixels,
) async {
  if (decodedWhole(bytes) && maxPixels > maxWholePicturePixels) {
    maxPixels = maxWholePicturePixels;
  }
  ui.ImageDescriptor? descriptor;
  ui.Codec? codec;
  try {
    descriptor = await ui.ImageDescriptor.encoded(buffer);
    final pixels = descriptor.width * descriptor.height;
    if (pixels <= 0 || pixels > maxPixels) return 0;
    if (pixels <= maxAnimationPixels) return pixels;
    // Counting the frames decodes none of them.
    codec = await descriptor.instantiateCodec();
    return codec.frameCount == 1 ? pixels : 0;
  } catch (_) {
    return 0;
  } finally {
    codec?.dispose();
    descriptor?.dispose();
  }
}

/// Whether a picture is decoded at its full size whatever size it is shown: all but JPEG
/// and WebP, which are decoded straight at a smaller size.
bool decodedWhole(Uint8List bytes) {
  final jpeg =
      bytes.length > 3 &&
      bytes[0] == 0xff &&
      bytes[1] == 0xd8 &&
      bytes[2] == 0xff;
  final webp =
      bytes.length > 12 &&
      String.fromCharCodes(bytes.sublist(0, 4)) == 'RIFF' &&
      String.fromCharCodes(bytes.sublist(8, 12)) == 'WEBP';
  return !jpeg && !webp;
}

/// Pixels of pictures decoded whole being decoded now, and those waiting for their turn.
int _decoding = 0;
final _turns = <(int, Completer<void>)>[];

Future<void> _startDecoding(int pixels) async {
  if (_decoding > 0 && _decoding + pixels > decodingPixels ||
      _turns.isNotEmpty) {
    final turn = Completer<void>();
    _turns.add((pixels, turn));
    await turn.future;
  } else {
    _decoding += pixels;
  }
}

void _endDecoding(int pixels) {
  _decoding -= pixels;
  while (_turns.isNotEmpty) {
    final (next, turn) = _turns.first;
    if (_decoding > 0 && _decoding + next > decodingPixels) break;
    _turns.removeAt(0);
    _decoding += next;
    turn.complete();
  }
}

/// A codec whose frames are decoded in turn with other large pictures (see
/// [decodingPixels]).
class _InTurn implements ui.Codec {
  _InTurn(this._codec, this._pixels);

  final ui.Codec _codec;
  final int _pixels;

  @override
  int get frameCount => _codec.frameCount;

  @override
  int get repetitionCount => _codec.repetitionCount;

  @override
  Future<ui.FrameInfo> getNextFrame() async {
    await _startDecoding(_pixels);
    try {
      return await _codec.getNextFrame();
    } finally {
      _endDecoding(_pixels);
    }
  }

  @override
  void dispose() => _codec.dispose();
}

/// A codec that gives only the first frame of a picture of several.
class _FirstFrame implements ui.Codec {
  _FirstFrame(this._codec);

  final ui.Codec _codec;

  @override
  int get frameCount => 1;

  @override
  int get repetitionCount => 0;

  @override
  Future<ui.FrameInfo> getNextFrame() => _codec.getNextFrame();

  @override
  void dispose() => _codec.dispose();
}

/// [MemoryImage] for bytes someone else sent: decoded only if the header promises at most
/// [maxPixels]; otherwise the image fails to load and shows nothing. A picture of several
/// frames shows its first.
@immutable
class SafeMemoryImage extends ImageProvider<SafeMemoryImage> {
  const SafeMemoryImage(this.bytes, {this.maxPixels = maxSmallPicturePixels});

  final Uint8List bytes;
  final int maxPixels;

  @override
  Future<SafeMemoryImage> obtainKey(ImageConfiguration configuration) =>
      SynchronousFuture<SafeMemoryImage>(this);

  @override
  ImageStreamCompleter loadImage(
    SafeMemoryImage key,
    ImageDecoderCallback decode,
  ) => MultiFrameImageStreamCompleter(codec: _load(decode), scale: 1);

  /// The bytes are copied once: the buffer the header is checked in is the one decoded.
  Future<ui.Codec> _load(ImageDecoderCallback decode) async {
    final buffer = await ui.ImmutableBuffer.fromUint8List(bytes);
    final pixels = await _pixelsToDecode(bytes, buffer, maxPixels);
    if (pixels == 0) {
      buffer.dispose();
      throw StateError('picture too large or damaged');
    }
    // Decoding lets the buffer go.
    var codec = await decode(buffer);
    // Only the first frame: animations play only where the app plays them, in turns and
    // at the size drawn (moving.dart). A GIF, WebP or animated PNG drawn here would decode
    // every frame at full size as fast as the screen draws.
    if (codec.frameCount > 1) codec = _FirstFrame(codec);
    // Small pictures (avatars, animations) go at once; large ones decoded whole take turns.
    if (maxPixels <= maxSmallPicturePixels || !decodedWhole(bytes)) {
      return codec;
    }
    return _InTurn(codec, pixels);
  }

  /// By the bytes, not the list: the same picture handed over again at a refresh comes
  /// from the cache rather than being decoded again.
  @override
  bool operator ==(Object other) =>
      other is SafeMemoryImage &&
      other.maxPixels == maxPixels &&
      samePicture(other.bytes, bytes);

  @override
  int get hashCode => Object.hash(_hashOf(bytes), maxPixels);
}

/// Whether two pictures have the same bytes. The core hands over a new list with the same
/// bytes at every refresh, so they are compared by content, after a hash kept per list.
bool samePicture(Uint8List? a, Uint8List? b) {
  if (identical(a, b)) return true;
  if (a == null || b == null || a.length != b.length) return false;
  if (_hashOf(a) != _hashOf(b)) return false;
  for (var i = 0; i < a.length; i++) {
    if (a[i] != b[i]) return false;
  }
  return true;
}

final _hashes = Expando<int>('picture hash');

/// A hash of [bytes] that goes with [samePicture], for maps of pictures by content.
int pictureHash(Uint8List bytes) => _hashOf(bytes);

/// The length and up to 4096 bytes spread over the picture, worked out once per list: a
/// large picture is not read whole for it.
int _hashOf(Uint8List bytes) => _hashes[bytes] ??= () {
  final step = bytes.length ~/ 4096 + 1;
  var hash = bytes.length;
  for (var i = 0; i < bytes.length; i += step) {
    hash = 0x1fffffff & (hash * 31 + bytes[i]);
  }
  return hash;
}();

/// A picture someone else sent, decoded at most [width] pixels wide when given.
ImageProvider safePicture(
  Uint8List bytes, {
  int? width,
  int maxPixels = maxSmallPicturePixels,
}) => ResizeImage.resizeIfNeeded(
  width,
  null,
  SafeMemoryImage(bytes, maxPixels: maxPixels),
);
