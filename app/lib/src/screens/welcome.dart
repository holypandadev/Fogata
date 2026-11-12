import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../app_state.dart';
import '../format.dart';
import '../host.dart';
import '../icons.dart';
import '../l10n.dart';
import '../names.dart';
import 'move_account.dart';
import 'recovery.dart';
import '../photo.dart';
import '../rust/api/app.dart' as api;
import '../rust/api/types.dart';
import '../theme.dart';
import '../version.dart';
import '../widgets.dart';
import '../window.dart';

/// First start: what Fogata is for, beside the card that makes this device's identity.
class Welcome extends StatelessWidget {
  const Welcome({super.key});

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        TitleBar(
          children: [
            IgnorePointer(
              child: Image.asset(
                'assets/images/logo.png',
                width: 18,
                height: 18,
                cacheWidth: (18 * MediaQuery.devicePixelRatioOf(context))
                    .round(),
              ),
            ),
            const SizedBox(width: 8),
            IgnorePointer(child: Text('Fogata', style: wordmark(c.fg))),
            const Spacer(),
            IgnorePointer(
              child: Text(
                'v$appVersion',
                style: tx(c.fg3, size: 12, monospace: true),
              ),
            ),
          ],
        ),
        Expanded(
          child: LayoutBuilder(
            builder: (context, box) {
              final wide = box.maxWidth >= 1080;
              // Side by side down to the smallest window: stacked, the card went
              // below the fold and its focused field scrolled the greeting away.
              final sideBySide = box.maxWidth >= 920;
              final compact = box.maxHeight < 720 || !wide;
              final intro = _Intro(compact: compact);
              final state = AppScope.of(context);
              final Widget card = state.restoring
                  ? const RestoreCard()
                  : state.bringing
                  ? const BringCard()
                  : const _IdentityCard();
              return Stack(
                children: [
                  // Narrower, the content reaches the avatars at the edges: the
                  // sky keeps only its lines and points.
                  Positioned.fill(child: _Sky(avatars: wide)),
                  Positioned.fill(
                    child: SingleChildScrollView(
                      child: ConstrainedBox(
                        constraints: BoxConstraints(minHeight: box.maxHeight),
                        child: Center(
                          child: Padding(
                            padding: EdgeInsets.symmetric(
                              horizontal: 32,
                              vertical: compact ? 12 : 24,
                            ),
                            child: sideBySide
                                ? Row(
                                    mainAxisSize: MainAxisSize.min,
                                    children: [
                                      intro,
                                      SizedBox(width: wide ? 96 : 56),
                                      // A little narrower in the smallest windows.
                                      Flexible(child: card),
                                    ],
                                  )
                                : Column(
                                    mainAxisSize: MainAxisSize.min,
                                    children: [
                                      intro,
                                      const SizedBox(height: 48),
                                      card,
                                    ],
                                  ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ],
              );
            },
          ),
        ),
      ],
    );
  }
}

/// What Fogata is for: the character greets from a call tile, the promise, and three
/// reasons it is different.
class _Intro extends StatelessWidget {
  const _Intro({required this.compact});
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final tileWidth = compact ? 272.0 : 320.0;
    return SizedBox(
      width: compact ? 380 : 460,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          _Tile(width: tileWidth),
          SizedBox(height: compact ? 28 : 40),
          // Its lines as written, the whole smaller where a language's are longer.
          FittedBox(
            fit: BoxFit.scaleDown,
            child: Text(
              l10n.welcomeHeadline,
              textAlign: TextAlign.center,
              softWrap: false,
              style: tx(
                c.fg,
                size: compact ? 42 : 50,
                weight: w600,
                height: 1.04,
                spacing: compact ? -1.6 : -2,
              ),
            ),
          ),
          const SizedBox(height: 16),
          ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 420),
            child: Text(
              l10n.welcomeIntro,
              textAlign: TextAlign.center,
              style: tx(c.fg2, size: 16),
            ),
          ),
          const SizedBox(height: 24),
          Wrap(
            alignment: WrapAlignment.center,
            spacing: 8,
            runSpacing: 8,
            children: [
              _Chip(Ic.lock, l10n.welcomeNoAccount),
              _Chip(Ic.shield, l10n.welcomeEncrypted),
              _Chip(Ic.hourglass, l10n.welcomeEphemeral),
            ],
          ),
        ],
      ),
    );
  }
}

/// The character in a call tile, talking.
class _Tile extends StatelessWidget {
  const _Tile({required this.width});
  final double width;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final image = width * .8;
    return Container(
      width: width,
      height: width * .75,
      clipBehavior: Clip.antiAlias,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(20),
        gradient: LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          colors: c.dark
              ? const [Color(0xFF11141A), Color(0xFF0A0C10)]
              : const [Color(0xFFE8EEF8), Color(0xFFDCE4F1)],
        ),
        boxShadow: [BoxShadow(color: c.green, spreadRadius: 2)],
      ),
      child: Stack(
        children: [
          Positioned(
            left: (width - image) / 2 + width * .03,
            bottom: -width * .075,
            child: Image.asset(
              'assets/images/welcome.png',
              width: image,
              height: image,
              semanticLabel: l10n.welcomeCharacter,
            ),
          ),
          Positioned(
            left: 12,
            bottom: 12,
            child: Container(
              constraints: const BoxConstraints(minHeight: 28),
              padding: const EdgeInsets.only(left: 11, right: 10),
              decoration: BoxDecoration(
                color: Colors.black.withValues(alpha: .62),
                borderRadius: BorderRadius.circular(999),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    'Fogata',
                    style: tx(const Color(0xFFEDEDED), size: 12, weight: w500),
                  ),
                  const SizedBox(width: 8),
                  // A few seconds of a voice, then still: the page may show a long time.
                  SpeakingBars(
                    color: c.green,
                    lasts: const Duration(seconds: 4),
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// A reason in a pill; it lifts a little under the pointer.
class _Chip extends StatefulWidget {
  const _Chip(this.icon, this.text);
  final Ic icon;
  final String text;

  @override
  State<_Chip> createState() => _ChipState();
}

class _ChipState extends State<_Chip> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final color = _hovered ? c.fg : c.fg2;
    return MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: AnimatedSlide(
        offset: Offset(0, _hovered ? -.04 : 0),
        duration: springTime,
        curve: spring,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 200),
          // Taller only when larger text needs it.
          constraints: const BoxConstraints(minHeight: 30),
          padding: const EdgeInsets.only(left: 10, right: 12),
          decoration: BoxDecoration(
            color: _hovered ? c.bg2 : c.bg1,
            borderRadius: BorderRadius.circular(999),
            border: Border.all(color: c.border),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              FgIcon(widget.icon, size: 14, color: color),
              const SizedBox(width: 7),
              Text(widget.text, style: tx(color, size: 13)),
            ],
          ),
        ),
      ),
    );
  }
}

/// Points of the sky, as fractions of the area: where they sit, how big they are, the
/// preset a friend shows (none for a plain point) and whether they are talking. Each
/// picture shows once, and not the character of the call tile.
typedef _Node = ({double x, double y, double size, int? preset, bool talking});

const List<_Node> _nodes = [
  (x: .082, y: .199, size: 54, preset: 2, talking: true),
  (x: .067, y: .709, size: 48, preset: 1, talking: false),
  (x: .918, y: .178, size: 48, preset: 4, talking: false),
  (x: .933, y: .699, size: 54, preset: 3, talking: true),
  (x: .292, y: .088, size: 5, preset: null, talking: false),
  (x: .708, y: .069, size: 5, preset: null, talking: false),
  (x: .032, y: .455, size: 5, preset: null, talking: false),
  (x: .971, y: .441, size: 5, preset: null, talking: false),
  (x: .361, y: .936, size: 5, preset: null, talking: false),
  (x: .646, y: .95, size: 5, preset: null, talking: false),
];

/// Lines between points, by index.
const _links = [
  (0, 4),
  (4, 5),
  (5, 2),
  (0, 6),
  (6, 1),
  (2, 7),
  (7, 3),
  (1, 8),
  (8, 9),
  (9, 3),
];

/// The background: a night sky (a pale one in the light theme) with the friends you talk
/// to, joined by lines.
class _Sky extends StatelessWidget {
  const _Sky({required this.avatars});
  final bool avatars;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final gap = c.dark ? Colors.black : c.bg;
    return LayoutBuilder(
      builder: (context, box) {
        final size = box.biggest;
        return Stack(
          children: [
            Positioned.fill(
              child: RepaintBoundary(
                child: CustomPaint(
                  painter: _SkyPainter(
                    dark: c.dark,
                    bg: c.bg,
                    avatars: avatars,
                  ),
                ),
              ),
            ),
            for (final node in _nodes)
              if (avatars && node.preset != null)
                Positioned(
                  left: node.x * size.width - node.size / 2,
                  top: node.y * size.height - node.size / 2,
                  child: Avatar(
                    avatar: UiAvatar(preset: node.preset!),
                    size: node.size,
                    ring: node.talking
                        ? c.green
                        : (c.dark ? Colors.white : Colors.black).withValues(
                            alpha: .1,
                          ),
                    ringGap: gap,
                  ),
                ),
          ],
        );
      },
    );
  }
}

class _SkyPainter extends CustomPainter {
  _SkyPainter({required this.dark, required this.bg, required this.avatars});
  final bool dark;
  final Color bg;

  /// Avatars sit on some points; without them those are points too.
  final bool avatars;

  @override
  void paint(Canvas canvas, Size size) {
    final rect = Offset.zero & size;
    canvas.drawRect(
      rect,
      Paint()
        ..shader = LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          colors: dark
              ? const [Color(0xFF000000), Color(0xFF030303)]
              : [const Color(0xFFEEF3FB), bg],
          stops: const [0, .8],
        ).createShader(rect),
    );

    if (dark) {
      final random = math.Random(7);
      const radii = [.8, .8, 1.0, 1.0, 1.0, 1.2, 1.6];
      final star = Paint();
      for (var i = 0; i < 110; i++) {
        final x = random.nextDouble();
        final y = random.nextDouble();
        final radius = radii[random.nextInt(radii.length)];
        star.color = Colors.white.withValues(
          alpha: .08 + random.nextDouble() * .32,
        );
        canvas.drawCircle(
          Offset(x * size.width, y * size.height),
          radius,
          star,
        );
      }
    }

    final blue = dark ? const Color(0xFF8CC4FF) : const Color(0xFF0060D1);
    final line = Paint()
      ..strokeWidth = 1
      ..color = blue.withValues(alpha: .12);
    Offset at(int i) =>
        Offset(_nodes[i].x * size.width, _nodes[i].y * size.height);
    for (final (a, b) in _links) {
      canvas.drawLine(at(a), at(b), line);
    }
    final point = Paint()..color = blue.withValues(alpha: .5);
    for (final (i, node) in _nodes.indexed) {
      if (node.preset == null) {
        canvas.drawCircle(at(i), node.size / 2, point);
      } else if (!avatars) {
        canvas.drawCircle(at(i), 2.5, point);
      }
    }
  }

  @override
  bool shouldRepaint(_SkyPainter old) =>
      old.dark != dark || old.bg != bg || old.avatars != avatars;
}

/// Asks the keychain for the key that encrypts this computer's data; when it keeps none,
/// the person chooses between trying again (after unlocking or starting it) and going on
/// unencrypted, knowingly. Whether the profile may be made now; null when the person
/// closed the question (Escape, or outside it) without choosing: not now, and the page
/// says so with its own "Try again", rather than asking again at once.
Future<bool?> _chooseHowDataIsKept(BuildContext context, String dataDir) async {
  while (context.mounted) {
    if (await api.keychainProblem(dataDir: dataDir) == null) return true;
    if (!context.mounted) return false;
    final c = context.fg;
    final plain = await showFgDialog<bool>(
      context,
      builder: (dialog) => Column(
        key: const ValueKey('keychain-problem'),
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.all(24),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  l10n.welcomeKeychainTitle,
                  style: tx(
                    c.fg,
                    size: 20,
                    weight: w600,
                    height: 1.3,
                    spacing: -.4,
                  ),
                ),
                const SizedBox(height: 6),
                Text(
                  l10n.welcomeKeychainText(onLinux ? 'linux' : 'other'),
                  style: tx(c.fg2, height: 1.45),
                ),
                const SizedBox(height: 10),
                Text(
                  l10n.welcomeKeychainPlainNote,
                  style: tx(c.fg3, size: 12, height: 1.45),
                ),
              ],
            ),
          ),
          Footer(
            children: [
              const Spacer(),
              FgButton(
                key: const ValueKey('keychain-plain'),
                text: l10n.welcomeKeychainPlain,
                kind: Kind.secondary,
                onTap: () => Navigator.of(dialog).pop(true),
              ),
              const SizedBox(width: 10),
              FgButton(
                text: l10n.welcomeKeychainRetry,
                onTap: () => Navigator.of(dialog).pop(false),
              ),
            ],
          ),
        ],
      ),
    );
    if (plain == null) return null;
    if (plain) {
      await api.keepUnencrypted(dataDir: dataDir);
      return true;
    }
    // "Try again": the keychain is asked again.
  }
  return false;
}

/// Asks before a new profile is made while the one this computer had is kept unread:
/// whether to make it.
Future<bool?> _confirmNewProfile(BuildContext context) {
  final c = context.fg;
  return showFgDialog<bool>(
    context,
    builder: (dialog) => Column(
      key: const ValueKey('profile-kept-confirm'),
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                l10n.welcomeProfileKeptTitle,
                style: tx(
                  c.fg,
                  size: 20,
                  weight: w600,
                  height: 1.3,
                  spacing: -.4,
                ),
              ),
              const SizedBox(height: 6),
              Text(
                l10n.welcomeProfileKeptConfirm,
                style: tx(c.fg2, height: 1.45),
              ),
            ],
          ),
        ),
        Footer(
          children: [
            const Spacer(),
            FgButton(
              text: l10n.commonCancel,
              kind: Kind.secondary,
              onTap: () => Navigator.of(dialog).pop(false),
            ),
            const SizedBox(width: 10),
            FgButton(
              key: const ValueKey('profile-kept-make'),
              text: l10n.welcomeProfileKeptMake,
              onTap: () => Navigator.of(dialog).pop(true),
            ),
          ],
        ),
      ],
    ),
  );
}

/// Above the card, while the profile this computer had could not be read: that it was
/// kept, where, and that nothing was deleted.
class _KeptNote extends StatelessWidget {
  const _KeptNote({required this.folder});
  final String folder;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Container(
      key: const ValueKey('profile-kept'),
      width: 420,
      padding: const EdgeInsets.fromLTRB(20, 14, 20, 16),
      decoration: BoxDecoration(
        color: c.bg1,
        borderRadius: BorderRadius.circular(18),
        border: Border.all(color: c.red),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            l10n.welcomeProfileKeptTitle,
            style: tx(c.fg, size: 14, weight: w600),
          ),
          const SizedBox(height: 4),
          SelectableText(
            l10n.welcomeProfileKeptText(folder),
            style: tx(c.fg2, size: 13, height: 1.45),
          ),
        ],
      ),
    );
  }
}

enum _Making { empty, ready, busy, done }

/// Making the identity: a name and a picture, previewed as the others will see them, and
/// the key made on this computer.
class _IdentityCard extends StatefulWidget {
  const _IdentityCard();

  @override
  State<_IdentityCard> createState() => _IdentityCardState();
}

class _IdentityCardState extends State<_IdentityCard> {
  final _name = TextEditingController();
  int _preset = 0;
  Uint8List? _photo;
  String? _fingerprint;
  String? _error;
  bool _busy = false;
  bool _done = false;

  /// The keychain could not be asked; the person can ask again.
  bool _keyFailed = false;
  bool _choosing = false;

  @override
  void initState() {
    super.initState();
    _name.addListener(() => setState(() {}));
    WidgetsBinding.instance.addPostFrameCallback((_) => _makeKey());
  }

  /// Makes this computer's key once the person chose how its data is kept: nothing of the
  /// profile is written before that.
  /// Until then (and when it fails) there is no key, and the profile cannot be made.
  Future<void> _makeKey() async {
    if (!mounted || _choosing) return;
    _choosing = true;
    final state = AppScope.read(context);
    try {
      final chosen = await _chooseHowDataIsKept(context, state.dataDir);
      if (chosen == null && mounted) {
        setState(() {
          _keyFailed = true;
          _error = l10n.welcomeKeychainTitle;
        });
      }
      if (chosen != true) return;
      final f = await api.deviceFingerprint(dataDir: state.dataDir);
      if (mounted) {
        setState(() {
          _fingerprint = f;
          _keyFailed = false;
          _error = null;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _keyFailed = true;
          _error = explain(e);
        });
      }
    } finally {
      _choosing = false;
    }
  }

  @override
  void dispose() {
    _name.dispose();
    super.dispose();
  }

  Future<bool> _newKey() async {
    // Nothing of the profile is written before the person chose how data is kept.
    if (_fingerprint == null) return false;
    final state = AppScope.read(context);
    try {
      final f = await api.newDeviceKey(dataDir: state.dataDir);
      if (mounted) setState(() => _fingerprint = f);
      return true;
    } catch (e) {
      if (mounted) setState(() => _error = explain(e));
      return false;
    }
  }

  Future<void> _photoPick() async {
    try {
      final photo = await pickPhoto();
      if (photo != null && mounted) setState(() => _photo = photo);
    } on PictureProblem catch (e) {
      if (mounted) setState(() => _error = e.message);
    } catch (e) {
      if (mounted) setState(() => _error = l10n.welcomeImageUnreadable);
    }
  }

  Future<void> _create() async {
    if (_busy || _done || _fingerprint == null) return;
    final state = AppScope.read(context);
    final name = tidyName(_name.text);
    // The field says what is wrong with it already.
    if (nameProblem(_name.text) != null) return;
    if (name.isEmpty) {
      setState(() => _error = l10n.welcomeWriteName);
      return;
    }
    // The profile this computer had is kept, unread: made only once the person knows.
    if (state.keptProfile != null) {
      _busy = true;
      final make = await _confirmNewProfile(context);
      _busy = false;
      if (make != true || !mounted) return;
    }
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await api.createProfile(
        dataDir: state.dataDir,
        name: name,
        preset: _preset,
        photo: _photo,
      );
      state.keptProfile = null;
      if (mounted) {
        setState(() {
          _busy = false;
          _done = true;
        });
      }
      // A moment on the green "Ready" before the app opens.
      await Future<void>.delayed(const Duration(milliseconds: 650));
      await state.start();
    } catch (e) {
      if (mounted) {
        setState(() {
          _busy = false;
          _done = false;
          _error = explain(e);
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final name = tidyName(_name.text);
    // Said while typing: a name of only blank characters, or with hidden ones, is not taken.
    final problem = nameProblem(_name.text);
    final making = _done
        ? _Making.done
        : _busy
        ? _Making.busy
        : name.isEmpty || problem != null || _fingerprint == null
        ? _Making.empty
        : _Making.ready;
    final error = _error ?? problem;
    // Tighter in a short window, so the link under the card stays in view.
    final short = MediaQuery.sizeOf(context).height < 720;
    final gap = short ? 14.0 : 22.0;
    final kept = AppScope.of(context).keptProfile;
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (kept != null) ...[
          _KeptNote(folder: kept),
          SizedBox(height: short ? 12 : 16),
        ],
        Container(
          width: 420,
          padding: EdgeInsets.fromLTRB(
            32,
            short ? 22 : 30,
            32,
            short ? 20 : 26,
          ),
          decoration: BoxDecoration(
            color: c.bg1,
            borderRadius: BorderRadius.circular(28),
            border: Border.all(color: c.border),
          ),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                l10n.welcomeCardTitle,
                textAlign: TextAlign.center,
                style: tx(c.fg, size: 20, weight: w600, spacing: -.35),
              ),
              const SizedBox(height: 4),
              Text(
                l10n.welcomeCardSub,
                textAlign: TextAlign.center,
                style: tx(c.fg3, size: 13),
              ),
              SizedBox(height: gap),
              _Preview(
                avatar: UiAvatar(preset: _preset, photo: _photo),
                name: name,
              ),
              SizedBox(height: gap),
              _Picker(
                preset: _preset,
                photo: _photo,
                onPreset: (i) => setState(() {
                  _preset = i;
                  _photo = null;
                }),
                onUpload: _photoPick,
              ),
              SizedBox(height: gap),
              _NamePill(controller: _name, making: making, onSubmit: _create),
              if (error != null) ...[
                const SizedBox(height: 12),
                Text(
                  error,
                  textAlign: TextAlign.center,
                  style: tx(c.red, size: 13),
                ),
              ],
              if (_keyFailed) ...[
                const SizedBox(height: 10),
                FgButton(
                  text: l10n.welcomeKeychainRetry,
                  kind: Kind.secondary,
                  onTap: _makeKey,
                ),
              ],
              SizedBox(height: gap),
              _KeyRow(fingerprint: _fingerprint, onNew: _newKey),
            ],
          ),
        ),
        const SizedBox(height: 16),
        ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 400),
          child: Text(
            l10n.welcomeNoServer,
            textAlign: TextAlign.center,
            style: tx(c.fg3, size: 12),
          ),
        ),
        SizedBox(height: short ? 14 : 24),
        const BringLink(),
        const SizedBox(height: 8),
        const RestoreLink(),
      ],
    );
  }
}

/// The picture and name as the others will see them; the picture changes with a fade and a
/// small zoom.
class _Preview extends StatelessWidget {
  const _Preview({required this.avatar, required this.name});
  final UiAvatar avatar;
  final String name;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final photo = avatar.photo;
    return Column(
      children: [
        Container(
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            boxShadow: [
              BoxShadow(color: c.borderStrong, spreadRadius: 4),
              BoxShadow(color: c.bg1, spreadRadius: 3),
            ],
          ),
          child: AnimatedSwitcher(
            duration: springTime,
            switchInCurve: spring,
            switchOutCurve: Curves.easeIn,
            transitionBuilder: (child, animation) => FadeTransition(
              opacity: animation,
              child: ScaleTransition(
                scale: Tween(begin: .86, end: 1.0).animate(animation),
                child: child,
              ),
            ),
            child: Avatar(
              key: ValueKey(
                photo == null ? 'preset ${avatar.preset}' : photo.hashCode,
              ),
              avatar: avatar,
              size: 80,
            ),
          ),
        ),
        const SizedBox(height: 12),
        AnimatedDefaultTextStyle(
          duration: const Duration(milliseconds: 200),
          style: tx(
            name.isEmpty ? c.fg3 : c.fg,
            size: 18,
            weight: w600,
            spacing: -.2,
          ),
          child: Text(
            name.isEmpty ? l10n.welcomeYourName : name,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
        ),
      ],
    );
  }
}

/// The preset pictures and an upload button in one pill; a white ring slides to the one
/// chosen.
class _Picker extends StatelessWidget {
  const _Picker({
    required this.preset,
    required this.photo,
    required this.onPreset,
    required this.onUpload,
  });
  final int preset;
  final Uint8List? photo;
  final ValueChanged<int> onPreset;
  final VoidCallback onUpload;

  static const _size = 40.0;
  static const _gap = 8.0;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final selected = photo == null ? preset : avatarPresets.length;
    final upload = photo;
    return Container(
      padding: const EdgeInsets.all(6),
      decoration: BoxDecoration(
        color: c.bg2,
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: c.border),
      ),
      child: Stack(
        clipBehavior: Clip.none,
        children: [
          AnimatedPositioned(
            duration: springTime,
            curve: spring,
            left: selected * (_size + _gap) - 4,
            top: -4,
            child: IgnorePointer(
              child: Container(
                width: _size + 8,
                height: _size + 8,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  border: Border.all(color: c.fg, width: 2),
                ),
              ),
            ),
          ),
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              for (var i = 0; i < avatarPresets.length; i++) ...[
                Springy(
                  onTap: () => onPreset(i),
                  tooltip: avatarPresets[i].name,
                  label: l10n.welcomePictureLabel(avatarPresets[i].name),
                  child: Avatar.preset(i, size: _size),
                ),
                const SizedBox(width: _gap),
              ],
              Springy(
                onTap: onUpload,
                tooltip: l10n.welcomeUploadPicture,
                child: upload == null
                    ? Container(
                        width: _size,
                        height: _size,
                        alignment: Alignment.center,
                        decoration: BoxDecoration(
                          shape: BoxShape.circle,
                          color: c.hover,
                        ),
                        child: FgIcon(Ic.plus, size: 16, color: c.fg2),
                      )
                    : Avatar(
                        avatar: UiAvatar(preset: 0, photo: upload),
                        size: _size,
                      ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

/// The name field and the button that makes the identity, in one pill. The button lights
/// up once there is a name, shrinks to a spinning circle while the identity is made, and
/// turns green when it is ready.
class _NamePill extends StatefulWidget {
  const _NamePill({
    required this.controller,
    required this.making,
    required this.onSubmit,
  });
  final TextEditingController controller;
  final _Making making;
  final VoidCallback onSubmit;

  @override
  State<_NamePill> createState() => _NamePillState();
}

class _NamePillState extends State<_NamePill> {
  final _focus = FocusNode();

  @override
  void initState() {
    super.initState();
    _focus.addListener(() => setState(() {}));
  }

  @override
  void dispose() {
    _focus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final focused = _focus.hasFocus;
    return AnimatedContainer(
      duration: const Duration(milliseconds: 200),
      height: 54,
      padding: const EdgeInsets.only(left: 20, right: 5),
      decoration: BoxDecoration(
        color: c.bg2,
        borderRadius: BorderRadius.circular(999),
        // Focused, a stronger border; no halo around it.
        border: Border.all(color: focused ? c.fg2 : c.edge),
      ),
      child: Row(
        children: [
          Expanded(
            child: Semantics(
              label: l10n.welcomeNameLabel,
              child: TextField(
                controller: widget.controller,
                focusNode: _focus,
                autofocus: true,
                readOnly:
                    widget.making == _Making.busy ||
                    widget.making == _Making.done,
                // Counted as the core counts a name: an emoji made of several
                // characters takes several.
                inputFormatters: const [CharacterLimit(40)],
                onSubmitted: (_) => widget.onSubmit(),
                cursorColor: c.blue,
                style: tx(c.fg, size: 15),
                decoration: InputDecoration.collapsed(
                  hintText: l10n.welcomeYourName,
                  hintStyle: tx(c.fg3, size: 15),
                ),
              ),
            ),
          ),
          const SizedBox(width: 8),
          _MorphButton(making: widget.making, onTap: widget.onSubmit),
        ],
      ),
    );
  }
}

class _MorphButton extends StatelessWidget {
  const _MorphButton({required this.making, required this.onTap});
  final _Making making;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    const green = greenSolid;
    final (bg, fg) = switch (making) {
      _Making.empty => (c.hover, c.fg3),
      _Making.ready || _Making.busy => (c.pri, c.onPri),
      _Making.done => (green, Colors.white),
    };
    final Widget content = switch (making) {
      _Making.empty || _Making.ready => Row(
        key: const ValueKey('create'),
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(l10n.welcomeCreate, style: tx(fg, weight: w500, height: 1.2)),
          const SizedBox(width: 8),
          FgIcon(Ic.arrowRight, size: 15, color: fg),
        ],
      ),
      _Making.busy => SizedBox(
        key: const ValueKey('busy'),
        width: 16,
        height: 16,
        child: CircularProgressIndicator(
          strokeWidth: 2,
          color: fg,
          backgroundColor: fg.withValues(alpha: .18),
        ),
      ),
      _Making.done => Row(
        key: const ValueKey('done'),
        mainAxisSize: MainAxisSize.min,
        children: [
          FgIcon(Ic.check, size: 15, color: fg),
          const SizedBox(width: 7),
          Text(l10n.welcomeReady, style: tx(fg, weight: w500, height: 1.2)),
        ],
      ),
    };
    return Springy(
      onTap: making == _Making.ready ? onTap : null,
      label: l10n.welcomeCreateLabel,
      hoverScale: 1.03,
      pressScale: .96,
      child: AnimatedSize(
        duration: springTime,
        curve: spring,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 220),
          height: 42,
          constraints: const BoxConstraints(minWidth: 42),
          padding: EdgeInsets.symmetric(
            horizontal: making == _Making.busy ? 0 : 17,
          ),
          alignment: Alignment.center,
          decoration: BoxDecoration(
            color: bg,
            borderRadius: BorderRadius.circular(999),
          ),
          child: AnimatedSwitcher(
            duration: const Duration(milliseconds: 200),
            transitionBuilder: (child, animation) => FadeTransition(
              opacity: animation,
              child: ScaleTransition(
                scale: Tween(begin: .8, end: 1.0).animate(animation),
                child: child,
              ),
            ),
            child: content,
          ),
        ),
      ),
    );
  }
}

/// The key made on this computer, by its fingerprint, and a round button for another one.
/// A new fingerprint rolls in character by character; the button shows a check for a
/// moment.
class _KeyRow extends StatefulWidget {
  const _KeyRow({required this.fingerprint, required this.onNew});
  final String? fingerprint;
  final Future<bool> Function() onNew;

  @override
  State<_KeyRow> createState() => _KeyRowState();
}

class _KeyRowState extends State<_KeyRow> {
  bool _made = false;
  Timer? _timer;

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  Future<void> _new() async {
    if (!await widget.onNew() || !mounted) return;
    setState(() => _made = true);
    _timer?.cancel();
    _timer = Timer(const Duration(milliseconds: 1200), () {
      if (mounted) setState(() => _made = false);
    });
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final text = widget.fingerprint ?? '···· ···· ···· ····';
    final style = tx(c.fg3, size: 12, monospace: true, spacing: .3);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        FgIcon(Ic.shield, size: 13, color: c.green),
        const SizedBox(width: 8),
        Semantics(
          label: l10n.welcomeKeyLabel(text),
          excludeSemantics: true,
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              for (var i = 0; i < text.length; i++)
                _RollingChar(text[i], index: i, style: style),
            ],
          ),
        ),
        const SizedBox(width: 8),
        Springy(
          onTap: _made ? null : _new,
          tooltip: l10n.welcomeNewKey,
          hoverScale: 1.1,
          child: Container(
            width: 24,
            height: 24,
            alignment: Alignment.center,
            decoration: BoxDecoration(shape: BoxShape.circle, color: c.bg2),
            child: AnimatedSwitcher(
              duration: const Duration(milliseconds: 200),
              transitionBuilder: (child, animation) =>
                  ScaleTransition(scale: animation, child: child),
              child: _made
                  ? FgIcon(
                      Ic.check,
                      key: const ValueKey('made'),
                      size: 12,
                      color: c.green,
                    )
                  : FgIcon(
                      Ic.retry,
                      key: const ValueKey('again'),
                      size: 12,
                      color: c.fg2,
                    ),
            ),
          ),
        ),
      ],
    );
  }
}

/// One character that rolls up when it changes; later characters start a little later.
class _RollingChar extends StatelessWidget {
  const _RollingChar(this.char, {required this.index, required this.style});
  final String char;
  final int index;
  final TextStyle style;

  @override
  Widget build(BuildContext context) {
    final delay = index * 25;
    final total = 260 + delay;
    final curve = Interval(delay / total, 1, curve: Curves.easeOutCubic);
    return ClipRect(
      child: AnimatedSwitcher(
        duration: Duration(milliseconds: total),
        switchInCurve: curve,
        switchOutCurve: curve,
        transitionBuilder: (child, animation) {
          final entering = child.key == ValueKey(char);
          final slide = Tween(
            begin: Offset(0, entering ? 1 : -1),
            end: Offset.zero,
          ).animate(animation);
          return SlideTransition(
            position: slide,
            child: FadeTransition(opacity: animation, child: child),
          );
        },
        layoutBuilder: (current, previous) =>
            Stack(children: [...previous, ?current]),
        child: Text(char, key: ValueKey(char), style: style),
      ),
    );
  }
}
