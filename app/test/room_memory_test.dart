import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';

UiChatLine line(String id, int sent) => UiChatLine(
  id: id,
  sender: 'ana',
  nick: 'Ana',
  text: id,
  timestampMs: sent,
  mine: false,
);

void main() {
  test('a line keeps the moment it first reached this device', () {
    final memory = RoomMemory(0);
    final late = line('a', 5_000); // sent by a clock that runs ahead
    final early = line('b', 1_000);
    memory.note(late, 10_000);
    memory.note(early, 10_500);
    // Seen again (a copy among the room's events): the first time stands.
    memory.note(late, 20_000);
    expect(memory.seenAt(late), 10_000);
    expect(memory.seenAt(early), 10_500);
    expect(memory.seenAt(line('c', 0)), isNull);
    // Shown in the order they arrived here, whatever their senders' clocks said.
    final shown = [late, early]
      ..sort((x, y) => memory.seenAt(x)!.compareTo(memory.seenAt(y)!));
    expect(shown.map((l) => l.id), ['a', 'b']);
  });

  test('people coming and going do not fill the memory', () {
    final memory = RoomMemory(0);
    memory.files['f'] = SeenFile('f', 'plan.pdf', 10, 'ana', 'Ana');
    memory.addNote(RoomNote.file(1, 'f', 'ana', 'Ana'));
    for (var i = 0; i < 5000; i++) {
      memory.addNote(RoomNote.joined(i, 'leo', 'Leo'));
      memory.addNote(RoomNote.left(i, 'leo', 'Leo'));
    }
    expect(memory.notes, hasLength(RoomMemory.maxNotes));
    expect(memory.notes.last.left, isTrue);
    // The file's note is gone, and so is the file.
    expect(memory.files, isEmpty);
    memory.files['g'] = SeenFile('g', 'notes.txt', 10, 'ana', 'Ana');
    memory.addNote(RoomNote.file(2, 'g', 'ana', 'Ana'));
    expect(memory.files.keys, ['g']);
  });

  test('only the latest chat lines keep their arrival', () {
    final memory = RoomMemory(0);
    for (var i = 0; i < 3000; i++) {
      memory.note(line('l$i', i), i);
    }
    expect(memory.seen, hasLength(RoomMemory.maxSeen));
    expect(memory.seenAt(line('l0', 0)), isNull);
    expect(memory.seenAt(line('l2999', 0)), 2999);
  });
}
