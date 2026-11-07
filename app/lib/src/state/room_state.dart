import 'dart:io';

import 'package:flutter/foundation.dart';

import '../photo.dart';
import '../rust/api/app.dart' as api;
import '../rust/api/types.dart';
import '../safe_image.dart';
import 'piece.dart';

/// A line of a room's timeline that the chat itself does not hold: someone came or went,
/// a file was shared. Only in memory, like everything said in a room.
class RoomNote {
  RoomNote.joined(this.atMs, this.device, this.name)
    : file = null,
      left = false;
  RoomNote.left(this.atMs, this.device, this.name) : file = null, left = true;
  RoomNote.file(this.atMs, this.file, this.device, this.name) : left = false;

  final int atMs;

  /// Who it is about, and their name when it happened: shown with the name they have now
  /// (see `AppState.nameOf`).
  final String? device;
  final String name;
  final String? file;
  final bool left;
}

/// A file seen in a room, remembered after its holders leave.
class SeenFile {
  SeenFile(this.id, this.name, this.size, this.byId, this.byName);
  final String id;
  final String name;
  final int size;
  final String byId;
  final String byName;
  int? goneMs;
  String? goneName;
}

/// A file dropped on a room or picked for it, waiting to be sent, and the folder it is
/// in (links followed), shown beside it.
class OutgoingFile {
  OutgoingFile(this.path, this.name, this.size, {this.folder = ''});
  final String path;
  final String name;
  final int size;
  final String folder;

  /// Where it is on its way: only files that are [OutgoingState.pending] or
  /// [OutgoingState.failed] wait in the chat; a shared one is done.
  OutgoingState state = OutgoingState.pending;
}

/// What became of a file waiting to be sent.
enum OutgoingState { pending, sending, shared, failed }

/// The files at [paths] as they wait to go, and how many of the paths were folders, which
/// cannot go. Only files of this computer go: a path that is not absolute (text another
/// application dropped, read from wherever the application started), a device or a pipe
/// (read without end), one that is gone or that comes twice is left out. Read without
/// holding up the window: a folder on a slow or network disk could take long to answer.
Future<(List<OutgoingFile>, int)> readOutgoing(Iterable<String> paths) async {
  final files = <OutgoingFile>[];
  var folders = 0;
  for (final path in paths) {
    if (!File(path).isAbsolute) continue;
    final type = await FileSystemEntity.type(path);
    if (type == FileSystemEntityType.directory) {
      folders++;
      continue;
    }
    if (type != FileSystemEntityType.file) continue;
    if (files.any((f) => f.path == path)) continue;
    // Windows takes either slash between folders.
    final name = path
        .split(Platform.isWindows ? RegExp(r'[\\/]') : RegExp('/'))
        .last;
    try {
      final real = File(await File(path).resolveSymbolicLinks());
      files.add(
        OutgoingFile(
          path,
          name,
          await File(path).length(),
          folder: real.parent.path,
        ),
      );
    } on FileSystemException {
      // Gone meanwhile.
    }
  }
  return (files, folders);
}

/// A local picture of a room, read up to [RoomState.maxPreviewBytes]: it may have grown
/// since its size was told.
Future<Uint8List> _readLocal(String path) async {
  final bytes = await readAtMost(
    File(path).openRead(0, RoomState.maxPreviewBytes + 1),
    RoomState.maxPreviewBytes,
  );
  if (bytes == null) throw FileSystemException('larger than it was said', path);
  return bytes;
}

/// What this window remembers of a group's room while it is open.
class RoomMemory {
  RoomMemory(this.startedMs);
  final int startedMs;
  final List<RoomNote> notes = [];
  final Map<String, SeenFile> files = {};
  final Set<String> others = {};

  /// When each chat line first reached this device, by message id: the chat is shown in
  /// that order, like the notes, so clocks that differ between devices do not reorder it.
  final Map<String, int> seen = {};
  int? joinedMs;

  /// Whether the room was read into this memory once ([RoomState.remember]): what comes
  /// after is new; what was there at the first reading was already said.
  bool read = false;

  /// People were in the room when this device joined it: it waits for them, and says so
  /// if they cannot be reached.
  bool othersExpected = false;

  /// Members were online but none was known to be in the room: it waits a moment in case
  /// one is, then shows the room as empty.
  bool othersMaybe = false;

  /// Most notes kept: older ones are forgotten, with the files only they showed, so
  /// people coming and going or sharing again and again do not fill the memory.
  static const maxNotes = 1000;

  /// Most chat lines whose arrival is kept, the oldest forgotten first: twice the lines
  /// a room's chat keeps.
  static const maxSeen = 1000;

  /// Adds a note, forgetting the oldest beyond [maxNotes].
  void addNote(RoomNote note) {
    notes.add(note);
    if (notes.length <= maxNotes) return;
    notes.removeRange(0, notes.length - maxNotes);
    final shown = {for (final n in notes) n.file};
    files.removeWhere((id, _) => !shown.contains(id));
  }

  /// Notes that `line` is here now, if it was not already.
  void note(UiChatLine line, int now) {
    seen.putIfAbsent(line.id, () => now);
    if (seen.length > maxSeen) seen.remove(seen.keys.first);
  }

  /// When `line` first reached this device, if known.
  int? seenAt(UiChatLine line) => seen[line.id];
}

/// The room shown (its group and voice room, what it holds and what this window learnt
/// of it), the rooms' memories, pictures fetched for their previews and the files waiting
/// to go.
class RoomState extends Piece {
  /// The id every group's first voice room has.
  static const firstRoom = '0000000000000000';

  /// The group whose room is shown, or whose call is.
  String? roomGroup;

  /// The voice room of [roomGroup] shown.
  String roomVoice = firstRoom;
  UiRoom? room;
  final Map<String, RoomMemory> memories = {};

  /// The tab of the room's side panel: 0 chat, 1 files.
  int roomTab = 0;

  /// Whether the room page was too small for the chat beside the whole row of controls
  /// when last drawn; the room page keeps it up to date.
  bool roomSmall = false;

  /// The call, or the room while its call has not started yet, whose chat was shown by
  /// hand on a small window.
  Object? chatSmallFor;

  /// Files dropped on a room or picked for it, by group, until Send: each can be taken out
  /// again, and they go with a message.
  final Map<String, List<OutgoingFile>> outgoing = {};

  List<OutgoingFile> get outgoingHere => outgoing[roomGroup] ?? const [];

  /// The memory of [group]'s room started at [startedMs]: a room opened again starts a
  /// new one, keeping when this device joined and whom it waits for.
  RoomMemory memoryFor(String group, int startedMs) {
    final memory = memories[group];
    if (memory != null && memory.startedMs == startedMs) return memory;
    final fresh = RoomMemory(startedMs);
    if (memory != null) {
      fresh.joinedMs = memory.joinedMs;
      fresh.othersExpected = memory.othersExpected;
      fresh.othersMaybe = memory.othersMaybe;
    }
    return memories[group] = fresh;
  }

  /// Notes what [r], [group]'s room as the core has it now, holds: who is in it, when each
  /// chat line came and the files shared.
  void remember(String group, UiRoom? r) {
    if (r == null) return;
    final memory = memoryFor(group, r.startedMs)..read = true;
    for (final p in r.participants) {
      if (!p.me) memory.others.add(p.person.id);
    }
    final now = DateTime.now().millisecondsSinceEpoch;
    for (final line in r.chat) {
      memory.note(line, now);
    }
    for (final f in r.files) {
      if (!memory.files.containsKey(f.id)) {
        memory.files[f.id] = SeenFile(
          f.id,
          f.name,
          f.size,
          f.sharedBy.id,
          f.sharedBy.name,
        );
        memory.addNote(
          RoomNote.file(now, f.id, f.sharedBy.id, f.sharedBy.name),
        );
      }
    }
  }

  /// Keeps a memory for each group's open room, a new one for a room opened again, and
  /// none for a room that closed.
  void rememberRooms(List<UiGroup> groups) {
    for (final g in groups) {
      final summary = g.room;
      if (summary == null) {
        memories.remove(g.id);
        continue;
      }
      final memory = memories[g.id];
      if (memory == null || memory.startedMs != summary.startedMs) {
        memories[g.id] = RoomMemory(summary.startedMs);
      }
    }
  }

  // ----- Previews -----

  /// Pictures of the room fetched for their previews, by file id. In memory only, like the
  /// rest of the room, and dropped when leaving it.
  final Map<String, Uint8List> previews = {};
  final Set<String> _previewing = {};

  /// Whether the picture of [id] is being fetched for its preview now.
  @visibleForTesting
  bool previewing(String id) => _previewing.contains(id);

  /// Pictures of the room shown as files instead: damaged, larger than a picture may be,
  /// past what is held at once, or not fetched after a few tries.
  final Set<String> noPreview = {};
  final Map<String, int> _previewFailures = {};

  /// Times a picture's preview is tried before it shows as a file.
  static const _previewTries = 3;

  /// Biggest picture fetched by itself for its preview.
  static const maxPreviewBytes = 10 * 1024 * 1024;

  /// Most bytes of pictures held for previews at once; past it, pictures show as files.
  static const maxPreviewTotal = 128 * 1024 * 1024;
  int _previewingBytes = 0;

  /// Lets go of the room's pictures, and of what was learnt of them.
  void dropPreviews() {
    previews.clear();
    noPreview.clear();
    _previewFailures.clear();
  }

  /// Fetches the room's pictures that have no preview yet: one shared from here is read
  /// from its file, another one comes from a device that has it, into memory. One that
  /// cannot be shown (see [noPreview]) shows as a file.
  void fetchPreviews(String group, UiRoom? r) {
    var held =
        previews.values.fold<int>(0, (sum, b) => sum + b.length) +
        _previewingBytes;
    for (final f in r?.files ?? const <UiFile>[]) {
      if (previews.containsKey(f.id) ||
          _previewing.contains(f.id) ||
          noPreview.contains(f.id) ||
          !isPicture(f.name) ||
          f.size > maxPreviewBytes) {
        continue;
      }
      if (held + f.size > maxPreviewTotal) {
        noPreview.add(f.id);
        continue;
      }
      _previewing.add(f.id);
      held += f.size;
      _previewingBytes += f.size;
      final path = f.path;
      final Future<Uint8List> fetch = path != null && f.downloading == null
          ? _readLocal(path)
          : api.fetchPreview(groupId: group, file: f.id);
      fetch
          .then((bytes) async {
            // Damaged, or larger than a picture may be once decoded.
            final shown = await fitsToDecode(bytes, maxPicturePixels);
            if (roomGroup != group) return;
            if (shown) {
              previews[f.id] = bytes;
            } else {
              noPreview.add(f.id);
            }
            changed();
          })
          .catchError((Object _) {
            // Tried again at the next change of the room, a few times.
            final tries = _previewFailures[f.id] =
                (_previewFailures[f.id] ?? 0) + 1;
            if (tries >= _previewTries && roomGroup == group) {
              noPreview.add(f.id);
              changed();
            }
          })
          .whenComplete(() {
            _previewing.remove(f.id);
            _previewingBytes -= f.size;
          });
    }
  }
}

/// The others' messages in [r] that [memory] has not read yet, as who wrote each and its
/// words, or the file's name for a file shared; [nameOf] names a chat line's writer.
/// None before the room was first read into [memory], or for another opening of it.
List<(String, String)> arrivals(
  RoomMemory? memory,
  UiRoom r,
  String Function(UiChatLine line) nameOf,
) {
  if (memory == null || !memory.read || memory.startedMs != r.startedMs) {
    return const [];
  }
  return [
    for (final line in r.chat)
      if (!line.mine && memory.seenAt(line) == null) (nameOf(line), line.text),
    for (final file in r.files)
      if (!file.mine && !memory.files.containsKey(file.id))
        (file.sharedBy.name, file.name),
  ];
}
