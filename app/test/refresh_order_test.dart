import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/rust/frb_generated.dart';

import 'fake_room.dart';

/// The core's side of the calls, the groups and the room: each question waits for the
/// test to answer it, in the order the test picks.
class FakeCore implements RustLibApi {
  final calls = <Completer<List<UiCall>>>[];
  final invitations = <Completer<List<UiGroupInvite>>>[];
  final rooms = <Completer<UiRoom?>>[];

  @override
  Future<List<UiCall>> crateApiAppCalls() {
    final asked = Completer<List<UiCall>>();
    calls.add(asked);
    return asked.future;
  }

  @override
  Future<List<String>> crateApiAppCallReturns() async => const [];

  @override
  Future<List<UiGroup>> crateApiAppGroups() async => const [];

  @override
  Future<List<UiGroupInvite>> crateApiAppGroupInvitations() {
    final asked = Completer<List<UiGroupInvite>>();
    invitations.add(asked);
    return asked.future;
  }

  @override
  Future<UiRoom?> crateApiAppRoom({
    required String groupId,
    required String voice,
  }) {
    final asked = Completer<UiRoom?>();
    rooms.add(asked);
    return asked.future;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

final core = FakeCore();

UiCall _call(String group) => UiCall(
  groupId: group,
  with_: other(0),
  outgoing: true,
  phase: UiCallPhase.connected,
  sinceMs: DateTime.now().millisecondsSinceEpoch,
  members: [other(0)],
  adding: const [],
  away: const [],
);

UiRoom _room(int people) => UiRoom(
  startedMs: 0,
  joined: true,
  participants: [
    for (var i = 0; i < people; i++)
      UiParticipant(person: other(i), away: false, me: false),
  ],
  chat: const [],
  files: const [],
);

Future<void> _settle() => Future<void>.delayed(Duration.zero);

void main() {
  setUpAll(() => RustLib.initMock(api: core));

  test(
    'the calls show the newest answer, even when an older one comes after it',
    () async {
      final state = AppState('/nonexistent/fogata-refresh-order-test');
      final older = state.refreshCalls();
      final newer = state.refreshCalls();
      await _settle();
      expect(core.calls, hasLength(2));
      // The call ended between the two questions: the newer answer has no call.
      core.calls[1].complete(const []);
      await newer;
      core.calls[0].complete([_call('g1')]);
      await older;
      expect(state.callState.calls, isEmpty);
    },
  );

  test('the groups show the newest answer too', () async {
    final state = AppState('/nonexistent/fogata-refresh-order-test');
    final older = state.refreshGroups();
    final newer = state.refreshGroups();
    await _settle();
    expect(core.invitations, hasLength(2));
    core.invitations[1].complete(const []);
    await newer;
    core.invitations[0].complete([
      UiGroupInvite(id: 'g2', name: 'Hill crew', by: other(1), members: 3),
    ]);
    await older;
    expect(state.contacts.invitations, isEmpty);
  });

  test('and so does the room', () async {
    final state = fakeRoom(people: 1);
    final older = state.refreshRoom();
    final newer = state.refreshRoom();
    await _settle();
    expect(core.rooms, hasLength(2));
    core.rooms[1].complete(_room(1));
    await newer;
    core.rooms[0].complete(_room(3));
    await older;
    expect(state.roomState.room?.participants, hasLength(1));
  });
}
