// A video playing over the window, as the rooms and the direct chats open one: what each
// plays from and what it shows above it are theirs, the player and its controls the same.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:fogata_video/fogata_video.dart';

import '../app_state.dart';
import '../icons.dart';
import '../l10n.dart';
import '../rust/api/types.dart';
import '../safe_image.dart';
import '../shown.dart';
import '../theme.dart';
import '../video_sound.dart';
import '../widgets.dart';
import '../window.dart';

/// `0:42`, or `1:02:05` past an hour.
String videoLength(int ms) {
  final d = Duration(milliseconds: ms);
  final s = (d.inSeconds % 60).toString().padLeft(2, '0');
  if (d.inHours > 0) {
    final m = (d.inMinutes % 60).toString().padLeft(2, '0');
    return '${d.inHours}:$m:$s';
  }
  return '${d.inMinutes}:$s';
}

/// Shows [child] over the window, the rest dimmed: a click beside it or Esc closes it.
Future<void> showOverWindow(BuildContext context, String label, Widget child) {
  return showGeneralDialog<void>(
    context: context,
    barrierDismissible: true,
    barrierLabel: label,
    barrierColor: const Color.fromRGBO(0, 0, 0, .9),
    transitionDuration: springTime,
    pageBuilder: (context, _, _) => child,
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

/// The file a video plays from, and what to do once the player has let go of it (a copy
/// made to play it goes then).
class VideoFile {
  const VideoFile(this.path, {this.letGo});
  final String path;
  final Future<void> Function()? letGo;
}

/// Why a video cannot play, said where it would.
class VideoProblem implements Exception {
  const VideoProblem(this.text);
  final String text;
}

/// A video playing over the window: what [top] shows above it (who sent it, Save, close),
/// and below it play, the time, a bar to move in it, the sound and, with [fullScreen],
/// the whole screen. Until it plays, its first picture with a spinner and [waiting]'s
/// words; [changes] tells when those may have changed.
class VideoViewer extends StatefulWidget {
  const VideoViewer({
    super.key,
    required this.state,
    required this.open,
    required this.cannotPlay,
    required this.video,
    required this.top,
    this.waiting,
    this.changes,
    this.fullScreen = false,
  });

  final AppState state;

  /// Where the video plays from; a [VideoProblem] says why it cannot.
  final Future<VideoFile> Function() open;

  /// Said when this computer cannot play it.
  final String cannotPlay;

  /// The video as it is known now: its size, length and first picture.
  final UiVideo? Function() video;

  /// What shows above the video, with the file it plays from once open.
  final Widget Function(BuildContext context, String? path) top;
  final String? Function()? waiting;
  final Listenable? changes;
  final bool fullScreen;

  @override
  State<VideoViewer> createState() => _VideoViewerState();
}

class _VideoViewerState extends State<VideoViewer> {
  VideoPlayer? _player;
  String? _path;
  String? _problem;
  VideoState _at = const VideoState();
  Timer? _tick;
  late final VideoSound _sound;
  bool _fullScreen = false;
  bool _closed = false;

  /// While the bar is dragged, where it is; the player follows when it is let go.
  double? _dragging;

  /// What the player's file wants once the player let go of it; done once.
  Future<void> Function()? _letGo;

  @override
  void initState() {
    super.initState();
    widget.changes?.addListener(_changed);
    _sound = VideoSound(widget.state, () {
      if (mounted) setState(() {});
    });
    _open();
  }

  /// While the video is being got, what is said of it changes as it goes.
  void _changed() {
    if (mounted && _player == null && _problem == null) setState(() {});
  }

  /// The player's file is let go of once [closing] (the player closing) is over.
  void _release(Future<void> closing) {
    final letGo = _letGo;
    if (letGo == null) return;
    _letGo = null;
    unawaited(closing.then((_) => letGo()).catchError((Object _) {}));
  }

  Future<void> _open() async {
    final VideoFile file;
    try {
      file = await widget.open();
    } on VideoProblem catch (problem) {
      if (mounted && !_closed) setState(() => _problem = problem.text);
      return;
    }
    _letGo = file.letGo;
    if (!mounted || _closed) {
      _release(Future<void>.value());
      return;
    }
    final player = await VideoPlayer.open(
      file.path,
      output: widget.state.prefs.audioOutput,
    );
    if (!mounted || _closed) {
      _release(player?.close() ?? Future<void>.value());
      return;
    }
    if (player == null) {
      _release(Future<void>.value());
      setState(() => _problem = widget.cannotPlay);
      return;
    }
    await _sound.attach(player);
    if (!mounted || _closed) {
      _release(player.close());
      return;
    }
    setState(() {
      _path = file.path;
      _player = player;
    });
    await player.play();
    // Closed while it started: dispose has closed the player, and no timer is left to it.
    if (!mounted || _closed) return;
    _tick = Timer.periodic(const Duration(milliseconds: 200), (_) async {
      // Where the video is, only for the screen.
      if (!windowShown.value) return;
      final at = await player.state();
      if (!mounted || _closed) return;
      setState(() {
        _at = at;
        if (at.error != null) _problem ??= widget.cannotPlay;
      });
    });
  }

  @override
  void dispose() {
    widget.changes?.removeListener(_changed);
    _closed = true;
    _tick?.cancel();
    _sound.dispose();
    // A player still opening is closed where it opens.
    if (_player case final player?) _release(player.close());
    if (_fullScreen) setWindowFullScreen(false);
    super.dispose();
  }

  Future<void> _toggle() async {
    final player = _player;
    if (player == null) return;
    if (_at.playing) {
      await player.pause();
    } else {
      await player.play();
    }
    final at = await player.state();
    if (mounted) setState(() => _at = at);
  }

  Future<void> _toggleFullScreen() async {
    setState(() => _fullScreen = !_fullScreen);
    await setWindowFullScreen(_fullScreen);
  }

  @override
  Widget build(BuildContext context) {
    final close = Navigator.of(context).pop;
    final video = widget.video();
    final ratio = video != null && video.width > 0 && video.height > 0
        ? video.width / video.height
        : 16 / 9;
    final player = _player;
    final duration = _at.durationMs > 0
        ? _at.durationMs
        : (video?.durationMs ?? 0);
    final position = _dragging != null
        ? (_dragging! * duration).round()
        : _at.positionMs.clamp(0, duration == 0 ? 0 : duration);
    final waiting = player == null && _problem == null;
    final words = waiting ? widget.waiting?.call() : null;
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.escape): close,
        const SingleActivator(LogicalKeyboardKey.space): _toggle,
      },
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
                top: _fullScreen ? 0 : 112,
                bottom: _fullScreen ? 0 : 40,
                left: _fullScreen ? 0 : 40,
                right: _fullScreen ? 0 : 40,
                child: Center(
                  child: AspectRatio(
                    aspectRatio: ratio,
                    child: ClipRRect(
                      borderRadius: BorderRadius.circular(_fullScreen ? 0 : 16),
                      child: Stack(
                        fit: StackFit.expand,
                        children: [
                          if (player != null)
                            GestureDetector(
                              onTap: _toggle,
                              child: ColoredBox(
                                color: Colors.black,
                                child: VideoView(player: player),
                              ),
                            )
                          else if (video != null)
                            Image(
                              image: safePicture(video.thumb),
                              fit: BoxFit.cover,
                              gaplessPlayback: true,
                            ),
                          if (waiting || _problem != null)
                            ColoredBox(
                              color: const Color.fromRGBO(0, 0, 0, .55),
                              child: Center(
                                child: _problem != null
                                    ? Text(
                                        _problem!,
                                        style: tx(Colors.white, size: 14),
                                      )
                                    : Column(
                                        mainAxisSize: MainAxisSize.min,
                                        children: [
                                          const Spinner(
                                            size: 18,
                                            color: Colors.white,
                                          ),
                                          if (words != null) ...[
                                            const SizedBox(height: 10),
                                            Text(
                                              words,
                                              style: tx(Colors.white, size: 13),
                                            ),
                                          ],
                                        ],
                                      ),
                              ),
                            ),
                          Positioned(
                            left: 0,
                            right: 0,
                            bottom: 18,
                            child: Center(child: _controls(duration, position)),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
              ),
              if (!_fullScreen)
                Positioned(
                  left: 24,
                  right: 24,
                  top: 56,
                  child: widget.top(context, _path),
                ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _controls(int duration, int position) {
    final player = _player;
    final fraction = duration == 0 ? 0.0 : position / duration;
    return Container(
      width: widget.fullScreen ? 560 : 520,
      height: 48,
      padding: const EdgeInsets.symmetric(horizontal: 8),
      decoration: BoxDecoration(
        color: const Color.fromRGBO(17, 17, 17, .82),
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: Colors.white.withValues(alpha: .08)),
      ),
      child: Row(
        children: [
          Springy(
            onTap: player == null ? null : _toggle,
            label: _at.playing ? l10n.roomPause : l10n.roomPlay,
            builder: (context, hovered) => Container(
              width: 34,
              height: 34,
              alignment: Alignment.center,
              decoration: BoxDecoration(
                color: hovered ? Colors.white : const Color(0xFFEDEDED),
                shape: BoxShape.circle,
              ),
              child: FgIcon(
                _at.playing ? Ic.pause : Ic.play,
                size: 15,
                stroke: 2.6,
                color: const Color(0xFF0A0A0A),
              ),
            ),
          ),
          const SizedBox(width: 10),
          SizedBox(
            width: 96,
            child: Text(
              '${videoLength(position)} / ${videoLength(duration)}',
              style: tx(Colors.white70, size: 12, monospace: true),
            ),
          ),
          Expanded(
            child: SliderTheme(
              data: SliderTheme.of(context).copyWith(
                activeTrackColor: Colors.white,
                inactiveTrackColor: Colors.white24,
                thumbColor: Colors.white,
                overlayColor: Colors.white12,
                trackHeight: 3,
                thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 6),
              ),
              child: Slider(
                value: fraction.clamp(0.0, 1.0),
                onChanged: player == null || duration == 0
                    ? null
                    : (v) => setState(() => _dragging = v),
                onChangeEnd: (v) async {
                  setState(() => _dragging = null);
                  await player?.seek((v * duration).round());
                },
              ),
            ),
          ),
          FgButton(
            icon: _sound.muted ? Ic.volumeOff : Ic.volume,
            kind: Kind.ghost,
            height: 32,
            iconColor: Colors.white70,
            tooltip: _sound.muted ? l10n.roomSoundOn : l10n.roomMuteVideo,
            onTap: player == null ? null : _sound.toggle,
          ),
          if (widget.fullScreen)
            FgButton(
              icon: _fullScreen ? Ic.fullScreenExit : Ic.fullScreen,
              kind: Kind.ghost,
              height: 32,
              iconColor: Colors.white70,
              tooltip: _fullScreen
                  ? l10n.roomLeaveFullScreen
                  : l10n.roomFullScreen,
              onTap: _toggleFullScreen,
            ),
        ],
      ),
    );
  }
}
