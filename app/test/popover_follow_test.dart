import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/popover.dart';
import 'package:fogata/src/theme.dart';

void main() {
  testWidgets('a popover moves with its button when the window changes size', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(800, 600);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(
      MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: Material(
          child: Align(
            alignment: Alignment.bottomCenter,
            child: Padding(
              padding: const EdgeInsets.only(bottom: 20),
              child: Builder(
                builder: (anchor) => TextButton(
                  onPressed: () => showPopover<void>(
                    anchor,
                    label: 'Panel',
                    builder: (_) => const SizedBox(
                      key: ValueKey('panel'),
                      width: 200,
                      height: 120,
                    ),
                  ),
                  child: const Text('Open'),
                ),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();
    Rect button() => tester.getRect(find.byType(TextButton));
    Rect panel() => tester.getRect(find.byKey(const ValueKey('panel')));
    expect(panel().left, moreOrLessEquals(button().left));
    expect(panel().bottom, moreOrLessEquals(button().top - 8));

    for (final size in const [Size(1400, 900), Size(500, 400)]) {
      tester.view.physicalSize = size;
      await tester.pumpAndSettle();
      expect(panel().left, moreOrLessEquals(button().left), reason: '$size');
      expect(
        panel().bottom,
        moreOrLessEquals(button().top - 8),
        reason: '$size',
      );
    }
  });
}
