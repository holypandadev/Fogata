part of '../room.dart';

// A screen or an enlarged camera filling the room: the spotlight with the strip under it,
// the stream with its controls over the picture (the dock), how well it arrives, and full
// screen.

/// What fills the room: a camera, someone's watched screen, or this device's own.
enum _Shown { camera, screen, ownScreen }

/// A watched screen or an enlarged camera, as large as the room allows in its own shape,
/// with the others in a strip right under it.
class _Spotlight extends StatelessWidget {
  const _Spotlight({
    required this.id,
    required this.title,
    required this.renderer,
    required this.closeLabel,
    required this.onClose,
    required this.strip,
    this.kind = _Shown.screen,
    this.mirror = false,
    this.watchers,
    this.from,
    this.avatar,
    this.soundOf,
  });

  /// Whose camera or screen it is.
  final String id;
  final String title;
  final RTCVideoRenderer renderer;
  final String closeLabel;
  final VoidCallback onClose;
  final List<Widget> strip;
  final _Shown kind;
  final bool mirror;

  /// For a screen: how many watch it.
  final int? watchers;

  /// Whose it is when it comes from someone else (see [_Stream.from]).
  final String? from;
  final UiAvatar? avatar;

  /// For someone else's screen with sound: whose.
  final String? soundOf;

  /// The strip's height; a little lower in a short window.
  static double _stripHeight(double room) => room < 420 ? 96 : 120;

  @override
  Widget build(BuildContext context) {
    final stream = _Stream(
      title: title,
      renderer: renderer,
      live: kind != _Shown.camera,
      mirror: mirror,
      watchers: watchers,
      from: from,
      avatar: avatar,
      soundOf: soundOf,
      closeLabel: closeLabel,
      onClose: onClose,
      onFullScreen: () => _fullScreen(context, id, kind),
    );
    return ValueListenableBuilder<RTCVideoValue>(
      valueListenable: renderer,
      builder: (context, video, _) => LayoutBuilder(
        builder: (context, box) {
          // The picture's own shape once it arrives, 16:9 until then.
          final aspect = video.width > 0 && video.height > 0
              ? video.aspectRatio.clamp(.5, 3.0)
              : 16 / 9;
          final strip = this.strip.isEmpty ? 0.0 : _stripHeight(box.maxHeight);
          final room = math.max(
            0.0,
            box.maxHeight - (strip > 0 ? strip + tileGap : 0),
          );
          var width = box.maxWidth;
          var height = width / aspect;
          if (height > room) {
            height = room;
            width = height * aspect;
          }
          // The strip sits right under the picture, and the two are centred together.
          return Center(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                SizedBox(width: width, height: height, child: stream),
                if (strip > 0) ...[
                  const SizedBox(height: tileGap),
                  SizedBox(
                    width: box.maxWidth,
                    height: strip,
                    child: _Strip(tiles: this.strip),
                  ),
                ],
              ],
            ),
          );
        },
      ),
    );
  }
}

/// A screen or camera with its controls over it: whose it is (and for a screen, that it
/// is live and who watches), how well it arrives, and at the bottom its sound, full screen
/// and the way back. The controls show while the pointer moves over the picture and hide
/// after [_idle] without moving; in full screen the pointer hides with them.
class _Stream extends StatefulWidget {
  const _Stream({
    required this.title,
    required this.renderer,
    required this.closeLabel,
    required this.onClose,
    required this.onFullScreen,
    this.live = false,
    this.mirror = false,
    this.watchers,
    this.from,
    this.avatar,
    this.soundOf,
    this.fullScreen = false,
  });

  final String title;
  final RTCVideoRenderer renderer;
  final String closeLabel;
  final VoidCallback onClose;
  final VoidCallback onFullScreen;

  /// A shared screen, as opposed to a camera.
  final bool live;
  final bool mirror;
  final int? watchers;

  /// Whose picture it is when it comes from someone else, for its quality; null for this
  /// device's own.
  final String? from;

  /// Their picture, beside a camera's title.
  final UiAvatar? avatar;

  /// Someone's screen with sound: whose, for its volume here.
  final String? soundOf;

  /// Drawn over the whole screen: no frame, and the button brings the room back.
  final bool fullScreen;

  @override
  State<_Stream> createState() => _StreamState();
}

/// Time without moving the pointer before the controls hide.
const _idle = Duration(milliseconds: 2500);

class _StreamState extends State<_Stream> {
  bool _shown = true;

  /// The pointer is on the controls: they stay.
  bool _held = false;
  bool _qualityOpen = false;
  Timer? _hide;
  Timer? _poll;
  StreamQuality? _quality;

  @override
  void initState() {
    super.initState();
    _arm();
    _watchQuality();
    windowShown.addListener(_watchQuality);
  }

  @override
  void didUpdateWidget(_Stream old) {
    super.didUpdateWidget(old);
    if (old.from != widget.from || old.live != widget.live) {
      _quality = null;
      _poll?.cancel();
      _poll = null;
      _watchQuality();
    }
  }

  @override
  void dispose() {
    windowShown.removeListener(_watchQuality);
    _hide?.cancel();
    _poll?.cancel();
    super.dispose();
  }

  /// Reads how the picture arrives every 2 s, only while the quality pill is shown and
  /// the window can be seen.
  void _watchQuality() {
    final want = widget.from != null && _shown && windowShown.value;
    if (!want) {
      _poll?.cancel();
      _poll = null;
      return;
    }
    if (_poll != null) return;
    _read();
    _poll = Timer.periodic(const Duration(seconds: 2), (_) => _read());
  }

  Future<void> _read() async {
    final from = widget.from;
    final call = AppScope.read(context).call;
    if (from == null || call == null) return;
    final quality = await call.qualityOf(from, screen: widget.live);
    if (mounted && quality != null) setState(() => _quality = quality);
  }

  void _arm() {
    _hide?.cancel();
    _hide = Timer(_idle, () {
      if (mounted && !_held) {
        setState(() {
          _shown = false;
          _qualityOpen = false;
        });
        _watchQuality();
      }
    });
  }

  void _wake() {
    if (!_shown) {
      setState(() => _shown = true);
      _watchQuality();
    }
    _arm();
  }

  void _away() {
    if (widget.fullScreen || _held) return;
    _hide?.cancel();
    setState(() {
      _shown = false;
      _qualityOpen = false;
    });
    _watchQuality();
  }

  void _hold(bool on) {
    _held = on;
    if (on) {
      _hide?.cancel();
    } else {
      _arm();
    }
  }

  /// One of the controls: it fades and slides `dy` away while hidden, and the pointer on
  /// it keeps all of them shown.
  Widget _reveal(Widget child, {double dy = 0}) => MouseRegion(
    onEnter: (_) => _hold(true),
    onExit: (_) => _hold(false),
    child: IgnorePointer(
      ignoring: !_shown,
      child: AnimatedOpacity(
        opacity: _shown ? 1 : 0,
        duration: const Duration(milliseconds: 250),
        child: AnimatedContainer(
          duration: springTime,
          curve: spring,
          transform: Matrix4.translationValues(0, _shown ? 0 : dy, 0),
          child: child,
        ),
      ),
    ),
  );

  Widget _scrim({required bool top}) => Positioned(
    left: 0,
    right: 0,
    top: top ? 0 : null,
    bottom: top ? null : 0,
    height: top ? 110 : 130,
    child: IgnorePointer(
      child: AnimatedOpacity(
        opacity: _shown ? 1 : 0,
        duration: const Duration(milliseconds: 300),
        child: DecoratedBox(
          decoration: BoxDecoration(
            gradient: LinearGradient(
              begin: top ? Alignment.topCenter : Alignment.bottomCenter,
              end: top ? Alignment.bottomCenter : Alignment.topCenter,
              colors: const [Color.fromRGBO(0, 0, 0, .55), Color(0x00000000)],
            ),
          ),
        ),
      ),
    ),
  );

  @override
  Widget build(BuildContext context) {
    final state = AppScope.only(context, roomCallParts);
    final from = widget.from;
    final path = from == null
        ? null
        : state.contacts.paths[state.roomState.roomGroup]?[from];
    return LayoutBuilder(
      builder: (context, box) => _build(context, state, box.maxWidth, path),
    );
  }

  Widget _build(
    BuildContext context,
    AppState state,
    double width,
    UiPath? path,
  ) {
    final c = context.fg;
    final watchers = widget.watchers;
    final avatar = widget.avatar;
    final from = widget.from;
    return MouseRegion(
      onEnter: (_) => _wake(),
      onHover: (_) => _wake(),
      onExit: (_) => _away(),
      cursor: widget.fullScreen && !_shown
          ? SystemMouseCursors.none
          : MouseCursor.defer,
      child: Container(
        clipBehavior: Clip.antiAlias,
        decoration: BoxDecoration(
          color: Colors.black,
          borderRadius: widget.fullScreen ? null : BorderRadius.circular(16),
          boxShadow: widget.fullScreen
              ? null
              : [BoxShadow(color: c.border, spreadRadius: 1)],
        ),
        child: Stack(
          children: [
            // Only until the first picture arrives, which gives the video its size.
            Positioned.fill(
              child: ValueListenableBuilder(
                valueListenable: widget.renderer,
                builder: (context, video, spinner) =>
                    video.width > 0 && video.height > 0
                    ? const SizedBox.shrink()
                    : spinner!,
                child: const Center(
                  child: Spinner(size: 20, color: Colors.white54),
                ),
              ),
            ),
            Positioned.fill(
              child: RTCVideoView(
                widget.renderer,
                mirror: widget.mirror,
                objectFit: RTCVideoViewObjectFit.RTCVideoViewObjectFitContain,
              ),
            ),
            _scrim(top: true),
            _scrim(top: false),
            Positioned(
              left: 14,
              top: 14,
              // Clear of the pill on the right; in a narrow picture, of its closed size.
              right: width >= 560
                  ? 220
                  : (from != null || widget.live ? 136 : 14),
              child: Align(
                alignment: Alignment.centerLeft,
                child: _reveal(
                  dy: -8,
                  _Glass(
                    padding: EdgeInsets.fromLTRB(
                      avatar != null || widget.live ? 5 : 12,
                      0,
                      12,
                      0,
                    ),
                    height: 36,
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        if (widget.live) ...[
                          Container(
                            height: heightForText(context, 22, 10),
                            padding: const EdgeInsets.symmetric(horizontal: 7),
                            alignment: Alignment.center,
                            decoration: BoxDecoration(
                              color: c.redSolid,
                              borderRadius: BorderRadius.circular(999),
                            ),
                            child: Text(
                              l10n.roomLive,
                              style: tx(
                                Colors.white,
                                size: 10,
                                weight: FontWeight.w700,
                                height: 1,
                                spacing: .5,
                              ),
                            ),
                          ),
                          const SizedBox(width: 8),
                        ] else if (avatar != null) ...[
                          Avatar(avatar: avatar, size: 26),
                          const SizedBox(width: 8),
                        ],
                        Flexible(
                          child: Text(
                            widget.title,
                            overflow: TextOverflow.ellipsis,
                            style: tx(Colors.white, size: 13, weight: w500),
                          ),
                        ),
                        // In a narrow picture the title keeps the room.
                        if (watchers != null && width >= 480) ...[
                          const SizedBox(width: 9),
                          Tooltip(
                            message: watchers == 0
                                ? l10n.roomNobodyWatching
                                : l10n.roomPeopleWatching(watchers),
                            child: Row(
                              children: [
                                const FgIcon(
                                  Ic.eye,
                                  size: 14,
                                  color: Colors.white70,
                                ),
                                const SizedBox(width: 4),
                                Text(
                                  '$watchers',
                                  style: tx(Colors.white70, size: 12),
                                ),
                              ],
                            ),
                          ),
                        ],
                      ],
                    ),
                  ),
                ),
              ),
            ),
            if (from != null)
              Positioned(
                right: 14,
                top: 14,
                child: _reveal(
                  dy: -8,
                  MouseRegion(
                    onEnter: (_) => setState(() => _qualityOpen = true),
                    onExit: (_) => setState(() => _qualityOpen = false),
                    child: StreamQualityPill(
                      quality: _quality,
                      path: path,
                      through: state.contacts.carrierName(
                        state.roomState.roomGroup,
                        from,
                      ),
                      open: _qualityOpen,
                    ),
                  ),
                ),
              ),
            // This device's own screen: how it goes out, and a click changes it.
            if (from == null && widget.live)
              Positioned(
                right: 14,
                top: 14,
                child: _reveal(
                  dy: -8,
                  Builder(
                    builder: (anchor) => Springy(
                      onTap: () => showStreamQuality(anchor),
                      tooltip: l10n.streamQualityTitle,
                      builder: (context, hovered) => _Glass(
                        padding: const EdgeInsets.fromLTRB(12, 0, 10, 0),
                        height: 36,
                        child: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            Text(
                              state.prefs.screenQuality.label,
                              style: tx(Colors.white, size: 13, weight: w500),
                            ),
                            const SizedBox(width: 6),
                            FgIcon(
                              Ic.chevronDown,
                              size: 13,
                              color: hovered ? Colors.white : Colors.white70,
                            ),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            Positioned(
              left: 0,
              right: 0,
              bottom: 16,
              child: Center(
                child: _reveal(
                  dy: 10,
                  _Glass(
                    padding: const EdgeInsets.all(6),
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        if (widget.soundOf case final id?) ...[
                          _DockVolume(state: state, id: id),
                          const SizedBox(width: 4),
                        ],
                        _DockButton(
                          icon: widget.fullScreen
                              ? Ic.fullScreenExit
                              : Ic.fullScreen,
                          tooltip: widget.fullScreen
                              ? l10n.roomExitFullScreen
                              : l10n.roomFullScreen,
                          onTap: widget.onFullScreen,
                        ),
                        Container(
                          width: 1,
                          height: 22,
                          margin: const EdgeInsets.symmetric(horizontal: 6),
                          color: Colors.white.withValues(alpha: .14),
                        ),
                        Springy(
                          onTap: widget.onClose,
                          label: widget.closeLabel,
                          builder: (context, hovered) => Container(
                            height: 40,
                            padding: const EdgeInsets.fromLTRB(14, 0, 16, 0),
                            decoration: BoxDecoration(
                              color: hovered
                                  ? Colors.white
                                  : const Color(0xFFEDEDED),
                              borderRadius: BorderRadius.circular(999),
                            ),
                            child: Row(
                              children: [
                                FgIcon(
                                  widget.live ? Ic.eyeOff : Ic.back,
                                  size: 15,
                                  color: const Color(0xFF0A0A0A),
                                ),
                                const SizedBox(width: 7),
                                Text(
                                  widget.closeLabel,
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
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// A dark see-through pill over the picture, blurring what is behind it.
class _Glass extends StatelessWidget {
  const _Glass({required this.child, this.padding, this.height});
  final Widget child;
  final EdgeInsets? padding;
  final double? height;

  @override
  // A solid tint rather than a blur: a blur over a playing video reads the picture
  // back on every frame.
  Widget build(BuildContext context) => Container(
    height: height,
    padding: padding,
    decoration: BoxDecoration(
      color: const Color.fromRGBO(16, 16, 16, .82),
      borderRadius: BorderRadius.circular(999),
      border: Border.all(color: Colors.white.withValues(alpha: .08)),
    ),
    child: child,
  );
}

/// A round button of the controls over the picture.
class _DockButton extends StatelessWidget {
  const _DockButton({
    required this.icon,
    required this.tooltip,
    required this.onTap,
  });
  final Ic icon;
  final String tooltip;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => Springy(
    onTap: onTap,
    tooltip: tooltip,
    builder: (context, hovered) => AnimatedContainer(
      duration: const Duration(milliseconds: 160),
      width: 40,
      height: 40,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: hovered ? Colors.white.withValues(alpha: .12) : null,
      ),
      child: FgIcon(icon, size: 17, color: Colors.white),
    ),
  );
}

/// The sound of someone's shared screen: the speaker mutes it, and its volume opens
/// beside it while the pointer is there.
class _DockVolume extends StatefulWidget {
  const _DockVolume({required this.state, required this.id});
  final AppState state;
  final String id;

  @override
  State<_DockVolume> createState() => _DockVolumeState();
}

class _DockVolumeState extends State<_DockVolume> {
  bool _open = false;

  @override
  Widget build(BuildContext context) {
    final state = widget.state;
    final id = widget.id;
    final silenced = state.prefs.screenSilenced.contains(id);
    final volume = state.prefs.screenVolumeOf(id);
    return MouseRegion(
      onEnter: (_) => setState(() => _open = true),
      onExit: (_) => setState(() => _open = false),
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 200),
        height: 40,
        decoration: BoxDecoration(
          color: _open ? Colors.white.withValues(alpha: .08) : null,
          borderRadius: BorderRadius.circular(999),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            _DockButton(
              icon: silenced || volume == 0 ? Ic.volumeOff : Ic.volume,
              tooltip: silenced
                  ? l10n.roomPlayScreenSound
                  : l10n.roomMuteScreenSound,
              onTap: () => state.setScreenSilenced(id, !silenced),
            ),
            AnimatedContainer(
              duration: springTime,
              // The spring overshoots: closing with it would ask for a width below
              // zero, which breaks the layout and, in a debug build, the pointer.
              curve: _open ? spring : Curves.easeOutCubic,
              width: _open ? 118 : 0,
              child: ClipRect(
                child: OverflowBox(
                  alignment: Alignment.centerLeft,
                  minWidth: 118,
                  maxWidth: 118,
                  child: Padding(
                    padding: const EdgeInsets.only(right: 12),
                    child: Tooltip(
                      message: l10n.volumePercent(volume),
                      child: VolumeSlider(
                        volume: volume,
                        dim: silenced,
                        light: true,
                        label: l10n.volumeScreenSound,
                        onChanged: (v) =>
                            state.setScreenVolume(id, v, save: false),
                        onChangeEnd: (v) => state.setScreenVolume(id, v),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// How well someone's picture arrives: bars and a word, and the figures under it while
/// the pointer is on it.
class StreamQualityPill extends StatelessWidget {
  const StreamQualityPill({
    super.key,
    required this.quality,
    required this.path,
    required this.open,
    this.through,
  });
  final StreamQuality? quality;
  final UiPath? path;
  final bool open;

  /// The member that carries the person sharing, when known.
  final String? through;

  static const _colors = [
    Color(0xFFFF6166),
    Color(0xFFF5A524),
    Color(0xFF3DD68C),
  ];
  static List<String> get _words => [
    l10n.roomQualityPoor,
    l10n.roomQualityOkay,
    l10n.roomQualityGood,
  ];
  static List<String> get _titles => [
    l10n.roomQualityPoorTitle,
    l10n.roomQualityOkayTitle,
    l10n.roomQualityGoodTitle,
  ];

  @override
  Widget build(BuildContext context) {
    final q = quality;
    final level = q?.level;
    final color = level == null ? Colors.white54 : _colors[level];
    final lit = switch (level) {
      2 => 4,
      1 => 2,
      0 => 1,
      _ => 0,
    };
    final height = q?.height;
    final fps = q?.fps;
    final short = [
      if (height != null && height > 0) '${height}p',
      if (fps != null && fps > 0) l10n.streamQualityFps(fps.round()),
    ].join(' · ');
    String figure(num? v, String Function(String) unit, {int digits = 0}) =>
        v == null
        ? '—'
        : unit(NumberFormat(digits == 0 ? '0' : '0.0', intlLocale).format(v));
    final path = this.path;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.end,
      mainAxisSize: MainAxisSize.min,
      children: [
        _Glass(
          height: 36,
          padding: const EdgeInsets.symmetric(horizontal: 12),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Row(
                crossAxisAlignment: CrossAxisAlignment.end,
                children: [
                  for (final (i, h) in const [
                    4.0,
                    7.0,
                    10.0,
                    13.0,
                  ].indexed) ...[
                    if (i > 0) const SizedBox(width: 2),
                    AnimatedContainer(
                      duration: const Duration(milliseconds: 300),
                      width: 3,
                      height: h,
                      decoration: BoxDecoration(
                        color: i < lit ? color : Colors.white24,
                        borderRadius: BorderRadius.circular(1),
                      ),
                    ),
                  ],
                ],
              ),
              const SizedBox(width: 9),
              Text(
                level == null ? l10n.roomQualityMeasuring : _words[level],
                style: tx(color, size: 12, weight: w500),
              ),
              if (short.isNotEmpty) ...[
                const SizedBox(width: 8),
                Text(short, style: tx(Colors.white70, size: 12)),
              ],
            ],
          ),
        ),
        AnimatedSize(
          duration: springTime,
          curve: spring,
          alignment: Alignment.topRight,
          child: !open || q == null
              ? const SizedBox(width: 0)
              : Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Container(
                    width: 240,
                    padding: const EdgeInsets.all(12),
                    decoration: BoxDecoration(
                      color: const Color.fromRGBO(16, 16, 16, .9),
                      borderRadius: BorderRadius.circular(16),
                      border: Border.all(
                        color: Colors.white.withValues(alpha: .08),
                      ),
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        Text(
                          _titles[q.level],
                          style: tx(Colors.white, size: 13, weight: w600),
                        ),
                        const SizedBox(height: 8),
                        _figure(
                          l10n.roomQualityDelay,
                          figure(q.delayMs, l10n.roomQualityMs),
                        ),
                        _figure(
                          l10n.roomQualityLost,
                          figure(
                            q.lossPercent,
                            l10n.roomQualityPercent,
                            digits: 1,
                          ),
                        ),
                        _figure(
                          l10n.roomQualityPicture,
                          q.width == null || q.height == null
                              ? '—'
                              : '${q.width}×${q.height}${fps == null ? '' : ' · ${l10n.streamQualityFps(fps.round())}'}',
                        ),
                        if (path != null)
                          _figure(
                            l10n.roomQualityPath,
                            capitalized(pathWords(path, through: through)),
                            mono: false,
                          ),
                        if (q.level < 2) ...[
                          const SizedBox(height: 4),
                          Text(
                            q.level == 0
                                ? l10n.roomQualityMayFreeze
                                : l10n.roomQualityMayShrink,
                            style: tx(Colors.white70, size: 12, height: 1.4),
                          ),
                        ],
                      ],
                    ),
                  ),
                ),
        ),
      ],
    );
  }

  // A long value, as the name of the member a picture comes through, is cut short.
  Widget _figure(String name, String value, {bool mono = true}) => Padding(
    padding: const EdgeInsets.only(bottom: 4),
    child: Row(
      children: [
        Text(name, style: tx(Colors.white60, size: 12)),
        const SizedBox(width: 12),
        Expanded(
          child: Text(
            value,
            textAlign: TextAlign.right,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: tx(Colors.white, size: 12, monospace: mono),
          ),
        ),
      ],
    ),
  );
}

/// Shows only the enlarged camera or the watched screen, over the whole screen, until
/// the button or Escape brings the room back, or the picture goes away.
Future<void> _fullScreen(BuildContext context, String id, _Shown kind) async {
  // The window leaves full screen however this ends, the room gone meanwhile included.
  try {
    await setWindowFullScreen(true);
    if (!context.mounted) return;
    await Navigator.of(context).push(
      PageRouteBuilder<void>(
        transitionDuration: const Duration(milliseconds: 120),
        reverseTransitionDuration: const Duration(milliseconds: 120),
        pageBuilder: (context, _, _) => _FullScreen(id: id, kind: kind),
        transitionsBuilder: (context, animation, _, child) =>
            FadeTransition(opacity: animation, child: child),
      ),
    );
  } finally {
    await setWindowFullScreen(false);
  }
}

class _FullScreen extends StatefulWidget {
  const _FullScreen({required this.id, required this.kind});
  final String id;
  final _Shown kind;

  @override
  State<_FullScreen> createState() => _FullScreenState();
}

class _FullScreenState extends State<_FullScreen> {
  bool _closing = false;

  void _close() {
    if (_closing) return;
    _closing = true;
    Navigator.of(context).pop();
  }

  /// What to show now, or null once it is gone or no longer chosen.
  _Stream? _stream(AppState state) {
    final call = state.callState.call;
    final me = state.contacts.profile?.person.id;
    if (call == null) return null;
    final person = state.roomState.room?.participants
        .where((p) => p.person.id == widget.id)
        .firstOrNull
        ?.person;
    if (person == null) return null;
    final peer = call.peers[widget.id];
    if (widget.kind == _Shown.ownScreen) {
      if (!call.ownScreen || !call.sharing) return null;
      return _Stream(
        title: l10n.pipYourScreen,
        renderer: call.localScreen,
        live: true,
        watchers: call.watchers,
        closeLabel: l10n.commonBack,
        onClose: () => call.showOwnScreen(false),
        onFullScreen: _close,
        fullScreen: true,
      );
    }
    if (widget.kind == _Shown.camera) {
      if (call.focused != widget.id) return null;
      final mine = widget.id == me;
      final renderer = mine
          ? (call.camera ? call.localCamera : null)
          : (peer != null && peer.cam ? peer.camera : null);
      if (renderer == null) return null;
      return _Stream(
        title: mine ? l10n.pipYourCamera : person.name,
        avatar: person.avatar,
        renderer: renderer,
        mirror: mine && state.prefs.mirrorCamera,
        from: mine ? null : widget.id,
        closeLabel: l10n.commonBack,
        onClose: () => call.focus(null),
        onFullScreen: _close,
        fullScreen: true,
      );
    }
    if (call.watching != widget.id || peer == null || !peer.sharing) {
      return null;
    }
    return _Stream(
      title: l10n.pipScreenOf(person.name),
      renderer: peer.screen,
      live: true,
      watchers: call.watchersOf(widget.id),
      from: widget.id,
      soundOf: peer.sound ? widget.id : null,
      closeLabel: l10n.pipStopWatching,
      onClose: () => call.watch(null),
      onFullScreen: _close,
      fullScreen: true,
    );
  }

  @override
  Widget build(BuildContext context) {
    final stream = _stream(AppScope.only(context, roomCallParts));
    if (stream == null) {
      // Back to the room.
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) _close();
      });
      return const ColoredBox(color: Colors.black);
    }
    // A page of its own: without Material under it, text is drawn underlined.
    return Material(
      color: Colors.black,
      child: CallbackShortcuts(
        bindings: {const SingleActivator(LogicalKeyboardKey.escape): _close},
        child: Focus(autofocus: true, child: stream),
      ),
    );
  }
}
