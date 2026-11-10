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
part 'room/controls.dart';
part 'room/files.dart';
part 'room/stream.dart';
part 'room/tiles.dart';

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
