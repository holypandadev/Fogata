import 'package:flutter/material.dart';

import '../app_state.dart';
import '../icons.dart';
import '../l10n.dart';
import '../rust/api/types.dart';
import '../theme.dart';
import '../widgets.dart';

/// Most people in a call at once, those being rung included (as the core counts them).
const maxInCall = 8;

/// The friends to ring into the call this device is in, over the button it came from.
Future<void> showAddToCall(BuildContext anchor) {
  final box = anchor.findRenderObject() as RenderBox?;
  final overlay =
      Overlay.of(anchor, rootOverlay: true).context.findRenderObject()
          as RenderBox?;
  if (box == null || overlay == null || !box.hasSize) return Future.value();
  final from = box.localToGlobal(Offset.zero, ancestor: overlay) & box.size;
  return showGeneralDialog<void>(
    context: anchor,
    barrierDismissible: true,
    barrierLabel: l10n.commonClose,
    barrierColor: Colors.transparent,
    transitionDuration: const Duration(milliseconds: 260),
    pageBuilder: (context, _, _) => CustomSingleChildLayout(
      delegate: _Above(from),
      child: const Material(
        type: MaterialType.transparency,
        child: AddToCallCard(),
      ),
    ),
    // Grows out of the button with the controls' spring.
    transitionBuilder: (context, animation, _, child) => FadeTransition(
      opacity: animation,
      child: ScaleTransition(
        alignment: Alignment.bottomCenter,
        scale: Tween(begin: .94, end: 1.0).animate(
          CurvedAnimation(
            parent: animation,
            curve: spring,
            reverseCurve: Curves.easeIn,
          ),
        ),
        child: child,
      ),
    ),
  );
}

/// Puts the card centred over the button it came from, inside the window.
class _Above extends SingleChildLayoutDelegate {
  _Above(this.from);
  final Rect from;

  static const _gap = 12.0;
  static const _margin = 12.0;

  @override
  BoxConstraints getConstraintsForChild(BoxConstraints constraints) =>
      constraints.loosen();

  @override
  Offset getPositionForChild(Size size, Size child) {
    final x = (from.center.dx - child.width / 2).clamp(
      _margin,
      size.width - child.width - _margin,
    );
    final y = from.top - _gap - child.height;
    return Offset(x, y.clamp(_margin, size.height - child.height - _margin));
  }

  @override
  bool shouldRelayout(_Above old) => old.from != from;
}

/// Friends who are online, each with a Call that rings them into the call; those in the
/// call already, being rung or offline say so instead.
class AddToCallCard extends StatefulWidget {
  const AddToCallCard({super.key});

  @override
  State<AddToCallCard> createState() => _AddToCallCardState();
}

class _AddToCallCardState extends State<AddToCallCard> {
  final _search = TextEditingController();

  @override
  void dispose() {
    _search.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final call = state.callState.activeCall;
    final inCall = {
      for (final p
          in state.roomState.room?.participants ?? const <UiParticipant>[])
        p.person.id,
    };
    final rung = {for (final p in call?.adding ?? const <UiPerson>[]) p.id};
    final query = _search.text.trim().toLowerCase();
    final friends =
        state.contacts.friends
            .where(
              (f) =>
                  query.isEmpty || f.person.name.toLowerCase().contains(query),
            )
            .toList()
          ..sort((a, b) {
            final on = (b.presence != UiPresence.offline ? 1 : 0).compareTo(
              a.presence != UiPresence.offline ? 1 : 0,
            );
            return on != 0
                ? on
                : a.person.name.toLowerCase().compareTo(
                    b.person.name.toLowerCase(),
                  );
          });
    final blocked = call?.addBlocked;
    return Container(
      key: const ValueKey('add-to-call'),
      width: 340,
      padding: const EdgeInsets.all(8),
      decoration: BoxDecoration(
        color: c.bg2,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: c.border),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(10, 8, 10, 4),
            child: Text(l10n.addToCallTitle, style: tx(c.fg, weight: w600)),
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(10, 0, 10, 8),
            child: Text(
              blocked ?? l10n.addToCallNote(maxInCall),
              style: tx(blocked == null ? c.fg2 : c.amber, size: 12),
            ),
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(4, 0, 4, 6),
            child: FgField(
              controller: _search,
              placeholder: l10n.addToCallSearch,
              height: 36,
              radius: 999,
              autofocus: true,
              leading: FgIcon(Ic.search, size: 14, color: c.fg3),
              onChanged: (_) => setState(() {}),
            ),
          ),
          ConstrainedBox(
            constraints: const BoxConstraints(maxHeight: 44.0 * 6),
            child: SingleChildScrollView(
              child: Column(
                children: [
                  for (final f in friends)
                    _Row(
                      friend: f,
                      inCall: inCall.contains(f.person.id),
                      rung: rung.contains(f.person.id),
                      blocked: blocked != null,
                    ),
                  if (friends.isEmpty)
                    Padding(
                      padding: const EdgeInsets.all(12),
                      child: Text(
                        query.isEmpty
                            ? l10n.addToCallNoFriends
                            : l10n.addToCallNoMatch,
                        style: tx(c.fg3, size: 13),
                      ),
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

class _Row extends StatelessWidget {
  const _Row({
    required this.friend,
    required this.inCall,
    required this.rung,
    required this.blocked,
  });
  final UiFriend friend;
  final bool inCall;
  final bool rung;
  final bool blocked;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    final person = friend.person;
    final offline = friend.presence == UiPresence.offline;
    final Widget end = inCall
        ? Text(l10n.addToCallInCall, style: tx(c.fg3, size: 12))
        : rung
        ? Text(l10n.addToCallCalling, style: tx(c.fg2, size: 12))
        : offline
        ? Text(l10n.addToCallOffline, style: tx(c.fg3, size: 12))
        : FgButton(
            key: ValueKey('add-call:${person.id}'),
            text: l10n.addToCallCall,
            kind: Kind.primary,
            height: 30,
            fontSize: 13,
            onTap: blocked ? null : () => state.addToCall(person.id),
          );
    return Opacity(
      opacity: offline && !inCall ? .45 : 1,
      child: SizedBox(
        height: 44,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 10),
          child: Row(
            children: [
              Avatar(avatar: person.avatar, size: 28),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  person.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(c.fg),
                ),
              ),
              const SizedBox(width: 8),
              end,
            ],
          ),
        ),
      ),
    );
  }
}
