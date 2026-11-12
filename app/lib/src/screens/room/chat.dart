part of '../room.dart';

// The room's side panel and its chat: messages, files shared and who came and went, in
// the order they reached this device, and the box to write in.

/// The side panel: chat and files.
class _Panel extends StatefulWidget {
  const _Panel({required this.state, required this.connecting});
  final AppState state;
  final bool connecting;

  @override
  State<_Panel> createState() => _PanelState();
}

class _PanelState extends State<_Panel> {
  @override
  Widget build(BuildContext context) {
    final state = widget.state;
    final available = state.roomState.room?.files.length ?? 0;
    final tab = state.roomState.roomTab;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        // The chat button among the controls hides and shows the chat and files.
        LineTabs(
          tabs: [
            (l10n.roomTabChat, null),
            (l10n.roomTabFiles, available > 0 ? available : null),
          ],
          selected: tab,
          onSelect: (i) => setState(() => state.roomState.roomTab = i),
          height: 48,
          padding: const EdgeInsets.symmetric(horizontal: 16),
          center: true,
        ),
        Expanded(
          child: tab == 0
              ? _Chat(
                  // Each room's chat its own, with its own draft and place.
                  key: ValueKey(state.roomState.draftHere),
                  state: state,
                  connecting: widget.connecting,
                )
              : _Files(state: state),
        ),
      ],
    );
  }
}

/// Someone known in the room or the group, for drawing their avatar.
UiPerson _person(AppState state, String id, String name) =>
    state.knownPerson(id) ??
    UiPerson(id: id, name: name, short: '', avatar: UiAvatar(preset: 0));

class _Chat extends StatefulWidget {
  const _Chat({super.key, required this.state, required this.connecting});
  final AppState state;
  final bool connecting;

  @override
  State<_Chat> createState() => _ChatState();
}

class _ChatState extends State<_Chat> {
  final _text = TextEditingController();
  final _focus = FocusNode();
  final _scroll = ScrollController();

  /// The newest entry at the last build ([_Entry.id]), to know when one comes. Not how
  /// many there are: a chat at its cap drops its oldest line as each new one comes.
  Object? _lastEntry;

  /// Looking for a message: only those that contain it are shown.
  final _search = TextEditingController();

  /// Files waiting at the last build: new ones put the cursor in the message box, so
  /// Enter sends them.
  int _lastWaiting = 0;

  /// Tells the others in the room this device is typing while the message box changes.
  late final _typing = TypingSender(widget.state.roomTyping);

  /// The room whose chat this is, for its draft ([RoomState.drafts]); the panel makes a
  /// new chat for another room.
  late final _room = widget.state.roomState.draftHere;

  @override
  void initState() {
    super.initState();
    _search.addListener(() => setState(() {}));
    // What was being written in this room's chat before it was last shown.
    final draft = widget.state.roomState.drafts[_room];
    if (draft != null) {
      _text.value = TextEditingValue(
        text: draft,
        selection: TextSelection.collapsed(offset: draft.length),
      );
    }
    _text.addListener(() {
      _typing.changed(_text.text);
      _keepDraft();
    });
  }

  void _keepDraft() {
    final room = _room;
    if (room == null) return;
    final drafts = widget.state.roomState.drafts;
    if (_text.text.isEmpty) {
      drafts.remove(room);
    } else {
      drafts[room] = _text.text;
    }
  }

  @override
  void dispose() {
    _typing.stopped();
    _text.dispose();
    _focus.dispose();
    _scroll.dispose();
    _search.dispose();
    super.dispose();
  }

  /// [written] as it is drawn ([shownText]), with the parts that match [query] marked.
  /// The matches are found in the text itself, never in a lowercased copy, whose letters
  /// can be longer (`İ` is two there) and would put the marks elsewhere. A long message
  /// whose first match is past its cut shows whole while searching, and with [bring] the
  /// chat scrolls to that match's line.
  Widget _marked(
    String written,
    RegExp? query,
    Fg c, {
    Key? key,
    bool bring = false,
  }) {
    final text = shownText(written);
    if (query == null) return LongText(emojiText(text, tx(c.fg)), key: key);
    final spans = <TextSpan>[];
    var at = 0;
    int? first;
    for (final m in query.allMatches(text)) {
      if (m.end == m.start) continue;
      first ??= m.start;
      if (m.start > at) spans.add(TextSpan(text: text.substring(at, m.start)));
      spans.add(
        TextSpan(
          text: text.substring(m.start, m.end),
          style: TextStyle(
            backgroundColor: c.amberSoft,
            color: c.amber,
            fontWeight: w600,
          ),
        ),
      );
      at = m.end;
    }
    spans.add(TextSpan(text: text.substring(at)));
    return LongText(
      TextSpan(children: spans),
      style: tx(c.fg),
      key: key,
      reveal: first,
      bringIntoView: bring,
    );
  }

  Future<void> _send() async {
    final text = _text.text;
    final files = widget.state.roomState.outgoingHere.isNotEmpty;
    if (text.trim().isEmpty && !files) return;
    _text.clear();
    _focus.requestFocus();
    final sent = files
        ? await widget.state.sendOutgoing(text)
        : await widget.state.sendChat(text);
    // It did not go: what was written is back in the box, unless more was written since.
    if (!sent && mounted && _text.text.trim().isEmpty) {
      final kept = widget.state.roomState.drafts[_room] ?? text;
      _text.value = TextEditingValue(
        text: kept,
        selection: TextSelection.collapsed(offset: kept.length),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = widget.state;
    final c = context.fg;
    final room = state.roomState.room;
    final memory = state.roomState.memories[state.roomState.roomGroup];
    final direct = state.callState.callIn(state.roomState.roomGroup);
    final query = _search.text.trim().toLowerCase();
    final pattern = query.isEmpty
        ? null
        : RegExp(RegExp.escape(query), caseSensitive: false, unicode: true);
    final entries = _entriesFor(room?.chat, memory, pattern);
    final newest = entries.lastOrNull?.id;
    if (query.isEmpty && newest != _lastEntry) {
      // Down to the newest line when the chat was at its end already or the line is this
      // device's own; someone reading further up stays where they are. What a search
      // shows counts for neither.
      final atEnd =
          _lastEntry == null ||
          !_scroll.hasClients ||
          _scroll.position.extentAfter < 80;
      final mine =
          entries.isNotEmpty &&
          entries.last.who == state.contacts.profile?.person.id;
      _lastEntry = newest;
      if (atEnd || mine) _toEnd();
    }

    // The newest message found, where a search shows its results: the chat goes to its
    // match when that is past the message's cut.
    final newestFound = pattern == null
        ? null
        : entries.reversed.where((e) => e.line != null).firstOrNull;

    /// What entry [e] shows: the message, the file's card or the note.
    Widget content(_Entry e) {
      if (e.line case final line?) {
        return _marked(
          line.text,
          pattern,
          c,
          key: ValueKey(e.id),
          bring: identical(e, newestFound),
        );
      }
      if (e.file case final seen?) {
        return isPicture(seen.name)
            ? _PictureCard(state: state, seen: seen)
            : isVideo(seen.name)
            ? _VideoCard(state: state, seen: seen)
            : _FileCard(state: state, seen: seen);
      }
      final note = e.note!;
      return Row(
        children: [
          if (note.left) ...[
            FgIcon(Ic.leave, size: 12, color: c.fg3),
            const SizedBox(width: 8),
          ],
          Flexible(
            child: Text.rich(
              TextSpan(
                children: [
                  TextSpan(
                    text: direct != null
                        ? (note.left
                              ? l10n.roomNoteLeftCall(
                                  state.nameOf(note.device, note.name),
                                )
                              : l10n.roomNoteJoinedCall(
                                  state.nameOf(note.device, note.name),
                                ))
                        : (note.left
                              ? l10n.roomNoteLeftRoom(
                                  state.nameOf(note.device, note.name),
                                )
                              : l10n.roomNoteEnteredRoom(
                                  state.nameOf(note.device, note.name),
                                )),
                  ),
                  TextSpan(
                    text: clock(note.atMs),
                    style: const TextStyle(fontFamily: mono),
                  ),
                ],
              ),
              style: tx(c.fg3, size: 12),
            ),
          ),
        ],
      );
    }

    final waiting = state.roomState.outgoingHere;
    if (waiting.length > _lastWaiting) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) _focus.requestFocus();
      });
    }
    _lastWaiting = waiting.length;
    final group = state.contacts.group(state.roomState.roomGroup);
    final someone = _expected(state).map((p) => p.name).firstOrNull;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 0),
          child: Center(
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 280),
              child: CallbackShortcuts(
                bindings: {
                  const SingleActivator(LogicalKeyboardKey.escape):
                      _search.clear,
                },
                child: FgField(
                  controller: _search,
                  height: 32,
                  radius: 999,
                  fontSize: 13,
                  background: c.bg2,
                  placeholder: l10n.roomSearchMessages,
                  leading: FgIcon(Ic.search, size: 14, color: c.fg3),
                  trailing: _search.text.isEmpty
                      ? null
                      : FgButton(
                          icon: Ic.close,
                          kind: Kind.ghost,
                          height: 24,
                          tooltip: l10n.roomClearSearch,
                          onTap: _search.clear,
                        ),
                  label: l10n.roomSearchMessages,
                ),
              ),
            ),
          ),
        ),
        Expanded(
          // What was said can be selected and copied.
          child: SelectionArea(
            child: Scrollbar(
              controller: _scroll,
              // Built as it scrolls: only the lines in view are made.
              child: ListView.builder(
                controller: _scroll,
                // The line that says who is typing comes right under it.
                padding: const EdgeInsets.fromLTRB(16, 16, 16, 4),
                itemCount: entries.length + 1,
                itemBuilder: (context, i) {
                  if (i == 0) {
                    return Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        if (query.isEmpty)
                          RuleText(
                            direct != null
                                ? l10n.roomChatGoneCall
                                : l10n.roomChatGoneRoom,
                          )
                        else if (entries.isEmpty)
                          Padding(
                            padding: const EdgeInsets.only(top: 24),
                            child: Text(
                              l10n.roomNoMatches(_search.text.trim()),
                              textAlign: TextAlign.center,
                              style: tx(c.fg3, size: 13),
                            ),
                          )
                        else
                          Text(
                            l10n.roomMatches(entries.length),
                            textAlign: TextAlign.center,
                            style: tx(c.fg3, size: 12),
                          ),
                        if (widget.connecting) ...[
                          const SizedBox(height: 16),
                          Text(
                            someone == null
                                ? l10n.roomReceivingMessages
                                : l10n.roomReceivingMessagesFrom(someone),
                            style: tx(c.fg2, size: 13),
                          ),
                          for (final w in const [.35, .25, .4]) ...[
                            const SizedBox(height: 16),
                            Row(
                              children: [
                                const Skeleton(
                                  width: 24,
                                  height: 24,
                                  radius: 12,
                                ),
                                const SizedBox(width: 10),
                                Expanded(
                                  child: LayoutBuilder(
                                    builder: (context, box) => Column(
                                      crossAxisAlignment:
                                          CrossAxisAlignment.start,
                                      children: [
                                        Skeleton(
                                          width: box.maxWidth * w,
                                          height: 10,
                                        ),
                                        const SizedBox(height: 6),
                                        Skeleton(
                                          width:
                                              box.maxWidth *
                                              (w + .45).clamp(0, 1),
                                          height: 10,
                                        ),
                                      ],
                                    ),
                                  ),
                                ),
                              ],
                            ),
                          ],
                        ],
                      ],
                    );
                  }
                  final e = entries[i - 1];
                  // A file sent right after its sender's message, or after another of
                  // their files, goes under it.
                  if (_under(entries, i - 1)) {
                    return Padding(
                      padding: const EdgeInsets.only(top: 8, left: 34),
                      child: content(e),
                    );
                  }
                  final who = e.who;
                  return Padding(
                    padding: const EdgeInsets.only(top: 16),
                    child: who == null
                        ? content(e)
                        : _Message(
                            person: _person(
                              state,
                              who,
                              e.file?.byName ?? e.line!.nick,
                            ),
                            time: e.time,
                            child: content(e),
                          ),
                  );
                },
              ),
            ),
          ),
        ),
        ValueListenableBuilder(
          valueListenable: state.typingChanged,
          builder: (context, _, _) => TypingLine(
            names: [
              for (final id in state.typingInRoom())
                state.nameOf(id, l10n.roomSomeone),
            ],
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 2),
          ),
        ),
        Container(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 16),
          decoration: BoxDecoration(
            border: Border(top: BorderSide(color: c.border)),
          ),
          child: CallbackShortcuts(
            bindings: {
              const SingleActivator(LogicalKeyboardKey.escape): () {
                if (state.roomState.outgoingHere.isNotEmpty) {
                  state.clearOutgoing();
                }
              },
            },
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                // A pill like the room's controls; taller with files waiting above.
                AnimatedContainer(
                  duration: const Duration(milliseconds: 200),
                  decoration: BoxDecoration(
                    color: c.bg2,
                    borderRadius: BorderRadius.circular(
                      waiting.isEmpty ? 999 : 22,
                    ),
                    // The box to write in is a control: its edge is seen at 3:1.
                    border: Border.all(color: c.edge),
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      if (waiting.isNotEmpty) ...[
                        Padding(
                          padding: const EdgeInsets.fromLTRB(8, 6, 8, 8),
                          child: OutgoingTray(state: state, files: waiting),
                        ),
                        Container(height: 1, color: c.border),
                      ],
                      Padding(
                        padding: const EdgeInsets.all(4),
                        child: Row(
                          children: [
                            FgButton(
                              icon: Ic.paperclip,
                              kind: Kind.ghost,
                              height: 32,
                              tooltip: waiting.isEmpty
                                  ? l10n.roomShareFiles
                                  : l10n.roomAddMoreFiles,
                              onTap: widget.connecting
                                  ? null
                                  : () => pickToShare(context),
                            ),
                            const SizedBox(width: 4),
                            Expanded(
                              child: EmojiComplete(
                                controller: _text,
                                max: maxMessage,
                                child: FgField(
                                  controller: _text,
                                  focusNode: _focus,
                                  height: 32,
                                  borderless: true,
                                  placeholder: waiting.isEmpty
                                      ? (direct == null
                                            ? (group == null
                                                  ? l10n.roomMessageTheRoom
                                                  : l10n.roomMessageTo(
                                                      group.name,
                                                    ))
                                            : direct.members.length > 1
                                            ? l10n.roomMessageTheCall
                                            : l10n.roomMessageTo(
                                                direct.with_.name,
                                              ))
                                      : l10n.roomAddMessageOptional,
                                  onSubmitted: (_) => _send(),
                                  maxLength: maxMessage,
                                  label: l10n.roomMessageLabel,
                                ),
                              ),
                            ),
                            // Near the limit, how much is left; a longer paste is cut
                            // there, and the others' apps would not show more.
                            ValueListenableBuilder(
                              valueListenable: _text,
                              builder: (context, value, _) {
                                final length = value.text.runes.length;
                                if (length < maxMessage - 200) {
                                  return const SizedBox.shrink();
                                }
                                return Padding(
                                  padding: const EdgeInsets.only(left: 6),
                                  child: Text(
                                    '${maxMessage - length}',
                                    style: tx(
                                      length >= maxMessage ? c.red : c.fg3,
                                      size: 12,
                                      monospace: true,
                                    ),
                                  ),
                                );
                              },
                            ),
                            const SizedBox(width: 2),
                            EmojiButton(
                              controller: _text,
                              focusNode: _focus,
                              size: 32,
                              max: maxMessage,
                            ),
                            const SizedBox(width: 2),
                            FgButton(
                              icon: Ic.send,
                              kind: Kind.primary,
                              height: 32,
                              tooltip: l10n.commonSend,
                              onTap: _send,
                            ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
                if (waiting.isNotEmpty) ...[
                  const SizedBox(height: 8),
                  Text.rich(
                    TextSpan(
                      children: [
                        ...withKey(
                          l10n.roomEnterToSend,
                          'Enter',
                          tx(c.fg2, size: 12, monospace: true),
                        ),
                        const TextSpan(text: '    '),
                        ...withKey(
                          l10n.roomEscToRemove,
                          'Esc',
                          tx(c.fg2, size: 12, monospace: true),
                        ),
                      ],
                    ),
                    style: tx(c.fg3, size: 12),
                  ),
                ],
              ],
            ),
          ),
        ),
      ],
    );
  }

  /// Whether entry [i] is a file that goes under the entry before it: the same person's,
  /// a few seconds apart.
  static bool _under(List<_Entry> entries, int i) {
    if (i == 0 || entries[i].file == null) return false;
    final who = entries[i].who;
    final before = entries[i - 1];
    return who != null &&
        before.who == who &&
        (entries[i].seen - before.seen).abs() <= 10000;
  }

  /// What [_entriesFor] last made, and from what.
  List<_Entry> _entries = const [];
  Object? _entriesKey;

  /// Everything in the chat in the order it reached this device; lines that came together
  /// (the room's past, on entering) keep their senders' order among them. While searching,
  /// only the messages and files that contain the words, as they are drawn. Made again
  /// only when the chat, the notes or the search change, not on each build.
  List<_Entry> _entriesFor(
    List<UiChatLine>? chat,
    RoomMemory? memory,
    RegExp? pattern,
  ) {
    final notes = memory?.notes;
    final key = (
      chat,
      chat?.length,
      memory,
      notes?.length,
      notes?.lastOrNull,
      memory?.files.length,
      pattern?.pattern,
    );
    if (key == _entriesKey) return _entries;
    final entries = <_Entry>[];
    for (final line in chat ?? const <UiChatLine>[]) {
      if (pattern != null && !pattern.hasMatch(shownText(line.text))) {
        continue;
      }
      entries.add(
        _Entry(
          memory?.seenAt(line) ?? line.timestampMs,
          line.timestampMs,
          line: line,
          who: line.sender,
        ),
      );
    }
    for (final note in notes ?? const <RoomNote>[]) {
      final id = note.file;
      if (id == null) {
        // Who came and went shows only while not searching.
        if (pattern == null) {
          entries.add(_Entry(note.atMs, note.atMs, note: note));
        }
        continue;
      }
      final seen = memory!.files[id];
      if (seen == null) continue;
      if (pattern != null && !pattern.hasMatch(seen.name)) continue;
      entries.add(
        _Entry(
          note.atMs,
          seen.sharedMs ?? note.atMs,
          file: seen,
          who: seen.byId,
        ),
      );
    }
    // Sorted keeping the order they were listed in where both times are the same, as
    // lines that came together have: a plain sort may swap those in a long chat.
    final order = {for (final (i, e) in entries.indexed) e: i};
    entries.sort(
      (a, b) => a.seen != b.seen
          ? a.seen.compareTo(b.seen)
          : a.time != b.time
          ? a.time.compareTo(b.time)
          : order[a]!.compareTo(order[b]!),
    );
    _entriesKey = key;
    return _entries = entries;
  }

  /// Down to the newest line. A list built as it scrolls knows its whole length only
  /// once its end is built, so it looks again after each jump.
  void _toEnd([int tries = 4]) {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_scroll.hasClients) return;
      final position = _scroll.position;
      if (position.pixels >= position.maxScrollExtent) return;
      _scroll.jumpTo(position.maxScrollExtent);
      if (tries > 1) _toEnd(tries - 1);
    });
  }
}

/// Something in the chat, before it is drawn: a message, a file or a note, and when it
/// reached this device.
class _Entry {
  _Entry(this.seen, this.time, {this.line, this.note, this.file, this.who});
  final int seen;
  final int time;
  final UiChatLine? line;
  final RoomNote? note;
  final SeenFile? file;

  /// Who wrote or shared it; none for a note.
  final String? who;

  /// Which entry it is, the same from one build to the next: the line's or the file's
  /// id, or the note itself (a room's memory keeps its notes).
  Object get id => switch (this) {
    _Entry(line: final line?) => ('line', line.id),
    _Entry(file: final file?) => ('file', file.id),
    _ => note!,
  };
}

class _Message extends StatelessWidget {
  const _Message({
    required this.person,
    required this.time,
    required this.child,
  });
  final UiPerson person;
  final int time;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final state = AppScope.read(context);
    // The writer's picture and name open their card: a friend's, the person's own, or a
    // member's that offers a friend request.
    Widget toCard(Widget child) => Builder(
      builder: (context) => Pressable(
        onTap: () => togglePersonCard(
          context,
          person.id,
          group: state.roomState.roomGroup,
        ),
        label: personCardLabel(state, person),
        builder: (_, _) => child,
      ),
    );
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        toCard(Avatar(avatar: person.avatar, size: 24)),
        const SizedBox(width: 10),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                crossAxisAlignment: CrossAxisAlignment.baseline,
                textBaseline: TextBaseline.alphabetic,
                children: [
                  Flexible(
                    child: toCard(
                      Text(
                        person.name,
                        overflow: TextOverflow.ellipsis,
                        style: tx(c.fg, weight: w500),
                      ),
                    ),
                  ),
                  const SizedBox(width: 8),
                  Text(
                    clock(time),
                    style: tx(c.fg3, size: 11, monospace: true),
                  ),
                ],
              ),
              const SizedBox(height: 2),
              child,
            ],
          ),
        ),
      ],
    );
  }
}
