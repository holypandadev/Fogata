import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/l10n.dart';

import 'fake_room.dart';

void main() {
  test(
    'without a sound server no call is made and the room says why',
    () async {
      final state = fakeRoom(people: 3)
        ..callState.call = null
        // The others are told through the core; here, nowhere.
        ..sendSignal = (group, device, payload) {};
      var asked = 0;
      state.soundServer = () async {
        asked++;
        return false;
      };
      await state.startCallForTests('g');
      expect(asked, 1);
      // No call at all: nothing shows as connecting, no button acts on nothing.
      expect(state.callState.call, isNull);
      expect(state.toasts.list, hasLength(1));
      expect(state.toasts.list.single.text, l10n.callNoSoundSystem);
      expect(state.toasts.list.single.error, isTrue);
      // The room itself stays open, with its chat.
      expect(state.roomState.roomGroup, 'g');
    },
  );

  test('a room left while the answer came makes no call either', () async {
    final state = fakeRoom(people: 2)..callState.call = null;
    state.soundServer = () async {
      state.roomState.roomGroup = null;
      return true;
    };
    await state.startCallForTests('g');
    expect(state.callState.call, isNull);
    expect(state.toasts.list, isEmpty);
  });

  test('a definite "none" stops a call, and a server silent twice', () async {
    Future<bool> after(List<String?> answers) {
      final left = [...answers];
      var asked = 0;
      return Call.soundServerAfter(() async {
        asked++;
        expect(asked, lessThanOrEqualTo(answers.length));
        return left.removeAt(0);
      });
    }

    expect(await after(['none']), isFalse);
    expect(await after(['ready']), isTrue);
    // Slow the first time, there the second: asked again, and it starts.
    expect(await after(['timeout', 'ready']), isTrue);
    expect(await after(['timeout', 'timeout']), isFalse);
    expect(await after(['timeout', 'none']), isFalse);
    // What cannot be told lets it start.
    expect(await after(['unknown']), isTrue);
    expect(await after([null]), isTrue);
  });

  test('a room left while a refusal came tells nobody', () async {
    final state = fakeRoom(people: 3)..callState.call = null;
    final sent = <String>[];
    state.sendSignal = (group, device, payload) => sent.add(device);
    state.soundServer = () async {
      state.roomState.roomGroup = null;
      return false;
    };
    await state.startCallForTests('g');
    expect(sent, isEmpty);
    expect(state.toasts.list, isEmpty);
    expect(state.noCallIn, isNull);
  });

  test('ending the call forgets the room without one', () async {
    final state = fakeRoom(people: 2)
      ..callState.call = null
      ..sendSignal = (group, device, payload) {};
    state.soundServer = () async => false;
    await state.startCallForTests('g');
    expect(state.noCallIn, 'g');
    await state.endCallForTests();
    expect(state.noCallIn, isNull);
  });
}
