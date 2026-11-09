import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/call.dart';

void main() {
  final start = DateTime(2026, 10, 25, 12);
  DateTime at(int seconds) => start.add(Duration(seconds: seconds));

  /// A fit past its first 10 s, which are not judged.
  ScreenFit settled() =>
      ScreenFit()..take(fps: 0, wanted: 30, steps: 3, now: at(-10));

  /// A reading every 2 s from [from] to [to] seconds at [fps] of 30 captured.
  bool readings(ScreenFit fit, int from, int to, double fps, {int steps = 3}) {
    var changed = false;
    for (var s = from; s <= to; s += 2) {
      changed |= fit.take(fps: fps, wanted: 30, steps: steps, now: at(s));
    }
    return changed;
  }

  test('the sizes go down from the screen\'s own in readable steps', () {
    expect(ScreenFit.sizes(900), [900, 720, 540]);
    expect(ScreenFit.sizes(1080), [1080, 720, 540]);
    expect(ScreenFit.sizes(1440), [1440, 1080, 720, 540]);
    expect(ScreenFit.sizes(720), [720, 540]);
    expect(ScreenFit.sizes(480), [480]);
  });

  test('a screen keeps its size while it moves well enough', () {
    final fit = settled();
    expect(readings(fit, 0, 60, 26), false);
    // Choppy for a moment only.
    expect(readings(fit, 62, 64, 3), false);
    expect(readings(fit, 66, 80, 12), false);
    expect(fit.step, 0);
  });

  test('a choppy screen steps down, one step every 6 s, to the last', () {
    final fit = settled();
    expect(readings(fit, 0, 2, 2), false);
    expect(fit.take(fps: 2, wanted: 30, steps: 3, now: at(4)), true);
    expect(fit.step, 1);
    readings(fit, 6, 10, 4);
    expect(fit.step, 2);
    // No smaller size to go to.
    readings(fit, 12, 40, 4);
    expect(fit.step, 2);
  });

  test('a smaller size that moves smoothly tries the one above', () {
    final fit = settled();
    readings(fit, 0, 4, 2);
    expect(fit.step, 1);
    readings(fit, 6, 10, 26);
    // A dip starts the count over.
    readings(fit, 12, 12, 15);
    expect(readings(fit, 14, 22, 26), false);
    expect(fit.take(fps: 26, wanted: 30, steps: 3, now: at(24)), true);
    expect(fit.step, 0);
  });

  test('a size above that does not last is tried again later each time', () {
    final fit = settled();
    readings(fit, 0, 4, 2);
    readings(fit, 6, 16, 28);
    expect(fit.step, 0);
    // Choppy again right away: back down, and twice the wait.
    readings(fit, 18, 22, 3);
    expect(fit.step, 1);
    expect(fit.wait, const Duration(seconds: 20));
    readings(fit, 24, 42, 28);
    expect(fit.step, 1);
    readings(fit, 44, 44, 28);
    expect(fit.step, 0);
    fit.reset();
    expect((fit.step, fit.wait), (0, const Duration(seconds: 10)));
  });

  test('the first 10 s of a share are not judged', () {
    final fit = ScreenFit();
    expect(readings(fit, 0, 8, 0), false);
    expect(fit.step, 0);
    // From then on, 6 s of choppy readings step it down.
    expect(readings(fit, 10, 12, 1), false);
    expect(readings(fit, 14, 14, 1), true);
    fit.reset();
    expect(readings(fit, 20, 28, 0), false);
    expect(fit.step, 0);
  });

  test('a screen captured slowly is judged against what is captured', () {
    final fit = settled();
    // A still window on macOS gives a frame now and then: all there is to send.
    for (var s = 0; s <= 30; s += 2) {
      fit.take(fps: 1, wanted: 1.2, steps: 3, now: at(s));
    }
    expect(fit.step, 0);
  });

  test(
    'a screen chosen at 60 frames a second steps down when well short of it',
    () {
      final fit = ScreenFit()..take(fps: 0, wanted: 60, steps: 3, now: at(-10));
      // 50 of 60 is smooth enough; 40 is not, though far over 10.
      for (var s = 0; s <= 20; s += 2) {
        fit.take(fps: 50, wanted: 60, steps: 3, now: at(s));
      }
      expect(fit.step, 0);
      for (var s = 22; s <= 26; s += 2) {
        fit.take(fps: 40, wanted: 60, steps: 3, now: at(s));
      }
      expect(fit.step, 1);
      // At 30 or under, only a choppy screen does.
      expect(ScreenFit.choppyUnder(30), 10);
      expect(ScreenFit.choppyUnder(15), 7.5);
      expect(ScreenFit.choppyUnder(60), 45);
    },
  );
}
