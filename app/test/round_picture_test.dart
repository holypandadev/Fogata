// A round picture lets go of the picture it drew once it draws another or goes.

import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/widgets.dart';

import 'wait.dart';

/// A PNG of one colour, [side] pixels wide.
Future<Uint8List> _png(int side, Color colour) async {
  final recorder = ui.PictureRecorder();
  Canvas(recorder).drawRect(
    Rect.fromLTWH(0, 0, side.toDouble(), side.toDouble()),
    Paint()..color = colour,
  );
  final image = await recorder.endRecording().toImage(side, side);
  final data = await image.toByteData(format: ui.ImageByteFormat.png);
  image.dispose();
  return data!.buffer.asUint8List();
}

void main() {
  testWidgets('a round picture lets go of what it drew', (tester) async {
    final pictures = <MemoryImage>[];
    await tester.runAsync(() async {
      for (final colour in [Colors.red, Colors.blue]) {
        pictures.add(MemoryImage(await _png(32, colour)));
      }
    });
    Widget round(ImageProvider source) => Directionality(
      textDirection: TextDirection.ltr,
      child: Center(child: RoundPicture(source: source, size: 32)),
    );

    /// The picture drawn now, if any.
    ui.Image? drawn() =>
        (tester
                        .widget<CustomPaint>(
                          find.descendant(
                            of: find.byType(RoundPicture),
                            matching: find.byType(CustomPaint),
                          ),
                        )
                        .painter
                    as dynamic)
                .image
            as ui.Image?;

    // Each listener of a picture is handed a handle of its own, the cache keeping
    // another: the one drawn is the widget's to let go of.
    final held = <ui.Image>[];
    for (final source in pictures) {
      await tester.pumpWidget(round(source));
      final before = held.lastOrNull;
      await pumpUntil(
        tester,
        () => drawn() != null && drawn() != before,
        what: 'picture ${held.length + 1} drawn',
      );
      held.add(drawn()!);
    }
    // The first, drawn no more.
    expect(held.first.debugDisposed, isTrue);
    expect(held.last.debugDisposed, isFalse);
    await tester.pumpWidget(const SizedBox());
    expect(held.last.debugDisposed, isTrue);
  });
}
