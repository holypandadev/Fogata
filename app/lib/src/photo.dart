import 'dart:isolate';
import 'dart:math' as math;
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:file_selector/file_selector.dart';

import 'jpeg.dart';
import 'l10n.dart';
import 'safe_image.dart';

/// Profile pictures travel inside the signed profile, so they stay small.
const maxPhotoBytes = 64 * 1024;

/// A group's picture travels inside an entry of its log, smaller still.
const maxGroupPictureBytes = 48 * 1024;

/// Most bytes of a picture picked from the computer, read whole before anything else is
/// checked: the pixels are held to [maxPicturePixels], which a photo of this size fits.
const maxPickedBytes = 32 * 1024 * 1024;

/// What [chunks] hold, or null when they hold more than [max] bytes: it stops reading at
/// that point, so a file that grew, or that never ends, costs at most [max] and a chunk.
Future<Uint8List?> readAtMost(Stream<List<int>> chunks, int max) async {
  final all = BytesBuilder(copy: false);
  await for (final chunk in chunks) {
    all.add(chunk);
    if (all.length > max) return null;
  }
  return all.takeBytes();
}

/// The bytes of a picked [file]: its size on disk is looked at first, and then it is read
/// with the same limit, so a file that grows between the two is caught too. Throws
/// [PictureProblem] for one over [maxBytes].
Future<Uint8List> readPicked(
  XFile file, {
  int maxBytes = maxPickedBytes,
}) async {
  if (await file.length() > maxBytes) throw PictureProblem(unreadablePicture);
  final bytes = await readAtMost(file.openRead(0, maxBytes + 1), maxBytes);
  if (bytes == null) throw PictureProblem(unreadablePicture);
  return bytes;
}

/// Lets the person pick a picture and makes a small square of it, at most [maxBytes].
Future<Uint8List?> pickPhoto({int maxBytes = maxPhotoBytes}) async {
  final file = await openFile(
    acceptedTypeGroups: [
      XTypeGroup(
        label: l10n.photoImages,
        extensions: const ['png', 'jpg', 'jpeg', 'webp'],
      ),
    ],
  );
  if (file == null) return null;
  return squarePicture(await readPicked(file), maxBytes: maxBytes);
}

/// The middle square of a picture, at most [maxBytes]. A photo (no see-through pixels)
/// becomes a JPEG 256 pixels wide, sharp in a call's tile on a high-density screen; a
/// PNG of a photo that size would not fit. A picture with see-through parts stays a PNG,
/// as large as fits.
Future<Uint8List?> squarePicture(
  Uint8List bytes, {
  int maxBytes = maxPhotoBytes,
}) async {
  final image = await _decode(bytes, (w, h) => 256 / (w < h ? w : h));
  try {
    final side = image.width < image.height ? image.width : image.height;
    final src = ui.Rect.fromLTWH(
      (image.width - side) / 2,
      (image.height - side) / 2,
      side.toDouble(),
      side.toDouble(),
    );
    final first = await _draw(image, src, 256, 256);
    final ByteData? pixels;
    try {
      pixels = await first.toByteData(format: ui.ImageByteFormat.rawRgba);
    } finally {
      first.dispose();
    }
    if (pixels != null && _opaque(pixels)) {
      final rgba = pixels.buffer.asUint8List();
      for (final quality in const [90, 84, 76, 68]) {
        final jpeg = await _jpeg(rgba, 256, 256, quality);
        if (jpeg.length <= maxBytes) return jpeg;
      }
    }
    for (final size in [192, 160, 128, 96, 72]) {
      final data = await _png(await _draw(image, src, size, size));
      if (data != null && data.lengthInBytes <= maxBytes) return data;
    }
    return null;
  } finally {
    image.dispose();
  }
}

/// A picture someone picked that cannot be used, and why, in words to show.
class PictureProblem implements Exception {
  const PictureProblem(this.message);
  final String message;

  @override
  String toString() => message;
}

/// The first picture of [bytes]; the caller lets it go. A picture of more pixels than
/// [maxPicturePixels] (fewer for one decoded whole) throws [PictureProblem] before any is
/// decoded, and one larger than needed is decoded at [scaleFor] of its size (never more
/// than 1): a large photo would hold hundreds of megabytes. The codec goes at once.
Future<ui.Image> _decode(
  Uint8List bytes,
  double Function(int width, int height) scaleFor,
) async {
  final fits = await fitsToDecode(bytes, maxPicturePixels);
  final buffer = await ui.ImmutableBuffer.fromUint8List(bytes);
  ui.ImageDescriptor? descriptor;
  ui.Codec? codec;
  try {
    try {
      descriptor = await ui.ImageDescriptor.encoded(buffer);
    } catch (_) {
      throw PictureProblem(unreadablePicture);
    }
    final width = descriptor.width;
    final height = descriptor.height;
    if (width <= 0 || height <= 0) {
      throw PictureProblem(unreadablePicture);
    }
    if (!fits) {
      final most = decodedWhole(bytes)
          ? maxWholePicturePixels
          : maxPicturePixels;
      throw PictureProblem(l10n.photoTooLarge(most ~/ 1000000));
    }
    final scale = math.min(1.0, scaleFor(width, height));
    codec = await descriptor.instantiateCodec(
      targetWidth: math.max(1, (width * scale).ceil()),
      targetHeight: math.max(1, (height * scale).ceil()),
    );
    return (await codec.getNextFrame()).image;
  } on PictureProblem {
    rethrow;
  } catch (_) {
    throw PictureProblem(unreadablePicture);
  } finally {
    codec?.dispose();
    descriptor?.dispose();
    buffer.dispose();
  }
}

/// What [PictureProblem] says of a picture that cannot be read.
String get unreadablePicture => l10n.photoUnreadable;

/// [src] of [image] drawn at [width] by [height]; the caller lets it go.
Future<ui.Image> _draw(
  ui.Image image,
  ui.Rect src,
  int width,
  int height,
) async {
  final recorder = ui.PictureRecorder();
  ui.Canvas(recorder).drawImageRect(
    image,
    src,
    ui.Rect.fromLTWH(0, 0, width.toDouble(), height.toDouble()),
    ui.Paint()..filterQuality = ui.FilterQuality.high,
  );
  final picture = recorder.endRecording();
  try {
    return await picture.toImage(width, height);
  } finally {
    picture.dispose();
  }
}

/// [image] as a PNG, letting the image go.
Future<Uint8List?> _png(ui.Image image) async {
  try {
    final data = await image.toByteData(format: ui.ImageByteFormat.png);
    return data?.buffer.asUint8List();
  } finally {
    image.dispose();
  }
}

/// [rgba] as a JPEG, encoded away from the window's own thread, which would stop
/// drawing meanwhile.
Future<Uint8List> _jpeg(Uint8List rgba, int width, int height, int quality) =>
    Isolate.run(() => encodeJpeg(rgba, width, height, quality: quality));

/// Whether every pixel is opaque, near enough.
bool _opaque(ByteData rgba) {
  for (var i = 3; i < rgba.lengthInBytes; i += 4) {
    if (rgba.getUint8(i) < 250) return false;
  }
  return true;
}

/// The top of a person's panel is three times as wide as it is tall.
const bannerAspect = 3.0;

/// A picture cut to fit the top of the person's panel, as a PNG up to 900 pixels wide. It
/// stays on this computer.
Future<Uint8List?> bannerPng(Uint8List bytes) async {
  final image = await _decode(
    bytes,
    (w, h) => 900 / math.min(w, h * bannerAspect),
  );
  try {
    final w = image.width.toDouble();
    final h = image.height.toDouble();
    final src = w / h > bannerAspect
        ? ui.Rect.fromLTWH((w - h * bannerAspect) / 2, 0, h * bannerAspect, h)
        : ui.Rect.fromLTWH(0, (h - w / bannerAspect) / 2, w, w / bannerAspect);
    final width = src.width < 900 ? src.width.round() : 900;
    final height = (width / bannerAspect).round();
    return await _png(await _draw(image, src, width, height));
  } finally {
    image.dispose();
  }
}

/// Whether a file is a video the chat can play, by its name.
bool isVideo(String name) {
  final dot = name.lastIndexOf('.');
  if (dot < 0) return false;
  return const {
    'mp4',
    'm4v',
    'mov',
    'webm',
    'mkv',
    'avi',
  }.contains(name.substring(dot + 1).toLowerCase());
}

/// Whether a file is a picture the chat can show, by its name.
bool isPicture(String name) {
  final dot = name.lastIndexOf('.');
  if (dot < 0) return false;
  return const {
    'png',
    'jpg',
    'jpeg',
    'gif',
    'webp',
    'bmp',
  }.contains(name.substring(dot + 1).toLowerCase());
}

/// Banners go to friends inside a signed message, so they stay small.
const maxFriendBannerBytes = 64 * 1024;

/// The banner made small enough for friends: the same picture, narrower, as a PNG of at
/// most [maxFriendBannerBytes]; null if even the smallest is too big.
Future<Uint8List?> friendBannerPng(Uint8List banner) async {
  if (banner.lengthInBytes <= maxFriendBannerBytes) return banner;
  // The banner is this device's own, made at most 900 pixels wide.
  final image = await _decode(banner, (_, _) => 1);
  try {
    final whole = ui.Rect.fromLTWH(
      0,
      0,
      image.width.toDouble(),
      image.height.toDouble(),
    );
    for (final width in [600, 480, 384, 300, 240, 180, 120]) {
      if (width >= image.width) continue;
      final height = (width / bannerAspect).round();
      final data = await _png(await _draw(image, whole, width, height));
      if (data != null && data.lengthInBytes <= maxFriendBannerBytes) {
        return data;
      }
    }
    return null;
  } finally {
    image.dispose();
  }
}

/// A group's banner is as wide as the side column and a little more than half as tall.
const groupBannerAspect = 16 / 9;

/// A still picture cut to fit a group's banner: the middle of it, at most 960 pixels wide,
/// as a JPEG (a PNG when it has see-through parts), at most [maxBytes].
Future<Uint8List?> groupBannerPicture(
  Uint8List bytes, {
  int maxBytes = 512 * 1024,
}) async {
  final image = await _decode(
    bytes,
    (w, h) => 960 / math.min(w, h * groupBannerAspect),
  );
  try {
    final w = image.width.toDouble();
    final h = image.height.toDouble();
    final src = w / h > groupBannerAspect
        ? ui.Rect.fromLTWH(
            (w - h * groupBannerAspect) / 2,
            0,
            h * groupBannerAspect,
            h,
          )
        : ui.Rect.fromLTWH(
            0,
            (h - w / groupBannerAspect) / 2,
            w,
            w / groupBannerAspect,
          );
    for (final most in const [960, 720, 480]) {
      final width = src.width < most ? src.width.round() : most;
      final height = (width / groupBannerAspect).round();
      final out = await _draw(image, src, width, height);
      try {
        final pixels = await out.toByteData(format: ui.ImageByteFormat.rawRgba);
        if (pixels != null && _opaque(pixels)) {
          final jpeg = await _jpeg(
            pixels.buffer.asUint8List(),
            width,
            height,
            84,
          );
          if (jpeg.length <= maxBytes) return jpeg;
        } else {
          final png = await out.toByteData(format: ui.ImageByteFormat.png);
          if (png != null && png.lengthInBytes <= maxBytes) {
            return png.buffer.asUint8List();
          }
        }
      } finally {
        out.dispose();
      }
    }
    return null;
  } finally {
    image.dispose();
  }
}
