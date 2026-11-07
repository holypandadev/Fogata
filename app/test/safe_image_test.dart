import 'dart:async';
import 'dart:io';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/painting.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/safe_image.dart';

import 'wait.dart';

/// CRC-32 as PNG chunks use it.
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

/// A PNG whose header says [side] by [side] pixels, with a deflate stream of zeros: a few
/// kilobytes that would ask for gigabytes once decoded.
Uint8List _bomb(int side) {
  final header = ByteData(13)
    ..setUint32(0, side)
    ..setUint32(4, side)
    ..setUint8(8, 8)
    ..setUint8(9, 6);
  final rows = ZLibCodec().encode(Uint8List(64 * 1024));
  return Uint8List.fromList([
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, //
    ..._chunk('IHDR', header.buffer.asUint8List()),
    ..._chunk('IDAT', rows),
    ..._chunk('IEND', const []),
  ]);
}

/// A GIF [side] pixels square with [frames] frames of one pixel each, shown one after the
/// other with no wait: tens of bytes that ask for a full-size frame for each.
Uint8List _animation(int side, int frames) {
  final size = ByteData(4)
    ..setUint16(0, side, Endian.little)
    ..setUint16(2, side, Endian.little);
  return Uint8List.fromList([
    ...'GIF89a'.codeUnits,
    ...size.buffer.asUint8List(),
    0x80, 0, 0, // a global table of two colours
    0, 0, 0, 0xff, 0xff, 0xff,
    for (var i = 0; i < frames; i++) ...[
      0x21, 0xf9, 4, 0, 0, 0, 0, 0, // no wait before the next frame
      0x2c, 0, 0, 0, 0, 1, 0, 1, 0, 0, // one pixel at the top left
      2, 2, 0x44, 0x01, 0, // clear, colour 0, end
    ],
    0x3b,
  ]);
}

List<int> _u32(int value) =>
    (ByteData(4)..setUint32(0, value)).buffer.asUint8List();

/// An animated PNG [side] pixels square with [frames] frames shown one after the other with
/// no wait. Flutter plays these too, though the app never treats a PNG as an animation.
Uint8List _apng(int side, int frames) {
  final header = ByteData(13)
    ..setUint32(0, side)
    ..setUint32(4, side)
    ..setUint8(8, 8)
    ..setUint8(9, 6);
  final rows = ZLibCodec().encode(Uint8List(side * (side * 4 + 1)));
  var sequence = 0;
  List<int> control() => _chunk('fcTL', [
    ..._u32(sequence++),
    ..._u32(side),
    ..._u32(side),
    ..._u32(0),
    ..._u32(0),
    0, 0, 0, 100, // no wait before the next frame
    0, 0,
  ]);
  return Uint8List.fromList([
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, //
    ..._chunk('IHDR', header.buffer.asUint8List()),
    ..._chunk('acTL', [..._u32(frames), ..._u32(0)]),
    ...control(),
    ..._chunk('IDAT', rows),
    for (var i = 1; i < frames; i++) ...[
      ...control(),
      ..._chunk('fdAT', [..._u32(sequence++), ...rows]),
    ],
    ..._chunk('IEND', const []),
  ]);
}

Future<Uint8List> _small() async {
  final recorder = ui.PictureRecorder();
  ui.Canvas(recorder).drawRect(
    const ui.Rect.fromLTWH(0, 0, 64, 64),
    ui.Paint()..color = const ui.Color(0xff5a1e9e),
  );
  final image = await recorder.endRecording().toImage(64, 64);
  final data = await image.toByteData(format: ui.ImageByteFormat.png);
  return data!.buffer.asUint8List();
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('a small picture is decoded', () async {
    final bytes = await _small();
    expect(await fitsToDecode(bytes, maxSmallPicturePixels), isTrue);
  });

  test('a picture that asks for too many pixels is never decoded', () async {
    final bomb = _bomb(30000);
    expect(bomb.length, lessThan(16 * 1024));
    expect(await fitsToDecode(bomb, maxPicturePixels), isFalse);
  });

  test('only a small animation is decoded', () async {
    // 9 megapixels: a still picture this large is decoded, an animation is not.
    expect(await fitsToDecode(_animation(3000, 1), maxPicturePixels), isTrue);
    expect(await fitsToDecode(_animation(3000, 2), maxPicturePixels), isFalse);
    expect(await fitsToDecode(_animation(2000, 2), maxPicturePixels), isTrue);
  });

  test('damaged bytes are refused', () async {
    expect(await fitsToDecode(Uint8List.fromList([1, 2, 3]), 100), isFalse);
  });

  test('the same picture in a new list is the same image', () {
    final bytes = Uint8List.fromList(List.generate(20000, (i) => i % 251));
    final again = Uint8List.fromList(bytes);
    final other = Uint8List.fromList(bytes)..[19999] ^= 1;
    expect(samePicture(bytes, again), isTrue);
    expect(samePicture(bytes, other), isFalse);
    expect(SafeMemoryImage(bytes), SafeMemoryImage(again));
    expect(SafeMemoryImage(bytes).hashCode, SafeMemoryImage(again).hashCode);
    expect(SafeMemoryImage(bytes), isNot(SafeMemoryImage(other)));
  });

  test('a picture decoded whole is held to fewer pixels than a photo', () async {
    // 25 megapixels of PNG: a JPEG may have that many, a PNG is decoded whole.
    expect(await fitsToDecode(_bomb(5000), maxPicturePixels), isFalse);
    expect(decodedWhole(_bomb(10)), isTrue);
    expect(
      decodedWhole(Uint8List.fromList([0xff, 0xd8, 0xff, 0xe0, 0, 0])),
      isFalse,
    );
    final webp = Uint8List.fromList([
      ...'RIFF'.codeUnits,
      0,
      0,
      0,
      0,
      ...'WEBPVP8 '.codeUnits,
    ]);
    expect(decodedWhole(webp), isFalse);
  });

  test('a picture is checked and decoded from one copy of its bytes', () async {
    final decoded = <ui.ImmutableBuffer>[];
    Future<ui.Codec> decode(
      ui.ImmutableBuffer buffer, {
      ui.TargetImageSizeCallback? getTargetSize,
    }) {
      decoded.add(buffer);
      return ui.instantiateImageCodecWithSize(buffer);
    }

    Future<Object> load(SafeMemoryImage image) {
      final done = Completer<Object>();
      image
          .loadImage(image, decode)
          .addListener(
            ImageStreamListener(
              (info, _) => done.complete(info.image),
              onError: (e, _) => done.complete(e),
            ),
          );
      return done.future;
    }

    final shown = await load(SafeMemoryImage(await _small()));
    expect(shown, isA<ui.Image>());
    expect((shown as ui.Image).width, 64);
    expect(decoded, hasLength(1));
    // Too large: never handed over to be decoded.
    expect(await load(SafeMemoryImage(_bomb(30000))), isA<StateError>());
    expect(decoded, hasLength(1));
  });

  testWidgets('a picture of several frames shows only its first', (
    tester,
  ) async {
    // 2000 by 2000 with 4000 frames of one pixel: each frame would be decoded whole.
    final pictures = {'gif': _animation(2000, 4000), 'apng': _apng(16, 5)};
    for (final MapEntry(key: kind, value: bytes) in pictures.entries) {
      PaintingBinding.instance.imageCache.clear();
      var shown = 0;
      final stream = SafeMemoryImage(
        bytes,
        maxPixels: maxPicturePixels,
      ).resolve(ImageConfiguration.empty);
      final listener = ImageStreamListener((info, _) {
        shown++;
        info.dispose();
      });
      stream.addListener(listener);
      await pumpUntil(
        tester,
        () => shown > 0,
        what: 'the first frame of $kind',
      );
      // Any more frames would come within this, real time and the test's both passing.
      for (var i = 0; i < 30; i++) {
        await tester.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 20)),
        );
        await tester.pump(const Duration(milliseconds: 20));
      }
      stream.removeListener(listener);
      expect(shown, 1, reason: kind);
    }
  });
}
