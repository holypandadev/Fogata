import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/format.dart';
import 'package:fogata/src/l10n.dart';

void main() {
  test('how long ago, up to a year', () {
    final now = DateTime(2026, 10, 28, 12);
    String daysAgo(int days) => ago(
      now.subtract(Duration(days: days)).millisecondsSinceEpoch,
      now: now,
    );
    expect(daysAgo(45), '1 month ago');
    expect(daysAgo(90), '3 months ago');
    expect(daysAgo(334), '11 months ago');
    expect(daysAgo(362), '11 months ago');
    expect(daysAgo(400), 'over a year ago');
  });

  test('sizes step up a unit rather than show 1000', () {
    expect(bytes(999), '999 B');
    expect(bytes(999400), '999 KB');
    expect(bytes(999600), '1.0 MB');
    expect(bytes(48200000), '48.2 MB');
    expect(bytes(999999999), '1.0 GB');
  });
  test(
    'a time out says when it ends with the words each language puts before it',
    () {
      addTearDown(() => useLanguage('en'));
      final now = DateTime(2026, 10, 28, 12);
      String until(String code, DateTime end) {
        useLanguage(code);
        final ms = end.millisecondsSinceEpoch;
        return l10n.activityTimedOut(
          'Ana',
          'no',
          'Leo',
          'no',
          'General',
          dayClock(ms, now: now),
          dayClockKind(ms, now: now),
        );
      }

      final tonight = DateTime(2026, 10, 28, 23, 5);
      final atOne = DateTime(2026, 10, 28, 1, 5);
      final yesterday = DateTime(2026, 10, 27, 23, 5);
      final earlier = DateTime(2026, 10, 8, 23, 5);
      expect(
        until('en', tonight),
        'Ana timed Leo out of “General” until 23:05',
      );
      expect(until('es', tonight), endsWith('hasta las 23:05'));
      expect(until('es', atOne), endsWith('hasta la 01:05'));
      expect(until('es', yesterday), endsWith('hasta ayer 23:05'));
      expect(until('es', earlier), contains('hasta el 8 '));
      expect(until('it', atOne), endsWith('fino all’01:05'));
      expect(until('it', yesterday), contains('fino a ieri'));
      // “il giorno” keeps the article whole before 8 and 11, which Italian elides.
      expect(until('it', earlier), contains('fino al giorno 8 '));
      expect(until('fr', earlier), contains('jusqu’au 8 '));
      expect(until('pt-BR', earlier), contains('até o dia 8 '));
      expect(until('de', earlier), contains('bis zum 8. '));
    },
  );

  test('the hour 1 is told apart for the article before a time', () {
    addTearDown(() => clock12 = false);
    int at(int hour) => DateTime(2026, 10, 28, hour, 5).millisecondsSinceEpoch;
    expect(clockHour(at(1)), 'one');
    expect(clockHour(at(13)), 'other');
    expect(clockHour(at(0)), 'other');
    clock12 = true;
    expect(clockHour(at(13)), 'one');
    expect(clockHour(at(12)), 'other');
  });
}
