part of '../room.dart';

// The room's controls under the call, in one pill like a dock: microphone, deafen,
// camera, screen, the chat and Leave; and while a direct call waits, how long it waits.

/// The room's controls in one pill, like a dock: microphone, deafen and camera (each with
/// the arrow of its device's panel), screen and the chat, then Leave; files go from the
/// chat's message box.
/// Leaving as the last one asks in place: Leave turns into "Leave and close", with what
/// goes said above, and a click anywhere else turns it back. In a narrow room the buttons
/// get smaller, Leave keeps only its icon, and "Leave and close" asks with the other
/// buttons out of its way, so the buttons keep their size.
/// What closing a room takes away: `1 message goes`, `3 files you shared go`.
@visibleForTesting
String whatGoes(int messages, int files) {
  if (messages == 0 && files == 0) return l10n.roomGoesNothing;
  if (files == 0) return l10n.roomGoesMessages(messages);
  if (messages == 0) return l10n.roomGoesFiles(files);
  return l10n.roomGoesBoth(messages, files);
}

class _Controls extends StatefulWidget {
  const _Controls({required this.connecting, this.chat});
  final bool connecting;

  /// Whether the chat beside the call is shown, for the button that hides or shows it;
  /// null when there is no chat to show.
  final bool? chat;

  @override
  State<_Controls> createState() => _ControlsState();
}

class _ControlsState extends State<_Controls> {
  bool _confirming = false;
  Timer? _back;

  @override
  void dispose() {
    _back?.cancel();
    super.dispose();
  }

  void _leave() {
    final state = AppScope.read(context);
    if (state.callState.callIn(state.roomState.roomGroup) != null) {
      state.hangUp();
      return;
    }
    if (phaseOf(state) != Phase.alone) {
      state.leaveRoom();
      return;
    }
    setState(() => _confirming = true);
    _back?.cancel();
    _back = Timer(const Duration(seconds: 6), () {
      if (mounted) setState(() => _confirming = false);
    });
  }

  void _stay() {
    _back?.cancel();
    if (_confirming) setState(() => _confirming = false);
  }

  /// What leaving takes away, for the line above the buttons.
  String _whatGoes(AppState state) {
    final room = state.roomState.room;
    final me = state.contacts.profile?.person.id;
    final messages = room?.chat.length ?? 0;
    final files = room?.files.where((f) => f.sharedBy.id == me).length ?? 0;
    return whatGoes(messages, files);
  }

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, box) => _bar(context, width: box.maxWidth),
  );

  Widget _bar(BuildContext context, {required double width}) {
    final compact = width < 460;
    // Where "Leave and close" would not fit beside the other buttons, it stands alone
    // while it asks, so no button gets smaller.
    final alone = _confirming && width < 520;
    final c = context.fg;
    final state = AppScope.only(context, roomCallParts);
    final call = state.callState.call;
    final connecting = widget.connecting;
    final side = compact ? 34.0 : 40.0;
    final gap = compact ? 2.0 : 4.0;
    Widget tool(
      Ic icon,
      String tip,
      VoidCallback? onTap, {
      Color? color,
      Color? fill,
      bool on = false,
    }) => Opacity(
      opacity: onTap == null ? .45 : 1,
      child: Springy(
        onTap: onTap,
        tooltip: tip,
        hoverScale: 1.1,
        pressScale: .9,
        builder: (context, hovered) => AnimatedContainer(
          duration: const Duration(milliseconds: 180),
          width: side,
          height: side,
          alignment: Alignment.center,
          decoration: BoxDecoration(
            color: fill ?? (hovered || on ? c.hover : null),
            shape: BoxShape.circle,
          ),
          child: FgIcon(icon, size: compact ? 16 : 18, color: color ?? c.fg2),
        ),
      ),
    );
    // A button with the arrow of its device's panel, in a pill the panel opens from.
    Widget withDevice(Widget button, DeviceKind kind, Color? fill) => Builder(
      builder: (pill) => AnimatedContainer(
        duration: const Duration(milliseconds: 180),
        decoration: BoxDecoration(
          color: fill,
          borderRadius: BorderRadius.circular(999),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            button,
            DeviceArrow(kind: kind, anchor: pill, height: side, gap: 16),
          ],
        ),
      ),
    );
    final muted = call != null && !call.mic;
    final mic = withDevice(
      tool(
        call == null || call.mic ? Ic.mic : Ic.micOff,
        call == null || call.mic
            ? l10n.roomMuteMicrophone
            : l10n.roomUnmuteMicrophone,
        call == null ? null : () => state.setMic(!call.mic),
        color: muted ? c.red : c.fg,
        fill: Colors.transparent,
      ),
      DeviceKind.input,
      muted ? c.redSoft : c.hover,
    );
    final sharing = call != null && call.sharing;
    // The screen and, beside it, how it goes out: like the microphone and its device.
    final screen = AnimatedContainer(
      duration: const Duration(milliseconds: 180),
      decoration: BoxDecoration(
        color: sharing ? c.hover : null,
        borderRadius: BorderRadius.circular(999),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          tool(
            sharing ? Ic.screenOff : Ic.screen,
            sharing ? l10n.roomStopSharingScreen : l10n.roomShareScreen,
            call == null ? null : () => _toggleScreen(context, call),
            color: sharing ? c.blue : c.fg2,
            fill: Colors.transparent,
          ),
          Builder(
            builder: (anchor) => Springy(
              onTap: () => showStreamQuality(anchor),
              tooltip: l10n.streamQualityTitle,
              hoverScale: 1.1,
              builder: (context, hovered) => Container(
                width: 24,
                height: side,
                alignment: Alignment.centerLeft,
                child: FgIcon(
                  Ic.chevronDown,
                  size: 13,
                  color: hovered ? c.fg : c.fg2,
                ),
              ),
            ),
          ),
        ],
      ),
    );
    final deafened = state.callState.deafened;
    final deafen = withDevice(
      tool(
        deafened ? Ic.headphonesOff : Ic.headphones,
        deafened ? l10n.roomUndeafen : l10n.roomDeafen,
        () => state.setDeafened(!deafened),
        color: deafened ? c.red : c.fg,
        fill: Colors.transparent,
      ),
      DeviceKind.output,
      deafened ? c.redSoft : null,
    );
    final cameraOn = call != null && call.camera;
    final camera = withDevice(
      tool(
        cameraOn ? Ic.camera : Ic.cameraOff,
        cameraOn ? l10n.roomCameraOff : l10n.roomCameraOn,
        call == null ? null : () => state.setCamera(!call.camera),
        color: cameraOn ? c.fg : c.fg3,
        fill: Colors.transparent,
      ),
      DeviceKind.camera,
      cameraOn ? c.hover : null,
    );
    final direct = state.callState.callIn(state.roomState.roomGroup);
    final leaveText = direct == null
        ? (connecting ? l10n.commonCancel : l10n.roomLeave)
        : (direct.phase == UiCallPhase.ringing
              ? l10n.commonCancel
              : l10n.roomHangUp);
    // A click anywhere but on the button turns "Leave and close" back into Leave.
    final Widget leave = _confirming
        ? TapRegion(
            key: const ValueKey('confirm'),
            onTapOutside: (_) => _stay(),
            child: FgButton(
              text: l10n.roomLeaveAndClose,
              icon: Ic.leave,
              kind: Kind.danger,
              height: side,
              onTap: () {
                _back?.cancel();
                state.leaveRoom();
              },
            ),
          )
        : FgButton(
            key: const ValueKey('leave'),
            text: compact ? null : leaveText,
            tooltip: compact ? leaveText : null,
            icon: direct == null ? Ic.leave : Ic.hangUp,
            kind: Kind.danger,
            height: side,
            onTap: _leave,
          );
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        AnimatedSize(
          duration: springTime,
          curve: spring,
          child: _confirming
              ? Padding(
                  padding: const EdgeInsets.only(bottom: 8),
                  child: Text(_whatGoes(state), style: tx(c.fg2, size: 12)),
                )
              : const SizedBox(width: double.infinity),
        ),
        // Scales down rather than overflow while the buttons change size.
        FittedBox(
          fit: BoxFit.scaleDown,
          child: Container(
            padding: EdgeInsets.all(compact ? 4 : 6),
            decoration: BoxDecoration(
              color: c.bg2,
              borderRadius: BorderRadius.circular(999),
              border: Border.all(color: c.border),
            ),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                AnimatedSize(
                  duration: springTime,
                  curve: Curves.easeOutCubic,
                  child: alone
                      ? SizedBox(height: side)
                      : Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            mic,
                            SizedBox(width: gap),
                            deafen,
                            SizedBox(width: gap),
                            camera,
                            SizedBox(width: gap),
                            screen,
                            if (call != null &&
                                call.sharing &&
                                call.canShareSound) ...[
                              SizedBox(width: gap),
                              tool(
                                call.shareSound ? Ic.volume : Ic.volumeOff,
                                call.soundWaitsForMic
                                    ? l10n.roomSoundWaitsForMic
                                    : call.shareSound
                                    ? l10n.roomStopSharingSound
                                    : l10n.roomShareSoundToo,
                                () => call.setShareSound(!call.shareSound),
                                color: call.sharingSound
                                    ? c.blue
                                    : (call.shareSound ? c.fg2 : c.fg3),
                                on: call.sharingSound,
                              ),
                            ],
                            if (direct != null &&
                                direct.phase == UiCallPhase.connected) ...[
                              SizedBox(width: gap),
                              Builder(
                                builder: (anchor) => tool(
                                  Ic.userPlus,
                                  direct.addBlocked ?? l10n.roomAddFriendToCall,
                                  direct.addBlocked == null
                                      ? () => showAddToCall(anchor)
                                      : null,
                                ),
                              ),
                            ],
                            if (widget.chat case final chat?) ...[
                              SizedBox(width: gap),
                              tool(
                                Ic.chat,
                                chat ? l10n.roomHideChat : l10n.roomShowChat,
                                () => state.setRoomChat(!chat),
                                color: chat ? c.fg : c.fg2,
                                on: chat,
                              ),
                            ],
                            Container(
                              width: 1,
                              height: 22,
                              color: c.border,
                              margin: EdgeInsets.symmetric(
                                horizontal: compact ? 4 : 6,
                              ),
                            ),
                          ],
                        ),
                ),
                AnimatedSize(
                  duration: springTime,
                  curve: spring,
                  child: AnimatedSwitcher(
                    duration: const Duration(milliseconds: 200),
                    transitionBuilder: (child, animation) => FadeTransition(
                      opacity: animation,
                      child: ScaleTransition(
                        scale: Tween(begin: .9, end: 1.0).animate(animation),
                        child: child,
                      ),
                    ),
                    child: leave,
                  ),
                ),
              ],
            ),
          ),
        ),
      ],
    );
  }
}

/// A screen's or window's thumbnail can be as large as the screen: decoded twice as wide
/// as its 96-pixel box, it still covers the box sharply when it is up to twice as wide as
/// the box's shape.
int _thumbnailWidth(BuildContext context) =>
    (96 * 2 * MediaQuery.devicePixelRatioOf(context)).round();

/// Starts or stops sharing a screen; with several screens or windows, asks which one.
Future<void> _toggleScreen(BuildContext context, Call call) async {
  if (call.sharing) {
    await call.stopSharing();
    return;
  }
  List<DesktopCapturerSource> sources;
  try {
    sources = await call.screens();
  } catch (_) {
    sources = const [];
  }
  if (sources.length <= 1) {
    await call.shareScreen(sources.firstOrNull);
    return;
  }
  if (!context.mounted) return;
  final c = context.fg;
  final chosen = await showFgDialog<DesktopCapturerSource>(
    context,
    maxWidth: 640,
    builder: (dialog) => Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(24, 24, 24, 12),
          child: Text(
            l10n.roomWhatToShare,
            style: tx(c.fg, size: 20, weight: w600, height: 1.3, spacing: -.4),
          ),
        ),
        Flexible(
          child: ListView(
            shrinkWrap: true,
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
            children: [
              for (final source in sources)
                Pressable(
                  onTap: () => Navigator.of(dialog).pop(source),
                  builder: (context, hovered) => Container(
                    padding: const EdgeInsets.all(10),
                    decoration: BoxDecoration(
                      color: hovered ? c.hover : null,
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: Row(
                      children: [
                        Container(
                          width: 96,
                          height: 54,
                          clipBehavior: Clip.antiAlias,
                          // Inside the outline, as when it was drawn under what is inside.
                          padding: const EdgeInsets.all(1),
                          decoration: BoxDecoration(
                            color: c.tile,
                            borderRadius: BorderRadius.circular(6),
                          ),
                          // Over what is inside, which would cover it at the rounded corners.
                          foregroundDecoration: BoxDecoration(
                            borderRadius: BorderRadius.circular(6),
                            border: Border.all(color: c.border),
                          ),
                          child: source.thumbnail == null
                              ? Center(
                                  child: FgIcon(
                                    Ic.screen,
                                    size: 18,
                                    color: c.fg3,
                                  ),
                                )
                              : Image.memory(
                                  source.thumbnail!,
                                  fit: BoxFit.cover,
                                  gaplessPlayback: true,
                                  cacheWidth: _thumbnailWidth(context),
                                ),
                        ),
                        const SizedBox(width: 12),
                        Expanded(
                          child: Text(
                            source.type == SourceType.Screen
                                ? l10n.roomScreenSource(source.name)
                                : source.name,
                            overflow: TextOverflow.ellipsis,
                            style: tx(c.fg),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
            ],
          ),
        ),
        Footer(
          children: [
            const Spacer(),
            FgButton(
              text: l10n.commonCancel,
              onTap: () => Navigator.of(dialog).pop(),
            ),
          ],
        ),
      ],
    ),
  );
  if (chosen != null) await call.shareScreen(chosen);
}

/// While someone has left a direct call: how long the last one left waits for the others.
class _WaitPill extends StatelessWidget {
  const _WaitPill({required this.call});
  final UiCall call;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    const wait = 60;
    return Ticker(
      builder: (context) {
        final since = call.aloneSinceMs;
        final left = since == null
            ? wait
            : (wait - (DateTime.now().millisecondsSinceEpoch - since) ~/ 1000)
                  .clamp(0, wait);
        final clock = '${left ~/ 60}:${(left % 60).toString().padLeft(2, '0')}';
        return Container(
          key: const ValueKey('call-wait'),
          height: 34,
          padding: const EdgeInsets.symmetric(horizontal: 14),
          decoration: BoxDecoration(
            color: c.hover,
            borderRadius: BorderRadius.circular(999),
          ),
          child: Text.rich(
            TextSpan(
              children: withKey(
                since == null ? l10n.roomCallWaits : l10n.roomCallWaitsAlone,
                clock,
                TextStyle(fontFamily: mono, color: c.fg),
              ),
            ),
            style: tx(c.fg2, size: 13, height: 34 / 13),
          ),
        );
      },
    );
  }
}
