import 'dart:io';
import 'dart:math' as math;
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/jpeg.dart';
import 'package:fogata/src/photo.dart';

int _crc(List<int> bytes) {
  var crc = 0xffffffff;
  for (final b in bytes) {
    crc ^= b;
    for (var i = 0; i < 8; i++) {
      crc = (crc & 1) != 0 ? (crc >> 1) ^ 0xedb88320 : crc >> 1;
    }
  }
  return crc ^ 0xffffffff;
}

List<int> _chunk(String type, List<int> data) {
  final body = [...type.codeUnits, ...data];
  final length = ByteData(4)..setUint32(0, data.length);
  final crc = ByteData(4)..setUint32(0, _crc(body));
  return [...length.buffer.asUint8List(), ...body, ...crc.buffer.asUint8List()];
}

/// A PNG whose header says [side] by [side] pixels: a few kilobytes that would ask for
/// gigabytes once decoded.
Uint8List _huge(int side) {
  final header = ByteData(13)
    ..setUint32(0, side)
    ..setUint32(4, side)
    ..setUint8(8, 8)
    ..setUint8(9, 6);
  return Uint8List.fromList([
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, //
    ..._chunk('IHDR', header.buffer.asUint8List()),
    ..._chunk('IDAT', ZLibCodec().encode(Uint8List(64 * 1024))),
    ..._chunk('IEND', const []),
  ]);
}

Future<String?> _problem(Future<Object?> Function() work) async {
  try {
    await work();
    return null;
  } on PictureProblem catch (e) {
    return e.message;
  }
}

/// A made-up photo: smooth shapes and a little noise, `alpha` everywhere.
Uint8List photoLike(int side, {int alpha = 255, double noise = 24}) {
  final random = math.Random(7);
  final pixels = Uint8List(side * side * 4);
  for (var y = 0; y < side; y++) {
    for (var x = 0; x < side; x++) {
      final i = (y * side + x) * 4;
      final face =
          math.sqrt(math.pow(x - side / 2, 2) + math.pow(y - side / 2.2, 2)) <
          side * .28;
      double n() => (random.nextDouble() - .5) * noise;
      pixels[i] = ((face ? 220 : 60 + 120 * y / side) + n())
          .clamp(0, 255)
          .toInt();
      pixels[i + 1] = ((face ? 170 : 90 + 60 * x / side) + n())
          .clamp(0, 255)
          .toInt();
      pixels[i + 2] = ((face ? 140 : 140 - 40 * y / side) + n())
          .clamp(0, 255)
          .toInt();
      pixels[i + 3] = alpha;
    }
  }
  return pixels;
}

Future<ui.Image> decode(Uint8List bytes) async =>
    (await (await ui.instantiateImageCodec(bytes)).getNextFrame()).image;

Future<Uint8List> pixelsOf(ui.Image image) async =>
    (await image.toByteData(format: ui.ImageByteFormat.rawRgba))!.buffer
        .asUint8List();

Future<Uint8List> png(Uint8List rgba, int side) async {
  final buffer = await ui.ImmutableBuffer.fromUint8List(rgba);
  final descriptor = ui.ImageDescriptor.raw(
    buffer,
    width: side,
    height: side,
    pixelFormat: ui.PixelFormat.rgba8888,
  );
  final image =
      (await (await descriptor.instantiateCodec()).getNextFrame()).image;
  return (await image.toByteData(format: ui.ImageByteFormat.png))!.buffer
      .asUint8List();
}

void main() {
  test('a JPEG made here decodes to nearly the same picture', () async {
    // A size that is not a multiple of 8 checks the edges too.
    const width = 60;
    const height = 44;
    final source = Uint8List(width * height * 4);
    for (var i = 0; i < width * height; i++) {
      final x = i % width;
      final y = i ~/ width;
      source[i * 4] = (x * 4) % 256;
      source[i * 4 + 1] = (y * 5) % 256;
      source[i * 4 + 2] = ((x + y) * 3) % 256;
      source[i * 4 + 3] = 255;
    }
    final jpeg = encodeJpeg(source, width, height, quality: 92);
    expect(jpeg.sublist(0, 2), [0xFF, 0xD8]);
    final image = await decode(jpeg);
    expect((image.width, image.height), (width, height));
    final back = await pixelsOf(image);
    var error = 0;
    for (var i = 0; i < source.length; i++) {
      if (i % 4 != 3) error += (back[i] - source[i]).abs();
    }
    expect(error / (width * height * 3), lessThan(4));
  });

  test('a photo becomes a 256-pixel JPEG within the limit', () async {
    final picture = await squarePicture(await png(photoLike(900), 900));
    expect(picture, isNotNull);
    expect(picture!.sublist(0, 2), [0xFF, 0xD8]);
    expect(picture.length, lessThanOrEqualTo(maxPhotoBytes));
    expect((await decode(picture)).width, 256);
    final group = await squarePicture(
      await png(photoLike(900), 900),
      maxBytes: maxGroupPictureBytes,
    );
    expect(group!.length, lessThanOrEqualTo(maxGroupPictureBytes));
  });

  test('a picture with see-through parts stays a PNG', () async {
    final picture = await squarePicture(
      await png(photoLike(300, alpha: 120), 300),
    );
    expect(picture!.sublist(1, 4), 'PNG'.codeUnits);
    expect(picture.length, lessThanOrEqualTo(maxPhotoBytes));
  });

  test(
    'a picture of too many pixels is refused before it is decoded',
    () async {
      // 200 megapixels: about 800 MB once decoded.
      final huge = _huge(14200);
      for (final make in <Future<Object?> Function(Uint8List)>[
        squarePicture,
        bannerPng,
        groupBannerPicture,
      ]) {
        expect(
          await _problem(() => make(huge)),
          'That picture is too large: at most 16 megapixels.',
        );
      }
      expect(
        await _problem(() => squarePicture(Uint8List.fromList([1, 2, 3]))),
        'That picture could not be read.',
      );
    },
  );

  test('a large picture makes the same small ones', () async {
    // Wider than tall, so the square, the banner and the group's banner each cut it.
    const width = 1800;
    const height = 900;
    final rgba = Uint8List(width * height * 4)
      ..fillRange(0, width * height * 4, 200);
    for (var i = 3; i < rgba.length; i += 4) {
      rgba[i] = 255;
    }
    final buffer = await ui.ImmutableBuffer.fromUint8List(rgba);
    final descriptor = ui.ImageDescriptor.raw(
      buffer,
      width: width,
      height: height,
      pixelFormat: ui.PixelFormat.rgba8888,
    );
    final image =
        (await (await descriptor.instantiateCodec()).getNextFrame()).image;
    final source = (await image.toByteData(format: ui.ImageByteFormat.png))!
        .buffer
        .asUint8List();
    final square = await decode((await squarePicture(source))!);
    expect((square.width, square.height), (256, 256));
    final banner = await decode((await bannerPng(source))!);
    expect((banner.width, banner.height), (900, 300));
    final group = await decode((await groupBannerPicture(source))!);
    expect((group.width, group.height), (960, 540));
  });

  test(
    'every picture made on the way is let go of, also when it fails',
    () async {
      // Made before counting: what the test makes is not counted.
      final photo = await png(photoLike(400), 400);
      final clear = await png(photoLike(300, alpha: 120), 300);
      final huge = _huge(30000);
      final broken = Uint8List.fromList([...photo.take(200)]);
      var images = 0, pictures = 0, made = 0;
      ui.Image.onDispose = (_) => images--;
      ui.Picture.onCreate = (_) => pictures++;
      ui.Picture.onDispose = (_) => pictures--;
      addTearDown(() {
        ui.Image.onCreate = null;
        ui.Image.onDispose = null;
        ui.Picture.onCreate = null;
        ui.Picture.onDispose = null;
      });
      ui.Image.onCreate = (_) {
        images++;
        made++;
      };
      expect(await squarePicture(photo), isNotNull);
      expect(await squarePicture(clear), isNotNull);
      // Nothing fits in a byte: every size is made and let go of.
      expect(await squarePicture(clear, maxBytes: 1), isNull);
      expect(await bannerPng(photo), isNotNull);
      expect(await _problem(() => squarePicture(huge)), isNotNull);
      expect(await _problem(() => squarePicture(broken)), isNotNull);
      expect(made, greaterThan(5));
      expect(images, 0, reason: 'pictures still held');
      expect(pictures, 0, reason: 'drawings still held');
    },
  );
}
