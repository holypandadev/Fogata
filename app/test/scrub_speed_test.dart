// Scrubbing a report's log takes moments, whatever its lines are made of: lines made to
// make regular expressions slow, and a long log as the core writes it. The time is told
// against a plain pass over the same lines in the same process, taken in turns with the
// scrubbing, so a computer busy with other tests (as when the whole suite runs) slows
// both alike and the check holds; a rule whose time grows with the square of a line's
// length still takes hundreds of times as long as the plain pass.

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/report.dart';

/// The names of a person with many friends and groups.
final _names = [
  'Marta Ruiz',
  'Leo',
  for (var i = 0; i < 300; i++) 'Person$i Surname',
];

/// Lines that make the rules of a careless scrubber take time growing with the square of
/// their length (or worse): many words, many dots, spaces in a Windows path, words the
/// core says with a name after them.
List<String> _adversarial(int length) {
  String filled(String part) =>
      (part * (length ~/ part.length + 1)).substring(0, length);
  return [
    'INFO x: ${filled('abc ')}',
    filled('a: '),
    filled('group x '),
    filled('cannot save group x '),
    'x ${filled('a.')}',
    filled('/a b '),
    'C:\\${filled('a b ')}',
    filled('a:'),
    filled("'a "),
    'x ${'a' * length}',
    'payload=${filled('abcdefghijklmnopqrstuvwxyz')}',
    filled('Marta Ruiz2 '),
    filled('Ｍａｒｔａ\u200B '),
    filled('1.2.3.4:56 '),
    filled('relay.example.com '),
    filled('x@y '),
  ];
}

/// Lines as the core and the window write them.
const _realistic = [
  '2026-11-05T10:20:30.123Z  INFO fogata_core::net: dial …Xk3A9q via /ip4/‹ip›/udp/‹port›/quic-v1 took 120 ms, group ‹name› synced 3 messages',
  '2026-11-05T10:20:30.456Z  WARN fogata_net::node: member 7Q4M·K2XR connected peer=…VtXk3A path=relay address=/ip6/‹ip›/udp/‹port›/quic-v1',
  '2026-11-05T10:20:31.000Z DEBUG libp2p_swarm: Connection established peer=…AB12cd endpoint=Dialer { address: /ip4/‹ip›/tcp/‹port›, role_override: Dialer } num_established=1',
  '2026-11-05T10:20:31.500Z ERROR fogata_core::direct: cannot write ‹path›: No space left on device (os error 28)',
  '2026-11-05T10:20:32.000Z  INFO fogata_ffi::voice: room b1946a… opus 48 kHz, jitter 12 ms, loss 0.4%',
  '2026-11-05T10:20:32.500Z ERROR app: FileSystemException: Cannot open file, path = ‹path› (OS Error: Permission denied, errno = 13)',
  '#0      _ReportPageState._refresh (package:fogata/src/screens/report.dart:653:20)',
];

/// A few plain expressions, each found once over a line: about the least a scrubber of
/// rules must do, and what the scrubber's time is told against.
final _plain = [
  RegExp(r'\b\w+\b'),
  RegExp(r'[A-Za-z0-9+/_\-]{8,}'),
  RegExp(r'\d+(?:\.\d+)*'),
];

/// The plain pass over [lines]: what the expressions find, so none is skipped.
int _plainPass(List<String> lines) {
  var found = 0;
  for (final line in lines) {
    for (final rule in _plain) {
      found += rule.allMatches(line).length;
    }
  }
  return found;
}

/// How many times as long as the plain pass scrubbing [lines] takes. Both go in turns, a
/// few lines at a time, so a moment the computer is busy with something else slows both.
/// [check] is given what each turn's scrubbing gave.
double _timesPlain(
  Scrubber scrubber,
  List<String> lines, {
  required int turn,
  void Function(String scrubbed)? check,
}) {
  final scrubbing = Stopwatch();
  final plain = Stopwatch();
  var found = 0;
  for (var at = 0; at < lines.length; at += turn) {
    final part = lines.sublist(at, (at + turn).clamp(0, lines.length));
    scrubbing.start();
    final out = scrubber.scrub(part.join('\n'));
    scrubbing.stop();
    plain.start();
    found += _plainPass(part);
    plain.stop();
    check?.call(out);
  }
  expect(found, greaterThan(0));
  final times = scrubbing.elapsedMicroseconds / plain.elapsedMicroseconds;
  // ignore: avoid_print
  print(
    '${lines.length} lines: scrubbed in ${scrubbing.elapsed}, '
    'the plain pass in ${plain.elapsed} (${times.toStringAsFixed(1)} times)',
  );
  return times;
}

void main() {
  test('a megabyte of lines made to be slow takes moments', () {
    final scrubber = Scrubber(_names, paths: const ['/home/mruiz']);
    final lines = <String>[];
    var size = 0;
    while (size < 1024 * 1024) {
      for (final line in _adversarial(16000)) {
        lines.add(line);
        size += line.length;
      }
    }
    var count = 0;
    final times = _timesPlain(
      scrubber,
      lines,
      turn: 4,
      check: (out) => count += '\n'.allMatches(out).length + 1,
    );
    expect(count, lines.length);
    // About twice the plain pass (whose lines are not cut, as the scrubber's are); one
    // rule that takes time growing with the square of a line's length makes it twenty.
    expect(times, lessThan(10));
  }, timeout: const Timeout(Duration(minutes: 2)));

  test('a log of 100 000 lines as the core writes it takes seconds', () {
    final scrubber = Scrubber(_names, paths: const ['/home/mruiz']);
    final lines = [
      for (var i = 0; i < 100000; i++) _realistic[i % _realistic.length],
    ];
    var kept = false;
    final times = _timesPlain(
      scrubber,
      lines,
      turn: 1000,
      check: (out) =>
          kept |= out.contains('took 120 ms, group ‹name› synced 3 messages'),
    );
    expect(kept, isTrue);
    // About six times the plain pass: dozens of rules, each tried only on the lines
    // that hold what it needs.
    expect(times, lessThan(20));
  }, timeout: const Timeout(Duration(minutes: 3)));
}
