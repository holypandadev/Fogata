part of '../direct.dart';

// The pictures, videos and files of a message, as they stand on this computer, and a
// picture or video shown over the window.

/// An attachment in the chat: a picture, a video or a file, as it stands here.
class _FileCard extends StatelessWidget {
  const _FileCard({
    required this.file,
    required this.item,
    required this.page,
    required this.writer,
  });
  final UiDirectFile file;
  final UiDirectItem item;
  final UiDirectPage page;
  final UiPerson writer;

  @override
  Widget build(BuildContext context) {
    if (file.state == UiFileState.removed) {
      return _Gone(file: file, page: page);
    }
    if (file.voice != null) {
      final state = AppScope.read(context);
      final canFetch = _canFetch(state, page);
      return VoiceCard(
        friend: page.friend.id,
        file: file,
        mine: item.mine,
        canFetch: canFetch,
        onFetch: () => state.direct.fetch(file.id),
        notHere: file.state != UiFileState.here && !canFetch
            ? capitalized(availableWhen(state, page))
            : null,
      );
    }
    final path = file.path;
    return switch (file.kind) {
      // A large one is not read into memory at every refresh: it shows as a file.
      UiFileKind.picture when path != null && file.size <= _pictureMax =>
        _Picture(file: file, path: path, mine: item.mine, writer: writer),
      UiFileKind.video when file.video != null => _Video(
        file: file,
        page: page,
        writer: writer,
      ),
      _ => _Document(file: file, page: page),
    };
  }
}

/// Both are online and the chat connected: files come now.
bool _canFetch(AppState state, UiDirectPage page) =>
    page.connected &&
    page.online &&
    page.endedMs == null &&
    !state.prefs.appearOffline;

/// When a file that is not here yet can come: `available when Sofía is online`.
String availableWhen(AppState state, UiDirectPage page) =>
    state.prefs.appearOffline && page.online
    ? l10n.directAvailableWhenYouOnline
    : l10n.directAvailableWhenOnline(page.friend.name);

/// Most bytes of pictures read from the chat's folder kept for the screen
/// ([DirectChats.pictures]): the newest few, so a long chat does not keep every picture
/// in memory.
const _picturesKept = 64 * 1024 * 1024;

/// Biggest picture read to show in the chat, as in a room ([RoomState.maxPreviewBytes]);
/// a larger one shows as a file.
const _pictureMax = RoomState.maxPreviewBytes;

/// Whether a file kept here is sealed with the data folder's key: then only the core reads
/// it.
Future<bool> _sealed(String path) async {
  final head = await File(path)
      .openRead(0, 7)
      .fold<List<int>>([], (all, part) => all..addAll(part));
  return String.fromCharCodes(head) == ' fogata';
}

/// Reads a picture kept here, through the core when it is sealed, into [pictures].
Future<Uint8List?> _read(
  Map<String, Uint8List> pictures,
  String path,
  String friend,
  String file,
) async {
  final kept = pictures.remove(path);
  if (kept != null) {
    pictures[path] = kept;
    return kept;
  }
  try {
    final local = File(path);
    if (await local.length() > _pictureMax) return null;
    final bytes = await _sealed(path)
        ? await directFileBytes(
            deviceId: friend,
            file: file,
            maxBytes: _pictureMax,
          )
        : await readAtMost(local.openRead(0, _pictureMax + 1), _pictureMax);
    // A file that grew since its size was looked at is not kept.
    if (bytes == null) return null;
    pictures[path] = bytes;
    var total = pictures.values.fold<int>(0, (n, b) => n + b.length);
    while (total > _picturesKept && pictures.length > 1) {
      final oldest = pictures.keys.first;
      total -= pictures.remove(oldest)!.length;
    }
    return bytes;
  } catch (_) {
    return null;
  }
}

class _Picture extends StatefulWidget {
  const _Picture({
    required this.file,
    required this.path,
    required this.mine,
    required this.writer,
  });
  final UiDirectFile file;
  final String path;
  final bool mine;
  final UiPerson writer;

  @override
  State<_Picture> createState() => _PictureState();
}

class _PictureState extends State<_Picture> {
  Uint8List? _bytes;
  bool _failed = false;

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void didUpdateWidget(_Picture old) {
    super.didUpdateWidget(old);
    if (old.path != widget.path) _load();
  }

  /// Reads the picture, and shows it as a file when it is no picture this computer can
  /// draw (damaged, or one that would take too much memory once decoded).
  Future<void> _load() async {
    final direct = AppScope.read(context).direct;
    final friend = direct.open;
    if (friend == null) return;
    final bytes = await _read(
      direct.pictures,
      widget.path,
      friend,
      widget.file.id,
    );
    final fits = bytes != null && await fitsToDecode(bytes, maxPicturePixels);
    if (!mounted) return;
    setState(() {
      _bytes = fits ? bytes : null;
      _failed = !fits;
    });
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    final file = widget.file;
    final picture = _bytes;
    if (_failed) return _Document(file: file, page: state.direct.page!);
    final dpr = MediaQuery.devicePixelRatioOf(context);
    final settings = state.direct.storage?.settings;
    final byItself =
        !widget.mine &&
        file.size <= autoPictureBytes &&
        (settings?.autoPictures ?? true);
    final caption = [
      file.name,
      bytes(file.size),
      if (byItself) l10n.directDownloadedByItself,
    ].join(' · ');
    return _Hovered(
      builder: (hovered) => Pressable(
        onTap: picture == null
            ? null
            : () => _showPicture(context, picture, file, widget.writer),
        label: l10n.directOpenFile(file.name),
        focusRadius: 14,
        builder: (context, _) => AnimatedScale(
          scale: hovered ? 1.02 : 1,
          duration: springTime,
          curve: spring,
          child: Container(
            width: 300,
            height: 200,
            clipBehavior: Clip.antiAlias,
            // Inside the outline, as when it was drawn under what is inside.
            padding: const EdgeInsets.all(1),
            decoration: BoxDecoration(
              color: c.bg2,
              borderRadius: BorderRadius.circular(14),
            ),
            // Over what is inside, which would cover it at the rounded corners.
            foregroundDecoration: BoxDecoration(
              borderRadius: BorderRadius.circular(14),
              border: Border.all(color: c.grid),
            ),
            child: Stack(
              fit: StackFit.expand,
              children: [
                if (picture != null)
                  ChatPicture(
                    bytes: picture,
                    still: Image(
                      image: safePicture(
                        picture,
                        width: (300 * dpr).round(),
                        maxPixels: maxPicturePixels,
                      ),
                      fit: BoxFit.cover,
                      gaplessPlayback: true,
                      errorBuilder: (_, _, _) => const SizedBox.shrink(),
                    ),
                  ),
                Positioned(
                  left: 8,
                  bottom: 8,
                  right: 8,
                  child: Align(
                    alignment: Alignment.centerLeft,
                    child: _OverPill(text: caption),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// A video's first picture, darkened until it is here, with the way to get or play it.
class _Video extends StatelessWidget {
  const _Video({required this.file, required this.page, required this.writer});
  final UiDirectFile file;
  final UiDirectPage page;
  final UiPerson writer;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    final video = file.video!;
    final here = file.state == UiFileState.here && file.path != null;
    final coming = file.state == UiFileState.coming;
    final dpr = MediaQuery.devicePixelRatioOf(context);
    final length = videoLength(video.lengthMs);
    return _Hovered(
      builder: (hovered) => Pressable(
        key: ValueKey('direct-video:${file.id}'),
        onTap: here
            ? () => _showVideo(context, file, page.friend.id, writer)
            : null,
        label: l10n.directPlayFile(file.name),
        focusRadius: 14,
        builder: (context, _) => AnimatedScale(
          scale: here && hovered ? 1.02 : 1,
          duration: springTime,
          curve: spring,
          child: Container(
            width: 300,
            height: 169,
            clipBehavior: Clip.antiAlias,
            decoration: BoxDecoration(
              color: c.bg2,
              borderRadius: BorderRadius.circular(14),
            ),
            child: Stack(
              fit: StackFit.expand,
              children: [
                ColorFiltered(
                  // Darker until it is here, so what to do with it reads over it.
                  colorFilter: ColorFilter.matrix(_brightness(here ? 1 : .55)),
                  child: Image(
                    image: safePicture(video.thumb, width: (300 * dpr).round()),
                    fit: BoxFit.cover,
                    gaplessPlayback: true,
                    errorBuilder: (_, _, _) => const SizedBox.shrink(),
                  ),
                ),
                if (here) ...[
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
                    child: _OverPill(text: length, mono: true),
                  ),
                  Positioned(
                    right: 8,
                    bottom: 8,
                    child: _OverPill(text: bytes(file.size)),
                  ),
                ] else
                  Center(
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        if (coming)
                          _ComingPill(file: file)
                        else
                          _LightButton(
                            key: ValueKey('direct-get:${file.id}'),
                            icon: Ic.download,
                            text: l10n.directDownloadSize(bytes(file.size)),
                            onTap: _canFetch(state, page)
                                ? () => state.direct.fetch(file.id)
                                : null,
                          ),
                        const SizedBox(height: 8),
                        Text(
                          _canFetch(state, page) || coming
                              ? l10n.directVideoPlaysWhenHere(file.name, length)
                              : l10n.directVideoNotHere(
                                  file.name,
                                  length,
                                  availableWhen(state, page),
                                ),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: tx(const Color(0xFFD4D4D4), size: 11),
                        ),
                      ],
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// A file (or a picture or video that cannot be drawn): its name and size, and the way
/// to get it or save it.
class _Document extends StatelessWidget {
  const _Document({required this.file, required this.page});
  final UiDirectFile file;
  final UiDirectPage page;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    final here = file.state == UiFileState.here;
    final coming = file.state == UiFileState.coming;
    return Container(
      width: 320,
      padding: const EdgeInsets.fromLTRB(12, 10, 10, 10),
      decoration: BoxDecoration(
        color: c.bg1,
        borderRadius: BorderRadius.circular(14),
        border: Border.all(color: c.border),
      ),
      child: Row(
        children: [
          Container(
            width: 34,
            height: 34,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              color: c.hover,
              borderRadius: BorderRadius.circular(10),
            ),
            child: FgIcon(Ic.file, size: 16, color: c.fg2),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  file.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(c.fg, size: 13, weight: w500, height: 1.35),
                ),
                Text(
                  coming
                      ? l10n.directGettingIt(
                          file.size > 0 ? file.received * 100 ~/ file.size : 0,
                        )
                      : here || _canFetch(state, page)
                      ? bytes(file.size)
                      : l10n.directFileNotHere(
                          bytes(file.size),
                          availableWhen(state, page),
                        ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(c.fg3, size: 12, height: 1.35),
                ),
              ],
            ),
          ),
          const SizedBox(width: 8),
          if (coming)
            Spinner(size: 14, color: c.fg3)
          else if (here)
            FgButton(
              text: l10n.commonSave,
              icon: Ic.download,
              height: 28,
              fontSize: 12,
              padding: const EdgeInsets.symmetric(horizontal: 10),
              tooltip: l10n.directSaveToDownloads,
              onTap: () => state.direct.save(file.id),
            )
          else
            FgButton(
              key: ValueKey('direct-get:${file.id}'),
              text: l10n.commonDownload,
              icon: Ic.download,
              height: 28,
              fontSize: 12,
              padding: const EdgeInsets.symmetric(horizontal: 10),
              onTap: _canFetch(state, page)
                  ? () => state.direct.fetch(file.id)
                  : null,
            ),
        ],
      ),
    );
  }
}

/// An attachment removed here to free space or cleared: the message stays, and it can be
/// asked for again while the friend still has it.
class _Gone extends StatelessWidget {
  const _Gone({required this.file, required this.page});
  final UiDirectFile file;
  final UiDirectPage page;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    return Container(
      width: 320,
      padding: const EdgeInsets.fromLTRB(12, 10, 10, 10),
      decoration: BoxDecoration(
        color: c.bg1,
        borderRadius: BorderRadius.circular(14),
        border: Border.all(color: c.border),
      ),
      child: Row(
        children: [
          Container(
            width: 34,
            height: 34,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              color: c.hover,
              borderRadius: BorderRadius.circular(10),
            ),
            child: FgIcon(Ic.fileX, size: 16, color: c.fg3),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  '${file.name} · ${bytes(file.size)}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(c.fg2, size: 13, height: 1.35),
                ),
                Text(
                  l10n.directRemovedForSpace,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(c.fg3, size: 12, height: 1.35),
                ),
              ],
            ),
          ),
          const SizedBox(width: 8),
          FgButton(
            key: ValueKey('direct-again:${file.id}'),
            text: l10n.directAskAgain,
            height: 28,
            fontSize: 12,
            padding: const EdgeInsets.symmetric(horizontal: 10),
            tooltip: _canFetch(state, page)
                ? null
                : capitalized(availableWhen(state, page)),
            onTap: _canFetch(state, page)
                ? () => state.direct.fetch(file.id)
                : null,
          ),
        ],
      ),
    );
  }
}

class _ComingPill extends StatelessWidget {
  const _ComingPill({required this.file});
  final UiDirectFile file;

  @override
  Widget build(BuildContext context) {
    final percent = file.size > 0 ? file.received * 100 ~/ file.size : 0;
    return Container(
      height: 34,
      padding: const EdgeInsets.symmetric(horizontal: 14),
      decoration: BoxDecoration(
        color: const Color.fromRGBO(16, 16, 16, .72),
        borderRadius: BorderRadius.circular(999),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Spinner(size: 13, color: Colors.white),
          const SizedBox(width: 8),
          Text(
            l10n.directGettingIt(percent),
            style: tx(Colors.white, size: 13, weight: w500, height: 1.2),
          ),
        ],
      ),
    );
  }
}

/// A colour matrix that scales how bright a picture is.
List<double> _brightness(double b) => [
  b, 0, 0, 0, 0, //
  0, b, 0, 0, 0, //
  0, 0, b, 0, 0, //
  0, 0, 0, 1, 0,
];

/// A little dark pill over a picture.
class _OverPill extends StatelessWidget {
  const _OverPill({required this.text, this.mono = false});
  final String text;
  final bool mono;

  @override
  Widget build(BuildContext context) => Container(
    height: 22,
    padding: const EdgeInsets.symmetric(horizontal: 8),
    alignment: Alignment.center,
    decoration: BoxDecoration(
      color: const Color.fromRGBO(16, 16, 16, .72),
      borderRadius: BorderRadius.circular(999),
    ),
    child: Text(
      text,
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
      style: tx(Colors.white, size: 11, monospace: mono, height: 1.2),
    ),
  );
}

/// A light pill or round button over a dark picture.
class _LightButton extends StatelessWidget {
  const _LightButton({
    super.key,
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
        height: 34,
        width: text == null ? 34 : null,
        padding: text == null ? null : const EdgeInsets.fromLTRB(13, 0, 15, 0),
        alignment: text == null ? Alignment.center : null,
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

/// What sits over a picture or video shown large: who sent it, its name, Save and close.
class _Top extends StatelessWidget {
  const _Top({required this.file, required this.writer});
  final UiDirectFile file;
  final UiPerson writer;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    return Row(
      children: [
        Avatar(avatar: writer.avatar, size: 32),
        const SizedBox(width: 10),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                writer.name,
                style: tx(Colors.white, size: 14, weight: w600),
              ),
              Text(
                '${file.name} · ${bytes(file.size)}',
                overflow: TextOverflow.ellipsis,
                style: tx(Colors.white60, size: 12),
              ),
            ],
          ),
        ),
        _LightButton(
          text: l10n.commonSave,
          icon: Ic.download,
          onTap: () => state.direct.save(file.id),
        ),
        const SizedBox(width: 8),
        _LightButton(
          icon: Ic.close,
          tooltip: l10n.commonClose,
          onTap: Navigator.of(context).pop,
        ),
      ],
    );
  }
}

/// A picture large over the window; a click beside it or Esc closes it.
Future<void> _showPicture(
  BuildContext context,
  Uint8List picture,
  UiDirectFile file,
  UiPerson writer,
) => showOverWindow(
  context,
  l10n.directClosePicture,
  Builder(
    builder: (context) {
      final close = Navigator.of(context).pop;
      var screen = View.of(context).display.size;
      if (screen.isEmpty) {
        screen =
            MediaQuery.sizeOf(context) * MediaQuery.devicePixelRatioOf(context);
      }
      return CallbackShortcuts(
        bindings: {const SingleActivator(LogicalKeyboardKey.escape): close},
        // A scope: Tab goes round what floats here, not the page behind.
        child: FocusScope(
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
                    child: ClipRRect(
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
                Positioned(
                  left: 24,
                  right: 24,
                  top: 56,
                  child: _Top(file: file, writer: writer),
                ),
              ],
            ),
          ),
        ),
      );
    },
  ),
);

@visibleForTesting
Future<void> showDirectVideo(
  BuildContext context,
  UiDirectFile file,
  String friend,
  UiPerson writer,
) => _showVideo(context, file, friend, writer);

/// A video kept here, playing over the window with the system's own player. A sealed one
/// plays from a plain copy, which goes when the player closes.
Future<void> _showVideo(
  BuildContext context,
  UiDirectFile file,
  String friend,
  UiPerson writer,
) => showOverWindow(
  context,
  l10n.directCloseVideo,
  VideoViewer(
    state: AppScope.read(context),
    open: () async {
      String? path = file.path;
      String? copy;
      try {
        if (path != null && await _sealed(path)) {
          path = copy = await directFileToPlay(deviceId: friend, file: file.id);
        }
      } catch (_) {
        path = null;
      }
      if (path == null) throw VideoProblem(l10n.directCannotPlay);
      // Only this player's copy goes with it; the copies of other players stay.
      final made = copy;
      return VideoFile(
        path,
        letGo: made == null ? null : () => forgetDirectFileCopy(path: made),
      );
    },
    cannotPlay: l10n.directCannotPlay,
    video: () => file.video,
    top: (context, _) => _Top(file: file, writer: writer),
  ),
);
