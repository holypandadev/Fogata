import 'dart:async';
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/foundation.dart';
import 'package:flutter/painting.dart';

// Round pictures and discs with a smooth edge on every renderer.
//
// Renderers smooth the edge of a shape in their own way, and with only a few samples per
// pixel the edge of a circle shows steps. Here the edge is in the pixels of an image
// instead: each pixel on the edge is as see-through as the part of it outside the shape,
// and the image is drawn as a plain rectangle whose border is see-through, so there is no
// edge of a shape left for the renderer to smooth. The images are made at about twice the
// size they are drawn at, so their edge comes out a pixel wide on screen.

/// A picture cut to a square with round corners, [corner] of its side (.5: a circle),
/// the longer side cut in the middle, over [under] where it is see-through. One pixel of
/// see-through border stays around the shape: [paintRound] draws it so that the shape
/// fills the rectangle it is given.
@immutable
class RoundImage extends ImageProvider<RoundImage> {
  const RoundImage(this.source, {this.corner = .5, this.under});

  final ImageProvider source;
  final double corner;
  final Color? under;

  @override
  Future<RoundImage> obtainKey(ImageConfiguration configuration) =>
      SynchronousFuture<RoundImage>(this);

  @override
  ImageStreamCompleter loadImage(RoundImage key, ImageDecoderCallback decode) =>
      OneFrameImageStreamCompleter(_load(decode));

  Future<ImageInfo> _load(ImageDecoderCallback decode) async {
    // Only the cut picture is kept in the image cache, not the whole one as well.
    final key = await source.obtainKey(ImageConfiguration.empty);
    final stream = source.loadImage(key, decode);
    final got = Completer<ui.Image>();
    final listener = ImageStreamListener(
      (info, _) {
        if (!got.isCompleted) got.complete(info.image.clone());
        info.dispose();
      },
      onError: (error, stack) {
        if (!got.isCompleted) {
          got.completeError(error, stack ?? StackTrace.empty);
        }
      },
    );
    stream.addListener(listener);
    final ui.Image whole;
    try {
      whole = await got.future;
    } finally {
      stream.removeListener(listener);
    }
    try {
      final side = math.min(whole.width, whole.height);
      final data = await whole.toByteData();
      if (data == null || side < 3) throw StateError('picture not readable');
      final pixels = _cut(
        data.buffer.asUint8List(),
        whole.width,
        (whole.width - side) ~/ 2,
        (whole.height - side) ~/ 2,
        side,
        corner,
        under: under,
      );
      return ImageInfo(image: await _image(pixels, side));
    } finally {
      whole.dispose();
    }
  }

  @override
  bool operator ==(Object other) =>
      other is RoundImage &&
      other.source == source &&
      other.corner == corner &&
      other.under == under;

  @override
  int get hashCode => Object.hash(source, corner, under);
}

/// The middle square of [side] pixels of premultiplied RGBA [rgba] (rows [width] pixels
/// long, the square from [left], [top]), over an opaque [under] if given, each pixel
/// multiplied by how much of it a square with round corners covers: the shape keeps one
/// see-through pixel all around.
Uint8List _cut(
  Uint8List rgba,
  int width,
  int left,
  int top,
  int side,
  double corner, {
  Color? under,
}) {
  if (under != null) {
    final r = (under.r * 255).round(), g = (under.g * 255).round();
    final b = (under.b * 255).round();
    for (var i = 0; i < rgba.length; i += 4) {
      final rest = 255 - rgba[i + 3];
      if (rest == 0) continue;
      rgba[i] = math.min(255, rgba[i] + (r * rest / 255).round());
      rgba[i + 1] = math.min(255, rgba[i + 1] + (g * rest / 255).round());
      rgba[i + 2] = math.min(255, rgba[i + 2] + (b * rest / 255).round());
      rgba[i + 3] = 255;
    }
  }
  final out = Uint8List(side * side * 4);
  final half = side / 2 - 1;
  final radius = (corner * side - 1).clamp(0, half).toDouble();
  final inner = half - radius;
  final centre = side / 2;
  for (var y = 0; y < side; y++) {
    final qy = (y + .5 - centre).abs() - inner;
    final from = ((top + y) * width + left) * 4;
    final to = y * side * 4;
    for (var x = 0; x < side; x++) {
      final qx = (x + .5 - centre).abs() - inner;
      // How far the pixel's centre is outside the shape (negative inside).
      final ox = math.max(qx, 0.0);
      final oy = math.max(qy, 0.0);
      final outside =
          math.sqrt(ox * ox + oy * oy) +
          math.min(math.max(qx, qy), 0.0) -
          radius;
      final cover = (.5 - outside).clamp(0.0, 1.0);
      if (cover == 0) continue;
      final i = from + x * 4;
      final o = to + x * 4;
      if (cover == 1) {
        out[o] = rgba[i];
        out[o + 1] = rgba[i + 1];
        out[o + 2] = rgba[i + 2];
        out[o + 3] = rgba[i + 3];
      } else {
        // Premultiplied: every channel scales with the cover.
        out[o] = (rgba[i] * cover).round();
        out[o + 1] = (rgba[i + 1] * cover).round();
        out[o + 2] = (rgba[i + 2] * cover).round();
        out[o + 3] = (rgba[i + 3] * cover).round();
      }
    }
  }
  return out;
}

Future<ui.Image> _image(Uint8List pixels, int side) async {
  final buffer = await ui.ImmutableBuffer.fromUint8List(pixels);
  final descriptor = ui.ImageDescriptor.raw(
    buffer,
    width: side,
    height: side,
    pixelFormat: ui.PixelFormat.rgba8888,
  );
  final codec = await descriptor.instantiateCodec();
  try {
    return (await codec.getNextFrame()).image;
  } finally {
    codec.dispose();
    descriptor.dispose();
    buffer.dispose();
  }
}

/// About twice [pixels], rounded up to a step (16, or 64 from 128 on), so that a picture
/// that changes size with the window is not made again at every size.
int twiceAsLarge(double pixels) {
  final twice = math.max(pixels * 2, 1);
  final step = twice < 128 ? 16 : 64;
  return (twice / step).ceil() * step;
}

/// Draws [image], made by [RoundImage], so that its shape fills [rect].
void paintRound(Canvas canvas, Rect rect, ui.Image image, {Paint? paint}) {
  final side = image.width.toDouble();
  // The see-through pixel around the shape falls just outside the rectangle.
  final border = rect.width / (side - 2);
  canvas.drawImageRect(
    image,
    Rect.fromLTWH(0, 0, side, side),
    rect.inflate(border),
    (paint ?? Paint())
      ..isAntiAlias = true
      ..filterQuality = FilterQuality.medium,
  );
}

/// White discs made once per size, from which every plain disc is drawn in its colour.
class Discs {
  Discs._();

  static final Map<int, ui.Image> _made = {};
  static final Set<int> _making = {};

  /// Tells painters that a disc they asked for is there now.
  static final ready = ValueNotifier<int>(0);

  /// The disc for drawing one [pixels] wide on screen, or null while it is being made.
  static ui.Image? _of(double pixels) {
    final side = twiceAsLarge(pixels).clamp(32, 1024);
    final disc = _made[side];
    if (disc != null || !_making.add(side)) return disc;
    final white = Uint8List(side * side * 4)
      ..fillRange(0, side * side * 4, 255);
    _image(_cut(white, side, 0, 0, side, .5), side).then(
      (image) {
        _made[side] = image;
        _making.remove(side);
        ready.value++;
      },
      onError: (_) {
        _making.remove(side);
      },
    );
    return null;
  }

  /// Keeps of what was drawn in the layer below only what a disc filling [rect] covers.
  static void paintMask(Canvas canvas, Rect rect, double scale) {
    final mask = Paint()..blendMode = BlendMode.dstIn;
    final disc = _of(rect.width * scale);
    if (disc == null) {
      canvas.drawOval(rect, mask);
      return;
    }
    paintRound(canvas, rect, disc, paint: mask);
  }

  /// Draws a disc of [colour] filling [rect], [scale] screen pixels per point.
  static void paint(Canvas canvas, Rect rect, Color colour, double scale) {
    final disc = _of(rect.width * scale);
    if (disc == null) {
      // Only until the disc is made: the renderer's own circle.
      canvas.drawCircle(rect.center, rect.width / 2, Paint()..color = colour);
      return;
    }
    paintRound(
      canvas,
      rect,
      disc,
      paint: Paint()..colorFilter = ColorFilter.mode(colour, BlendMode.srcIn),
    );
  }
}
