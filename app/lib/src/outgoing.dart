import 'dart:async';
import 'dart:io';
import 'dart:typed_data';

import 'package:desktop_drop/desktop_drop.dart';
import 'package:flutter/material.dart';

import 'app_state.dart';
import 'format.dart';
import 'icons.dart';
import 'l10n.dart';
import 'photo.dart' show readAtMost;
import 'private_places.dart';
import 'safe_image.dart';
import 'theme.dart';
import 'typing.dart' show styledSpans;
import 'widgets.dart';

/// Where files dropped on a [DropZone] go: what the zone says while they are dragged over
/// it, and how they are added.
class DropTo {
  const DropTo({
    required this.title,
    required this.note,
    required this.leftOut,
    required this.at,
    required this.add,
  });

  /// The room's chat: the files wait there for Send, with a message.
  factory DropTo.room(AppState state) => DropTo(
    title: l10n.outgoingDropTitle(
      state.contacts.group(state.roomState.roomGroup)?.name ??
          l10n.outgoingTheRoom,
    ),
    note: l10n.outgoingDropNote,
    leftOut: l10n.outgoingPrivateLeftOut,
    at: () => state.roomState.roomGroup,
    add: state.addOutgoing,
  );

  /// The direct chat shown, with the friend called [name]: the files wait in its message
  /// box.
  factory DropTo.direct(AppState state, String name) => DropTo(
    title: l10n.directDropTitle(name),
    note: l10n.directDropNote,
    leftOut: l10n.directPrivateLeftOut,
    at: () => state.direct.open,
    add: state.direct.addOutgoing,
  );

  /// The big title over what files are dragged on.
  final String title;

  /// What happens to them, under the title.
  final String note;

  /// The error said when `n` hidden or private files were left out of a drop.
  final String Function(int n) leftOut;

  /// The chat the files go to now. Looking at a drop's files takes a moment: when this
  /// changed meanwhile, the drop is not added to the chat shown since.
  final Object? Function() at;

  /// Adds the files to that chat.
  final Future<void> Function(List<String> paths) add;
}

/// Files dragged over a chat (the room, or a direct chat): it dims and says where they
/// go. Dropped, they wait in the chat for Send, with a message.
class DropZone extends StatefulWidget {
  const DropZone({
    super.key,
    required this.state,
    required this.enabled,
    required this.child,
    this.to,
  });
  final AppState state;
  final bool enabled;
  final Widget child;

  /// Where the files go: the room when null.
  final DropTo? to;

  @override
  State<DropZone> createState() => _DropZoneState();
}

class _DropZoneState extends State<DropZone> {
  bool _over = false;

  /// The drops being added, one after the other, so they wait in the order they came.
  Future<void> _adding = Future.value();

  DropTo get _to => widget.to ?? DropTo.room(widget.state);

  /// Adds what was dropped, after the drops before it.
  void _add(List<String> paths) {
    if (paths.isEmpty) return;
    final to = _to;
    final at = to.at();
    _adding = _adding.then((_) => _addTo(to, at, paths)).catchError((_) {});
  }

  /// Adds what was dropped on the chat [at]. A web page can start a drag whose list names
  /// any file of this computer, whatever the browser, so those that are hidden or in a
  /// private place ([privatePaths], which also follows links and `..`) are never added by
  /// dragging: the rest is, and the person is told how to share such a file on purpose
  /// (picked with the chat's own button, which no page can do for them).
  Future<void> _addTo(DropTo to, Object? at, List<String> paths) async {
    final private = (await privatePaths(paths)).toSet();
    // The chat may have changed meanwhile.
    if (!mounted || to.at() != at) return;
    if (private.isNotEmpty) {
      widget.state.say(to.leftOut(private.length), error: true);
    }
    final chosen = [
      for (final p in paths)
        if (!private.contains(p)) p,
    ];
    if (chosen.isNotEmpty) await to.add(chosen);
  }

  @override
  Widget build(BuildContext context) {
    final to = _to;
    return DropTarget(
      enable: widget.enabled,
      onDragEntered: (_) => setState(() => _over = true),
      onDragExited: (_) => setState(() => _over = false),
      onDragDone: (details) {
        setState(() => _over = false);
        // Files of this computer only: see readOutgoing.
        _add([
          for (final f in details.files)
            if (f.path.isNotEmpty && File(f.path).isAbsolute) f.path,
        ]);
      },
      child: Stack(
        fit: StackFit.expand,
        children: [
          widget.child,
          if (_over)
            Positioned.fill(
              child: IgnorePointer(
                child: ColoredBox(
                  color: const Color(0xB8000000),
                  child: Center(child: _DropCard(to: to)),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

class _DropCard extends StatelessWidget {
  const _DropCard({required this.to});
  final DropTo to;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Dashed(
      color: c.blue,
      radius: 16,
      width: 2,
      child: Container(
        width: 560,
        padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 40),
        decoration: BoxDecoration(
          color: c.bg1,
          borderRadius: BorderRadius.circular(16),
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(
              width: 64,
              height: 64,
              decoration: BoxDecoration(
                color: c.blueSoft,
                borderRadius: BorderRadius.circular(16),
              ),
              child: Center(child: FgIcon(Ic.upload, size: 30, color: c.blue)),
            ),
            const SizedBox(height: 14),
            Text(
              to.title,
              textAlign: TextAlign.center,
              style: tx(c.fg, size: 22, weight: w600, spacing: -.2),
            ),
            const SizedBox(height: 14),
            ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 440),
              child: Text(
                to.note,
                textAlign: TextAlign.center,
                style: tx(c.fg2),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

const _count = '\u{E000}', _size = '\u{E001}';

/// The files waiting for Send, above the message box: each with its size, a small
/// picture for images, and a button to take it out.
class OutgoingTray extends StatelessWidget {
  const OutgoingTray({super.key, required this.state, required this.files});
  final AppState state;
  final List<OutgoingFile> files;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final total = files.fold<int>(0, (n, f) => n + f.size);
    final count = tx(c.fg, size: 12, monospace: true);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            const SizedBox(width: 4),
            Expanded(
              child: Text.rich(
                TextSpan(
                  children: styledSpans(
                    l10n.outgoingToShare(files.length, _count, _size),
                    {
                      _count: TextSpan(text: '${files.length}', style: count),
                      _size: TextSpan(text: bytes(total), style: count),
                    },
                  ),
                ),
                style: tx(c.fg2, size: 12),
              ),
            ),
            FgButton(
              text: l10n.outgoingRemoveAll,
              kind: Kind.ghost,
              height: 24,
              fontSize: 12,
              onTap: state.clearOutgoing,
            ),
          ],
        ),
        for (final f in files) ...[
          const SizedBox(height: 6),
          _OutgoingRow(state: state, file: f),
        ],
      ],
    );
  }
}

const _pictures = {'png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp'};

/// [folder] as the tray shows it: inside the person's home, from `~`; a long one by its
/// last two folders, since the line is cut at its end and where the path ends is what
/// tells it apart (`…/home/.ssh`).
String shownFolder(String folder, {Map<String, String>? environment}) {
  final env = environment ?? Platform.environment;
  final home = Platform.isWindows ? env['USERPROFILE'] : env['HOME'];
  final sep = Platform.pathSeparator;
  var shown = folder;
  if (home != null && home.length >= 2) {
    final base = home.endsWith(sep) ? home.substring(0, home.length - 1) : home;
    if (folder == base) {
      shown = '~';
    } else if (folder.startsWith('$base$sep')) {
      shown = '~${folder.substring(base.length)}';
    }
  }
  final parts = shown.split(sep);
  if (parts.length > 4) {
    return '…$sep${parts.sublist(parts.length - 2).join(sep)}';
  }
  return shown;
}

class _OutgoingRow extends StatelessWidget {
  const _OutgoingRow({required this.state, required this.file});
  final AppState state;
  final OutgoingFile file;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final kind = file.name.contains('.')
        ? file.name.split('.').last.toLowerCase()
        : '';
    final icon = ColoredBox(
      color: c.hover,
      child: Center(child: FgIcon(Ic.file, size: 18, color: c.fg2)),
    );
    return Container(
      height: 48,
      padding: const EdgeInsets.all(6),
      decoration: BoxDecoration(
        color: c.bg2,
        borderRadius: BorderRadius.circular(6),
        border: Border.all(color: c.border),
      ),
      child: Row(
        children: [
          ClipRRect(
            borderRadius: BorderRadius.circular(4),
            child: SizedBox(
              width: 36,
              height: 36,
              child: _pictures.contains(kind)
                  ? _Thumb(file: file, fallback: icon)
                  : icon,
            ),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Text(
                  file.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(c.fg, size: 13, weight: w500, height: 1.4),
                ),
                // Where it is, so one dropped from somewhere unexpected is seen
                // before it goes.
                Text.rich(
                  TextSpan(
                    children: [
                      TextSpan(
                        text: bytes(file.size),
                        style: tx(
                          c.fg3,
                          size: 11,
                          monospace: true,
                          height: 1.4,
                        ),
                      ),
                      if (file.folder.isNotEmpty)
                        TextSpan(text: ' · ${shownFolder(file.folder)}'),
                    ],
                  ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(c.fg3, size: 11, height: 1.4),
                ),
              ],
            ),
          ),
          FgButton(
            icon: Ic.close,
            kind: Kind.ghost,
            height: 28,
            tooltip: l10n.outgoingRemoveFile(file.name),
            onTap: () => state.removeOutgoing(file),
          ),
        ],
      ),
    );
  }
}

/// The small picture of a file waiting to go: read only when no larger than a room's
/// previews, and decoded only if its header promises no more pixels than a picture shared
/// in a room may have, as for every other picture ([safePicture]). [fallback] until then,
/// and for good otherwise.
class _Thumb extends StatefulWidget {
  const _Thumb({required this.file, required this.fallback});
  final OutgoingFile file;
  final Widget fallback;

  @override
  State<_Thumb> createState() => _ThumbState();
}

class _ThumbState extends State<_Thumb> {
  Uint8List? _bytes;

  @override
  void initState() {
    super.initState();
    _read();
  }

  @override
  void didUpdateWidget(_Thumb old) {
    super.didUpdateWidget(old);
    if (old.file.path != widget.file.path) {
      _bytes = null;
      _read();
    }
  }

  Future<void> _read() async {
    final file = widget.file;
    if (file.size > RoomState.maxPreviewBytes) return;
    final Uint8List? bytes;
    try {
      // It may have grown since it was dropped: reading stops at the limit.
      bytes = await readAtMost(
        File(file.path).openRead(0, RoomState.maxPreviewBytes + 1),
        RoomState.maxPreviewBytes,
      );
    } catch (_) {
      return;
    }
    if (bytes == null) return;
    if (mounted && widget.file.path == file.path) {
      setState(() => _bytes = bytes);
    }
  }

  @override
  Widget build(BuildContext context) {
    final bytes = _bytes;
    if (bytes == null) return widget.fallback;
    return Image(
      image: safePicture(
        bytes,
        // Twice the drawn width in physical pixels: enough for the shorter side of most
        // pictures to cover the square.
        width: (72 * MediaQuery.devicePixelRatioOf(context)).ceil(),
        maxPixels: maxPicturePixels,
      ),
      fit: BoxFit.cover,
      errorBuilder: (_, _, _) => widget.fallback,
    );
  }
}
