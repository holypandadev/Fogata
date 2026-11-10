part of '../room.dart';

// The room's files: their cards in the chat, the files tab, and pictures and videos
// shown over the window.

/// Where a file stands for this device.
UiFile? _current(AppState state, String id) {
  for (final f in state.roomState.room?.files ?? const <UiFile>[]) {
    if (f.id == id) return f;
  }
  return null;
}

String _holders(AppState state, UiFile f) {
  final me = state.contacts.profile?.person.id;
  final names = [
    for (final h in f.holders)
      if (h.id != me) h.name,
  ];
  if (names.isEmpty) return f.sharedBy.name;
  if (names.length == 1) return names.first;
  if (names.length == 2) return l10n.roomOrTwo(names.first, names.last);
  return l10n.roomOrMore(
    names.sublist(0, names.length - 1).join(l10n.listSeparator),
    names.last,
  );
}

class _FileCard extends StatelessWidget {
  const _FileCard({required this.state, required this.seen});
  final AppState state;
  final SeenFile seen;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final f = _current(state, seen.id);
    if (f == null) {
      return Dashed(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
          child: Row(
            children: [
              FgIcon(Ic.file, size: 16, color: c.fg3),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      seen.name,
                      overflow: TextOverflow.ellipsis,
                      style: tx(
                        c.fg3,
                        size: 13,
                        decoration: TextDecoration.lineThrough,
                      ),
                    ),
                    Text(
                      l10n.roomFileNotAvailable(
                        state.nameOf(seen.byId, seen.byName),
                      ),
                      style: tx(c.fg3, size: 12),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      );
    }
    final me = state.contacts.profile?.person.id;
    final mine = f.mine;
    final downloading = f.downloading;
    final size = bytes(f.size);
    final path = f.path;
    final open = path != null && downloading == null
        ? () => showInFolderOrSay(path, state.say)
        : null;
    final Widget footer;
    if (downloading != null) {
      final pct = f.size == 0 ? 0 : (downloading * 100 ~/ f.size);
      footer = Row(
        children: [
          Expanded(
            child: ClipRRect(
              borderRadius: BorderRadius.circular(2),
              child: LinearProgressIndicator(
                value: pct / 100,
                minHeight: 4,
                backgroundColor: c.hover,
                color: c.blueSolid,
              ),
            ),
          ),
          const SizedBox(width: 8),
          Text(
            l10n.volumePercent(pct),
            style: tx(c.fg2, size: 12, monospace: true),
          ),
        ],
      );
    } else if (mine) {
      footer = Row(
        children: [
          FgIcon(Ic.upload, size: 12, color: c.blue),
          const SizedBox(width: 6),
          Text(l10n.roomSharingFromHere, style: tx(c.blue, size: 12)),
        ],
      );
    } else {
      footer = Row(
        children: [
          Dot(c.green),
          const SizedBox(width: 6),
          Expanded(
            child: Text(
              l10n.roomAvailableWhile(_holders(state, f)),
              style: tx(c.green, size: 12),
            ),
          ),
        ],
      );
    }
    return Container(
      clipBehavior: Clip.antiAlias,
      // Inside the outline, as when it was drawn under what is inside.
      padding: const EdgeInsets.all(1),
      decoration: BoxDecoration(
        color: c.bg2,
        borderRadius: BorderRadius.circular(8),
      ),
      // Over what is inside, which would cover it at the rounded corners.
      foregroundDecoration: BoxDecoration(
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: c.border),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
            child: Row(
              children: [
                Expanded(
                  // Once the file is on this computer, clicking it shows it in its folder.
                  child: Pressable(
                    onTap: open,
                    builder: (context, hovered) => Row(
                      children: [
                        Container(
                          width: 32,
                          height: 32,
                          alignment: Alignment.center,
                          decoration: BoxDecoration(
                            color: c.bg1,
                            borderRadius: BorderRadius.circular(6),
                            border: Border.all(color: c.border),
                          ),
                          child: FgIcon(Ic.file, size: 15, color: c.fg),
                        ),
                        const SizedBox(width: 10),
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(
                                f.name,
                                overflow: TextOverflow.ellipsis,
                                style: tx(
                                  c.fg,
                                  size: 13,
                                  weight: w500,
                                  decoration: hovered
                                      ? TextDecoration.underline
                                      : null,
                                ),
                              ),
                              Text(
                                f.sharedBy.id == me && f.sent > 0
                                    ? l10n.roomSizeDownloads(size, f.sent)
                                    : size,
                                style: tx(c.fg3, size: 11, monospace: true),
                              ),
                            ],
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
                if (!mine && downloading == null)
                  FgButton(
                    icon: Ic.download,
                    height: 30,
                    tooltip: l10n.roomDownloadFile(f.name),
                    onTap: () => state.download(f.id),
                  )
                else if (open != null)
                  FgButton(
                    icon: Ic.folder,
                    height: 30,
                    tooltip: l10n.roomShowInFolder(f.name),
                    onTap: open,
                  ),
              ],
            ),
          ),
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 7),
            decoration: BoxDecoration(
              border: Border(top: BorderSide(color: c.border)),
            ),
            child: footer,
          ),
        ],
      ),
    );
  }
}

/// A picture shared in the room, shown in the chat: fetched by itself (see
/// [AppState.previews]) and opened large with a click. One too big, or gone before it
/// arrived, shows as a file.
class _PictureCard extends StatelessWidget {
  const _PictureCard({required this.state, required this.seen});
  final AppState state;
  final SeenFile seen;

  static const _maxHeight = 240.0;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final picture = state.roomState.previews[seen.id];
    final f = _current(state, seen.id);
    if (picture == null) {
      if (f == null ||
          f.size > RoomState.maxPreviewBytes ||
          state.roomState.noPreview.contains(seen.id)) {
        return _FileCard(state: state, seen: seen);
      }
      return Container(
        height: 150,
        decoration: BoxDecoration(
          color: c.bg2,
          borderRadius: BorderRadius.circular(14),
          border: Border.all(color: c.border),
        ),
        alignment: Alignment.center,
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Spinner(size: 14, color: c.fg3),
            const SizedBox(width: 8),
            Text(
              l10n.roomGettingFile(seen.name),
              overflow: TextOverflow.ellipsis,
              style: tx(c.fg3, size: 12),
            ),
          ],
        ),
      );
    }
    final dpr = MediaQuery.devicePixelRatioOf(context);
    final caption = '${seen.name} · ${bytes(seen.size)}';
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        LayoutBuilder(
          builder: (context, box) => ConstrainedBox(
            constraints: BoxConstraints(
              maxWidth: box.maxWidth,
              maxHeight: _maxHeight,
            ),
            child: _Hovered(
              builder: (hovered) => Stack(
                children: [
                  Pressable(
                    onTap: () => _showPicture(context, state, seen),
                    label: l10n.roomOpenFile(seen.name),
                    focusRadius: 14,
                    builder: (context, _) => ClipRRect(
                      borderRadius: BorderRadius.circular(14),
                      child: AnimatedScale(
                        scale: hovered ? 1.02 : 1,
                        duration: springTime,
                        curve: spring,
                        child: ChatPicture(
                          bytes: picture,
                          still: Image(
                            image: safePicture(
                              picture,
                              width: (box.maxWidth * dpr).round(),
                              maxPixels: maxPicturePixels,
                            ),
                            gaplessPlayback: true,
                            frameBuilder: (context, child, frame, sync) =>
                                AnimatedOpacity(
                                  opacity: frame == null ? 0 : 1,
                                  duration: const Duration(milliseconds: 200),
                                  child: child,
                                ),
                          ),
                        ),
                      ),
                    ),
                  ),
                  Positioned(
                    right: 8,
                    top: 8,
                    child: AnimatedOpacity(
                      opacity: hovered ? 1 : 0,
                      duration: const Duration(milliseconds: 200),
                      child: IgnorePointer(
                        ignoring: !hovered,
                        child: Springy(
                          onTap: () => state.savePicture(seen.id, seen.name),
                          tooltip: l10n.roomSaveToDownloads,
                          builder: (context, over) => Container(
                            width: 30,
                            height: 30,
                            alignment: Alignment.center,
                            decoration: BoxDecoration(
                              shape: BoxShape.circle,
                              color: Color.fromRGBO(
                                16,
                                16,
                                16,
                                over ? .9 : .72,
                              ),
                              border: Border.all(
                                color: Colors.white.withValues(alpha: .1),
                              ),
                            ),
                            child: const FgIcon(
                              Ic.download,
                              size: 14,
                              color: Colors.white,
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                  Positioned(
                    left: 8,
                    bottom: 8,
                    right: 46,
                    child: IgnorePointer(
                      child: AnimatedOpacity(
                        opacity: hovered ? 1 : 0,
                        duration: const Duration(milliseconds: 200),
                        child: Align(
                          alignment: Alignment.centerLeft,
                          child: Container(
                            height: 22,
                            padding: const EdgeInsets.symmetric(horizontal: 8),
                            alignment: Alignment.center,
                            decoration: BoxDecoration(
                              color: const Color.fromRGBO(16, 16, 16, .72),
                              borderRadius: BorderRadius.circular(999),
                            ),
                            child: Text(
                              caption,
                              overflow: TextOverflow.ellipsis,
                              style: tx(Colors.white, size: 11, height: 1.2),
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
        if (f == null) ...[
          const SizedBox(height: 6),
          Text(
            l10n.roomPictureKept(state.nameOf(seen.byId, seen.byName)),
            style: tx(c.fg3, size: 11),
          ),
        ],
      ],
    );
  }
}

/// Builds its child knowing whether the pointer is over it.
class _Hovered extends StatefulWidget {
  const _Hovered({required this.builder});
  final Widget Function(bool hovered) builder;

  @override
  State<_Hovered> createState() => _HoveredState();
}

class _HoveredState extends State<_Hovered> {
  bool _on = false;

  @override
  Widget build(BuildContext context) => MouseRegion(
    onEnter: (_) => setState(() => _on = true),
    onExit: (_) => setState(() => _on = false),
    child: widget.builder(_on),
  );
}

/// A picture of the room large over the window: who sent it, saving it and closing it.
/// A click beside it or Esc closes it.
Future<void> _showPicture(BuildContext context, AppState state, SeenFile seen) {
  return showGeneralDialog<void>(
    context: context,
    barrierDismissible: true,
    barrierLabel: l10n.roomClosePicture,
    barrierColor: const Color.fromRGBO(0, 0, 0, .9),
    transitionDuration: springTime,
    pageBuilder: (context, _, _) => _PictureView(state: state, seen: seen),
    transitionBuilder: (context, animation, _, child) {
      final t = CurvedAnimation(parent: animation, curve: spring);
      return FadeTransition(
        opacity: animation,
        child: ScaleTransition(
          scale: Tween(begin: .94, end: 1.0).animate(t),
          child: child,
        ),
      );
    },
  );
}

class _PictureView extends StatelessWidget {
  const _PictureView({required this.state, required this.seen});
  final AppState state;
  final SeenFile seen;

  @override
  Widget build(BuildContext context) {
    final picture = state.roomState.previews[seen.id];
    final close = Navigator.of(context).pop;
    final sender = _person(state, seen.byId, seen.byName);
    // Decoded no larger than the screen it is shown on, in its pixels.
    var screen = View.of(context).display.size;
    if (screen.isEmpty) {
      screen =
          MediaQuery.sizeOf(context) * MediaQuery.devicePixelRatioOf(context);
    }
    return CallbackShortcuts(
      bindings: {const SingleActivator(LogicalKeyboardKey.escape): close},
      child: Focus(
        autofocus: true,
        child: Material(
          type: MaterialType.transparency,
          child: Stack(
            children: [
              Positioned.fill(
                child: GestureDetector(
                  behavior: HitTestBehavior.opaque,
                  onTap: close,
                ),
              ),
              Positioned.fill(
                top: 112,
                bottom: 40,
                left: 40,
                right: 40,
                child: Center(
                  child: picture == null
                      ? const SizedBox.shrink()
                      : ClipRRect(
                          borderRadius: BorderRadius.circular(16),
                          child: ChatPicture(
                            bytes: picture,
                            still: Image(
                              image: ResizeImage(
                                SafeMemoryImage(
                                  picture,
                                  maxPixels: maxPicturePixels,
                                ),
                                width: screen.width.round(),
                                height: screen.height.round(),
                                policy: ResizeImagePolicy.fit,
                              ),
                              fit: BoxFit.contain,
                              gaplessPlayback: true,
                            ),
                          ),
                        ),
                ),
              ),
              // Below the window's title bar.
              Positioned(
                left: 24,
                right: 24,
                top: 56,
                child: Row(
                  children: [
                    Avatar(avatar: sender.avatar, size: 32),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            sender.name,
                            style: tx(Colors.white, size: 14, weight: w600),
                          ),
                          Text(
                            '${seen.name} · ${bytes(seen.size)}',
                            overflow: TextOverflow.ellipsis,
                            style: tx(Colors.white60, size: 12),
                          ),
                        ],
                      ),
                    ),
                    Springy(
                      onTap: () => state.savePicture(seen.id, seen.name),
                      label: l10n.roomSaveFile,
                      builder: (context, hovered) => Container(
                        height: 36,
                        padding: const EdgeInsets.fromLTRB(13, 0, 16, 0),
                        decoration: BoxDecoration(
                          color: hovered
                              ? Colors.white
                              : const Color(0xFFEDEDED),
                          borderRadius: BorderRadius.circular(999),
                        ),
                        child: Row(
                          children: [
                            const FgIcon(
                              Ic.download,
                              size: 14,
                              color: Color(0xFF0A0A0A),
                            ),
                            const SizedBox(width: 7),
                            Text(
                              l10n.roomSaveFile,
                              style: tx(
                                const Color(0xFF0A0A0A),
                                size: 13,
                                weight: w600,
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                    const SizedBox(width: 8),
                    Springy(
                      onTap: close,
                      tooltip: l10n.commonClose,
                      builder: (context, hovered) => Container(
                        width: 36,
                        height: 36,
                        alignment: Alignment.center,
                        decoration: BoxDecoration(
                          shape: BoxShape.circle,
                          color: Colors.white.withValues(
                            alpha: hovered ? .18 : .1,
                          ),
                        ),
                        child: const FgIcon(
                          Ic.close,
                          size: 15,
                          color: Colors.white,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _Files extends StatelessWidget {
  const _Files({required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final me = state.contacts.profile?.person.id;
    final files = state.roomState.room?.files ?? const <UiFile>[];
    final memory = state.roomState.memories[state.roomState.roomGroup];
    final gone = [
      for (final s in memory?.files.values ?? const <SeenFile>[])
        if (!files.any((f) => f.id == s.id)) s,
    ];
    Widget heading(String text) => Padding(
      padding: const EdgeInsets.only(top: 6),
      child: Text(text, style: tx(c.fg3, size: 12, weight: w500)),
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Expanded(
          child: ListView(
            padding: const EdgeInsets.all(16),
            children: [
              Container(
                padding: const EdgeInsets.symmetric(
                  horizontal: 12,
                  vertical: 10,
                ),
                decoration: BoxDecoration(
                  color: c.bg2,
                  borderRadius: BorderRadius.circular(8),
                  border: Border.all(color: c.border),
                ),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    FgIcon(Ic.info, size: 16, color: c.fg2),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Text(
                        l10n.roomFilesNote,
                        style: tx(c.fg2, size: 13),
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 12),
              heading(l10n.roomFilesAvailable(files.length)),
              for (final f in files) ...[
                const SizedBox(height: 12),
                _FileRow(state: state, file: f, me: me),
              ],
              if (gone.isNotEmpty) ...[
                const SizedBox(height: 12),
                heading(l10n.roomFilesGone(gone.length)),
                for (final s in gone) ...[
                  const SizedBox(height: 12),
                  Dashed(
                    child: Padding(
                      padding: const EdgeInsets.all(12),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            s.name,
                            style: tx(
                              c.fg3,
                              size: 13,
                              decoration: TextDecoration.lineThrough,
                            ),
                          ),
                          Text.rich(
                            TextSpan(
                              children: [
                                TextSpan(
                                  text: l10n.roomPersonLeft(
                                    state.nameOf(s.byId, s.byName),
                                  ),
                                ),
                                if (s.goneMs != null) ...[
                                  const TextSpan(text: ' · '),
                                  TextSpan(
                                    text: clock(s.goneMs!),
                                    style: const TextStyle(fontFamily: mono),
                                  ),
                                ],
                              ],
                            ),
                            style: tx(c.fg3, size: 12),
                          ),
                        ],
                      ),
                    ),
                  ),
                ],
              ],
            ],
          ),
        ),
        Container(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 16),
          decoration: BoxDecoration(
            border: Border(top: BorderSide(color: c.border)),
          ),
          child: Pressable(
            onTap: () => pickToShare(context),
            focusRadius: 8,
            builder: (context, hovered) => Dashed(
              child: Container(
                padding: const EdgeInsets.all(14),
                decoration: BoxDecoration(
                  color: hovered ? c.hover : null,
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Column(
                  children: [
                    Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        FgIcon(Ic.plus, size: 15, color: c.fg),
                        const SizedBox(width: 8),
                        Text(
                          l10n.roomShareFiles,
                          style: tx(c.fg, weight: w500),
                        ),
                      ],
                    ),
                    const SizedBox(height: 2),
                    Text(l10n.roomPickOrDrop, style: tx(c.fg3, size: 12)),
                  ],
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class _FileRow extends StatelessWidget {
  const _FileRow({required this.state, required this.file, required this.me});
  final AppState state;
  final UiFile file;
  final String? me;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final f = file;
    final mineFirst = f.sharedBy.id == me;
    final downloading = f.downloading;
    final from = mineFirst
        ? l10n.roomFileYours
        : l10n.roomFileFrom(f.sharedBy.name);
    final Widget meta;
    final Widget? action;
    final Widget? note;
    if (downloading != null) {
      final pct = f.size == 0 ? 0 : downloading * 100 ~/ f.size;
      meta = Text.rich(
        TextSpan(
          children: [
            TextSpan(text: '$from · '),
            TextSpan(
              text: '${bytes(downloading)} / ${bytes(f.size)}',
              style: const TextStyle(fontFamily: mono),
            ),
          ],
        ),
        style: tx(c.fg3, size: 12),
      );
      action = Text(
        l10n.volumePercent(pct),
        style: tx(c.fg2, size: 12, monospace: true),
      );
      note = ClipRRect(
        borderRadius: BorderRadius.circular(2),
        child: LinearProgressIndicator(
          value: pct / 100,
          minHeight: 4,
          backgroundColor: c.hover,
          color: c.blueSolid,
        ),
      );
    } else {
      meta = Text.rich(
        TextSpan(
          children: [
            TextSpan(text: '$from · '),
            TextSpan(
              text: bytes(f.size),
              style: const TextStyle(fontFamily: mono),
            ),
            if (mineFirst && f.sent > 0)
              TextSpan(text: ' · ${l10n.roomDownloads(f.sent)}'),
          ],
        ),
        style: tx(c.fg3, size: 12),
      );
      final path = f.path;
      action = f.mine
          ? Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                if (path != null) ...[
                  FgButton(
                    icon: Ic.folder,
                    height: 28,
                    tooltip: l10n.roomShowInFolder(f.name),
                    onTap: () => showInFolderOrSay(path, state.say),
                  ),
                  const SizedBox(width: 6),
                ],
                FgButton(
                  text: l10n.roomStopSharingFile,
                  height: 28,
                  fontSize: 13,
                  padding: const EdgeInsets.symmetric(horizontal: 10),
                  onTap: () => state.stopSharing(f.id),
                ),
              ],
            )
          : FgButton(
              text: l10n.commonDownload,
              height: 28,
              fontSize: 13,
              padding: const EdgeInsets.symmetric(horizontal: 10),
              onTap: () => state.download(f.id),
            );
      note = f.mine
          ? Row(
              children: [
                FgIcon(Ic.upload, size: 12, color: c.blue),
                const SizedBox(width: 6),
                Text(l10n.roomSharingFromHere, style: tx(c.blue, size: 12)),
              ],
            )
          : Row(
              children: [
                Dot(c.green),
                const SizedBox(width: 6),
                Expanded(
                  child: Text(
                    l10n.roomAvailableWhile(_holders(state, f)),
                    style: tx(c.green, size: 12),
                  ),
                ),
              ],
            );
    }
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: c.border),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    // Two lines before it is shortened, and whole under the pointer:
                    // names often differ only at the end.
                    Tooltip(
                      message: f.name,
                      child: Text(
                        f.name,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: tx(c.fg, size: 13, weight: w500),
                      ),
                    ),
                    meta,
                  ],
                ),
              ),
              const SizedBox(width: 10),
              action,
            ],
          ),
          const SizedBox(height: 8),
          note,
        ],
      ),
    );
  }
}

/// A video in the chat: its first picture, how long it lasts and its size, as the device
/// that shares it read them. A click plays it over the window. A video without its first
/// picture is a file card.
class _VideoCard extends StatelessWidget {
  const _VideoCard({required this.state, required this.seen});
  final AppState state;
  final SeenFile seen;

  @override
  Widget build(BuildContext context) {
    final f = _current(state, seen.id);
    final video = f?.video;
    if (f == null || video == null) {
      return _FileCard(state: state, seen: seen);
    }
    final ratio = video.width > 0 && video.height > 0
        ? video.width / video.height
        : 16 / 9;
    final dpr = MediaQuery.devicePixelRatioOf(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        LayoutBuilder(
          builder: (context, box) {
            final width = math.min(box.maxWidth, 320.0);
            return _Hovered(
              builder: (hovered) => Pressable(
                key: ValueKey('video:${seen.id}'),
                onTap: () => _showVideo(context, state, seen),
                label: l10n.roomPlayVideo(seen.name),
                focusRadius: 14,
                builder: (context, _) => AnimatedScale(
                  scale: hovered ? 1.02 : 1,
                  duration: springTime,
                  curve: spring,
                  child: ClipRRect(
                    borderRadius: BorderRadius.circular(14),
                    child: SizedBox(
                      width: width,
                      height: width / ratio,
                      child: Stack(
                        fit: StackFit.expand,
                        children: [
                          // Made by another device: decoded only if small.
                          Image(
                            image: safePicture(
                              video.thumb,
                              width: (width * dpr).round(),
                            ),
                            fit: BoxFit.cover,
                            gaplessPlayback: true,
                          ),
                          Center(
                            child: AnimatedScale(
                              scale: hovered ? 1.08 : 1,
                              duration: springTime,
                              curve: spring,
                              child: Container(
                                width: 52,
                                height: 52,
                                alignment: Alignment.center,
                                decoration: BoxDecoration(
                                  color: const Color.fromRGBO(0, 0, 0, .55),
                                  shape: BoxShape.circle,
                                  border: Border.all(
                                    color: Colors.white.withValues(alpha: .18),
                                  ),
                                ),
                                child: const FgIcon(
                                  Ic.play,
                                  size: 20,
                                  stroke: 2.6,
                                  color: Colors.white,
                                ),
                              ),
                            ),
                          ),
                          Positioned(
                            left: 8,
                            bottom: 8,
                            child: _OverPill(
                              text: videoLength(video.durationMs),
                              mono: true,
                            ),
                          ),
                          Positioned(
                            right: 8,
                            bottom: 8,
                            child: _OverPill(text: bytes(f.size)),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
              ),
            );
          },
        ),
        const SizedBox(height: 4),
        Text(
          seen.name,
          overflow: TextOverflow.ellipsis,
          style: tx(context.fg.fg3, size: 11),
        ),
      ],
    );
  }
}

/// A little dark pill over a picture.
class _OverPill extends StatelessWidget {
  const _OverPill({required this.text, this.mono = false});
  final String text;
  final bool mono;

  @override
  Widget build(BuildContext context) => Container(
    height: 20,
    padding: const EdgeInsets.symmetric(horizontal: 7),
    alignment: Alignment.center,
    decoration: BoxDecoration(
      color: const Color.fromRGBO(0, 0, 0, .6),
      borderRadius: BorderRadius.circular(999),
    ),
    child: Text(
      text,
      style: tx(Colors.white, size: 11, monospace: mono, height: 1.2),
    ),
  );
}

@visibleForTesting
Future<void> showRoomVideo(
  BuildContext context,
  AppState state,
  SeenFile seen,
) => _showVideo(context, state, seen);

/// A video of the room playing over the window: who shared it, Save and close above. A
/// video someone else shares is fetched first, with its progress.
Future<void> _showVideo(
  BuildContext context,
  AppState state,
  SeenFile seen,
) => showOverWindow(
  context,
  l10n.roomCloseVideo,
  VideoViewer(
    state: state,
    open: () async {
      final f = _current(state, seen.id);
      if (f == null) throw VideoProblem(l10n.roomVideoGone);
      final path = await state.videoToPlay(f);
      if (path == null) throw VideoProblem(l10n.roomVideoNotFetched);
      return VideoFile(path);
    },
    cannotPlay: l10n.roomVideoCannotPlay,
    video: () => _current(state, seen.id)?.video,
    // Over the window, as a route: the fetch's progress comes with the room.
    changes: state.roomState,
    waiting: () {
      final f = _current(state, seen.id);
      final got = f?.downloading;
      return got != null && (f?.size ?? 0) > 0
          ? l10n.roomGettingVideoPercent(got * 100 ~/ f!.size)
          : l10n.roomGettingVideo;
    },
    fullScreen: true,
    top: (context, path) {
      final sender = _person(state, seen.byId, seen.byName);
      return Row(
        children: [
          Avatar(avatar: sender.avatar, size: 32),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  sender.name,
                  style: tx(Colors.white, size: 14, weight: w600),
                ),
                Text(
                  '${seen.name} · ${bytes(seen.size)}',
                  overflow: TextOverflow.ellipsis,
                  style: tx(Colors.white60, size: 12),
                ),
              ],
            ),
          ),
          _LightButton(
            text: l10n.roomSaveFile,
            icon: Ic.download,
            onTap: path == null ? null : () => state.saveVideo(path, seen.name),
          ),
          const SizedBox(width: 8),
          _LightButton(
            icon: Ic.close,
            tooltip: l10n.commonClose,
            onTap: Navigator.of(context).pop,
          ),
        ],
      );
    },
  ),
);

/// A light pill or round button over a dark layer.
class _LightButton extends StatelessWidget {
  const _LightButton({
    required this.icon,
    required this.onTap,
    this.text,
    this.tooltip,
  });
  final Ic icon;
  final String? text;
  final String? tooltip;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) => Opacity(
    opacity: onTap == null ? .5 : 1,
    child: Springy(
      onTap: onTap,
      label: text ?? tooltip,
      tooltip: tooltip,
      builder: (context, hovered) => Container(
        height: 36,
        width: text == null ? 36 : null,
        padding: text == null ? null : const EdgeInsets.fromLTRB(13, 0, 16, 0),
        alignment: Alignment.center,
        decoration: BoxDecoration(
          color: hovered ? Colors.white : const Color(0xFFEDEDED),
          borderRadius: BorderRadius.circular(999),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            FgIcon(icon, size: 14, color: const Color(0xFF0A0A0A)),
            if (text != null) ...[
              const SizedBox(width: 7),
              Text(
                text!,
                style: tx(const Color(0xFF0A0A0A), size: 13, weight: w600),
              ),
            ],
          ],
        ),
      ),
    ),
  );
}
