import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/state/call_state.dart';

import 'fake_room.dart';

UiCall _call(String group, UiCallPhase phase) => UiCall(
  groupId: group,
  with_: other(0),
  outgoing: phase != UiCallPhase.incoming,
  phase: phase,
  sinceMs: DateTime.now().millisecondsSinceEpoch,
  members: [other(0)],
  adding: const [],
  away: const [],
);

void main() {
  late CallState calls;
  late List<UiSound?> told;
  late int rang;

  setUp(() {
    calls = CallState();
    told = [];
    rang = 0;
    calls
      ..ringer = told.add
      ..onRing = () => rang++;
  });

  tearDown(() => calls.dispose());

  test('a call ringing here rings once until it stops ringing', () {
    calls
      ..calls = [_call('c', UiCallPhase.incoming)]
      ..ring()
      ..ring();
    expect(told, [UiSound.ring]);
    expect(rang, 1);
    calls
      ..calls = []
      ..ring();
    expect(told, [UiSound.ring, null]);
    expect(calls.ringing, isNull);
  });

  test('ringing a friend plays the ringback until answered', () {
    calls
      ..calls = [_call('c', UiCallPhase.ringing)]
      ..ring();
    expect(told, [UiSound.ringback]);
    expect(rang, 0);
    calls
      ..calls = [_call('c', UiCallPhase.connected)]
      ..ring();
    expect(told, [UiSound.ringback, null]);
  });

  test('a friend calling while this device rings someone plays the ring', () {
    calls
      ..calls = [_call('c', UiCallPhase.ringing)]
      ..ring()
      ..calls = [
        _call('c', UiCallPhase.ringing),
        _call('d', UiCallPhase.incoming),
      ]
      ..ring();
    expect(told, [UiSound.ringback, UiSound.ring]);
  });

  test('answering stops the ring at once, before the core says so', () {
    calls
      ..calls = [_call('c', UiCallPhase.incoming)]
      ..ring()
      ..hush('c');
    expect(told, [UiSound.ring, null]);
    // The list still shows it ringing until it is refreshed.
    calls.ring();
    expect(told, [UiSound.ring, null]);
    calls
      ..calls = [_call('c', UiCallPhase.connected)]
      ..ring();
    expect(told, [UiSound.ring, null]);
    // The next call from the same friend rings again.
    calls
      ..calls = [_call('c', UiCallPhase.incoming)]
      ..ring();
    expect(told, [UiSound.ring, null, UiSound.ring]);
  });

  test('an answer that does not go through lets the call ring again', () {
    calls
      ..calls = [_call('c', UiCallPhase.incoming)]
      ..ring()
      ..hush('c')
      ..unhush('c');
    expect(told, [UiSound.ring, null, UiSound.ring]);
  });

  test('hanging up stops the ringback, another call still rings', () {
    calls
      ..calls = [
        _call('c', UiCallPhase.ringing),
        _call('d', UiCallPhase.incoming),
      ]
      ..ring()
      ..hush('d');
    expect(told, [UiSound.ring, UiSound.ringback]);
    calls.hush('c');
    expect(told, [UiSound.ring, UiSound.ringback, null]);
  });

  test('the ring stops when the calls go', () {
    calls
      ..calls = [_call('c', UiCallPhase.incoming)]
      ..ring()
      ..dispose();
    expect(told, [UiSound.ring, null]);
    calls = CallState();
  });
}
