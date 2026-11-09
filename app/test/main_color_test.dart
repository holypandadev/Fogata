import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/main_color.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/theme.dart';

/// [count] pixels of one colour, as RGBA.
List<int> _pixels(int count, int r, int g, int b, [int a = 255]) => [
  for (var i = 0; i < count; i++) ...[r, g, b, a],
];

/// A PNG of [side] by [side] pixels: grey, with a red square in its top left quarter.
Future<Uint8List> _photo(int side) async {
  final recorder = ui.PictureRecorder();
  final canvas = ui.Canvas(recorder);
  final s = side.toDouble();
  canvas.drawRect(
    ui.Rect.fromLTWH(0, 0, s, s),
    ui.Paint()..color = const ui.Color(0xFF808080),
  );
  canvas.drawRect(
    ui.Rect.fromLTWH(0, 0, s / 2, s / 2),
    ui.Paint()..color = const ui.Color(0xFFE02020),
  );
  final image = await recorder.endRecording().toImage(side, side);
  final png = await image.toByteData(format: ui.ImageByteFormat.png);
  image.dispose();
  return png!.buffer.asUint8List();
}

void main() {
  test('the colour that covers most of a picture is its main colour', () {
    final rgba = Uint8List.fromList([
      ..._pixels(70, 20, 60, 200),
      ..._pixels(30, 200, 200, 30),
    ]);
    final colour = mainColourOf(rgba)!;
    expect(colour, const Color(0xFF143CC8));
  });

  test('a vivid colour counts more than grey', () {
    // A third of the picture red, the rest grey: the red wins.
    final rgba = Uint8List.fromList([
      ..._pixels(60, 128, 128, 128),
      ..._pixels(30, 224, 32, 32),
    ]);
    expect(mainColourOf(rgba), const Color(0xFFE02020));
  });

  test(
    'see-through pixels do not count, and a picture of only them has none',
    () {
      final rgba = Uint8List.fromList([
        ..._pixels(90, 255, 0, 0, 0),
        ..._pixels(10, 0, 160, 0),
      ]);
      expect(mainColourOf(rgba), const Color(0xFF00A000));
      expect(mainColourOf(Uint8List.fromList(_pixels(20, 9, 9, 9, 0))), isNull);
      expect(mainColourOf(Uint8List(0)), isNull);
    },
  );

  testWidgets('a photo is read for its main colour once', (tester) async {
    final photo = (await tester.runAsync(() => _photo(64)))!;
    final colour = await tester.runAsync(() => mainColour(photo));
    expect(colour, isNotNull);
    expect(colour!.r, greaterThan(.7));
    expect(colour.g, lessThan(.3));
    // Kept: asked again, it is the same.
    expect(await tester.runAsync(() => mainColour(photo)), colour);
  });

  testWidgets('a preset avatar has its own colour at once', (tester) async {
    Color? seen;
    await tester.pumpWidget(
      MainColour(
        avatar: const UiAvatar(preset: 0),
        builder: (context, colour) {
          seen = colour;
          return const SizedBox();
        },
      ),
    );
    expect(seen, avatarPresets[0].color);
  });
}
