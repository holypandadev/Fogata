import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../app_state.dart';
import '../format.dart';
import '../icons.dart';
import '../l10n.dart';
import '../rust/api/types.dart';
import '../theme.dart';
import '../widgets.dart';
import 'home.dart' show Ticker;
import 'pip.dart' show SizeReport, cornerCards;
import 'room.dart' show confirmCalling, confirmLeaving, leaveRoomChecked;

/// Green of the buttons that answer a call.
const _answer = greenSolid;

/// Answers a ringing call, first asking before closing the room this device is alone in,
/// as any other way out of it does.
Future<void> _answerChecked(
  BuildContext context,
  AppState state,
  UiCall call, {
  bool? micOff,
}) async {
  if (!await confirmLeaving(context)) return;
  // It may have stopped ringing while the question was open.
  if (state.callState.callIn(call.groupId)?.phase != UiCallPhase.incoming) {
    return;
  }
  await state.acceptCall(call, micOff: micOff);
}

/// A friend rings: a card in the middle of the window. A click beside it puts it aside,
/// into the corner, where it rings on until answered or over.
class IncomingCallLayer extends StatefulWidget {
  const IncomingCallLayer({super.key});

  @override
  State<IncomingCallLayer> createState() => _IncomingCallLayerState();
}

class _IncomingCallLayerState extends State<IncomingCallLayer> {
  /// Starts as the microphone was left.
  late bool _micOff = !AppScope.read(context).prefs.joinWithMic;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final call = state.callState.incomingCall;
    if (call == null || ringsInCorner(state, call)) {
      return const SizedBox.shrink();
    }
    final c = context.fg;
    void aside() => state.putRingAside(call);
    return Positioned.fill(
      // Esc puts the call aside, as a click beside the card does; Tab goes round the
      // card, not the page behind. Coming by itself, it leaves the focus in a message
      // being written or a dialog: F6 goes to it then.
      child: CallbackShortcuts(
        bindings: {const SingleActivator(LogicalKeyboardKey.escape): aside},
        child: FocusLayer(
          polite: true,
          child: Stack(
            children: [
              Positioned.fill(
                child: Backdrop(onTap: aside, color: c.scrim, rightClick: true),
              ),
              Center(
                child: _Pop(
                  alignment: Alignment.center,
                  child: Container(
                    width: 380,
                    padding: const EdgeInsets.fromLTRB(28, 24, 28, 22),
                    decoration: BoxDecoration(
                      color: c.bg2,
                      borderRadius: BorderRadius.circular(24),
                      border: Border.all(color: c.border),
                    ),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Rings(
                          color: c.green,
                          size: 150,
                          child: Avatar(
                            avatar: call.with_.avatar,
                            size: 80,
                            id: call.with_.id,
                          ),
                        ),
                        const SizedBox(height: 10),
                        // Read out as the call comes, wherever the focus is.
                        Semantics(
                          liveRegion: true,
                          hint: l10n.callLayersGoToCall,
                          child: Text(
                            l10n.callLayersIsCalling(call.with_.name),
                            textAlign: TextAlign.center,
                            style: tx(c.fg, size: 20, weight: w600),
                          ),
                        ),
                        const SizedBox(height: 2),
                        Text.rich(
                          TextSpan(
                            children: [
                              // A call of more than two says who else is in it.
                              if (others(call).isNotEmpty)
                                TextSpan(
                                  text: l10n.callLayersWith(others(call)),
                                )
                              else ...[
                                TextSpan(text: l10n.callLayersDirectCall),
                                TextSpan(
                                  text: call.with_.short,
                                  style: const TextStyle(fontFamily: mono),
                                ),
                              ],
                            ],
                          ),
                          textAlign: TextAlign.center,
                          style: tx(c.fg2, size: 13),
                        ),
                        const SizedBox(height: 20),
                        Row(
                          children: [
                            Expanded(
                              child: _CallButton(
                                text: l10n.commonDecline,
                                icon: Ic.hangUp,
                                color: c.redSolid,
                                onTap: () => state.declineCall(call),
                              ),
                            ),
                            const SizedBox(width: 12),
                            Expanded(
                              child: _CallButton(
                                text: l10n.commonAccept,
                                icon: Ic.phone,
                                color: _answer,
                                onTap: () => _answerChecked(
                                  context,
                                  state,
                                  call,
                                  micOff: _micOff,
                                ),
                              ),
                            ),
                          ],
                        ),
                        const SizedBox(height: 14),
                        Pressable(
                          onTap: () => setState(() => _micOff = !_micOff),
                          label: l10n.callLayersAnswerMicOff,
                          builder: (context, hovered) => Row(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              FgSwitch(on: _micOff, small: true),
                              const SizedBox(width: 8),
                              // Said once, as the label.
                              ExcludeSemantics(
                                child: Text(
                                  l10n.callLayersAnswerMicOff,
                                  style: tx(hovered ? c.fg : c.fg2, size: 13),
                                ),
                              ),
                            ],
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// The others in a call that rings here, besides the one ringing, by name.
String others(UiCall call) => names([
  for (final p in call.members)
    if (p.id != call.with_.id) p.name,
]);

/// Whether a ringing call shows in the corner instead of the middle: while this device is
/// in a room, or once it was put aside.
bool ringsInCorner(AppState state, UiCall call) =>
    state.contacts.joinedGroup != null ||
    state.callState.ringAside == call.groupId;

/// The cards in the window's bottom right corner, one above the other: a missed call, a
/// call ringing in the corner and the room or call this device is in while another page
/// is shown.
class CornerCards extends StatelessWidget {
  const CornerCards({super.key});

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final ringing = state.callState.incomingCall;
    final missed =
        state.notices.missedNote != null &&
        state.nav.page != Screen.friends &&
        ringing == null;
    final cards = <Widget>[
      if (missed) const _MissedCard(),
      if (ringing != null && ringsInCorner(state, ringing))
        _SmallCard(
          key: ValueKey(ringing.groupId),
          state: state,
          call: ringing,
          room: state.contacts.joinedGroup?.name,
        ),
      if (inCallElsewhere(state)) const _InCallCard(),
    ];
    // The floating picture of a call sits above the cards and the toasts beside them.
    if (cards.isEmpty) {
      return const SizeReport(onSize: _report, child: SizedBox.shrink());
    }
    return Positioned(
      right: 24,
      bottom: 24,
      child: SizeReport(
        onSize: _report,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            for (final (i, card) in cards.indexed) ...[
              if (i > 0) const SizedBox(height: 12),
              _Pop(
                alignment: Alignment.bottomRight,
                child: card is _MissedCard
                    ? _Leaving(
                        leaving: state.notices.missedNoteLeaving,
                        child: card,
                      )
                    : card,
              ),
            ],
          ],
        ),
      ),
    );
  }
}

void _report(Size size) => cornerCards.value = size;

/// Whether this device is in a room or a call while another page is shown.
bool inCallElsewhere(AppState state) {
  final group = state.roomState.roomGroup;
  if (group == null || state.nav.page == Screen.room) return false;
  final call = state.callState.callIn(group);
  if (call != null) return call.phase != UiCallPhase.incoming;
  return state.contacts.group(group)?.room?.joined ?? false;
}

class _SmallCard extends StatelessWidget {
  const _SmallCard({
    super.key,
    required this.state,
    required this.call,
    required this.room,
  });
  final AppState state;
  final UiCall call;

  /// The room answering leaves, if this device is in one.
  final String? room;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final room = this.room;
    return Container(
      width: 360,
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: c.bg2,
        borderRadius: BorderRadius.circular(20),
        border: Border.all(color: c.border),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            children: [
              Rings(
                color: c.green,
                size: 56,
                child: Avatar(avatar: call.with_.avatar, size: 36),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      l10n.callLayersIsCalling(call.with_.name),
                      style: tx(c.fg, weight: w500, height: 1.3),
                    ),
                    if (room != null)
                      Text(
                        l10n.callLayersAnsweringLeaves(room),
                        overflow: TextOverflow.ellipsis,
                        style: tx(c.amber, size: 12),
                      )
                    else
                      Text.rich(
                        TextSpan(
                          children: [
                            TextSpan(text: l10n.callLayersDirectCall),
                            TextSpan(
                              text: call.with_.short,
                              style: const TextStyle(fontFamily: mono),
                            ),
                          ],
                        ),
                        style: tx(c.fg2, size: 12),
                      ),
                  ],
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              Expanded(
                child: FgButton(
                  text: l10n.commonDecline,
                  height: 36,
                  fontSize: 13,
                  onTap: () => state.declineCall(call),
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: _CallButton(
                  text: room == null
                      ? l10n.commonAccept
                      : l10n.callLayersLeaveAndAnswer,
                  color: _answer,
                  height: 36,
                  onTap: () => _answerChecked(context, state, call),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

/// Fades and sinks a little as it goes, as a toast does.
class _Leaving extends StatelessWidget {
  const _Leaving({required this.leaving, required this.child});
  final bool leaving;
  final Widget child;

  @override
  Widget build(BuildContext context) => AnimatedOpacity(
    opacity: leaving ? 0 : 1,
    duration: const Duration(milliseconds: 240),
    child: AnimatedSlide(
      offset: Offset(0, leaving ? .3 : 0),
      duration: const Duration(milliseconds: 240),
      child: child,
    ),
  );
}

/// A missed call: it goes by itself after a few seconds, like a toast, unless the pointer
/// is on it; also with a click on it, a call back, or when Friends is opened. The bell
/// keeps it.
class _MissedCard extends StatelessWidget {
  const _MissedCard();

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final id = state.notices.missedNote;
    final when = state.notices.missedCalls[id];
    UiPerson? who;
    for (final f in state.contacts.friends) {
      if (f.person.id == id) who = f.person;
    }
    if (id == null || when == null || who == null) {
      return const SizedBox.shrink();
    }
    final person = who;
    final together = state.sharedRoom(person.id);
    return MouseRegion(
      onEnter: (_) => state.notices.holdMissedNote(true),
      onExit: (_) => state.notices.holdMissedNote(false),
      child: GestureDetector(
        key: const ValueKey('missed-call-note'),
        behavior: HitTestBehavior.opaque,
        onTap: state.notices.dismissMissedNote,
        child: Container(
          width: 380,
          padding: const EdgeInsets.fromLTRB(14, 12, 10, 12),
          decoration: BoxDecoration(
            color: c.bg2,
            borderRadius: BorderRadius.circular(20),
            border: Border.all(color: c.border),
          ),
          child: Row(
            children: [
              Avatar(avatar: person.avatar, size: 32),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      state.notices.missedCallTitle(person),
                      style: tx(c.fg, weight: w500, height: 1.3),
                    ),
                    Text(
                      dayClock(when),
                      style: tx(c.fg2, size: 12, monospace: true),
                    ),
                  ],
                ),
              ),
              FgButton(
                text: state.callState.callText(person.id, missed: true),
                kind: Kind.primary,
                height: 30,
                fontSize: 12,
                tooltip: together == null
                    ? null
                    : l10n.callLayersBothIn(together),
                onTap: state.callState.activeCall == null && together == null
                    ? () async {
                        if (!await confirmCalling(context, person.id)) {
                          return;
                        }
                        state.notices.missedCalls.remove(person.id);
                        state.notices.dismissMissedNote();
                        await state.callFriend(person.id);
                      }
                    : null,
              ),
              const SizedBox(width: 4),
              FgButton(
                icon: Ic.close,
                kind: Kind.ghost,
                height: 30,
                tooltip: l10n.callLayersDismiss,
                onTap: state.notices.dismissMissedNote,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// The room or call this device is in, while another page is shown: who, for how long,
/// the microphone and the way out. A click on it goes back to the room or call.
class _InCallCard extends StatelessWidget {
  const _InCallCard();

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final groupId = state.roomState.roomGroup!;
    final call = state.callState.callIn(groupId);
    final group = state.contacts.group(groupId);
    final title = call != null
        ? l10n.callLayersCallWith(state.callNames(call))
        : group?.name ?? l10n.callLayersRoom;
    final since =
        call?.sinceMs ??
        state.roomState.memories[groupId]?.joinedMs ??
        group?.room?.startedMs ??
        DateTime.now().millisecondsSinceEpoch;
    final others = [
      for (final p in group?.room?.participants ?? const <UiPerson>[])
        if (p.id != state.contacts.profile?.person.id) p,
    ];
    final ringing = call?.phase == UiCallPhase.ringing;
    final mic = state.callState.call?.mic ?? state.prefs.joinWithMic;
    final Widget leading = call != null
        ? Avatar(avatar: call.with_.avatar, size: 36)
        : Container(
            width: 36,
            height: 36,
            alignment: Alignment.center,
            decoration: BoxDecoration(color: c.hover, shape: BoxShape.circle),
            child: FgIcon(groupIcon(group?.icon ?? 0), size: 16, color: c.fg),
          );
    return Pressable(
      key: const ValueKey('in-call-card'),
      onTap: call != null
          ? state.showCall
          : () => state.showRoomTab(groupId, state.roomState.roomTab),
      label: l10n.callLayersBackTo(title),
      builder: (context, hovered) => AnimatedContainer(
        duration: const Duration(milliseconds: 160),
        width: 320,
        padding: const EdgeInsets.fromLTRB(12, 10, 10, 10),
        decoration: BoxDecoration(
          color: hovered ? Color.lerp(c.bg2, c.fg, .04) : c.bg2,
          borderRadius: BorderRadius.circular(20),
          border: Border.all(color: c.border),
        ),
        child: Row(
          children: [
            leading,
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    overflow: TextOverflow.ellipsis,
                    style: tx(c.fg, size: 13, weight: w500, height: 1.3),
                  ),
                  Ticker(
                    builder: (context) => Text(
                      ringing
                          ? l10n.callLayersRinging(elapsed(since))
                          : call != null
                          ? l10n.callLayersInCall(elapsed(since))
                          : others.isEmpty
                          ? l10n.callLayersOnlyYou(elapsed(since))
                          : l10n.callLayersInRoom(
                              others.length + 1,
                              elapsed(since),
                            ),
                      style: tx(
                        ringing ? c.amber : c.green,
                        size: 12,
                        height: 1.3,
                      ),
                    ),
                  ),
                ],
              ),
            ),
            FgButton(
              icon: mic ? Ic.mic : Ic.micOff,
              kind: Kind.ghost,
              height: 32,
              iconColor: mic ? c.fg : c.red,
              tooltip: mic ? l10n.callLayersMute : l10n.callLayersUnmute,
              onTap: () => state.setMic(!mic),
            ),
            const SizedBox(width: 2),
            FgButton(
              icon: call != null ? Ic.hangUp : Ic.leave,
              kind: Kind.danger,
              height: 32,
              tooltip: call != null
                  ? l10n.callLayersHangUp
                  : l10n.callLayersLeaveRoom,
              onTap: call != null
                  ? state.hangUp
                  : () => leaveRoomChecked(context),
            ),
          ],
        ),
      ),
    );
  }
}

/// A pill button of a call: coloured, white text, a spring when pressed.
class _CallButton extends StatelessWidget {
  const _CallButton({
    required this.text,
    required this.color,
    required this.onTap,
    this.icon,
    this.height = 44,
  });
  final String text;
  final Ic? icon;
  final Color color;
  final VoidCallback onTap;
  final double height;

  @override
  Widget build(BuildContext context) {
    final icon = this.icon;
    return Springy(
      onTap: onTap,
      label: text,
      hoverScale: 1.03,
      builder: (context, hovered) => AnimatedContainer(
        duration: const Duration(milliseconds: 160),
        height: height,
        alignment: Alignment.center,
        decoration: BoxDecoration(
          color: hovered ? Color.lerp(color, Colors.white, .08) : color,
          borderRadius: BorderRadius.circular(999),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (icon != null) ...[
              FgIcon(icon, size: 16, color: Colors.white),
              const SizedBox(width: 8),
            ],
            Text(text, style: tx(Colors.white, size: 14, weight: w500)),
          ],
        ),
      ),
    );
  }
}

/// Comes in with a spring.
class _Pop extends StatelessWidget {
  const _Pop({required this.child, required this.alignment});
  final Widget child;
  final Alignment alignment;

  @override
  Widget build(BuildContext context) => TweenAnimationBuilder<double>(
    tween: Tween(begin: 0, end: 1),
    duration: springTime,
    curve: spring,
    builder: (context, t, child) => Opacity(
      opacity: t.clamp(0, 1),
      child: Transform.scale(
        scale: .94 + .06 * t,
        alignment: alignment,
        child: child,
      ),
    ),
    child: child,
  );
}
