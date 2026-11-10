part of '../direct.dart';

// The box to write in, with the files waiting to go over it; the note that the chat's
// history here was altered; and what shows instead of the box once the two are no longer
// friends.

class _Composer extends StatefulWidget {
  const _Composer({super.key, required this.page});
  final UiDirectPage page;

  @override
  State<_Composer> createState() => _ComposerState();
}

class _ComposerState extends State<_Composer> {
  final _text = TextEditingController();
  final _focus = FocusNode();
  int _lastWaiting = 0;

  late final AppState _app;

  /// Tells the friend this device is typing while the message box changes, while both
  /// are online.
  late final _typing = TypingSender(
    (on) => _app.direct.typing(
      widget.page.friend.id,
      on && writeBlocked(_app, widget.page) == null,
    ),
  );

  @override
  void initState() {
    super.initState();
    _app = AppScope.read(context);
    // What was being written in this chat before another was opened.
    final draft = _app.direct.drafts[widget.page.friend.id];
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
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _focus.requestFocus();
    });
  }

  @override
  void dispose() {
    _typing.stopped();
    _app.direct.voice.leftChat(widget.page.friend.id);
    _text.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _keepDraft() {
    final drafts = _app.direct.drafts;
    final friend = widget.page.friend.id;
    if (_text.text.isEmpty) {
      drafts.remove(friend);
    } else {
      drafts[friend] = _text.text;
    }
  }

  Future<void> _send() async {
    final app = AppScope.read(context);
    if (writeBlocked(app, widget.page) != null) return;
    final direct = app.direct;
    final text = _text.text;
    if (text.trim().isEmpty && direct.waiting.isEmpty) return;
    _text.clear();
    _focus.requestFocus();
    final sent = await direct.send(text);
    // It did not go: what was written is back in the box, unless more was written since.
    if (!sent && mounted && _text.text.trim().isEmpty) {
      final kept = direct.drafts[widget.page.friend.id] ?? text;
      _text.value = TextEditingValue(
        text: kept,
        selection: TextSelection.collapsed(offset: kept.length),
      );
    }
  }

  /// Esc with files waiting: asks before taking them all out of the message.
  Future<void> _removeWaiting(int count) async {
    final direct = AppScope.read(context).direct;
    final friend = direct.open;
    final ok = await confirmDialog(
      context,
      title: l10n.directRemoveFilesTitle(count),
      text: l10n.directRemoveFilesText,
      action: l10n.commonRemove,
    );
    if (ok && direct.open == friend) direct.clearOutgoing();
    if (mounted) _focus.requestFocus();
  }

  Future<void> _pick() async {
    final direct = AppScope.read(context).direct;
    final files = await openFiles();
    if (files.isEmpty) return;
    direct.addOutgoing(files.map((f) => f.path));
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.only(context, directParts);
    final c = context.fg;
    final page = widget.page;
    final waiting = state.direct.waiting;
    final going = state.direct.sendingHere;
    final files = going.isNotEmpty || waiting.isNotEmpty;
    if (waiting.length > _lastWaiting) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) _focus.requestFocus();
      });
    }
    _lastWaiting = waiting.length;
    // Nothing can be written while either of the two is offline: what was written stays
    // in the box, and comes back with it.
    final blocked = writeBlocked(state, page);
    final voice = state.direct.voice;
    if (blocked != null) {
      // Recording stops with the microphone off; the recording waits for both online.
      if (voice.recordingFor == page.friend.id) {
        WidgetsBinding.instance.addPostFrameCallback(
          (_) => voice.pauseRecording(),
        );
      }
      return Padding(
        padding: const EdgeInsets.fromLTRB(24, 0, 24, 18),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            const SizedBox(height: 18),
            Container(
              key: const ValueKey('direct-locked'),
              height: 46,
              padding: const EdgeInsets.symmetric(horizontal: 16),
              decoration: BoxDecoration(
                color: c.bg2,
                borderRadius: BorderRadius.circular(999),
                border: Border.all(color: c.border),
              ),
              child: Row(
                children: [
                  FgIcon(Ic.lock, size: 14, color: c.fg3),
                  const SizedBox(width: 9),
                  Expanded(
                    child: Text(
                      blocked,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: tx(c.fg2, size: 13),
                    ),
                  ),
                ],
              ),
            ),
            if (waiting.isNotEmpty || _text.text.trim().isNotEmpty)
              Padding(
                padding: const EdgeInsets.fromLTRB(4, 6, 4, 0),
                child: Text(l10n.directDraftKept, style: tx(c.fg3, size: 11)),
              ),
          ],
        ),
      );
    }
    return Padding(
      padding: const EdgeInsets.fromLTRB(24, 0, 24, 18),
      child: CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.escape): () {
            if (voice.recordingIn(page.friend.id)) {
              voice.escape();
            } else if (waiting.isNotEmpty) {
              _removeWaiting(waiting.length);
            }
          },
        },
        child: ListenableBuilder(
          listenable: voice,
          builder: (context, _) => _box(context, voice, waiting, going, files),
        ),
      ),
    );
  }

  /// The box to write in, or to record in, with what goes over and under it.
  Widget _box(
    BuildContext context,
    VoiceMessages voice,
    List<OutgoingFile> waiting,
    List<OutgoingFile> going,
    bool files,
  ) {
    final state = AppScope.read(context);
    final c = context.fg;
    final page = widget.page;
    final name = page.friend.name;
    final recording = voice.recordingIn(page.friend.id);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (voice.micProblem && !recording) const MicProblemNote(),
        if (recording && voice.micStopped) const MicProblemNote(stopped: true),
        ValueListenableBuilder(
          valueListenable: state.typingChanged,
          builder: (context, _, _) => TypingLine(
            names: [if (state.direct.typingHere(page.friend.id)) name],
            padding: const EdgeInsets.only(left: 4, bottom: 2),
          ),
        ),
        AnimatedContainer(
          duration: const Duration(milliseconds: 200),
          decoration: BoxDecoration(
            color: c.bg2,
            borderRadius: BorderRadius.circular(files ? 22 : 999),
            // The box to write in is a control: its edge is seen at 3:1.
            border: Border.all(color: c.edge),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (files) ...[
                Padding(
                  padding: const EdgeInsets.fromLTRB(10, 8, 10, 8),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    spacing: 6,
                    children: [
                      for (final f in going) _Waiting(file: f, sending: true),
                      for (final f in waiting) _Waiting(file: f),
                    ],
                  ),
                ),
                Container(height: 1, color: c.border),
              ],
              Padding(
                padding: const EdgeInsets.fromLTRB(6, 5, 5, 5),
                child: recording
                    ? RecordingBar(friend: page.friend.id)
                    : Row(
                        children: [
                          Springy(
                            onTap: _pick,
                            tooltip: l10n.directAddFilesTooltip,
                            label: l10n.directAddFiles,
                            builder: (context, hovered) => Container(
                              width: 34,
                              height: 34,
                              alignment: Alignment.center,
                              decoration: BoxDecoration(
                                color: hovered
                                    ? Color.lerp(c.hover, c.fg, .06)
                                    : c.hover,
                                shape: BoxShape.circle,
                              ),
                              child: FgIcon(Ic.plus, size: 15, color: c.fg2),
                            ),
                          ),
                          const SizedBox(width: 8),
                          Expanded(
                            child: EmojiComplete(
                              controller: _text,
                              max: maxMessage,
                              child: FgField(
                                key: const ValueKey('direct-message'),
                                controller: _text,
                                focusNode: _focus,
                                height: 34,
                                borderless: true,
                                placeholder: waiting.isEmpty
                                    ? l10n.directMessagePlaceholder(name)
                                    : l10n.directAddMessageOptional,
                                onSubmitted: (_) => _send(),
                                maxLength: maxMessage,
                                label: l10n.directMessagePlaceholder(name),
                              ),
                            ),
                          ),
                          ValueListenableBuilder(
                            valueListenable: _text,
                            builder: (context, value, _) {
                              final length = value.text.runes.length;
                              if (length < maxMessage - 200) {
                                return const SizedBox.shrink();
                              }
                              return Padding(
                                padding: const EdgeInsets.only(
                                  left: 6,
                                  right: 4,
                                ),
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
                          const SizedBox(width: 4),
                          EmojiButton(
                            controller: _text,
                            focusNode: _focus,
                            max: maxMessage,
                          ),
                          const SizedBox(width: 4),
                          MicButton(friend: page.friend.id),
                          const SizedBox(width: 4),
                          Springy(
                            key: const ValueKey('direct-send'),
                            onTap: _send,
                            tooltip: l10n.commonSend,
                            label: l10n.commonSend,
                            builder: (context, hovered) => Container(
                              width: 34,
                              height: 34,
                              alignment: Alignment.center,
                              decoration: BoxDecoration(
                                color: hovered
                                    ? c.pri.withValues(alpha: .88)
                                    : c.pri,
                                shape: BoxShape.circle,
                              ),
                              child: FgIcon(
                                Ic.send,
                                size: 14,
                                stroke: 2.4,
                                color: c.onPri,
                              ),
                            ),
                          ),
                        ],
                      ),
              ),
            ],
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(4, 6, 4, 0),
          child: recording
              ? RecordingUnder(
                  microphone: voice.micName,
                  fellBack: voice.fellBack,
                  escDeletes: voice.escDeletes,
                )
              : Text(l10n.directAutoDownloadNote, style: tx(c.fg3, size: 11)),
        ),
      ],
    );
  }
}

/// A file waiting for Send above the message box, with a button to take it out; or, once
/// sent, saying so until the message shows in the chat.
class _Waiting extends StatelessWidget {
  const _Waiting({required this.file, this.sending = false});
  final OutgoingFile file;
  final bool sending;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    return Row(
      children: [
        FgIcon(Ic.file, size: 15, color: c.fg2),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            file.name,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: tx(c.fg, size: 13),
          ),
        ),
        const SizedBox(width: 8),
        Text(bytes(file.size), style: tx(c.fg3, size: 12, monospace: true)),
        const SizedBox(width: 4),
        if (sending)
          SizedBox(
            height: 26,
            child: Center(
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 6),
                child: Text(l10n.directSending, style: tx(c.fg3, size: 12)),
              ),
            ),
          )
        else
          FgButton(
            icon: Ic.close,
            kind: Kind.ghost,
            height: 26,
            tooltip: l10n.directRemoveFile(file.name),
            onTap: () => state.direct.removeOutgoing(file),
          ),
      ],
    );
  }
}

/// Instead of the message box, once the two are no longer friends.
/// The chat's history on this computer was not as Fogata last saved it: changed, cut
/// short or put back as an older copy, by a disk fault or by someone with the files.
class _Altered extends StatelessWidget {
  const _Altered();

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Container(
      key: const ValueKey('direct-altered'),
      margin: const EdgeInsets.fromLTRB(24, 12, 24, 0),
      padding: const EdgeInsets.fromLTRB(14, 10, 14, 10),
      decoration: BoxDecoration(
        color: c.amberSoft,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: c.amberLine),
      ),
      child: Row(
        children: [
          FgIcon(Ic.warning, size: 16, color: c.amber),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              l10n.directAltered,
              style: tx(c.fg2, size: 13, height: 1.4),
            ),
          ),
        ],
      ),
    );
  }
}

class _Ended extends StatelessWidget {
  const _Ended({required this.page});
  final UiDirectPage page;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    final name = page.friend.name;
    return Container(
      margin: const EdgeInsets.fromLTRB(24, 8, 24, 18),
      padding: const EdgeInsets.fromLTRB(16, 10, 8, 10),
      decoration: BoxDecoration(
        color: c.bg2,
        borderRadius: BorderRadius.circular(22),
        border: Border.all(color: c.border),
      ),
      child: Row(
        children: [
          Expanded(
            child: Text(l10n.directEnded(name), style: tx(c.fg2, size: 13)),
          ),
          const SizedBox(width: 12),
          FgButton(
            key: const ValueKey('direct-delete'),
            text: l10n.directDeleteChat,
            icon: Ic.trash,
            kind: Kind.danger,
            height: 32,
            fontSize: 13,
            onTap: () async {
              final yes = await showFgDialog<bool>(
                context,
                maxWidth: 420,
                builder: (context) => _ConfirmDelete(name: name),
              );
              if (yes ?? false) await state.direct.delete(page.friend.id);
            },
          ),
        ],
      ),
    );
  }
}

class _ConfirmDelete extends StatelessWidget {
  const _ConfirmDelete({required this.name});
  final String name;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                l10n.directDeleteTitle(name),
                style: tx(c.fg, size: 16, weight: w600),
              ),
              const SizedBox(height: 6),
              Text(l10n.directDeleteText(name), style: tx(c.fg2, size: 13)),
            ],
          ),
        ),
        Footer(
          children: [
            const Spacer(),
            FgButton(
              text: l10n.commonCancel,
              height: 32,
              fontSize: 13,
              onTap: () => Navigator.of(context).pop(false),
            ),
            const SizedBox(width: 8),
            FgButton(
              text: l10n.directDelete,
              kind: Kind.danger,
              height: 32,
              fontSize: 13,
              onTap: () => Navigator.of(context).pop(true),
            ),
          ],
        ),
      ],
    );
  }
}
