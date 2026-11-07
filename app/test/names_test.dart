import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';

UiPerson person(String id, String name, {int preset = 0}) => UiPerson(
  id: id,
  name: name,
  short: '7Q4M·K2XR',
  avatar: UiAvatar(preset: preset),
);

UiMember member(UiPerson p) => UiMember(
  away: false,
  person: p,
  rank: 1,
  manager: false,
  me: false,
  waitingForKey: false,
  fingerprint: '7Q4M K2XR 9PLD 3VHC',
);

void main() {
  test('a room note shows the name its person has now', () {
    final state = AppState('/nonexistent/fogata-names-test')
      ..roomState.roomGroup = 'g'
      ..contacts.members['g'] = [member(person('leo', 'Leo'))];
    final memory = RoomMemory(0);
    memory.notes.add(RoomNote.joined(1, 'leo', 'Leo'));
    memory.notes.add(RoomNote.file(2, 'f', 'leo', 'Leo'));
    state.roomState.memories['g'] = memory;
    expect(state.nameOf('leo', 'Leo'), 'Leo');
    // Leo renames himself: what the room noted of him shows the new name.
    state.contacts.members['g'] = [
      member(person('leo', 'Leonardo', preset: 3)),
    ];
    for (final note in memory.notes) {
      expect(state.nameOf(note.device, note.name), 'Leonardo');
    }
    expect(state.knownPerson('leo')?.avatar.preset, 3);
  });

  test('a group left behind does not lend its old names', () {
    final state = AppState('/nonexistent/fogata-names-test')
      ..contacts.members['left'] = [member(person('leo', 'Leo'))]
      ..contacts.friends = [
        UiFriend(
          person: person('leo', 'Leonardo'),
          fingerprint: '7Q4M K2XR',
          presence: UiPresence.online,
          delivered: true,
        ),
      ];
    expect(state.nameOf('leo', 'Leo'), 'Leonardo');
  });

  test('someone no longer known here keeps the name they had', () {
    final state = AppState('/nonexistent/fogata-names-test')
      ..contacts.friends = [
        UiFriend(
          person: person('sofia', 'Sofía'),
          fingerprint: '7Q4M K2XR',
          presence: UiPresence.online,
          delivered: true,
        ),
      ];
    // A friend met in a direct call: known through the friends list.
    expect(state.nameOf('sofia', 'Sofi'), 'Sofía');
    state.contacts.friends = [];
    expect(state.nameOf('sofia', 'Sofi'), 'Sofi');
    expect(state.nameOf(null, 'Someone'), 'Someone');
  });
}
