import 'dart:async';
import 'dart:ui' show ImageFilter;

import 'package:flutter/material.dart';

import '../app_state.dart';
import '../format.dart';
import '../icons.dart';
import '../l10n.dart';
import '../rust/api/account_move.dart' as api;
import '../state/account_move.dart';
import '../theme.dart';
import '../widgets.dart';
import '../window.dart';

// Moving the account to another computer, or bringing it here: the new computer's card
// (in place of the welcome page's profile card), the settings' page, and what the old
// computer shows afterwards. The two computers check a short code and the account goes
// straight from one to the other (the core's `transfer`).

/// Starts showing a code here to bring an account from another computer. [replacing]:
/// this computer's own profile goes once the account is here, and its core stops now.
Future<void> bringHere(AppState state, {bool replacing = false}) async {
  if (replacing) await state.stopForMove();
  final move = AccountMove(sending: false)
    ..ownFingerprint = state.contacts.profile?.fingerprint;
  state.accountMove?.dispose();
  state.accountMove = move;
  state.bringing = true;
  state.replacing = replacing || state.replacing;
  state.notifyListeners();
  try {
    move.follow(
      api.moveReceive(dataDir: state.dataDir, computer: computerName()),
    );
  } catch (e) {
    move.fail('other', explain(e));
  }
}

/// Stops bringing an account here: back to the welcome page, or to this computer's own
/// profile.
Future<void> stopBringing(AppState state) async {
  if (!(state.accountMove?.cancel() ?? true)) {
    state.say(l10n.moveTooLateBeingPlaced);
    return;
  }
  state.accountMove = null;
  state.bringing = false;
  final replacing = state.replacing;
  state.replacing = false;
  state.notifyListeners();
  if (replacing) await state.resumeAfterMove();
}

/// The old computer: moves this account to the computer showing [code]. The core stops
/// first, so nothing of the account changes while it goes. With the folder "moving", this
/// finishes that move.
///
/// The move is told at once, still [MoveStep.starting] while the core stops.
AccountMove moveAway(AppState state, String code) {
  final move = AccountMove(sending: true)..code = code;
  state.accountMove?.dispose();
  state.accountMove = move;
  _followSending(state, move);
  unawaited(_sendAfterStopping(state, move, code));
  return move;
}

/// The whole application follows a move from here, whatever page shows: its question
/// shows over everything ([MoveQuestion]), and its end brings the core back or shows that
/// the account moved.
void _followSending(AppState state, AccountMove move) {
  var step = move.step;
  var answered = move.answered;
  var ended = false;
  move.addListener(() {
    if (move.step != step || move.answered != answered) {
      step = move.step;
      answered = move.answered;
      state.notifyListeners();
    }
    if (move.over && !ended) {
      ended = true;
      unawaited(afterMovingAway(state, move));
    }
  });
}

Future<void> _sendAfterStopping(
  AppState state,
  AccountMove move,
  String code,
) async {
  await state.stopForMove();
  try {
    move.follow(
      api.moveSend(
        dataDir: state.dataDir,
        code: code,
        computer: computerName(),
      ),
    );
  } catch (e) {
    move.fail('other', explain(e));
  }
}

/// After a move from this computer ended: "moved" (or "moving") shows instead of the app;
/// anything else, and the account is still here and the core starts again.
Future<void> afterMovingAway(AppState state, AccountMove move) async {
  await state.loadMoveState();
  if (state.moveState?.kind != 'none') {
    state.notifyListeners();
    return;
  }
  await state.resumeAfterMove();
}

// ----- Shared pieces -----

/// Where a message's placeholder goes when its words are drawn in parts.
const _mark = '\u{FFFC}';

/// [message], made with [_mark] for one of its placeholders, as spans with [inner] in
/// that place: a part drawn in its own style wherever the language puts it.
List<InlineSpan> _around(String message, InlineSpan inner) {
  final at = message.indexOf(_mark);
  if (at < 0) return [TextSpan(text: message)];
  return [
    TextSpan(text: message.substring(0, at)),
    inner,
    TextSpan(text: message.substring(at + _mark.length)),
  ];
}

/// The two computers, the old one and the new one, and what is between them.
class _Machines extends StatelessWidget {
  const _Machines({this.oldOn = false, this.oldOff = false});
  final bool oldOn;

  /// The old computer no longer uses the account.
  final bool oldOff;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    Widget box(Ic icon, bool on, {bool off = false}) => Opacity(
      opacity: off ? .45 : 1,
      child: Container(
        width: 58,
        height: 58,
        decoration: BoxDecoration(
          color: c.bg2,
          borderRadius: BorderRadius.circular(16),
          border: Border.all(color: on ? c.greenLine : c.border),
        ),
        child: Center(
          child: FgIcon(icon, size: 28, color: on ? c.green : c.fg2),
        ),
      ),
    );
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        box(Ic.laptop, oldOn, off: oldOff),
        const SizedBox(width: 14),
        Container(width: 80, height: 2, color: c.fg),
        const SizedBox(width: 14),
        box(Ic.monitor, true),
      ],
    );
  }
}

/// A list in a box, a row per line.
class _List extends StatelessWidget {
  const _List({required this.rows});
  final List<Widget> rows;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Container(
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: c.border),
      ),
      child: Column(
        children: [
          for (var i = 0; i < rows.length; i++)
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
              decoration: BoxDecoration(
                border: i == 0
                    ? null
                    : Border(top: BorderSide(color: c.border)),
              ),
              child: rows[i],
            ),
        ],
      ),
    );
  }
}

Widget _row(BuildContext context, Ic icon, String text, {String? right}) {
  final c = context.fg;
  return Row(
    children: [
      SizedBox(
        width: 20,
        child: Center(child: FgIcon(icon, size: 17, color: c.fg2)),
      ),
      const SizedBox(width: 12),
      Expanded(child: Text(text, style: tx(c.fg, size: 13.5))),
      if (right != null)
        Flexible(
          child: Text(
            right,
            textAlign: TextAlign.right,
            overflow: TextOverflow.ellipsis,
            style: tx(c.fg3, size: 12.5),
          ),
        ),
    ],
  );
}

/// What moves: the account's name and picture, its friends, groups and chats.
List<Widget> _whatMoves(
  BuildContext context,
  api.UiMoveSummary s, {
  bool keys = false,
  bool settings = false,
}) {
  final c = context.fg;
  final p = s.profile;
  return [
    Row(
      children: [
        Avatar(avatar: p.person.avatar, size: 32, id: p.person.id),
        const SizedBox(width: 12),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                p.person.name,
                overflow: TextOverflow.ellipsis,
                style: tx(c.fg, size: 13.5, weight: w600),
              ),
              Text(
                p.fingerprint,
                style: tx(c.fg3, size: 11.5, monospace: true),
              ),
            ],
          ),
        ),
      ],
    ),
    _row(context, Ic.users, l10n.moveFriends(s.friends)),
    _row(
      context,
      Ic.nodes,
      keys ? l10n.moveGroupsWithKeys(s.groups) : l10n.moveGroups(s.groups),
    ),
    _row(
      context,
      Ic.chat,
      l10n.moveChats(s.chats),
      right: l10n.moveInAll(bytes(s.bytes)),
    ),
    if (settings)
      _row(
        context,
        Ic.settings,
        l10n.moveYourSettings,
        right: l10n.moveNotDevices,
      ),
  ];
}

/// A pill that says what is going on, with a spinner or a dot.
class _Status extends StatelessWidget {
  const _Status({required this.text, this.dot});
  final String text;
  final Color? dot;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: c.border),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (dot != null) Dot(dot!) else Spinner(size: 12, color: c.fg2),
          const SizedBox(width: 8),
          Flexible(child: Text(text, style: tx(c.fg2, size: 13))),
        ],
      ),
    );
  }
}

/// A note in a soft box of its colour: amber, red, green or plain.
class _Note extends StatelessWidget {
  const _Note({required this.icon, required this.child, this.tone = 'amber'});
  final Ic icon;
  final Widget child;
  final String tone;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final (bg, line, iconColor) = switch (tone) {
      'red' => (c.redSoft, c.redLine, c.red),
      'green' => (c.greenSoft, c.greenLine, c.green),
      'plain' => (c.bg2, c.border, c.fg2),
      _ => (c.amberSoft, c.amberLine, c.amber),
    };
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        color: bg,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: line),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(top: 1),
            child: FgIcon(icon, size: 16, color: iconColor),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: DefaultTextStyle.merge(
              style: tx(tone == 'plain' ? c.fg2 : c.fg, size: 13),
              child: child,
            ),
          ),
        ],
      ),
    );
  }
}

/// A button for a step that cannot be undone: red words on a soft red pill.
class _SoftDanger extends StatelessWidget {
  const _SoftDanger({
    super.key,
    required this.text,
    required this.icon,
    this.onTap,
  });
  final String text;
  final Ic icon;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Opacity(
      opacity: onTap == null ? .45 : 1,
      child: Springy(
        onTap: onTap,
        label: text,
        focusRadius: 999,
        hoverScale: 1.03,
        pressScale: .96,
        builder: (context, hovered) => AnimatedContainer(
          duration: const Duration(milliseconds: 160),
          height: 36,
          padding: const EdgeInsets.symmetric(horizontal: 16),
          decoration: BoxDecoration(
            color: hovered ? Color.lerp(c.redSoft, c.red, .08) : c.redSoft,
            borderRadius: BorderRadius.circular(999),
            border: Border.all(color: c.redLine),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              FgIcon(icon, size: 15, color: c.red),
              const SizedBox(width: 8),
              Flexible(
                child: Text(
                  text,
                  overflow: TextOverflow.ellipsis,
                  style: tx(c.red, size: 14, weight: w500, height: 1.2),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// The steps of a move and how far it got.
class _Progress extends StatelessWidget {
  const _Progress({
    required this.move,
    required this.title,
    required this.foot,
  });
  final AccountMove move;
  final String title;
  final String foot;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final s = move.summary;
    final finishing = move.step == MoveStep.finishing;
    final percent = (move.share * 100).floor();
    final left = move.timeLeftWords;
    // The files go in this order: the identity and the profile first, the friends file,
    // the group logs, the chats, the attachments last.
    final early = move.share > .02 || finishing;
    Widget line(String state, String text, {String? right}) {
      final icon = switch (state) {
        'done' => FgIcon(Ic.checkCircle, size: 17, color: c.green),
        'now' => Spinner(size: 13, color: c.fg),
        _ => Dot(c.fg3),
      };
      return Padding(
        padding: const EdgeInsets.symmetric(vertical: 6),
        child: Row(
          children: [
            SizedBox(width: 18, child: Center(child: icon)),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                text,
                style: tx(state == 'todo' ? c.fg2 : c.fg, size: 13.5),
              ),
            ),
            if (right != null)
              Text(right, style: tx(c.fg3, size: 12, monospace: true)),
          ],
        ),
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(title, style: tx(c.fg, size: 20, weight: w600, spacing: -.35)),
        const SizedBox(height: 4),
        Text(
          finishing
              ? l10n.moveFinishingBoth
              : left == null
              ? l10n.movePercent(percent)
              : l10n.movePercentLeft(percent, left),
          style: tx(c.fg2, size: 13),
        ),
        const SizedBox(height: 18),
        ClipRRect(
          borderRadius: BorderRadius.circular(999),
          child: SizedBox(
            height: 6,
            child: Stack(
              children: [
                Positioned.fill(child: ColoredBox(color: c.hover)),
                FractionallySizedBox(
                  widthFactor: move.share,
                  child: Container(
                    decoration: BoxDecoration(
                      color: c.fg,
                      borderRadius: BorderRadius.circular(999),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
        const SizedBox(height: 14),
        line(early ? 'done' : 'now', l10n.moveStepIdentity),
        line(
          early ? 'done' : 'todo',
          l10n.moveStepFriends,
          right: s == null ? null : '${s.friends}',
        ),
        line(
          early ? 'done' : 'todo',
          l10n.moveStepGroups,
          right: s == null ? null : '${s.groups}',
        ),
        line(
          finishing ? 'done' : (early ? 'now' : 'todo'),
          l10n.moveStepChats,
          right: l10n.moveBytesOf(bytes(move.done), bytes(move.total)),
        ),
        line(finishing ? 'now' : 'todo', l10n.moveStepFinishing),
        const SizedBox(height: 14),
        Text(foot, style: tx(c.fg3, size: 12.5)),
      ],
    );
  }
}

// ----- The new computer -----

/// The new computer's card: the code to type on the old one, then what comes, how far it
/// got, and the account in place. Shown on the welcome page instead of the profile card.
class BringCard extends StatefulWidget {
  const BringCard({super.key});

  @override
  State<BringCard> createState() => _BringCardState();
}

class _BringCardState extends State<BringCard> {
  bool _shown = true;
  Timer? _tick;
  AccountMove? _move;
  late final AppState _state;

  @override
  void initState() {
    super.initState();
    _state = AppScope.read(context);
    if (_state.accountMove == null || _state.accountMove!.sending) {
      // Shown by itself: a move that came whole waits to be finished.
      WidgetsBinding.instance.addPostFrameCallback((_) => bringHere(_state));
    }
    _tick = Timer.periodic(const Duration(seconds: 1), (_) {
      if (mounted) setState(() {});
    });
  }

  @override
  void dispose() {
    _tick?.cancel();
    _move?.removeListener(_changed);
    super.dispose();
  }

  void _changed() {
    if (mounted) setState(() {});
  }

  /// Follows the move the state holds now.
  AccountMove? _follow() {
    final move = _state.accountMove;
    if (move != _move) {
      _move?.removeListener(_changed);
      move?.addListener(_changed);
      _move = move;
    }
    return move;
  }

  Future<void> _open() async {
    if (_state.replacing) {
      // Everything of the profile that was here is gone: start afresh as the account.
      await _state.stopping.restart();
      return;
    }
    _state.accountMove = null;
    _state.bringing = false;
    _state.hasProfile = true;
    await _state.loadMoveState();
    await _state.prefs.load();
    await _state.start();
  }

  Future<void> _discard() async {
    final yes = await _confirmDiscard(context);
    if (!yes || !mounted) return;
    if (!(_state.accountMove?.cancel() ?? true)) {
      _state.say(l10n.moveTooLateDiscard);
      return;
    }
    final thrown = await _state.attempt(
      () => api.moveDiscard(dataDir: _state.dataDir),
    );
    if (thrown == null) return;
    if (!thrown) {
      // It was being put in place, and finished: this computer holds the account now.
      await _state.loadMoveState();
      await _state.stopping.restart();
      return;
    }
    await _state.loadMoveState();
    await stopBringing(_state);
    if (_state.hasProfile && !_state.started) await _state.start();
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final move = _follow();
    final staged = _state.moveState?.kind == 'staged';
    final body = switch (move?.step) {
      null || MoveStep.starting => const Padding(
        padding: EdgeInsets.symmetric(vertical: 60),
        child: Center(child: Spinner(size: 18)),
      ),
      MoveStep.looking => _code(move!, staged),
      MoveStep.matched => _matched(move!),
      MoveStep.moving || MoveStep.finishing => _Progress(
        move: move!,
        title: switch (move.profile?.person.name) {
          final name? => l10n.moveBringingNamed(name),
          null => l10n.moveBringingYours,
        },
        foot: l10n.moveKeepBothOn,
      ),
      MoveStep.done => _done(move!),
      MoveStep.failed => _failed(move!),
    };
    final back = move == null || !move.over;
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          key: const ValueKey('bring-card'),
          width: 420,
          padding: const EdgeInsets.fromLTRB(32, 26, 32, 30),
          decoration: BoxDecoration(
            color: c.bg1,
            borderRadius: BorderRadius.circular(28),
            border: Border.all(color: c.border),
          ),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (back &&
                  move?.step != MoveStep.moving &&
                  move?.step != MoveStep.finishing) ...[
                Align(
                  alignment: Alignment.centerLeft,
                  child: FgButton(
                    text: l10n.commonBack,
                    icon: Ic.back,
                    kind: Kind.ghost,
                    height: 28,
                    padding: const EdgeInsets.symmetric(horizontal: 8),
                    onTap: () => stopBringing(_state),
                  ),
                ),
                const SizedBox(height: 10),
              ],
              AnimatedSwitcher(
                duration: springTime,
                switchInCurve: spring,
                child: KeyedSubtree(key: ValueKey(move?.step), child: body),
              ),
            ],
          ),
        ),
        if (move?.step == MoveStep.looking) ...[
          const SizedBox(height: 16),
          ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 400),
            child: Text(
              l10n.moveKeepOpenNote,
              textAlign: TextAlign.center,
              style: tx(c.fg3, size: 12),
            ),
          ),
        ],
      ],
    );
  }

  Widget _title(String title, String sub) {
    final c = context.fg;
    return Column(
      children: [
        Text(
          title,
          textAlign: TextAlign.center,
          style: tx(c.fg, size: 20, weight: w600, spacing: -.35),
        ),
        const SizedBox(height: 6),
        Text(sub, textAlign: TextAlign.center, style: tx(c.fg2, size: 13)),
      ],
    );
  }

  Widget _code(AccountMove move, bool staged) {
    final c = context.fg;
    final code = move.code ?? '';
    final text = Text(
      code,
      key: const ValueKey('move-code'),
      textAlign: TextAlign.center,
      style: tx(c.fg, size: 34, weight: w500, monospace: true, spacing: 2.7),
    );
    Widget step(int n, List<InlineSpan> words) => Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
            width: heightForText(context, 20, 11),
            height: heightForText(context, 20, 11),
            margin: const EdgeInsets.only(top: 1),
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              border: Border.all(color: c.borderStrong),
            ),
            child: Center(
              // A tighter line only where large text would not fit the circle.
              child: Text(
                '$n',
                style: largeText(context)
                    ? tx(c.fg, size: 11, height: 1.2)
                    : tx(c.fg, size: 11),
              ),
            ),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Text.rich(
              TextSpan(children: words),
              style: tx(c.fg2, size: 13.5),
            ),
          ),
        ],
      ),
    );
    TextSpan bold(String s) => TextSpan(
      text: s,
      style: tx(c.fg, size: 13.5, weight: w500),
    );
    final from = _state.moveState?.computer ?? '';
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        staged
            ? _title(
                switch (_state.moveState?.name) {
                  final name? when name.isNotEmpty =>
                    l10n.moveFinishBringingNamed(name),
                  _ => l10n.moveFinishBringingYours,
                },
                from.isEmpty || from == 'the other computer'
                    ? l10n.moveEverythingCame
                    : l10n.moveEverythingCameFrom(from),
              )
            : _title(l10n.moveBringHereTitle, l10n.moveBringHereSub),
        const SizedBox(height: 18),
        Container(
          padding: const EdgeInsets.fromLTRB(12, 18, 12, 14),
          decoration: BoxDecoration(
            color: c.bg2,
            borderRadius: BorderRadius.circular(14),
            border: Border.all(color: c.border),
          ),
          child: Column(
            children: [
              _shown
                  ? text
                  : ExcludeSemantics(
                      child: ImageFiltered(
                        imageFilter: ImageFilter.blur(sigmaX: 8, sigmaY: 8),
                        child: text,
                      ),
                    ),
              const SizedBox(height: 10),
              Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  FgButton(
                    icon: _shown ? Ic.eyeOff : Ic.eye,
                    height: 32,
                    tooltip: _shown ? l10n.moveHideCode : l10n.moveShowCode,
                    onTap: () => setState(() => _shown = !_shown),
                  ),
                  const SizedBox(width: 8),
                  Text.rich(
                    TextSpan(
                      children: _around(
                        l10n.moveExpiresIn(_mark),
                        TextSpan(
                          text: countdown(move.expiresMs),
                          style: tx(c.fg3, size: 12.5, monospace: true),
                        ),
                      ),
                    ),
                    style: tx(c.fg3, size: 12.5),
                  ),
                ],
              ),
            ],
          ),
        ),
        const SizedBox(height: 18),
        if (staged)
          step(
            1,
            _around(
              l10n.moveStepFogataShows(_mark),
              bold(l10n.moveWaitingToConfirm),
            ),
          )
        else
          step(1, [TextSpan(text: l10n.moveStepOpenThere)]),
        if (staged)
          step(
            2,
            _around(
              l10n.moveStepTypeUnder(_mark),
              bold(l10n.moveFinishTheMove),
            ),
          )
        else ...[
          step(
            2,
            _around(l10n.moveStepGoTo(_mark), bold(l10n.moveSettingsPath)),
          ),
          step(
            3,
            _around(
              l10n.moveStepTypeUnder(_mark),
              bold(l10n.moveToAnotherComputer),
            ),
          ),
        ],
        const SizedBox(height: 4),
        Center(child: _Status(text: l10n.moveWaitingForOther)),
        if (move.wrongTries > 0) ...[
          const SizedBox(height: 12),
          Text(
            l10n.moveWrongCodesTyped(move.wrongTries),
            textAlign: TextAlign.center,
            style: tx(c.amber, size: 12.5),
          ),
        ],
        if (staged) ...[
          const SizedBox(height: 12),
          Center(
            child: FgButton(
              text: l10n.moveThrowAwayInstead,
              kind: Kind.ghost,
              height: 30,
              onTap: _discard,
            ),
          ),
        ],
      ],
    );
  }

  Widget _matched(AccountMove move) {
    final s = move.summary!;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const Center(child: _Machines(oldOn: true)),
        const SizedBox(height: 20),
        _title(
          l10n.moveFoundOther,
          move.lan ? l10n.moveFoundSameNetwork : l10n.moveFoundInternet,
        ),
        const SizedBox(height: 18),
        _List(rows: _whatMoves(context, s)),
        const SizedBox(height: 16),
        if (move.sameAccount) ...[
          _Note(
            icon: Ic.info,
            tone: 'plain',
            child: Text(l10n.moveSameAccountNote),
          ),
          const SizedBox(height: 16),
        ],
        // This computer's own yes first: what comes takes the place of what is here.
        if (move.answered)
          Center(
            child: _Status(text: l10n.moveConfirmThere, dot: context.fg.amber),
          )
        else
          Row(
            children: [
              Expanded(
                child: FgButton(
                  text: l10n.commonCancel,
                  height: 40,
                  expand: true,
                  onTap: () => move.confirm(false),
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                flex: 2,
                child: FgButton(
                  key: const ValueKey('move-take'),
                  text: l10n.moveTakeYes,
                  trailing: Ic.arrowRight,
                  kind: Kind.primary,
                  height: 40,
                  expand: true,
                  onTap: () => move.confirm(true),
                ),
              ),
            ],
          ),
      ],
    );
  }

  Widget _done(AccountMove move) {
    final c = context.fg;
    final s = move.summary;
    final name = s?.profile.person.name ?? _state.moveState?.name ?? '';
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (s != null)
          Center(
            child: Avatar(
              avatar: s.profile.person.avatar,
              size: 84,
              ring: c.greenLine,
            ),
          ),
        const SizedBox(height: 14),
        _title(
          name.isEmpty ? l10n.moveWelcomeBack : l10n.moveWelcomeBackNamed(name),
          l10n.moveDoneSub,
        ),
        const SizedBox(height: 18),
        if (s != null) ...[
          _List(rows: _whatMoves(context, s).sublist(1)),
          const SizedBox(height: 16),
        ],
        FgButton(
          key: const ValueKey('move-open'),
          text: l10n.moveOpenFogata,
          trailing: Ic.arrowRight,
          kind: Kind.primary,
          height: 40,
          expand: true,
          onTap: _open,
        ),
      ],
    );
  }

  Widget _failed(AccountMove move) {
    final c = context.fg;
    final failure = move.failure;
    if (failure == 'expired' || failure == 'too_many_tries') {
      final expired = failure == 'expired';
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _title(l10n.moveBringHereTitle, l10n.moveBringHereSub),
          const SizedBox(height: 18),
          Opacity(
            opacity: .5,
            child: Container(
              padding: const EdgeInsets.fromLTRB(12, 18, 12, 14),
              decoration: BoxDecoration(
                color: c.bg2,
                borderRadius: BorderRadius.circular(14),
                border: Border.all(color: c.border),
              ),
              child: Column(
                children: [
                  Text(
                    move.code ?? '',
                    style:
                        tx(
                          c.fg,
                          size: 34,
                          weight: w500,
                          monospace: true,
                          spacing: 2.7,
                        ).copyWith(
                          decoration: TextDecoration.lineThrough,
                          decorationThickness: 2,
                        ),
                  ),
                  const SizedBox(height: 8),
                  Text(
                    expired
                        ? l10n.moveCodeExpiredLabel
                        : l10n.moveCodeStoppedLabel,
                    style: tx(c.fg3, size: 12.5),
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 16),
          _Note(
            icon: expired ? Ic.hourglass : Ic.warning,
            tone: 'plain',
            child: Text(
              expired ? l10n.moveCodeExpiredNote : l10n.moveCodeStoppedNote,
            ),
          ),
          const SizedBox(height: 16),
          FgButton(
            key: const ValueKey('move-new-code'),
            text: l10n.moveNewCode,
            icon: Ic.retry,
            kind: Kind.primary,
            height: 40,
            expand: true,
            onTap: () => bringHere(_state),
          ),
        ],
      );
    }
    if (failure == 'later') {
      // Everything came and the other computer stopped using the account: it is put in
      // place when Fogata starts here again.
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Center(
            child: _Glyph(icon: Ic.hourglass, tone: 'amber'),
          ),
          const SizedBox(height: 18),
          _title(l10n.moveAlmostThere, l10n.moveLaterSub),
          const SizedBox(height: 18),
          FgButton(
            key: const ValueKey('move-start-again'),
            text: l10n.moveStartAgain,
            icon: Ic.retry,
            kind: Kind.primary,
            height: 40,
            expand: true,
            onTap: _state.stopping.restart,
          ),
        ],
      );
    }
    final unconfirmed = failure == 'unconfirmed';
    final declined = failure == 'declined';
    final (title, sub) = unconfirmed
        ? (l10n.moveAlmostThere, l10n.moveUnconfirmedSub)
        : declined
        ? (l10n.moveStoppedThereTitle, l10n.moveStoppedThereSub)
        : failure == 'other'
        ? (
            l10n.moveCouldNotStartTitle,
            move.reasonWords ?? l10n.moveSomethingWrongHere,
          )
        : (l10n.moveHalfwayTitle, l10n.moveHalfwaySub);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Center(
          child: _Glyph(icon: Ic.warning, tone: 'amber'),
        ),
        const SizedBox(height: 18),
        _title(title, sub),
        const SizedBox(height: 18),
        FgButton(
          key: const ValueKey('move-try-again'),
          text: unconfirmed ? l10n.moveShowCodeToFinish : l10n.moveTryNewCode,
          icon: Ic.retry,
          kind: Kind.primary,
          height: 40,
          expand: true,
          onTap: () async {
            await _state.loadMoveState();
            await bringHere(_state);
          },
        ),
        const SizedBox(height: 10),
        Center(
          child: FgButton(
            text: _state.replacing
                ? switch (_state.contacts.profile?.person.name) {
                    final name? => l10n.moveBackToNamed(name),
                    null => l10n.moveBackToThisProfile,
                  }
                : l10n.moveNewProfileInstead,
            kind: Kind.ghost,
            height: 30,
            onTap: unconfirmed ? _discard : () => stopBringing(_state),
          ),
        ),
      ],
    );
  }
}

/// A round mark in a soft circle of its colour.
class _Glyph extends StatelessWidget {
  const _Glyph({required this.icon, required this.tone});
  final Ic icon;
  final String tone;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final (bg, line, color) = switch (tone) {
      'green' => (c.greenSoft, c.greenLine, c.green),
      'red' => (c.redSoft, c.redLine, c.red),
      _ => (c.amberSoft, c.amberLine, c.amber),
    };
    return Container(
      width: 64,
      height: 64,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: bg,
        border: Border.all(color: line),
      ),
      child: Center(child: FgIcon(icon, size: 28, color: color)),
    );
  }
}

Future<bool> _confirmDiscard(BuildContext context) async {
  final c = context.fg;
  final yes = await showFgDialog<bool>(
    context,
    builder: (dialog) => Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                l10n.moveDiscardTitle,
                style: tx(c.fg, size: 18, weight: w600, spacing: -.2),
              ),
              const SizedBox(height: 6),
              Text(l10n.moveDiscardText, style: tx(c.fg2)),
            ],
          ),
        ),
        Footer(
          children: [
            const Spacer(),
            FgButton(
              text: l10n.moveKeepIt,
              onTap: () => Navigator.of(dialog).pop(false),
            ),
            const SizedBox(width: 10),
            FgButton(
              text: l10n.moveThrowAway,
              kind: Kind.danger,
              onTap: () => Navigator.of(dialog).pop(true),
            ),
          ],
        ),
      ],
    ),
  );
  return yes ?? false;
}

/// Under the welcome page's profile card: for people who already use Fogata elsewhere.
class BringLink extends StatelessWidget {
  const BringLink({super.key});

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final state = AppScope.read(context);
    return Springy(
      key: const ValueKey('bring-link'),
      onTap: () => bringHere(state),
      label: l10n.moveBringFromAnother,
      focusRadius: 999,
      hoverScale: 1.03,
      pressScale: .96,
      builder: (context, hovered) => AnimatedContainer(
        duration: const Duration(milliseconds: 160),
        // Two lines, rounder, where one does not fit.
        constraints: const BoxConstraints(minHeight: 38),
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 6),
        decoration: BoxDecoration(
          color: hovered ? c.hover : c.bg1,
          borderRadius: BorderRadius.circular(19),
          border: Border.all(color: c.border),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            FgIcon(Ic.swap, size: 15, color: c.fg2),
            const SizedBox(width: 10),
            Flexible(
              child: Text.rich(
                textAlign: TextAlign.center,
                TextSpan(
                  children: [
                    TextSpan(
                      text: '${l10n.moveAlreadyOnFogata}  ',
                      style: tx(c.fg2, size: 13.5),
                    ),
                    TextSpan(
                      text: l10n.moveBringFromAnother,
                      style: tx(c.fg, size: 13.5, weight: w600),
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

// ----- The settings' page -----

/// Settings → Move account: moving this account to another computer, and bringing one
/// here in its place.
class MoveAccountPage extends StatefulWidget {
  const MoveAccountPage({super.key});

  @override
  State<MoveAccountPage> createState() => _MoveAccountPageState();
}

class _MoveAccountPageState extends State<MoveAccountPage> {
  final _code = TextEditingController();
  late final AppState _state;
  AccountMove? _move;
  String? _problem;
  bool _replacing = false;

  @override
  void initState() {
    super.initState();
    _state = AppScope.read(context);
    // The button follows what is typed, and a problem said goes as the code changes.
    _code.addListener(() => setState(() => _problem = null));
    // Opened again while this computer's account is on its way: the page follows it.
    final going = _state.accountMove;
    if (going != null && going.sending && !going.over) {
      _move = going..addListener(_changed);
    }
  }

  @override
  void dispose() {
    _move?.removeListener(_changed);
    _code.dispose();
    super.dispose();
  }

  void _changed() {
    final move = _move;
    if (move != null && move.failure == 'wrong_code' && move.code != null) {
      _wrongCodes.add(_codeKey(move.code!));
    }
    if (mounted) setState(() {});
  }

  /// Codes typed here that did not match the one shown, as written without dashes.
  final _wrongCodes = <String>{};

  static String _codeKey(String text) =>
      text.toUpperCase().replaceAll(RegExp(r'[\s-]'), '');

  Future<void> _start() async {
    // The core is stopped for a recovery copy: a move waits until it is done.
    if (_state.stoppedBy != null) {
      _state.say(l10n.moveBusyWithCopy, error: true);
      return;
    }
    final text = _code.text;
    // Each code gets one try: one that did not match is not tried again, so whoever
    // answers in place of the new computer gets no second guess.
    if (_wrongCodes.contains(_codeKey(text))) {
      setState(() => _problem = l10n.moveCodeTriedBefore);
      return;
    }
    final problem = codeProblemWords(api.moveCodeProblem(code: text));
    if (problem != null) {
      setState(() => _problem = problem);
      return;
    }
    _move?.removeListener(_changed);
    setState(() => _problem = null);
    final move = moveAway(_state, text);
    move.addListener(_changed);
    setState(() => _move = move);
  }

  @override
  Widget build(BuildContext context) {
    final move = _move;
    final busy = move != null && !move.over;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (move != null &&
            (move.step == MoveStep.moving || move.step == MoveStep.finishing))
          Panel(
            child: Padding(
              padding: const EdgeInsets.all(20),
              child: _Progress(
                move: move,
                title: l10n.moveMovingTo(computerShown(move.computer)),
                foot: l10n.moveKeepBothOnOffline,
              ),
            ),
          )
        else
          _moveCard(move, busy),
        const SizedBox(height: 16),
        _bringCard(enabled: !busy),
      ],
    );
  }

  Widget _card(String title, String sub, List<Widget> children) {
    final c = context.fg;
    return Panel(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(title, style: tx(c.fg, size: 15, weight: w600, spacing: -.1)),
            const SizedBox(height: 4),
            Text(sub, style: tx(c.fg2, size: 13)),
            const SizedBox(height: 14),
            ...children,
          ],
        ),
      ),
    );
  }

  Widget _moveCard(AccountMove? move, bool busy) {
    final c = context.fg;
    Widget step(int n, List<InlineSpan> words) => Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
            width: heightForText(context, 20, 11),
            height: heightForText(context, 20, 11),
            margin: const EdgeInsets.only(top: 1),
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              border: Border.all(color: c.borderStrong),
            ),
            child: Center(
              // A tighter line only where large text would not fit the circle.
              child: Text(
                '$n',
                style: largeText(context)
                    ? tx(c.fg, size: 11, height: 1.2)
                    : tx(c.fg, size: 11),
              ),
            ),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Text.rich(
              TextSpan(children: words),
              style: tx(c.fg2, size: 13.5),
            ),
          ),
        ],
      ),
    );
    final failure = move?.step == MoveStep.failed ? move?.failure : null;
    final wrong = failure == 'wrong_code' || _problem != null;
    final field = FgField(
      key: const ValueKey('move-code-field'),
      controller: _code,
      placeholder: 'XXXXX-XXXXX',
      monospace: true,
      fontSize: 16,
      label: l10n.moveCodeFieldLabel,
      background: c.bg2,
      strong: wrong,
      onSubmitted: busy ? null : (_) => _start(),
    );
    return _card(l10n.moveToAnotherComputer, l10n.moveToAnotherSub, [
      step(
        1,
        _around(
          l10n.moveStepChoose(_mark),
          TextSpan(
            text: l10n.moveBringFromAnother,
            style: tx(c.fg, size: 13.5, weight: w500),
          ),
        ),
      ),
      step(2, [TextSpan(text: l10n.moveStepTypeItShows)]),
      const SizedBox(height: 4),
      Row(
        children: [
          Expanded(
            child: IgnorePointer(
              ignoring: busy,
              // A code that did not work: the field's own border turns red.
              child: DecoratedBox(
                key: ValueKey('move-code-${wrong ? 'wrong' : 'ok'}'),
                position: DecorationPosition.foreground,
                decoration: BoxDecoration(
                  borderRadius: BorderRadius.circular(8),
                  border: wrong
                      ? Border.all(color: c.red.withValues(alpha: .7))
                      : null,
                ),
                child: field,
              ),
            ),
          ),
          const SizedBox(width: 10),
          if (busy)
            FgButton(
              text: l10n.commonCancel,
              // Not while the core stops: it starts again once the move ends.
              onTap: move?.step == MoveStep.starting
                  ? null
                  : () {
                      if (move?.cancel() == false) {
                        _state.say(l10n.moveTooLateOtherHasIt);
                      }
                    },
            )
          else
            FgButton(
              key: const ValueKey('move-continue'),
              text: failure == null ? l10n.commonContinue : l10n.commonTryAgain,
              icon: failure == null ? null : Ic.retry,
              trailing: failure == null ? Ic.arrowRight : null,
              kind: Kind.primary,
              onTap: _code.text.trim().isEmpty ? null : _start,
            ),
        ],
      ),
      if (busy) ...[
        const SizedBox(height: 12),
        Align(
          alignment: Alignment.centerLeft,
          child: _Status(
            text: switch (move!) {
              AccountMove(step: MoveStep.matched, answered: true) =>
                l10n.moveConfirmThere,
              AccountMove(step: MoveStep.matched) => l10n.moveWaitingForAnswer,
              _ => l10n.moveLookingForCode,
            },
          ),
        ),
      ] else if (_problem != null) ...[
        const SizedBox(height: 12),
        _Note(icon: Ic.warningCircle, tone: 'red', child: Text(_problem!)),
      ] else if (failure != null && failure != 'cancelled') ...[
        const SizedBox(height: 12),
        _failure(failure, move!),
      ],
      const SizedBox(height: 10),
      Row(
        children: [
          FgIcon(Ic.info, size: 14, color: c.fg3),
          const SizedBox(width: 6),
          Expanded(
            child: Text(
              l10n.moveOfflineWhileMoving,
              style: tx(c.fg3, size: 12.5),
            ),
          ),
        ],
      ),
    ]);
  }

  Widget _failure(String failure, AccountMove move) {
    final c = context.fg;
    return switch (failure) {
      'wrong_code' => _Note(
        icon: Ic.warningCircle,
        tone: 'red',
        child: Text(l10n.moveWrongCodeNote),
      ),
      'no_contact' => _Note(
        icon: Ic.wifiOff,
        child: Text.rich(
          TextSpan(
            children: [
              TextSpan(
                text: '${l10n.moveCantReachNew} ',
                style: tx(c.fg, size: 13, weight: w600),
              ),
              TextSpan(text: l10n.moveCantReachNewNote),
            ],
          ),
        ),
      ),
      'declined' => _Note(icon: Ic.warning, child: Text(l10n.moveDeclinedNote)),
      'other' => _Note(
        icon: Ic.warning,
        child: Text(move.reasonWords ?? l10n.moveCouldNotStart),
      ),
      _ => _Note(icon: Ic.warning, child: Text(l10n.moveHalfwayNote)),
    };
  }

  Widget _bringCard({required bool enabled}) {
    final c = context.fg;
    final me =
        _state.contacts.profile?.person.name ?? l10n.moveThisProfileWords;
    final friends = _state.contacts.friends.length;
    final groups = _state.contacts.groups.length;
    final chats = _state.direct.chats.length;
    return _card(l10n.moveBringCardTitle, l10n.moveBringCardSub, [
      if (!_replacing)
        Row(
          children: [
            FgButton(
              key: const ValueKey('move-bring'),
              text: l10n.moveBringCardButton,
              icon: Ic.swap,
              onTap: enabled ? () => setState(() => _replacing = true) : null,
            ),
            const SizedBox(width: 12),
            Flexible(
              child: Text(
                l10n.moveReplacesHere(me),
                style: tx(c.fg3, size: 12.5),
              ),
            ),
          ],
        )
      else ...[
        _Note(
          icon: Ic.warning,
          child: Text.rich(
            TextSpan(
              children: _around(
                l10n.moveReplaceWarning(_mark, me, friends, groups, chats),
                TextSpan(
                  text: me,
                  style: tx(c.fg, size: 13, weight: w600),
                ),
              ),
            ),
          ),
        ),
        const SizedBox(height: 12),
        Wrap(
          spacing: 10,
          runSpacing: 10,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            _SoftDanger(
              key: const ValueKey('move-replace'),
              text: l10n.moveReplaceAndShow(me),
              icon: Ic.swap,
              onTap: () {
                // The core is stopped for a recovery copy: a move waits until it is done.
                if (_state.stoppedBy != null) {
                  _state.say(l10n.moveBusyWithCopy, error: true);
                  return;
                }
                _state.closeSettings();
                unawaited(bringHere(_state, replacing: true));
              },
            ),
            FgButton(
              text: l10n.commonCancel,
              kind: Kind.ghost,
              onTap: () => setState(() => _replacing = false),
            ),
          ],
        ),
      ],
    ]);
  }
}

/// The old computer's question once the code matched: the new computer's name and what
/// goes there.
class ConfirmMove extends StatelessWidget {
  const ConfirmMove({super.key, required this.move, required this.onAnswer});
  final AccountMove move;

  /// The person's answer: move (`true`) or not.
  final ValueChanged<bool> onAnswer;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final s = move.summary!;
    return Padding(
      key: const ValueKey('move-confirm'),
      padding: const EdgeInsets.all(24),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const Center(child: _Machines()),
          const SizedBox(height: 16),
          Text(
            l10n.moveConfirmTitle(computerShown(move.computer)),
            style: tx(c.fg, size: 18, weight: w600, spacing: -.2),
          ),
          const SizedBox(height: 6),
          Text(
            move.lan ? l10n.moveConfirmSameNetwork : l10n.moveConfirmInternet,
            style: tx(c.fg2, size: 13.5),
          ),
          if (move.replaces != null) ...[
            const SizedBox(height: 6),
            Text(
              l10n.moveConfirmReplaces(move.replaces!),
              style: tx(c.fg2, size: 13.5),
            ),
          ],
          const SizedBox(height: 16),
          _List(rows: _whatMoves(context, s, keys: true, settings: true)),
          const SizedBox(height: 14),
          _Note(icon: Ic.warning, child: Text(l10n.moveConfirmNote)),
          const SizedBox(height: 18),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              FgButton(text: l10n.commonCancel, onTap: () => onAnswer(false)),
              const SizedBox(width: 10),
              FgButton(
                key: const ValueKey('move-yes'),
                text: l10n.moveConfirmYes,
                trailing: Ic.arrowRight,
                kind: Kind.primary,
                onTap: () => onAnswer(true),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

// ----- The old computer afterwards -----

/// Instead of the app on the old computer once its account moved ("moved"), or while the
/// new computer has not confirmed the end ("moving").
class MovedScreen extends StatefulWidget {
  const MovedScreen({super.key});

  @override
  State<MovedScreen> createState() => _MovedScreenState();
}

class _MovedScreenState extends State<MovedScreen> {
  final _code = TextEditingController();
  AccountMove? _move;
  String? _problem;
  bool _busy = false;
  late final AppState _state;

  @override
  void initState() {
    super.initState();
    _state = AppScope.read(context);
    _code.addListener(() => setState(() => _problem = null));
  }

  @override
  void dispose() {
    _move?.removeListener(_changed);
    _code.dispose();
    super.dispose();
  }

  void _changed() {
    if (mounted) setState(() {});
  }

  Future<void> _finish() async {
    final problem = codeProblemWords(api.moveCodeProblem(code: _code.text));
    if (problem != null) {
      setState(() => _problem = problem);
      return;
    }
    // As a first move: the application follows it, and asks before it ends.
    final move = moveAway(_state, _code.text)..addListener(_changed);
    setState(() => _move = move);
  }

  Future<void> _forget() async {
    setState(() => _busy = true);
    final done = await _state.succeeds(
      () => api.moveForget(dataDir: _state.dataDir),
    );
    if (!mounted) return;
    setState(() => _busy = false);
    if (!done) return;
    await _state.loadMoveState();
    _state.hasProfile = false;
    _state.started = false;
    _state.offlineForMove = false;
    _state.notifyListeners();
  }

  Future<void> _keep() async {
    final c = context.fg;
    final yes = await showFgDialog<bool>(
      context,
      builder: (dialog) => Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.all(24),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  l10n.moveKeepTitle,
                  style: tx(c.fg, size: 18, weight: w600, spacing: -.2),
                ),
                const SizedBox(height: 6),
                Text(l10n.moveKeepText, style: tx(c.fg2)),
              ],
            ),
          ),
          Footer(
            children: [
              const Spacer(),
              FgButton(
                text: l10n.commonCancel,
                onTap: () => Navigator.of(dialog).pop(false),
              ),
              const SizedBox(width: 10),
              FgButton(
                text: l10n.moveKeepYes,
                kind: Kind.danger,
                onTap: () => Navigator.of(dialog).pop(true),
              ),
            ],
          ),
        ],
      ),
    );
    if (yes != true || !mounted) return;
    final done = await _state.succeeds(
      () => api.moveKeep(dataDir: _state.dataDir),
    );
    if (!done) return;
    await _state.loadMoveState();
    // Online again here, whether the core stopped for the move in this run or not.
    _state.offlineForMove = false;
    _state.hasProfile = true;
    _state.notifyListeners();
    await _state.start();
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    // Built again as the folder's state changes: a move finished from here ends "moved".
    final moved = AppScope.watch<AppState>(context).moveState;
    final moving = moved?.kind == 'moving';
    final to = computerShown(moved?.computer);
    final name = moved?.name ?? '';
    final at = moved == null || moved.atMs == 0 ? null : moved.atMs;
    final Widget content;
    if (moving) {
      final move = _move;
      final working = move != null && !move.over;
      content = Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const Center(
            child: _Glyph(icon: Ic.hourglass, tone: 'amber'),
          ),
          const SizedBox(height: 18),
          Text(
            l10n.moveWaitingFor(to),
            textAlign: TextAlign.center,
            style: tx(c.fg, size: 24, weight: w600, spacing: -.3),
          ),
          const SizedBox(height: 8),
          Text(
            l10n.moveWaitingText,
            textAlign: TextAlign.center,
            style: tx(c.fg2),
          ),
          const SizedBox(height: 20),
          Panel(
            child: Padding(
              padding: const EdgeInsets.all(20),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(
                    l10n.moveFinishTheMove,
                    style: tx(c.fg, size: 15, weight: w600),
                  ),
                  const SizedBox(height: 4),
                  Text(l10n.moveFinishText, style: tx(c.fg2, size: 13)),
                  const SizedBox(height: 14),
                  Row(
                    children: [
                      Expanded(
                        child: IgnorePointer(
                          ignoring: working,
                          child: FgField(
                            key: const ValueKey('move-finish-field'),
                            controller: _code,
                            placeholder: 'XXXXX-XXXXX',
                            monospace: true,
                            fontSize: 16,
                            background: c.bg2,
                            onSubmitted: (_) => _finish(),
                          ),
                        ),
                      ),
                      const SizedBox(width: 10),
                      FgButton(
                        key: const ValueKey('move-finish'),
                        text: l10n.moveFinish,
                        kind: Kind.primary,
                        busy: working,
                        onTap: _code.text.trim().isEmpty ? null : _finish,
                      ),
                    ],
                  ),
                  if (_problem != null) ...[
                    const SizedBox(height: 12),
                    _Note(
                      icon: Ic.warningCircle,
                      tone: 'red',
                      child: Text(_problem!),
                    ),
                  ] else if (move != null && move.step == MoveStep.failed) ...[
                    const SizedBox(height: 12),
                    _Note(
                      icon: Ic.warning,
                      child: Text(
                        move.failure == 'wrong_code'
                            ? l10n.moveFinishWrongCode
                            : move.failure == 'no_contact'
                            ? l10n.moveFinishNoContact
                            : move.failureWords,
                      ),
                    ),
                  ],
                ],
              ),
            ),
          ),
          const SizedBox(height: 4),
          Center(
            child: FgButton(
              text: l10n.moveKeepThisOne,
              kind: Kind.ghost,
              height: 30,
              onTap: working ? null : _keep,
            ),
          ),
        ],
      );
    } else {
      content = Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const Center(child: _Machines(oldOff: true)),
          const SizedBox(height: 20),
          Text(
            name.isEmpty
                ? l10n.moveMovedYours(to)
                : l10n.moveMovedNamed(name, to),
            textAlign: TextAlign.center,
            style: tx(c.fg, size: 24, weight: w600, spacing: -.3),
          ),
          const SizedBox(height: 8),
          Text(
            _movedNote(at, name),
            textAlign: TextAlign.center,
            style: tx(c.fg2),
          ),
          const SizedBox(height: 20),
          Panel(
            child: Padding(
              padding: const EdgeInsets.all(20),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    l10n.moveLeftHere,
                    style: tx(c.fg, size: 15, weight: w600),
                  ),
                  const SizedBox(height: 4),
                  Text(l10n.moveLeftHereText, style: tx(c.fg2, size: 13)),
                  const SizedBox(height: 14),
                  Wrap(
                    spacing: 10,
                    runSpacing: 10,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: [
                      _SoftDanger(
                        key: const ValueKey('move-forget'),
                        text: l10n.moveDeleteData,
                        icon: Ic.trash,
                        onTap: _busy ? null : _forget,
                      ),
                      FgButton(
                        text: l10n.commonNotNow,
                        kind: Kind.ghost,
                        onTap: () => quitApplication(),
                      ),
                    ],
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 4),
          Text(
            l10n.moveAfterDeleting,
            textAlign: TextAlign.center,
            style: tx(c.fg3, size: 12.5),
          ),
        ],
      );
    }
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
                // Decoded at the size it shows, not the file's 256 pixels.
                cacheWidth: (18 * MediaQuery.devicePixelRatioOf(context))
                    .round(),
              ),
            ),
            const SizedBox(width: 8),
            IgnorePointer(child: Text('Fogata', style: wordmark(c.fg))),
            const Spacer(),
          ],
        ),
        Expanded(
          child: Center(
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(32),
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 520),
                child: content,
              ),
            ),
          ),
        ),
      ],
    );
  }
}

/// Under the account that moved: when (`Today at 23:14.`, `Yesterday 23:14.`,
/// `Oct 21, 23:14.`), then that this computer no longer uses it.
String _movedNote(int? at, String name) {
  final note = name.isEmpty
      ? l10n.moveMovedNoteYou
      : l10n.moveMovedNoteNamed(name);
  if (at == null) return note;
  final day = dayClock(at);
  final when = day == clock(at)
      ? l10n.moveMovedTodayAt(day, clockHour(at))
      : l10n.moveMovedOn(capitalized(day));
  return l10n.moveMovedWhenAndNote(when, note);
}

/// Why typed text is not a code, in words; null for a code.
String? codeProblemWords(api.UiCodeProblem? problem) => switch (problem) {
  null => null,
  api.UiCodeProblem.notACode => l10n.moveCodeNotACode,
  api.UiCodeProblem.mistyped => l10n.moveCodeMistyped,
};

/// [child], what the window shows, with the old computer's question ([MoveQuestion]) over
/// it while a move from here asks it, until it is answered. The child keeps its place, and
/// so its state, as the question comes and goes: a page under it (the one the move was
/// started from, with what it follows) goes on as it was.
class WithMoveQuestion extends StatelessWidget {
  const WithMoveQuestion({super.key, required this.child});
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final move = AppScope.watch<AppState>(context).accountMove;
    final asking =
        move != null &&
        move.sending &&
        move.step == MoveStep.matched &&
        !move.answered;
    return Stack(
      fit: StackFit.expand,
      children: [
        child,
        if (asking) MoveQuestion(move: move),
      ],
    );
  }
}

/// The old computer's one question, over whatever the window shows, once the code matched:
/// what goes and to where. The application follows the move itself, so leaving the page it
/// was started from does not lose the question.
class MoveQuestion extends StatelessWidget {
  const MoveQuestion({super.key, required this.move});
  final AccountMove move;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Positioned.fill(
      child: ColoredBox(
        color: c.scrim,
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 40),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 460),
              child: Material(
                type: MaterialType.transparency,
                child: Container(
                  clipBehavior: Clip.antiAlias,
                  // Inside the outline, as when it was drawn under what is inside.
                  padding: const EdgeInsets.all(1),
                  decoration: BoxDecoration(
                    color: c.bg1,
                    borderRadius: BorderRadius.circular(16),
                  ),
                  // Over what is inside, which would cover it at the rounded corners.
                  foregroundDecoration: BoxDecoration(
                    borderRadius: BorderRadius.circular(16),
                    border: Border.all(color: c.border),
                  ),
                  child: ConfirmMove(move: move, onAnswer: move.confirm),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
