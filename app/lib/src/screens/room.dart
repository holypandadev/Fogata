import 'dart:async';
import 'dart:math' as math;

import 'package:file_selector/file_selector.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:intl/intl.dart' show NumberFormat;

import '../main_color.dart';
import '../app_state.dart';
import '../devices.dart';
import '../emoji.dart' show emojiText;
import '../call.dart';
import '../format.dart';
import '../icons.dart';
import '../l10n.dart';
import '../outgoing.dart';
import '../photo.dart';
import '../quality.dart';
import '../reveal.dart';
import '../rust/api/types.dart';
import '../moving.dart' show ChatPicture;
import '../safe_image.dart';
import '../shown.dart';
import '../shown_text.dart';
import '../theme.dart';
import '../typing.dart';
import '../volume.dart';
import '../widgets.dart';
import '../window.dart';
import 'add_to_call.dart';
import 'emoji_picker.dart';
import 'friend_card.dart' show personCardLabel, togglePersonCard;
import 'home.dart' show Ticker;
import 'members.dart' show confirmDialog;
import 'stream_quality.dart';
import 'video_view.dart';
import 'why_unreached.dart';

part 'room/chat.dart';
part 'room/files.dart';

enum Phase { loading, connecting, live, alone, failed }

/// The parts of the state the room page shows: the room, its call, who is who and the
/// person's settings. Toasts, direct chats, the bell and the page around it do not build
/// it again.
const roomParts = {RoomState, CallState, Contacts, Prefs};

/// [roomParts] and what the room's call tells as it goes, for what shows the call's
/// people and controls.
const roomCallParts = {RoomState, CallState, LiveCall, Contacts, Prefs};

/// Builds again when the room's call tells something (someone connects, mutes, shares),
/// besides when what holds it does: the people and controls follow the call while the
/// chat beside them stays as it is.
class LiveWatch extends StatelessWidget {
  const LiveWatch({super.key, required this.builder});
  final WidgetBuilder builder;

  @override
  Widget build(BuildContext context) {
    AppScope.watch<LiveCall>(context);
    return builder(context);
  }
}

/// How long the room waits for the people that were in it before saying it could not
/// reach them.
const _patience = Duration(seconds: 25);

/// How long the room waits, when only members were online, before showing it as empty.
const _glance = Duration(seconds: 4);

Phase phaseOf(AppState state) {
  final room = state.roomState.room;
  if (room == null) return Phase.loading;
  final others = room.participants.where((p) => !p.me).length;
  if (others > 0) return Phase.live;
  final memory = state.roomState.memories[state.roomState.roomGroup];
  if (memory != null && memory.others.isEmpty) {
    final joined = memory.joinedMs ?? 0;
    final waited = DateTime.now().millisecondsSinceEpoch - joined;
    if (memory.othersExpected) {
      return waited < _patience.inMilliseconds
          ? Phase.connecting
          : Phase.failed;
    }
    if (memory.othersMaybe && waited < _glance.inMilliseconds) {
      return Phase.connecting;
    }
  }
  return Phase.alone;
}

/// Shows a group's room, first asking before hanging up a direct call or closing the room
/// this device is alone in.
Future<void> openRoomChecked(
  BuildContext context,
  String group, {
  String? voice,
}) async {
  final state = AppScope.read(context);
  final target = voice ?? state.contacts.voiceToOpen(group);
  // A room a manager keeps this device out of: Join stays off, and says until when.
  final room = state.contacts.voiceRoom(group, target);
  if (room != null && state.contacts.joinedVoice(group) != target) {
    if (AppState.barrierText(room) case final why?) {
      state.say(why);
      return;
    }
  }
  if (!await confirmLeaving(context, group: group, voice: target)) return;
  await state.openRoom(group, voice: target);
}

/// Rings a friend, first asking before declining someone else's ring or closing the room
/// this device is alone in.
Future<void> callFriendChecked(BuildContext context, String friend) async {
  final state = AppScope.read(context);
  if (!await confirmCalling(context, friend)) return;
  state.notices.missedCalls.remove(friend);
  await state.callFriend(friend);
}

/// Leaves the room this device is in, first asking if it is the last one there.
Future<void> leaveRoomChecked(BuildContext context) async {
  final state = AppScope.read(context);
  final current = state.contacts.joinedGroup;
  if (current != null &&
      (current.room?.participants.length ?? 0) <= 1 &&
      !await _confirmClose(context, state, current.id)) {
    return;
  }
  await state.leaveRoom();
}

/// Before ringing `friend`: whether to go on. One call at a time: a ring waiting here from
/// someone else is declined, so asks first; then as [confirmLeaving]. A ring from `friend`
/// is answered instead.
Future<bool> confirmCalling(BuildContext context, String friend) async {
  final state = AppScope.read(context);
  final ringing = state.callState.incomingCall;
  if (ringing != null && ringing.with_.id != friend) {
    final caller = ringing.with_.name;
    final callee = state.contacts.friends
        .where((f) => f.person.id == friend)
        .firstOrNull
        ?.person
        .name;
    final ok = await confirmDialog(
      context,
      title: l10n.roomDeclineCallTitle(caller),
      text: callee == null
          ? l10n.roomDeclineCallTextSomeone(caller)
          : l10n.roomDeclineCallText(caller, callee),
      action: l10n.roomDeclineAndCall,
    );
    if (!ok || !context.mounted) return false;
  }
  return confirmLeaving(context);
}

/// Before going into another room (`group`'s `voice` room) or a call: whether to go on,
/// asking first when that hangs up the direct call this device is in or closes the room
/// it is alone in. One room or call at a time.
Future<bool> confirmLeaving(
  BuildContext context, {
  String? group,
  String? voice,
}) async {
  final state = AppScope.read(context);
  final direct = state.callState.activeCall;
  if (direct != null) {
    if (direct.groupId == group) return true;
    return confirmDialog(
      context,
      title: l10n.roomHangUpTitle(direct.with_.name),
      text: l10n.roomHangUpText,
      action: l10n.roomHangUp,
    );
  }
  final current = state.contacts.joinedGroup;
  if (current == null ||
      (current.id == group &&
          state.contacts.joinedVoice(current.id) == voice) ||
      (current.room?.participants.length ?? 0) > 1) {
    return true;
  }
  return _confirmClose(context, state, current.id);
}

Future<bool> _confirmClose(
  BuildContext context,
  AppState state,
  String group,
) async {
  final c = context.fg;
  final room = state.roomState.roomGroup == group ? state.roomState.room : null;
  final me = state.contacts.profile?.person.id;
  final messages = room?.chat.length;
  final shared = room?.files.where((f) => f.sharedBy.id == me).length;
  final ok = await showFgDialog<bool>(
    context,
    builder: (dialog) => Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                l10n.roomCloseTitle,
                style: tx(
                  c.fg,
                  size: 20,
                  weight: w600,
                  height: 1.3,
                  spacing: -.4,
                ),
              ),
              const SizedBox(height: 6),
              Text(l10n.roomCloseText, style: tx(c.fg2)),
              if (messages != null) ...[
                const SizedBox(height: 16),
                Container(
                  clipBehavior: Clip.antiAlias,
                  // Inside the outline, as when it was drawn under what is inside.
                  padding: const EdgeInsets.all(1),
                  decoration: BoxDecoration(
                    borderRadius: BorderRadius.circular(8),
                  ),
                  // Over what is inside, which would cover it at the rounded corners.
                  foregroundDecoration: BoxDecoration(
                    borderRadius: BorderRadius.circular(8),
                    border: Border.all(color: c.border),
                  ),
                  child: Column(
                    children: [
                      _CountRow(
                        l10n.roomCloseChatMessages,
                        messages,
                        border: true,
                      ),
                      _CountRow(l10n.roomCloseFilesShared, shared ?? 0),
                    ],
                  ),
                ),
              ],
            ],
          ),
        ),
        Footer(
          children: [
            const Spacer(),
            FgButton(
              text: l10n.roomStay,
              onTap: () => Navigator.of(dialog).pop(false),
            ),
            const SizedBox(width: 10),
            FgButton(
              text: l10n.roomLeaveAndClose,
              kind: Kind.danger,
              onTap: () => Navigator.of(dialog).pop(true),
            ),
          ],
        ),
      ],
    ),
  );
  return ok ?? false;
}

class _CountRow extends StatelessWidget {
  const _CountRow(this.label, this.count, {this.border = false});
  final String label;
  final int count;
  final bool border;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Container(
      height: 40,
      padding: const EdgeInsets.symmetric(horizontal: 12),
      decoration: BoxDecoration(
        border: border ? Border(bottom: BorderSide(color: c.border)) : null,
      ),
      child: Row(
        children: [
          Expanded(child: Text(label, style: tx(c.fg2, size: 13))),
          Text('$count', style: tx(c.fg, size: 13, monospace: true)),
        ],
      ),
    );
  }
}

/// Picks files to share in the room: they wait in the chat for Send, with a message.
Future<void> pickToShare(BuildContext context) async {
  final state = AppScope.read(context);
  final files = await openFiles();
  if (files.isEmpty) return;
  state.addOutgoing(files.map((f) => f.path));
}

/// The coloured pill next to the room's name in the header.
class RoomStatusPill extends StatelessWidget {
  const RoomStatusPill({super.key});

  @override
  Widget build(BuildContext context) {
    final state = AppScope.only(context, roomParts);
    final c = context.fg;
    return Ticker(
      builder: (context) => switch (phaseOf(state)) {
        Phase.live => Pill(
          text: l10n.roomLivePrefix,
          monoTail: elapsed(state.roomState.room!.startedMs),
          color: c.green,
          background: c.greenSoft,
          leading: Dot(c.green),
        ),
        Phase.connecting || Phase.loading => Pill(
          text: l10n.roomPillConnecting,
          color: c.blue,
          background: c.blueSoft,
          leading: Spinner(size: 10, stroke: 1.5, color: c.blue),
        ),
        Phase.alone => Pill(
          text: l10n.roomPillOnlyYou,
          color: c.amber,
          background: c.amberSoft,
          leading: Dot(c.amber),
        ),
        Phase.failed => Pill(
          text: l10n.roomPillError,
          color: c.red,
          background: c.redSoft,
          leading: Dot(c.red),
        ),
      },
    );
  }
}

/// Rebuilds its child when the room's phase changes with time (waiting for the others
/// turns into "could not connect"), checking once a second; everything else rebuilds with
/// the state, so the tiles and the chat are not redrawn every second.
class _PhaseWatch extends StatefulWidget {
  const _PhaseWatch({required this.builder});
  final Widget Function(BuildContext context, Phase phase) builder;

  @override
  State<_PhaseWatch> createState() => _PhaseWatchState();
}

class _PhaseWatchState extends State<_PhaseWatch> {
  Timer? _timer;
  Phase? _shown;

  @override
  void initState() {
    super.initState();
    _timer = Timer.periodic(const Duration(seconds: 1), (_) {
      // Nothing to show while the window is hidden; the first check after catches up.
      if (!mounted || !windowShown.value) return;
      if (phaseOf(AppScope.read(context)) != _shown) setState(() {});
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final phase = _shown = phaseOf(AppScope.only(context, roomParts));
    return widget.builder(context, phase);
  }
}

/// A group's room: the people, the controls, and the chat and files beside them.
class RoomPage extends StatelessWidget {
  const RoomPage({super.key});

  @override
  Widget build(BuildContext context) {
    return _PhaseWatch(
      builder: (context, watched) {
        final state = AppScope.only(context, roomParts);
        final call = state.callState.callIn(state.roomState.roomGroup);
        // The last one left in a direct call waits there for the others, as in the call.
        final waiting =
            call != null &&
            call.phase == UiCallPhase.connected &&
            watched == Phase.alone;
        final phase = waiting ? Phase.live : watched;
        final ending = state.callState.callEnding;
        if (ending != null && ending.group == state.roomState.roomGroup) {
          return _CallEnded(state: state, ending: ending);
        }
        if (call != null && call.phase == UiCallPhase.ringing) {
          return Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Expanded(child: _Calling(call: call)),
                const SizedBox(height: 12),
                const _Controls(connecting: false),
              ],
            ),
          );
        }
        // Files dropped anywhere on the room go to its chat while it can take them.
        return DropZone(
          state: state,
          enabled: phase == Phase.live || phase == Phase.alone,
          child: LayoutBuilder(
            builder: (context, box) =>
                _withChat(context, state, phase, call, box.maxWidth),
          ),
        );
      },
    );
  }

  Widget _withChat(
    BuildContext context,
    AppState state,
    Phase phase,
    UiCall? call,
    double width,
  ) {
    final c = context.fg;
    // A small room starts with the chat hidden. A little room to spare before it counts
    // as wide again, so a window resized around the edge does not flip back and forth.
    state.roomState.roomSmall =
        width < (state.roomState.roomSmall ? _smallRoom + 24 : _smallRoom);
    final chat = state.roomChatShown;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Expanded(
          child: LiveWatch(
            builder: (context) => Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  if (phase == Phase.alone) ...[
                    _AloneBanner(state: state),
                    const SizedBox(height: 12),
                  ],
                  if (phase == Phase.connecting || phase == Phase.loading) ...[
                    _Connecting(state: state),
                    const SizedBox(height: 12),
                  ],
                  Expanded(
                    child: switch (phase) {
                      Phase.failed => _Failed(state: state),
                      Phase.alone => _Alone(state: state),
                      Phase.connecting ||
                      Phase.loading => _Tiles(state: state, connecting: true),
                      Phase.live => _Tiles(state: state),
                    },
                  ),
                  if (call != null &&
                      call.phase == UiCallPhase.connected &&
                      (call.away.isNotEmpty || call.aloneSinceMs != null)) ...[
                    const SizedBox(height: 12),
                    Center(child: _WaitPill(call: call)),
                  ],
                  // Also when the room could not be reached: the microphone may be open.
                  const SizedBox(height: 12),
                  const OwnNetworkLine(),
                  _Controls(
                    connecting:
                        phase == Phase.connecting || phase == Phase.loading,
                    chat: phase == Phase.failed ? null : chat,
                  ),
                ],
              ),
            ),
          ),
        ),
        if (phase != Phase.failed)
          _ChatAside(
            shown: chat,
            child: Container(
              width: _chatWidth,
              decoration: BoxDecoration(
                color: c.bg1,
                border: Border(left: BorderSide(color: c.border)),
              ),
              child: _Panel(
                state: state,
                connecting: phase == Phase.connecting || phase == Phase.loading,
              ),
            ),
          ),
      ],
    );
  }
}

/// The width of the room's chat and files beside the call.
const _chatWidth = 340.0;

/// Narrower than this, a room page with the chat beside the call leaves the controls less
/// than their whole row (460 points, see [_Controls]): about a window under 1144 points.
const _smallRoom = 460 + _chatWidth + 32;

/// The chat beside the call, sliding out of the way while hidden. Hidden, it keeps what
/// was typed and where it was read, and takes no keys; an emoji picker opened from it
/// stays inside it, and closes as it starts to hide.
class _ChatAside extends StatelessWidget {
  const _ChatAside({required this.shown, required this.child});
  final bool shown;
  final Widget child;

  @override
  Widget build(BuildContext context) => TweenAnimationBuilder<double>(
    tween: Tween(end: shown ? _chatWidth : 0),
    duration: springTime,
    curve: spring,
    builder: (context, width, child) => SizedBox(
      width: width.clamp(0, _chatWidth),
      child: ClipRect(
        child: OverflowBox(
          alignment: Alignment.centerLeft,
          minWidth: _chatWidth,
          maxWidth: _chatWidth,
          child: Visibility(
            visible: width > .5,
            maintainState: true,
            child: ExcludeFocus(
              excluding: !shown,
              child: PickerArea(shown: shown, child: child!),
            ),
          ),
        ),
      ),
    ),
    child: child,
  );
}

class _AloneBanner extends StatelessWidget {
  const _AloneBanner({required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final someoneLeft =
        state
            .roomState
            .memories[state.roomState.roomGroup]
            ?.others
            .isNotEmpty ??
        false;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      decoration: BoxDecoration(
        color: c.amberSoft,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: c.amberLine),
      ),
      child: Row(
        children: [
          FgIcon(Ic.warning, size: 16, color: c.amber),
          const SizedBox(width: 10),
          Expanded(
            child: Text.rich(
              TextSpan(
                children: [
                  TextSpan(
                    text: someoneLeft ? l10n.roomLastOne : l10n.roomOnlyYouIn,
                    style: const TextStyle(fontWeight: w600),
                  ),
                  TextSpan(text: l10n.roomAloneWarning),
                ],
              ),
              style: tx(c.amber),
            ),
          ),
        ],
      ),
    );
  }
}

/// A participant's tile: their avatar, or later their camera.
class _Tile extends StatelessWidget {
  const _Tile({
    required this.person,
    required this.label,
    this.avatarSize = 88,
    this.away = false,
    this.connecting = false,
    this.connected = false,
    this.video,
    this.mirror = false,
    this.speaking = false,
    this.muted = false,
    this.volume = 100,
    this.silenced = false,
    this.quality = false,
    this.onTap,
  });

  final UiPerson person;
  final String label;
  final double avatarSize;

  /// Shows how the person's connection is, top right, and dims them while it is lost.
  final bool quality;

  /// A click on the tile: enlarges a camera.
  final VoidCallback? onTap;
  final bool away;
  final bool connecting;
  final bool connected;

  /// Camera to show instead of the avatar.
  final RTCVideoRenderer? video;
  final bool mirror;
  final bool speaking;
  final bool muted;

  /// How loud this person plays on this device, in percent, and whether not at all.
  final int volume;
  final bool silenced;

  @override
  Widget build(BuildContext context) {
    final video = this.video;
    if (video == null) return _build(context, null);
    // The picture once its first frame came; until then, as with no camera, the avatar.
    return ValueListenableBuilder<RTCVideoValue>(
      valueListenable: video,
      builder: (context, value, _) =>
          _build(context, value.width > 0 ? video : null),
    );
  }

  Widget _build(BuildContext context, RTCVideoRenderer? video) {
    final c = context.fg;
    // As wide as its words, and a long name is cut short in a narrow tile. Screen
    // readers hear it as the tile's label instead.
    final name = ExcludeSemantics(
      child: Container(
        height: 26,
        padding: const EdgeInsets.symmetric(horizontal: 8),
        decoration: BoxDecoration(
          color: connecting ? c.bg1 : const Color.fromRGBO(0, 0, 0, .6),
          borderRadius: BorderRadius.circular(6),
          boxShadow: connecting
              ? [BoxShadow(color: c.border, spreadRadius: 1)]
              : null,
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (connected) ...[
              const Dot(Color(0xFF3DD68C)),
              const SizedBox(width: 6),
            ],
            Flexible(
              child: Text(
                label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: tx(
                  connecting ? c.fg2 : Colors.white,
                  size: 12,
                  weight: w500,
                  height: 1.2,
                ),
              ),
            ),
            if (silenced) ...[
              const SizedBox(width: 6),
              FgIcon(Ic.volumeOff, size: 12, color: c.red),
            ] else if (volume != 100) ...[
              const SizedBox(width: 6),
              Text(
                l10n.volumePercent(volume),
                style: tx(
                  connecting ? c.fg3 : Colors.white70,
                  size: 11,
                  monospace: true,
                  height: 1.2,
                ),
              ),
            ],
            if (speaking) ...[
              const SizedBox(width: 8),
              const SpeakingBars(color: Color(0xFF3DD68C)),
            ] else if (muted) ...[
              const SizedBox(width: 6),
              const FgIcon(Ic.micOff, size: 12, color: Colors.white),
            ],
          ],
        ),
      ),
    );
    final tag = Positioned(
      left: 10,
      right: 10,
      bottom: 10,
      child: Align(alignment: Alignment.centerLeft, child: name),
    );
    // To a screen reader, the tile is its name, whether the microphone is off and how
    // loud it plays here, said once: the icons and the name's tag say it to the eye.
    var said = muted ? l10n.roomTileMicOff(label) : label;
    if (silenced) {
      said = l10n.roomTileMutedByYou(said);
    } else if (volume != 100) {
      said = l10n.roomTileVolume(said, l10n.volumePercent(volume));
    }
    if (connecting) {
      return Semantics(
        container: true,
        label: said,
        child: Skeleton(radius: 12, child: Stack(children: [tag])),
      );
    }
    // Without a picture, the tile takes on the main colour of the avatar shown in it,
    // mixed into the usual one so the name and the icons on it still read.
    Widget tinted(Widget Function(Color colour) build) => video != null
        ? build(c.tile)
        : MainColour(
            avatar: person.avatar,
            builder: (context, colour) => build(
              colour == null ? c.tile : Color.lerp(c.tile, colour, .35)!,
            ),
          );
    final tile = tinted(
      (colour) => AnimatedContainer(
        duration: const Duration(milliseconds: 240),
        decoration: BoxDecoration(
          color: colour,
          borderRadius: BorderRadius.circular(12),
          boxShadow: [
            BoxShadow(
              color: speaking ? c.green : c.border,
              spreadRadius: speaking ? 2 : 1,
            ),
          ],
        ),
        child: Stack(
          children: [
            if (video != null)
              Positioned.fill(
                child: QualityDim(
                  id: quality ? person.id : null,
                  child: ClipRRect(
                    borderRadius: BorderRadius.circular(12),
                    child: RTCVideoView(
                      video,
                      mirror: mirror,
                      objectFit:
                          RTCVideoViewObjectFit.RTCVideoViewObjectFitCover,
                    ),
                  ),
                ),
              )
            else
              Positioned.fill(
                child: _TileAvatar(
                  avatar: person.avatar,
                  id: person.id,
                  size: avatarSize,
                  dim: away,
                  quality: quality ? person.id : null,
                  name: name,
                ),
              ),
            if (video != null) tag,
            if (quality)
              Positioned(right: 10, top: 10, child: TileQuality(id: person.id)),
          ],
        ),
      ),
    );
    final onTap = this.onTap;
    if (onTap == null) {
      return Semantics(label: said, child: tile);
    }
    // Reached with Tab and Enter too, with the focus ring around it.
    return Pressable(
      onTap: onTap,
      label: said,
      focusRadius: 12,
      builder: (context, _) => tile,
    );
  }
}

/// A tile's avatar, as large as asked and in the middle, with the [name] at the bottom
/// left. In a low tile (in the strip under a screen or camera, or in a full grid) the
/// avatar gets smaller, still in the middle, to stay clear of the name.
class _TileAvatar extends StatelessWidget {
  const _TileAvatar({
    required this.avatar,
    required this.id,
    required this.size,
    required this.dim,
    required this.quality,
    required this.name,
  });
  final UiAvatar avatar;
  final String id;
  final double size;
  final bool dim;

  /// Whose connection dims the avatar while it is lost.
  final String? quality;
  final Widget name;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, box) {
      Widget face(double size) => QualityDim(
        id: quality,
        child: Avatar(avatar: avatar, size: size, dim: dim, id: id),
      );
      // The name's band at the bottom (10 + 26) and a gap (2), kept free above the
      // avatar too so it stays in the middle.
      final fits = math
          .min(box.maxWidth - 24, box.maxHeight - 76)
          .clamp(24, size)
          .toDouble();
      return Stack(
        children: [
          Center(child: face(fits)),
          Positioned(
            left: 10,
            right: 10,
            bottom: 10,
            child: Align(alignment: Alignment.centerLeft, child: name),
          ),
        ],
      );
    },
  );
}

/// How tiles fill an area: how many columns, the size of each, and whether they scroll.
typedef GridFit = ({int columns, Size tile, bool scrolls});

/// The least a tile is drawn at: smaller, its name and controls would not fit. Below it
/// the tiles keep this size and scroll.
const minTile = Size(160, 100);

/// Space between tiles.
const tileGap = 12.0;

/// Where `n` tiles go in `area`: as large as it allows, never taller than 3:4 of their
/// width nor wider than 16:9. When even the best fit is smaller than [minTile], as many
/// columns as fit at its width, and the rows scroll.
@visibleForTesting
GridFit gridFit(int n, Size area) {
  GridFit best = (columns: 1, tile: Size.zero, scrolls: false);
  for (var columns = 1; columns <= n; columns++) {
    final rows = (n / columns).ceil();
    final width = (area.width - tileGap * (columns - 1)) / columns;
    final fit = (area.height - tileGap * (rows - 1)) / rows;
    if (width <= 0 || fit <= 0) continue;
    final height = math.min(width * .75, fit);
    final tile = Size(math.min(width, height * 16 / 9), height);
    if (tile.width * tile.height > best.tile.width * best.tile.height + .5) {
      best = (columns: columns, tile: tile, scrolls: false);
    }
  }
  if (best.tile.width >= minTile.width && best.tile.height >= minTile.height) {
    return best;
  }
  final columns = ((area.width + tileGap) / (minTile.width + tileGap))
      .floor()
      .clamp(1, math.max(1, n))
      .toInt();
  final width = math.max(
    minTile.width,
    (area.width - tileGap * (columns - 1)) / columns,
  );
  return (
    columns: columns,
    tile: Size(width, math.max(minTile.height, width * 9 / 16)),
    scrolls: true,
  );
}

/// Tiles in rows and columns, centred, as large as [gridFit] allows; they scroll when
/// there are too many to keep a usable size.
class _Grid extends StatelessWidget {
  const _Grid({required this.children});
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final n = children.length;
    return LayoutBuilder(
      builder: (context, box) {
        final fit = gridFit(n, box.biggest);
        final rows = (n / fit.columns).ceil();
        final grid = Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            for (var r = 0; r < rows; r++) ...[
              if (r > 0) const SizedBox(height: tileGap),
              Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  for (
                    var col = 0;
                    col < fit.columns && r * fit.columns + col < n;
                    col++
                  ) ...[
                    if (col > 0) const SizedBox(width: tileGap),
                    SizedBox.fromSize(
                      size: fit.tile,
                      child: children[r * fit.columns + col],
                    ),
                  ],
                ],
              ),
            ],
          ],
        );
        if (!fit.scrolls) return Center(child: grid);
        return PageScroll(
          child: ConstrainedBox(
            constraints: BoxConstraints(
              minWidth: box.maxWidth,
              minHeight: box.maxHeight,
            ),
            child: Center(child: grid),
          ),
        );
      },
    );
  }
}

/// The others under a watched screen or an enlarged camera: one row of tiles, centred.
/// When they do not all fit, the row scrolls sideways, with the mouse wheel or the
/// arrows at its ends.
class _Strip extends StatefulWidget {
  const _Strip({required this.tiles});
  final List<Widget> tiles;

  /// The narrowest and widest a tile of the strip is drawn.
  static const minWidth = 136.0;
  static const maxWidth = 220.0;

  @override
  State<_Strip> createState() => _StripState();
}

class _StripState extends State<_Strip> {
  final _scroll = ScrollController();
  bool _back = false;
  bool _forward = false;

  /// Whether [_ends] runs after the frame being built: once per frame, however many
  /// builds it has.
  bool _endsPending = false;

  @override
  void initState() {
    super.initState();
    _scroll.addListener(_ends);
  }

  @override
  void dispose() {
    _scroll.dispose();
    super.dispose();
  }

  /// Shows an arrow at each end with more tiles past it.
  void _ends() {
    if (!mounted || !_scroll.hasClients) return;
    final at = _scroll.position;
    final back = at.pixels > 1;
    final forward = at.pixels < at.maxScrollExtent - 1;
    if (back != _back || forward != _forward) {
      setState(() {
        _back = back;
        _forward = forward;
      });
    }
  }

  void _page(int direction) {
    if (!_scroll.hasClients) return;
    final at = _scroll.position;
    _scroll.animateTo(
      (at.pixels + direction * at.viewportDimension * .8).clamp(
        0,
        at.maxScrollExtent,
      ),
      duration: springTime,
      curve: Curves.easeOutCubic,
    );
  }

  /// The mouse wheel turned up or down moves the row sideways.
  void _wheel(PointerSignalEvent event) {
    if (event is! PointerScrollEvent || !_scroll.hasClients) return;
    final delta = event.scrollDelta.dx != 0
        ? event.scrollDelta.dx
        : event.scrollDelta.dy;
    if (delta == 0) return;
    GestureBinding.instance.pointerSignalResolver.register(event, (_) {
      final at = _scroll.position;
      _scroll.jumpTo((at.pixels + delta).clamp(0, at.maxScrollExtent));
    });
  }

  Widget _arrow(Ic icon, String tip, int direction) => Springy(
    onTap: () => _page(direction),
    tooltip: tip,
    builder: (context, hovered) {
      final c = context.fg;
      return Container(
        width: 32,
        height: 32,
        alignment: Alignment.center,
        decoration: BoxDecoration(
          color: hovered ? Color.lerp(c.bg2, c.fg, .06) : c.bg2,
          shape: BoxShape.circle,
          border: Border.all(color: c.borderStrong),
        ),
        child: FgIcon(icon, size: 14, color: c.fg),
      );
    },
  );

  @override
  Widget build(BuildContext context) {
    final tiles = widget.tiles;
    final n = tiles.length;
    return LayoutBuilder(
      builder: (context, box) {
        final width = ((box.maxWidth - tileGap * (n - 1)) / n).clamp(
          _Strip.minWidth,
          _Strip.maxWidth,
        );
        if (n * width + tileGap * (n - 1) <= box.maxWidth + .5) {
          return Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              for (var i = 0; i < n; i++) ...[
                if (i > 0) const SizedBox(width: tileGap),
                SizedBox(width: width, child: tiles[i]),
              ],
            ],
          );
        }
        if (!_endsPending) {
          _endsPending = true;
          WidgetsBinding.instance.addPostFrameCallback((_) {
            _endsPending = false;
            _ends();
          });
        }
        return Stack(
          children: [
            Positioned.fill(
              child: Listener(
                onPointerSignal: _wheel,
                child: ListView.separated(
                  controller: _scroll,
                  scrollDirection: Axis.horizontal,
                  itemCount: n,
                  separatorBuilder: (context, _) =>
                      const SizedBox(width: tileGap),
                  itemBuilder: (context, i) =>
                      SizedBox(width: width, child: tiles[i]),
                ),
              ),
            ),
            if (_back)
              Positioned(
                left: 6,
                top: 0,
                bottom: 0,
                child: Center(
                  child: _arrow(Ic.chevronLeft, l10n.roomEarlierPeople, -1),
                ),
              ),
            if (_forward)
              Positioned(
                right: 6,
                top: 0,
                bottom: 0,
                child: Center(
                  child: _arrow(Ic.chevronRight, l10n.roomMorePeople, 1),
                ),
              ),
          ],
        );
      },
    );
  }
}

/// Builds again each time who is talking in [call] changes, while the rest of the room
/// stays as it is.
class Talking extends StatelessWidget {
  const Talking({super.key, required this.call, required this.builder});
  final Call? call;
  final WidgetBuilder builder;

  @override
  Widget build(BuildContext context) {
    final talking = call?.talking;
    if (talking == null) return builder(context);
    return ListenableBuilder(
      listenable: talking,
      builder: (context, _) => builder(context),
    );
  }
}

class _Tiles extends StatelessWidget {
  const _Tiles({required this.state, this.connecting = false});
  final AppState state;
  final bool connecting;

  @override
  Widget build(BuildContext context) {
    final room = state.roomState.room;
    final profile = state.contacts.profile!;
    if (connecting) {
      final expected = _expected(state);
      final paths = state.contacts.paths[state.roomState.roomGroup] ?? const {};
      return _Grid(
        children: [
          _Tile(
            person: profile.person,
            label: l10n.roomTileYou(profile.person.name),
            avatarSize: 72,
          ),
          for (final p in expected)
            paths.containsKey(p.id)
                ? _Tile(
                    person: p,
                    label: p.name,
                    avatarSize: 72,
                    connected: true,
                  )
                : _Tile(
                    person: p,
                    label: l10n.roomTileConnecting(p.name),
                    connecting: true,
                  ),
        ],
      );
    }
    final call = state.callState.call;
    final people = [...room!.participants]
      ..sort((a, b) => a.me == b.me ? 0 : (a.me ? 1 : -1));
    // The camera enlarged here, while it is on.
    final focusedId = call?.focused;
    final focusedPeer = focusedId == null ? null : call!.peers[focusedId];
    final focusedPerson = people
        .where((p) => p.person.id == focusedId)
        .firstOrNull
        ?.person;
    final RTCVideoRenderer? focusedCamera = focusedPerson == null
        ? null
        : focusedId == profile.person.id
        ? (call!.camera ? call.localCamera : null)
        : (focusedPeer != null && focusedPeer.connected && focusedPeer.cam
              ? focusedPeer.camera
              : null);
    final tiles = <Widget>[];
    // Shared screens first: this device's own, and a card for each of the others',
    // which is only sent here once watched.
    if (call != null && call.sharing && !call.ownScreen) {
      tiles.add(
        _OwnShareCard(
          watchers: call.watchers,
          onPreview: () => call.showOwnScreen(true),
        ),
      );
    }
    UiParticipant? watched;
    for (final p in people) {
      final peer = call?.peers[p.person.id];
      if (p.me || peer == null || !peer.sharing || !peer.connected) continue;
      if (call!.watching == p.person.id) {
        watched = p;
      } else {
        tiles.add(
          _LiveCard(person: p.person, onWatch: () => call.watch(p.person.id)),
        );
      }
    }
    for (final p in people) {
      // The enlarged camera is not repeated in the strip under it.
      if (focusedCamera != null && p.person.id == focusedId) continue;
      if (p.me) {
        final camera = call != null && call.camera;
        tiles.add(
          Talking(
            call: call,
            builder: (context) => _Tile(
              person: p.person,
              label: l10n.roomTileYou(p.person.name),
              video: camera ? call.localCamera : null,
              mirror: state.prefs.mirrorCamera,
              speaking: call?.speaking ?? false,
              muted: call != null && !call.mic,
              quality: true,
              onTap: camera ? () => call.focus(p.person.id) : null,
            ),
          ),
        );
        continue;
      }
      final peer = call?.peers[p.person.id];
      final reached = peer != null && peer.connected;
      // A camera whose picture failed for good shows the avatar, as one that is off.
      final camera = reached && peer.cam && !peer.camera.failed;
      tiles.add(
        VolumeTarget(
          person: p.person,
          group: state.roomState.roomGroup,
          voice: state.roomState.roomVoice,
          child: Talking(
            call: call,
            builder: (context) => _Tile(
              person: p.person,
              label: p.away
                  ? l10n.roomTileAway(p.person.name)
                  : (peer != null && peer.voiceless
                        ? l10n.roomTileNoSound(p.person.name)
                        : call != null && !reached
                        ? l10n.roomTileConnecting(p.person.name)
                        : p.person.name),
              away: p.away,
              video: camera ? peer.camera : null,
              speaking: reached && peer.speaking,
              muted: reached && !peer.mic,
              volume: state.prefs.volumeOf(p.person.id),
              silenced: state.prefs.silenced.contains(p.person.id),
              quality: true,
              onTap: camera ? () => call!.focus(p.person.id) : null,
            ),
          ),
        ),
      );
    }
    // A direct call: who left a moment ago and may come back, and who is being rung.
    final direct = state.callState.callIn(state.roomState.roomGroup);
    if (direct != null) {
      final inRoom = {for (final p in people) p.person.id};
      for (final p in direct.away) {
        if (inRoom.contains(p.id)) continue;
        tiles.add(
          _Tile(person: p, label: l10n.roomTileLeft(p.name), away: true),
        );
      }
      for (final p in direct.adding) {
        if (inRoom.contains(p.id)) continue;
        tiles.add(
          _RingingTile(person: p, onCancel: () => state.cancelAdd(p.id)),
        );
      }
    }
    if (call != null && call.ownScreen && call.sharing) {
      return _Spotlight(
        id: profile.person.id,
        kind: _Shown.ownScreen,
        title: l10n.pipYourScreen,
        renderer: call.localScreen,
        watchers: call.watchers,
        closeLabel: l10n.commonBack,
        onClose: () => call.showOwnScreen(false),
        strip: tiles,
      );
    }
    if (call != null && focusedCamera != null && focusedPerson != null) {
      final mine = focusedPerson.id == profile.person.id;
      return _Spotlight(
        id: focusedPerson.id,
        kind: _Shown.camera,
        title: mine ? l10n.pipYourCamera : focusedPerson.name,
        avatar: focusedPerson.avatar,
        renderer: focusedCamera,
        mirror: mine && state.prefs.mirrorCamera,
        from: mine ? null : focusedPerson.id,
        closeLabel: l10n.commonBack,
        onClose: () => call.focus(null),
        strip: tiles,
      );
    }
    if (watched != null && call != null) {
      final id = watched.person.id;
      return _Spotlight(
        id: id,
        title: l10n.pipScreenOf(watched.person.name),
        renderer: call.peers[id]!.screen,
        watchers: call.watchersOf(id),
        from: id,
        closeLabel: l10n.pipStopWatching,
        onClose: () => call.watch(null),
        soundOf: call.peers[id]!.sound ? id : null,
        strip: tiles,
      );
    }
    return _Grid(children: tiles);
  }
}

/// Someone shares their screen: nothing of it comes here until it is watched.
class _LiveCard extends StatelessWidget {
  const _LiveCard({required this.person, required this.onWatch});
  final UiPerson person;
  final VoidCallback onWatch;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return LayoutBuilder(
      builder: (context, box) {
        // Small in the strip under a watched screen, or in a short tile where the two lines
        // of words would not fit with the picture: the name and the button only.
        final small = box.maxHeight < 200 || box.maxWidth < 200;
        return Container(
          decoration: BoxDecoration(
            color: c.tile,
            borderRadius: BorderRadius.circular(12),
            boxShadow: [BoxShadow(color: c.border, spreadRadius: 1)],
          ),
          child: Stack(
            children: [
              const Positioned(left: 10, top: 10, child: _LivePill()),
              Center(
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (!small) ...[
                        Avatar(avatar: person.avatar, size: 64, id: person.id),
                        const SizedBox(height: 14),
                      ],
                      Text(
                        small ? person.name : l10n.roomIsSharing(person.name),
                        overflow: TextOverflow.ellipsis,
                        style: tx(c.fg, weight: w600),
                      ),
                      if (!small)
                        Text(
                          l10n.roomNothingSentUntilWatch,
                          textAlign: TextAlign.center,
                          style: tx(c.fg2, size: 13),
                        ),
                      SizedBox(height: small ? 8 : 14),
                      FgButton(
                        text: l10n.roomWatch,
                        icon: Ic.screen,
                        kind: Kind.primary,
                        height: small ? 30 : 36,
                        onTap: onWatch,
                      ),
                    ],
                  ),
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

/// This device's own share, in the tiles: that it is shared and to how many, without its
/// picture. Drawn live, the picture showed in the window shared whenever that was Fogata's
/// (or the screen under it), was captured again with it, and so on without end: every frame
/// changed, and the share went out at a few frames a second. Under Wayland nothing can
/// leave Fogata's window out of what the system's dialog shares, so the picture shows only
/// when asked, enlarged, with a word of that.
class _OwnShareCard extends StatelessWidget {
  const _OwnShareCard({required this.watchers, required this.onPreview});
  final int watchers;
  final VoidCallback onPreview;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return LayoutBuilder(
      builder: (context, box) {
        // Small in the strip, or in a short tile: the words and the button only.
        final small = box.maxHeight < 200 || box.maxWidth < 200;
        return Container(
          decoration: BoxDecoration(
            color: c.tile,
            borderRadius: BorderRadius.circular(12),
            boxShadow: [BoxShadow(color: c.border, spreadRadius: 1)],
          ),
          child: Stack(
            children: [
              const Positioned(left: 10, top: 10, child: _LivePill()),
              Center(
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (!small) ...[
                        FgIcon(Ic.screen, size: 32, color: c.fg2),
                        const SizedBox(height: 12),
                      ],
                      Text(
                        l10n.roomSharingYourScreen,
                        textAlign: TextAlign.center,
                        overflow: TextOverflow.ellipsis,
                        style: tx(c.fg, weight: w600),
                      ),
                      Text(
                        watchers == 0
                            ? l10n.roomNobodyWatching
                            : l10n.roomPeopleWatching(watchers),
                        overflow: TextOverflow.ellipsis,
                        style: tx(c.fg2, size: 13),
                      ),
                      SizedBox(height: small ? 8 : 14),
                      FgButton(
                        text: l10n.roomShowPreview,
                        icon: Ic.eye,
                        kind: Kind.secondary,
                        height: small ? 30 : 36,
                        onTap: onPreview,
                      ),
                      if (!small) ...[
                        const SizedBox(height: 10),
                        Text(
                          l10n.roomPreviewMirror,
                          textAlign: TextAlign.center,
                          style: tx(c.fg3, size: 12),
                        ),
                      ],
                    ],
                  ),
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

class _LivePill extends StatelessWidget {
  const _LivePill();

  @override
  Widget build(BuildContext context) => Container(
    // Taller with larger text, so the word is never cut.
    height: heightForText(context, 20, 11),
    padding: const EdgeInsets.symmetric(horizontal: 7),
    alignment: Alignment.center,
    decoration: BoxDecoration(
      color: context.fg.redSolid,
      borderRadius: BorderRadius.circular(4),
    ),
    child: Text(
      l10n.roomLive,
      style: tx(Colors.white, size: 11, weight: w600, height: 1, spacing: .4),
    ),
  );
}

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
              curve: spring,
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

/// This device rings a friend: their picture with rings going out, and how long it rings.
class _Calling extends StatelessWidget {
  const _Calling({required this.call});
  final UiCall call;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final person = call.with_;
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Rings(
            color: c.amber,
            size: 260,
            child: Avatar(avatar: person.avatar, size: 120, id: person.id),
          ),
          const SizedBox(height: 24),
          Text(
            l10n.roomCallingName(person.name),
            textAlign: TextAlign.center,
            style: tx(c.fg, size: 26, weight: w600, spacing: -.4),
          ),
          const SizedBox(height: 6),
          Text(
            l10n.roomRingsFor30,
            textAlign: TextAlign.center,
            style: tx(c.fg2),
          ),
        ],
      ),
    );
  }
}

/// A call that did not go through: why, and a way to close it or ring again.
class _CallEnded extends StatelessWidget {
  const _CallEnded({required this.state, required this.ending});
  final AppState state;
  final CallEnding ending;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final title = ending.title;
    final again =
        ending.reason != UiCallEnd.declined && ending.reason != UiCallEnd.busy;
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Opacity(
            opacity: .5,
            child: Avatar(avatar: ending.person.avatar, size: 88),
          ),
          const SizedBox(height: 18),
          Text(
            title,
            textAlign: TextAlign.center,
            style: tx(c.fg, size: 20, weight: w600),
          ),
          // A friend online that the call could not reach: what this side's network says.
          if (ending.reason == UiCallEnd.unreachable)
            ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 420),
              child: WhyUnreached(
                firewall: state.notices.firewallCommand != null,
                load: state.reachability,
                center: true,
                size: 13,
                top: 10,
              ),
            ),
          const SizedBox(height: 18),
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              FgButton(text: l10n.commonClose, onTap: state.closeCallPage),
              if (again) ...[
                const SizedBox(width: 8),
                FgButton(
                  text: state.callState.callReturns.contains(ending.person.id)
                      ? l10n.roomReturnToCall
                      : l10n.roomCallAgain,
                  icon: Ic.phone,
                  kind: Kind.primary,
                  onTap: () async {
                    if (!await confirmCalling(context, ending.person.id)) {
                      return;
                    }
                    await state.callAgain();
                  },
                ),
              ],
            ],
          ),
        ],
      ),
    );
  }
}

/// Who was in the room before this device came in.
List<UiPerson> _expected(AppState state) {
  final me = state.contacts.profile?.person.id;
  final summary = state.contacts.group(state.roomState.roomGroup)?.room;
  return [
    for (final p in summary?.participants ?? const <UiPerson>[])
      if (p.id != me) p,
  ];
}

class _Alone extends StatelessWidget {
  const _Alone({required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final profile = state.contacts.profile!;
    final memory = state.roomState.memories[state.roomState.roomGroup];
    final gone = <String>[];
    for (final n in memory?.notes ?? const <RoomNote>[]) {
      final name = state.nameOf(n.device, n.name);
      if (n.left && !gone.contains(name)) gone.add(name);
    }
    final hint = Text(
      gone.isEmpty ? l10n.roomAloneHint : l10n.roomAlreadyLeft(names(gone)),
      style: tx(c.fg3, size: 13),
    );
    // A screen shared before anyone comes in shows beside this device's tile, as it will
    // with others; it is only drawn here until someone comes in to send it to.
    if (state.callState.call?.sharing ?? false) {
      return Column(
        children: [
          Expanded(child: _Tiles(state: state)),
          const SizedBox(height: 12),
          hint,
        ],
      );
    }
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 640),
            child: AspectRatio(
              aspectRatio: 16 / 9,
              child: Talking(
                call: state.callState.call,
                builder: (context) => _Tile(
                  person: profile.person,
                  label: l10n.roomTileYou(profile.person.name),
                  avatarSize: 104,
                  video:
                      state.callState.call != null &&
                          state.callState.call!.camera
                      ? state.callState.call!.localCamera
                      : null,
                  mirror: state.prefs.mirrorCamera,
                  speaking: state.callState.call?.speaking ?? false,
                  muted:
                      state.callState.call != null &&
                      !state.callState.call!.mic,
                ),
              ),
            ),
          ),
          const SizedBox(height: 12),
          hint,
        ],
      ),
    );
  }
}

class _Connecting extends StatelessWidget {
  const _Connecting({required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) =>
      Ticker(builder: (context) => _build(context));

  Widget _build(BuildContext context) {
    final c = context.fg;
    final expected = _expected(state);
    final paths = state.contacts.paths[state.roomState.roomGroup] ?? const {};
    final reached = expected.where((p) => paths.containsKey(p.id)).length;
    final joined =
        state.roomState.memories[state.roomState.roomGroup]?.joinedMs;
    final seconds = joined == null
        ? 0.0
        : (DateTime.now().millisecondsSinceEpoch - joined) / 1000;
    final rows = <Widget>[
      _Step(done: paths.isNotEmpty || reached > 0, text: l10n.roomGroupFound),
      for (final p in expected)
        paths.containsKey(p.id)
            ? _Step(
                done: true,
                name: p.name,
                text: pathWords(
                  paths[p.id]!,
                  through: state.contacts.carrierName(
                    state.roomState.roomGroup,
                    p.id,
                  ),
                ),
              )
            : _Step(done: false, name: p.name, text: l10n.roomLookingForRoute),
    ];
    return Panel(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
            decoration: BoxDecoration(
              border: Border(bottom: BorderSide(color: c.border)),
            ),
            child: Row(
              children: [
                const Spinner(size: 14),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    l10n.roomConnectingTitle,
                    style: tx(c.fg, weight: w600),
                  ),
                ),
                // With nobody known to be in the room yet, no count of them.
                Text(
                  expected.isEmpty
                      ? l10n.roomSeconds(seconds.toStringAsFixed(1))
                      : l10n.roomReachedOf(
                          reached,
                          expected.length,
                          l10n.roomSeconds(seconds.toStringAsFixed(1)),
                        ),
                  style: tx(c.fg3, size: 12, monospace: true),
                ),
              ],
            ),
          ),
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 6),
            child: Column(children: rows),
          ),
        ],
      ),
    );
  }
}

/// Text with one part set apart, as spans: [message] with [key] (a key's name, a clock) in
/// [keyStyle] where its placeholder goes.
List<InlineSpan> withKey(
  String Function(String key) message,
  String key,
  TextStyle keyStyle,
) {
  const mark = '\u{E000}';
  final parts = message(mark).split(mark);
  return [
    for (final (i, part) in parts.indexed) ...[
      if (i > 0) TextSpan(text: key, style: keyStyle),
      if (part.isNotEmpty) TextSpan(text: part),
    ],
  ];
}

/// How someone is reached, in words; [through] names the member that carries them.
String pathWords(UiPath path, {String? through}) => switch (path) {
  UiPath.lan => l10n.roomPathLan,
  UiPath.direct => l10n.roomPathDirect,
  UiPath.holePunched => l10n.roomPathDirect,
  UiPath.throughMember =>
    through == null
        ? l10n.roomPathThroughMember
        : l10n.roomPathThrough(through),
};

class _Step extends StatelessWidget {
  const _Step({required this.done, required this.text, this.name});
  final bool done;
  final String text;

  /// Who the step is about, before the text.
  final String? name;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return SizedBox(
      height: 34,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16),
        child: Row(
          children: [
            if (done)
              Container(
                width: 16,
                height: 16,
                alignment: Alignment.center,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: c.green,
                ),
                child: FgIcon(Ic.check, size: 11, color: c.bg1, stroke: 3),
              )
            else
              const Padding(
                padding: EdgeInsets.symmetric(horizontal: 1),
                child: Spinner(size: 14),
              ),
            const SizedBox(width: 10),
            // One line each, the name and what is happening shortened when the
            // column is narrow: two lines overflowed the row into the next one.
            if (name case final name?) ...[
              Flexible(
                child: Text(
                  name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(done ? c.fg : c.fg2, size: 13),
                ),
              ),
              Text(' · ', style: tx(c.fg3, size: 13)),
            ],
            Flexible(
              child: Text(
                text,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: tx(done ? c.fg : c.fg2, size: 13),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _Failed extends StatelessWidget {
  const _Failed({required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final expected = _expected(state);
    return Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 560),
        child: Panel(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Padding(
                padding: const EdgeInsets.all(24),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Container(
                      width: 40,
                      height: 40,
                      alignment: Alignment.center,
                      decoration: BoxDecoration(
                        shape: BoxShape.circle,
                        color: c.redSoft,
                      ),
                      child: FgIcon(Ic.wifiOff, size: 20, color: c.red),
                    ),
                    const SizedBox(height: 16),
                    Text(
                      l10n.roomCouldNotConnect,
                      style: tx(
                        c.fg,
                        size: 20,
                        weight: w600,
                        height: 1.3,
                        spacing: -.4,
                      ),
                    ),
                    const SizedBox(height: 6),
                    Text(l10n.roomCouldNotConnectText, style: tx(c.fg2)),
                    WhyUnreached(
                      firewall: state.notices.firewallCommand != null,
                      load: state.reachability,
                      size: 13,
                      top: 10,
                    ),
                    if (expected.isNotEmpty) ...[
                      const SizedBox(height: 16),
                      Container(
                        clipBehavior: Clip.antiAlias,
                        // Inside the outline, as when it was drawn under what is inside.
                        padding: const EdgeInsets.all(1),
                        decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(8),
                        ),
                        // Over what is inside, which would cover it at the rounded corners.
                        foregroundDecoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(8),
                          border: Border.all(color: c.border),
                        ),
                        child: Column(
                          children: [
                            for (var i = 0; i < expected.length; i++)
                              Container(
                                height: 44,
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 12,
                                ),
                                decoration: BoxDecoration(
                                  border: i < expected.length - 1
                                      ? Border(
                                          bottom: BorderSide(color: c.border),
                                        )
                                      : null,
                                ),
                                child: Row(
                                  children: [
                                    Avatar(
                                      avatar: expected[i].avatar,
                                      size: 22,
                                    ),
                                    const SizedBox(width: 10),
                                    Expanded(
                                      child: Text(
                                        expected[i].name,
                                        style: tx(c.fg, size: 13),
                                      ),
                                    ),
                                    Text(
                                      l10n.roomNoAnswer,
                                      style: tx(
                                        c.red,
                                        size: 12,
                                        monospace: true,
                                      ),
                                    ),
                                  ],
                                ),
                              ),
                          ],
                        ),
                      ),
                    ],
                  ],
                ),
              ),
              Footer(
                children: [
                  Expanded(
                    child: Align(
                      alignment: Alignment.centerLeft,
                      child: Pressable(
                        onTap: () {
                          state.nav.networkCheckOnOpen = true;
                          state.openSettings();
                        },
                        builder: (context, hovered) => Text(
                          l10n.roomSeeNetworkCheck,
                          style: tx(
                            c.blue,
                            size: 13,
                            decoration: hovered
                                ? TextDecoration.underline
                                : null,
                          ),
                        ),
                      ),
                    ),
                  ),
                  FgButton(text: l10n.commonBack, onTap: state.leaveRoom),
                  const SizedBox(width: 10),
                  FgButton(
                    text: l10n.commonTryAgain,
                    icon: Ic.retry,
                    kind: Kind.primary,
                    onTap: () {
                      final memory =
                          state.roomState.memories[state.roomState.roomGroup];
                      memory?.joinedMs = DateTime.now().millisecondsSinceEpoch;
                      state.refreshRoom();
                    },
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}

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

/// Someone being rung into the call: their picture in rings, and a way to stop.
class _RingingTile extends StatelessWidget {
  const _RingingTile({required this.person, required this.onCancel});
  final UiPerson person;
  final VoidCallback onCancel;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Container(
      key: ValueKey('calling-tile:${person.id}'),
      decoration: BoxDecoration(
        color: c.bg1,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: c.border),
      ),
      child: Center(
        child: FittedBox(
          fit: BoxFit.scaleDown,
          child: Padding(
            padding: const EdgeInsets.all(12),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Rings(
                  color: c.fg3,
                  size: 120,
                  child: Avatar(avatar: person.avatar, size: 72),
                ),
                const SizedBox(height: 6),
                Text(l10n.roomCallingName(person.name), style: tx(c.fg2)),
                const SizedBox(height: 10),
                FgButton(
                  text: l10n.commonCancel,
                  height: 30,
                  fontSize: 13,
                  onTap: onCancel,
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
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
