import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/icons.dart';

import 'fake_room.dart';

/// Where the drawing of [icon] sits in its box, drawn [size] pixels wide: the middle of
/// what it covers, from the box's own middle.
Future<Offset> offCentre(WidgetTester tester, Ic icon, double size) async {
  final key = GlobalKey();
  await tester.pumpWidget(
    Directionality(
      textDirection: TextDirection.ltr,
      child: Center(
        child: RepaintBoundary(
          key: key,
          child: FgIcon(icon, size: size, color: const Color(0xFFFFFFFF)),
        ),
      ),
    ),
  );
  final boundary =
      key.currentContext!.findRenderObject()! as RenderRepaintBoundary;
  final pixels = (await tester.runAsync(() async {
    final image = await boundary.toImage();
    return image.toByteData(format: ui.ImageByteFormat.rawRgba);
  }))!.buffer.asUint8List();
  final side = size.round();
  var (left, top, right, bottom) = (side, side, -1, -1);
  for (var y = 0; y < side; y++) {
    for (var x = 0; x < side; x++) {
      if (pixels[(y * side + x) * 4 + 3] < 128) continue;
      if (x < left) left = x;
      if (x > right) right = x;
      if (y < top) top = y;
      if (y > bottom) bottom = y;
    }
  }
  return Offset((left + right + 1) / 2, (top + bottom + 1) / 2) -
      Offset(side / 2, side / 2);
}

void main() {
  setUpAll(loadFonts);

  testWidgets('the pins are drawn in the middle of their box', (tester) async {
    for (final icon in [Ic.pin, Ic.unpin]) {
      final off = await offCentre(tester, icon, 64);
      // Within a pixel at 64: the font draws the pin two pixels up and right of it.
      expect(off.dx.abs(), lessThanOrEqualTo(1), reason: '$icon: $off');
      expect(off.dy.abs(), lessThanOrEqualTo(1), reason: '$icon: $off');
    }
    // An icon drawn in the middle by the font stays where it is.
    final plus = await offCentre(tester, Ic.plus, 64);
    expect(plus.distance, lessThanOrEqualTo(1));
  });
}
