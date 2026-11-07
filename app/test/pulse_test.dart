import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/shown.dart';
import 'package:fogata/src/widgets.dart';

void main() {
  testWidgets('pulses share one clock that stops while the window is hidden', (
    tester,
  ) async {
    final phases = <double>[];
    Widget pulse() => SlowPulse(
      period: const Duration(seconds: 1),
      builder: (context, phase) {
        phases.add(phase);
        return const SizedBox(width: 4, height: 4);
      },
    );
    await tester.pumpWidget(
      Row(textDirection: TextDirection.ltr, children: [pulse(), pulse()]),
    );
    phases.clear();
    await tester.pump(slowFrame);
    // Both stepped once, together.
    expect(phases.length, 2);
    expect(phases.first, phases.last);

    windowShown.value = false;
    await tester.pump(slowFrame);
    phases.clear();
    await tester.pump(const Duration(seconds: 1));
    expect(phases, isEmpty);

    windowShown.value = true;
    await tester.pump(slowFrame);
    expect(phases.length, 2);
    await tester.pumpWidget(const SizedBox());
  });
}
