import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/call.dart';

void main() {
  final start = DateTime(2026, 10, 30, 12);
  DateTime at(int seconds) => start.add(Duration(seconds: seconds));

  /// A fit past its first 6 s, which are not judged.
  CameraFit settled() =>
      CameraFit()..take(kbps: 900, stepKbps: 900, now: at(-10));

  /// A reading every 2 s from [from] to [to] seconds of a connection that can send
  /// [kbps] for the camera, of a step of 900 kbit/s.
  bool readings(CameraFit fit, int from, int to, double kbps) {
    var changed = false;
    for (var s = from; s <= to; s += 2) {
      changed |= fit.take(kbps: kbps, stepKbps: 900, now: at(s));
    }
    return changed;
  }

  test('each share of the step\'s bitrate gets its size', () {
    expect(CameraFit.stepFor(900, 900), 0);
    expect(CameraFit.stepFor(675, 900), 0);
    expect(CameraFit.stepFor(674, 900), 1);
    expect(CameraFit.stepFor(360, 900), 1);
    expect(CameraFit.stepFor(250, 900), 2);
    expect(CameraFit.stepFor(150, 900), 3);
    // A step of 1500 kbit/s (two people) wants more for the same size.
    expect(CameraFit.stepFor(900, 1500), 1);
    expect(CameraFit.factors, [1.0, 1.5, 2.0, 3.0]);
  });

  test('the first 6 s are not judged', () {
    final fit = CameraFit();
    expect(readings(fit, 0, 4, 100), false);
    expect(fit.step, 0);
    // Then two low readings in a row.
    expect(readings(fit, 6, 6, 100), false);
    expect(fit.take(kbps: 100, stepKbps: 900, now: at(8)), true);
    expect(fit.step, 3);
    expect(fit.factor, 3.0);
  });

  test('a connection that carries the step keeps the step\'s size', () {
    final fit = settled();
    expect(readings(fit, 0, 120, 880), false);
    // One low reading alone changes nothing.
    expect(readings(fit, 122, 122, 200), false);
    expect(readings(fit, 124, 160, 700), false);
    expect(fit.step, 0);
  });

  test('two low readings make it smaller at once, by the larger they want', () {
    final fit = settled();
    expect(readings(fit, 0, 0, 250), false);
    // 250 wants half the size, 400 a size 1.5 times smaller: the larger goes.
    expect(fit.take(kbps: 400, stepKbps: 900, now: at(2)), true);
    expect(fit.step, 1);
    expect(readings(fit, 4, 6, 250), true);
    expect(fit.step, 2);
    expect(readings(fit, 8, 10, 100), true);
    expect(fit.step, 3);
    // No smaller size to go to.
    expect(readings(fit, 12, 40, 50), false);
  });

  test('a larger size comes back after 10 s that all want it', () {
    final fit = settled();
    readings(fit, 0, 2, 100);
    expect(fit.step, 3);
    // The connection carries more again; the least of the readings decides how much
    // larger.
    expect(readings(fit, 4, 6, 900), false);
    expect(readings(fit, 8, 12, 400), false);
    expect(fit.take(kbps: 900, stepKbps: 900, now: at(14)), true);
    expect(fit.step, 1);
    // A reading that wants no more than the size it has starts the count over.
    readings(fit, 16, 20, 900);
    readings(fit, 22, 22, 400);
    expect(readings(fit, 24, 32, 900), false);
    expect(fit.take(kbps: 900, stepKbps: 900, now: at(34)), true);
    expect(fit.step, 0);
  });

  test(
    'a larger size goes a step at a time when that is all the readings want',
    () {
      final fit = settled();
      readings(fit, 0, 2, 100);
      expect(fit.step, 3);
      expect(readings(fit, 4, 14, 250), true);
      expect(fit.step, 2);
    },
  );

  test('a larger size that does not last waits twice as long, up to 80 s', () {
    final fit = settled();
    readings(fit, 0, 2, 250);
    expect(fit.step, 2);
    readings(fit, 4, 14, 400);
    expect(fit.step, 1);
    var t = 16;
    for (final wait in [20, 40, 80, 80]) {
      // Falls short again within 20 s.
      readings(fit, t, t + 2, 250);
      expect(fit.step, 2);
      expect(fit.wait, Duration(seconds: wait));
      t += 4;
      expect(readings(fit, t, t + wait - 2, 400), false);
      expect(fit.take(kbps: 400, stepKbps: 900, now: at(t + wait)), true);
      expect(fit.step, 1);
      t += wait + 2;
    }
    // One that lasts keeps the wait it had for the next time.
    readings(fit, t, t + 60, 400);
    expect(fit.step, 1);
    expect(fit.wait, const Duration(seconds: 80));
  });

  test(
    'starting over brings the step\'s size back and judges nothing at first',
    () {
      final fit = settled();
      readings(fit, 0, 2, 100);
      expect(fit.step, 3);
      fit.reset();
      expect(fit.step, 0);
      expect(fit.factor, 1.0);
      expect(fit.wait, const Duration(seconds: 10));
      expect(readings(fit, 100, 104, 100), false);
      expect(fit.step, 0);
    },
  );

  test('a reading without a rate is not judged', () {
    final fit = settled();
    readings(fit, 0, 0, 100);
    expect(fit.take(kbps: 0, stepKbps: 900, now: at(2)), false);
    expect(fit.take(kbps: 100, stepKbps: 900, now: at(4)), true);
  });
}
