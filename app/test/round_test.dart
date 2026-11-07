import 'dart:async';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/round.dart';

import 'wait.dart';

/// A square picture of one colour, [side] pixels wide.
Future<ui.Image> plain(int side, Color colour) {
  final recorder = ui.PictureRecorder();
  Canvas(recorder).drawRect(
    Rect.fromLTWH(0, 0, side.toDouble(), side.toDouble()),
    Paint()..color = colour,
  );
  return recorder.endRecording().toImage(side, side);
}

/// The alpha of every pixel of [image], row by row.
Future<List<int>> alphas(ui.Image image) async {
  final data = (await image.toByteData())!.buffer.asUint8List();
  return [for (var i = 3; i < data.length; i += 4) data[i]];
}

class _Picture extends ImageProvider<_Picture> {
  _Picture(this.image);
  final ui.Image image;

  @override
  Future<_Picture> obtainKey(ImageConfiguration configuration) async => this;

  @override
  ImageStreamCompleter loadImage(_Picture key, ImageDecoderCallback decode) =>
      OneFrameImageStreamCompleter(
        Future.value(ImageInfo(image: image.clone())),
      );
}

Future<ui.Image> load(ImageProvider provider) async {
  final stream = provider.resolve(ImageConfiguration.empty);
  final image = Completer<ui.Image>();
  stream.addListener(
    ImageStreamListener(
      (info, _) => image.complete(info.image),
      onError: (e, s) => image.completeError(e, s),
    ),
  );
  return image.future;
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('a round picture has a smooth edge in its own pixels', () async {
    const side = 64;
    final round = await load(
      RoundImage(_Picture(await plain(side, Colors.white))),
    );
    expect(round.width, side);
    expect(round.height, side);
    final a = await alphas(round);
    int at(int x, int y) => a[y * side + x];
    // Solid in the middle, nothing in the corners nor on the border pixel.
    expect(at(32, 32), 255);
    expect(at(0, 0), 0);
    expect(at(2, 2), 0);
    for (var i = 0; i < side; i++) {
      expect(at(i, 0), 0, reason: 'top border at $i');
      expect(at(0, i), 0, reason: 'left border at $i');
    }
    // Along the edge, pixels in between: partly covered, not steps of all or nothing.
    final edge = [
      for (var y = 0; y < side; y++)
        for (var x = 0; x < side; x++)
          if (at(x, y) > 0 && at(x, y) < 255) at(x, y),
    ];
    expect(edge.length, greaterThan(side * 2));
    expect(edge.toSet().length, greaterThan(10));
  });

  test(
    'a long picture is cut in its middle, and corners can be rounded only',
    () async {
      final recorder = ui.PictureRecorder();
      final canvas = Canvas(recorder);
      canvas.drawRect(
        const Rect.fromLTWH(0, 0, 32, 64),
        Paint()..color = Colors.red,
      );
      canvas.drawRect(
        const Rect.fromLTWH(32, 0, 64, 64),
        Paint()..color = Colors.white,
      );
      canvas.drawRect(
        const Rect.fromLTWH(96, 0, 32, 64),
        Paint()..color = Colors.red,
      );
      final long = await recorder.endRecording().toImage(128, 64);
      final cut = await load(RoundImage(_Picture(long), corner: .1));
      expect(cut.width, 64);
      final data = (await cut.toByteData())!.buffer.asUint8List();
      // White only: the red ends are cut away.
      for (var i = 0; i < data.length; i += 4) {
        expect(data[i + 1], data[i], reason: 'pixel ${i ~/ 4}');
      }
      final a = await alphas(cut);
      // A small round corner: the middle of each side is solid up to the border pixel.
      expect(a[32 * 64 + 1], 255);
      expect(a[1 * 64 + 32], 255);
      expect(a[1 * 64 + 1], lessThan(255));
    },
  );

  test('pictures are made about twice as large as drawn', () {
    expect(twiceAsLarge(18), 48);
    expect(twiceAsLarge(24), 48);
    expect(twiceAsLarge(88), 192);
    expect(twiceAsLarge(208), 448);
  });

  testWidgets('discs are made once per size and told when ready', (
    tester,
  ) async {
    var told = 0;
    void listener() => told++;
    Discs.ready.addListener(listener);
    addTearDown(() => Discs.ready.removeListener(listener));
    await tester.runAsync(() async {
      final recorder = ui.PictureRecorder();
      // First use: the renderer's own circle while the disc is made.
      Discs.paint(
        Canvas(recorder),
        const Rect.fromLTWH(0, 0, 20, 20),
        Colors.green,
        1,
      );
      await until(() => told > 0, what: 'the disc made');
      recorder.endRecording();
    });
    expect(told, 1);
  });
}
