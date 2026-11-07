import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/widgets.dart';

Future<ScrollPosition> _page(WidgetTester tester) async {
  tester.view.physicalSize = const Size(800, 600);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    const MaterialApp(
      home: Material(
        child: PageScroll(child: SizedBox(height: 3000, width: 800)),
      ),
    ),
  );
  return tester.state<ScrollableState>(find.byType(Scrollable)).position;
}

Future<void> _wheel(WidgetTester tester, double dy) =>
    tester.sendEventToBinding(
      PointerScrollEvent(
        position: const Offset(400, 300),
        scrollDelta: Offset(0, dy),
        kind: ui.PointerDeviceKind.mouse,
      ),
    );

void main() {
  testWidgets('a notch of the wheel glides to its place', (tester) async {
    final position = await _page(tester);
    await _wheel(tester, 100);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));
    // On its way, not there at once.
    expect(position.pixels, greaterThan(0));
    expect(position.pixels, lessThan(100));
    await tester.pump(wheelGlide);
    expect(position.pixels, 100);
  });

  testWidgets('notches in quick succession add up', (tester) async {
    final position = await _page(tester);
    for (var i = 0; i < 3; i++) {
      await _wheel(tester, 100);
      await tester.pump(const Duration(milliseconds: 40));
    }
    await tester.pump(wheelGlide);
    await tester.pump(const Duration(milliseconds: 50));
    expect(position.pixels, 300);
    // And back up, stopping at the top.
    for (var i = 0; i < 5; i++) {
      await _wheel(tester, -100);
      await tester.pump(const Duration(milliseconds: 40));
    }
    await tester.pump(wheelGlide);
    await tester.pump(const Duration(milliseconds: 50));
    expect(position.pixels, 0);
  });

  testWidgets('it stops at the end', (tester) async {
    final position = await _page(tester);
    for (var i = 0; i < 40; i++) {
      await _wheel(tester, 100);
      await tester.pump(const Duration(milliseconds: 30));
    }
    await tester.pump(wheelGlide);
    await tester.pump(const Duration(milliseconds: 50));
    expect(position.pixels, position.maxScrollExtent);
  });

  testWidgets('small steps follow the hand at once', (tester) async {
    final position = await _page(tester);
    await _wheel(tester, 12);
    await tester.pump();
    expect(position.pixels, 12);
  });
}
