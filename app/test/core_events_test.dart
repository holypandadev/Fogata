import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/restart.dart';
import 'package:fogata/src/rust/api/types.dart';

import 'fake_room.dart';

void main() {
  setUpAll(() async {
    // As if this copy had just started again by itself: a stop shows the banner and
    // leaves the restart to the person, so no test starts a new copy of anything.
    final dir = await Directory.systemTemp.createTemp('fogata-events-test');
    await Directory('${dir.path}/window').create();
    final now = DateTime.now().millisecondsSinceEpoch;
    await File('${dir.path}/window/restart')
        .writeAsString(jsonEncode({'at': now - 1000, 'automatic': now - 1000}));
    await readRestart(dir.path);
    await dir.delete(recursive: true);
  });

  test(
    'the core\'s events ending by themselves say the network part stopped',
    () async {
      final events = StreamController<UiEvent>();
      final state = fakeRoom(people: 1)..listenTo(events.stream);
      expect(state.stopping.restartsByItself, isFalse);
      await events.close();
      await Future<void>.delayed(Duration.zero);
      expect(state.stopping.stopped, isTrue);
    },
  );

  test('an error in the core\'s events says it stopped too', () async {
    final events = StreamController<UiEvent>();
    final state = fakeRoom(people: 1)..listenTo(events.stream);
    events.addError(StateError('lost'));
    await Future<void>.delayed(Duration.zero);
    expect(state.stopping.stopped, isTrue);
  });

  test(
    'a stop for a move lets go of the events first: nothing is said',
    () async {
      final events = StreamController<UiEvent>(
        onCancel: () => Completer<void>().future,
      );
      final state = fakeRoom(people: 1)
        ..started = true
        ..listenTo(events.stream);
      await state.stopForMove().timeout(const Duration(seconds: 5));
      await events.close();
      await Future<void>.delayed(Duration.zero);
      expect(state.stopping.stopped, isFalse);
    },
  );

  test(
    'a stop for a move forgets how members were reached and who was typing',
    () async {
      final events = StreamController<UiEvent>();
      final state = fakeRoom(people: 2)
        ..started = true
        ..listenTo(events.stream);
      events
        ..add(
          const UiEvent(
            kind: UiEventKind.memberConnected,
            group: 'g',
            device: 'p0',
            path: UiPath.throughMember,
            through: 'p1',
            received: 0,
            total: 0,
          ),
        )
        ..add(
          const UiEvent(
            kind: UiEventKind.typing,
            group: 'g',
            device: 'p0',
            voice: AppState.firstRoom,
            received: 0,
            total: 0,
          ),
        );
      await Future<void>.delayed(Duration.zero);
      expect(state.contacts.paths['g'], {'p0': UiPath.throughMember});
      expect(state.typingInRoom(), ['p0']);
      await state.stopForMove().timeout(const Duration(seconds: 5));
      // The core started again says anew whom it reaches.
      await state.resumeAfterMove();
      expect(state.contacts.paths['g'] ?? const {}, isEmpty);
      expect(state.contacts.carriers['g'] ?? const {}, isEmpty);
      expect(state.typingInRoom(), isEmpty);
      await events.close();
    },
  );
}
