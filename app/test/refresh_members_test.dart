import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/rust/frb_generated.dart';

import 'fake_room.dart';

UiGroup _group(String id, String name) => UiGroup(
  id: id,
  name: name,
  localName: '',
  icon: 0,
  members: 2,
  online: 1,
  manager: false,
  rank: 0,
  outranks: const [],
  maxMembers: 20,
  maxInRoom: 8,
  voiceRooms: const [],
  openInvites: 0,
  needsUpdate: false,
  raiseAnyway: false,
);

UiMember _member(int i) => UiMember(
  person: other(i),
  rank: 0,
  manager: false,
  me: false,
  away: false,
  waitingForKey: false,
  fingerprint: '7Q4M K2XR 9PLD 3VH$i',
);

/// A core with two groups whose members are asked for one by one; [failing] groups' fail.
class FakeCore implements RustLibApi {
  final failing = <String>{};
  var names = ['Alpha', 'Beta'];

  /// When set, each group's members are answered only once the test lets them go; the
  /// groups asked for so far are in [asked].
  Completer<void>? held;
  final asked = <String>[];

  @override
  Future<List<UiGroup>> crateApiAppGroups() async => [
    _group('a', names[0]),
    _group('b', names[1]),
  ];

  @override
  Future<List<UiGroupInvite>> crateApiAppGroupInvitations() async => const [];

  @override
  Future<List<UiMember>> crateApiAppMembers({required String groupId}) async {
    asked.add(groupId);
    await held?.future;
    if (failing.contains(groupId)) throw StateError('no such group');
    return [_member(groupId == 'a' ? 1 : 2)];
  }

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

final core = FakeCore();

void main() {
  setUpAll(() => RustLib.initMock(api: core));

  test(
    'one group whose members cannot be read does not stop the rest',
    () async {
      final state = AppState('/nonexistent/fogata-refresh-members-test');
      await state.refreshGroups();
      expect(state.contacts.members['a'], hasLength(1));
      expect(state.contacts.members['b'], hasLength(1));
      final before = state.contacts.members['b'];
      // Beta goes away between the two questions; the groups are renamed meanwhile.
      core
        ..failing.add('b')
        ..names = ['Alpha 2', 'Beta 2'];
      await state.refreshGroups();
      expect(state.contacts.groups.map((g) => g.name), ['Alpha 2', 'Beta 2']);
      // Its members stay as they were seen; the others are read anew.
      expect(state.contacts.members['b'], before);
      expect(state.contacts.members['a'], hasLength(1));
      // Never read: nothing is made up.
      final fresh = AppState('/nonexistent/fogata-refresh-members-test');
      await fresh.refreshGroups();
      expect(fresh.contacts.groups, hasLength(2));
      expect(fresh.contacts.members['b'], isNull);
      expect(fresh.contacts.members['a'], hasLength(1));
    },
  );

  test('every group\'s members are asked for before any answers', () async {
    core
      ..failing.clear()
      ..asked.clear()
      ..held = Completer<void>();
    addTearDown(() => core.held = null);
    final state = AppState('/nonexistent/fogata-refresh-members-test');
    final refreshing = state.refreshGroups();
    // Both questions are out while none has been answered: one group after the other
    // would ask for the second only when the first is done.
    await pumpEventQueue();
    expect(core.asked, ['a', 'b']);
    core.held!.complete();
    await refreshing;
    expect(state.contacts.members.keys, containsAll(['a', 'b']));
  });
}
