import 'dart:async';
import 'dart:io';
import 'dart:math' as math;

import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';

import '../app_state.dart';
import '../format.dart';
import '../host.dart';
import '../icons.dart';
import '../l10n.dart';
import '../rust/api/recovery.dart' as api;
import '../theme.dart';
import '../widgets.dart';
import 'move_account.dart' show bringHere;

// Recovery copies: a file that brings the account back when this computer's keychain or
// disk is lost, opened only with a recovery key the person keeps (the core's `recovery`).
// Settings › Storage turns them on and makes them; the welcome page restores one.

/// Where the person saves a copy, offered under [name] in [folder]; null when they chose
/// nowhere. Tests put their own in place of the system's dialog.
@visibleForTesting
Future<String?> Function(String name, String? folder) chooseWhereToSave =
    (name, folder) async => (await getSaveLocation(
      suggestedName: name,
      initialDirectory: folder,
      acceptedTypeGroups: [_copies()],
    ))?.path;

/// The copy the person picks to restore; null when they picked none. Tests put their own
/// in place of the system's dialog.
@visibleForTesting
Future<String?> Function() chooseCopyToOpen = () async =>
    (await openFile(acceptedTypeGroups: [_copies()]))?.path;

/// Copies, as the system's dialogs show them, named in the language in use.
XTypeGroup _copies() =>
    XTypeGroup(label: l10n.recoveryFileType, extensions: const ['fogata-copy']);

String _fileName(String path) => File(path).uri.pathSegments.last;

/// Why typed text is not a recovery key, in words; null for a key.
String? keyProblemWords(api.UiKeyProblem? problem) => switch (problem) {
  null => null,
  api.UiKeyProblem.notAKey => l10n.recoveryKeyNotAKey,
  api.UiKeyProblem.mistyped => l10n.recoveryKeyMistyped,
};

/// Why a copy does not open, in words.
String copyProblemWords(api.UiCopyProblem problem) => switch (problem) {
  api.UiCopyProblem.notACopy => l10n.recoveryProblemNotACopy,
  api.UiCopyProblem.newerVersion => l10n.recoveryProblemNewer,
  api.UiCopyProblem.otherKey => l10n.recoveryProblemOtherKey,
  api.UiCopyProblem.damaged => l10n.recoveryProblemDamaged,
  api.UiCopyProblem.unproven => l10n.recoveryProblemUnproven,
};

/// A note in a soft colour with its icon: amber for a warning, plain otherwise.
class _Note extends StatelessWidget {
  const _Note({
    required this.icon,
    required this.text,
    this.more,
    this.warning = true,
  });
  final Ic icon;
  final String text;

  /// A second sentence, under the first.
  final String? more;
  final bool warning;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final (bg, line, iconColor) = warning
        ? (c.amberSoft, c.amberLine, c.amber)
        : (c.bg2, c.border, c.fg2);
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
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  text,
                  style: tx(warning ? c.fg : c.fg2, size: 13, height: 1.45),
                ),
                if (more != null)
                  Text(more!, style: tx(c.fg3, size: 12, height: 1.45)),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// A dialog's words over its footer: a title, then what [body] holds.
Widget _dialog(
  BuildContext context, {
  Key? key,
  required String title,
  required List<Widget> body,
  required List<Widget> footer,
}) {
  final c = context.fg;
  return Column(
    key: key,
    mainAxisSize: MainAxisSize.min,
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      Flexible(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                title,
                style: tx(c.fg, size: 18, weight: w600, spacing: -.2),
              ),
              const SizedBox(height: 8),
              ...body,
            ],
          ),
        ),
      ),
      Footer(children: footer),
    ],
  );
}

/// A yes or no to something that cannot be undone; whether it was yes.
Future<bool> _confirm(
  BuildContext context, {
  required String title,
  required String text,
  required String yes,
  bool danger = false,
}) async {
  final c = context.fg;
  final answer = await showFgDialog<bool>(
    context,
    maxWidth: 440,
    builder: (dialog) => _dialog(
      dialog,
      title: title,
      body: [Text(text, style: tx(c.fg2, height: 1.45))],
      footer: [
        const Spacer(),
        FgButton(
          text: l10n.commonCancel,
          onTap: () => Navigator.of(dialog).pop(false),
        ),
        const SizedBox(width: 10),
        FgButton(
          key: const ValueKey('recovery-confirm'),
          text: yes,
          kind: danger ? Kind.danger : Kind.primary,
          onTap: () => Navigator.of(dialog).pop(true),
        ),
      ],
    ),
  );
  return answer ?? false;
}

// ----- Settings › Storage -----

/// Settings › Storage: whether recovery copies are on and for which key, turning them on
/// or changing the key, making a copy, and deleting the account a restore replaced.
class RecoveryCopyPanel extends StatefulWidget {
  const RecoveryCopyPanel({super.key});

  @override
  State<RecoveryCopyPanel> createState() => _RecoveryCopyPanelState();
}

class _RecoveryCopyPanelState extends State<RecoveryCopyPanel> {
  late final AppState _app;
  api.UiRecoveryState? _state;

  /// Why the folder's copies could not be read.
  String? _problem;

  @override
  void initState() {
    super.initState();
    _app = AppScope.read(context);
    unawaited(_load());
  }

  Future<void> _load() async {
    try {
      final s = await api.recoveryState(dataDir: _app.dataDir);
      if (mounted) {
        setState(() {
          _state = s;
          _problem = null;
        });
      }
    } catch (e) {
      // A key the folder keeps that does not check out, say: the section says so, and
      // turning copies on again makes a new one.
      if (mounted) {
        setState(() {
          _state = const api.UiRecoveryState(isOn: false, replacedKept: false);
          _problem = explain(e);
        });
      }
    }
  }

  /// Turns copies on, or changes their key: a new key is shown once, and taken only once
  /// two of its groups were typed back.
  Future<void> _turnOn({required bool change}) async {
    if (change &&
        !await _confirm(
          context,
          title: l10n.recoveryChangeTitle,
          text: l10n.recoveryChangeText,
          yes: l10n.recoveryChangeConfirm,
        )) {
      return;
    }
    if (!mounted) return;
    final api.UiNewRecoveryKey key;
    try {
      key = api.recoveryNewKey();
    } catch (e) {
      _app.say(explain(e), error: true);
      return;
    }
    final on = await showFgDialog<bool>(
      context,
      maxWidth: 560,
      builder: (dialog) => NewKeyDialog(newKey: key, dataDir: _app.dataDir),
    );
    if (on != true) {
      // Not taken: it goes from memory.
      api.recoveryForgetNewKey();
      return;
    }
    _app.say(l10n.recoveryTurnedOn(key.id));
    await _load();
  }

  /// Makes a copy where the person chooses. The core stops while it is made, so nothing
  /// changes in what is read, and starts again after.
  Future<void> _makeCopy() async {
    final choice = await showFgDialog<_CopyChoice>(
      context,
      maxWidth: 480,
      builder: (dialog) => MakeCopyDialog(
        dataDir: _app.dataDir,
        folder: _app.prefs.downloadsDir,
      ),
    );
    if (choice == null) return;
    final work = _Work.copy();
    if (!await _app.stopForWork(work)) {
      _app.say(l10n.recoveryBusy, error: true);
      return;
    }
    try {
      final made = await api.recoveryExport(
        dataDir: _app.dataDir,
        to: choice.path,
        attachments: choice.attachments,
      );
      _app.say(
        l10n.recoveryCopyMade(_fileName(choice.path), bytes(made.bytes)),
        path: choice.path,
      );
    } catch (e) {
      _app.say(l10n.recoveryCopyFailed(explain(e)), error: true);
    } finally {
      // Whatever happened, the core starts again, and only this work starts it.
      await _app.resumeAfterWork(work);
    }
  }

  /// Deletes the account a restore replaced, once the person said so. The core stops
  /// meanwhile, as for a copy.
  Future<void> _deleteReplaced() async {
    if (!await _confirm(
      context,
      title: l10n.recoveryDeleteTitle,
      text: l10n.recoveryDeleteText,
      yes: l10n.recoveryDelete,
      danger: true,
    )) {
      return;
    }
    final work = _Work.forget();
    if (!await _app.stopForWork(work)) {
      _app.say(l10n.recoveryBusy, error: true);
      return;
    }
    try {
      await api.recoveryForgetReplaced(dataDir: _app.dataDir);
      _app.say(l10n.recoveryDeleted);
    } catch (e) {
      _app.say(explain(e), error: true);
    } finally {
      await _app.resumeAfterWork(work);
    }
    if (mounted) await _load();
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final app = AppScope.of(context);
    final s = _state;
    if (s == null) return const SizedBox.shrink();
    final id = s.keyId;
    // The core is stopped for a copy, a deletion or a move: nothing more starts meanwhile,
    // here or on this panel opened again.
    final busy = app.stoppedBy != null || app.offlineForMove;
    final copying = app.stoppedBy is _Work && (app.stoppedBy as _Work).copy;
    return Panel(
      key: const ValueKey('recovery-panel'),
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              crossAxisAlignment: CrossAxisAlignment.baseline,
              textBaseline: TextBaseline.alphabetic,
              children: [
                Expanded(
                  child: Text(
                    l10n.recoveryTitle,
                    style: tx(c.fg, weight: w600),
                  ),
                ),
                const SizedBox(width: 12),
                Text(
                  s.isOn && id != null ? l10n.recoveryOn(id) : l10n.recoveryOff,
                  key: const ValueKey('recovery-status'),
                  style: tx(s.isOn ? c.green : c.fg3, size: 13, weight: w500),
                ),
              ],
            ),
            const SizedBox(height: 4),
            Text(l10n.recoveryNote, style: tx(c.fg3, size: 12)),
            if (_problem != null) ...[
              const SizedBox(height: 8),
              Text(_problem!, style: tx(c.red, size: 12)),
            ],
            const SizedBox(height: 14),
            Wrap(
              spacing: 10,
              runSpacing: 10,
              children: [
                if (s.isOn)
                  FgButton(
                    key: const ValueKey('recovery-make'),
                    text: l10n.recoveryMakeCopy,
                    icon: Ic.download,
                    kind: Kind.primary,
                    height: 34,
                    fontSize: 13,
                    busy: copying,
                    onTap: busy ? null : _makeCopy,
                  ),
                FgButton(
                  key: const ValueKey('recovery-turn-on'),
                  text: s.isOn ? l10n.recoveryChangeKey : l10n.recoveryTurnOn,
                  icon: s.isOn ? null : Ic.shield,
                  kind: s.isOn ? Kind.ghost : Kind.primary,
                  height: 34,
                  fontSize: 13,
                  onTap: busy ? null : () => _turnOn(change: s.isOn),
                ),
              ],
            ),
            if (s.replacedKept) ...[
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 14),
                child: Container(height: 1, color: c.border),
              ),
              Row(
                children: [
                  Expanded(
                    child: Text(
                      l10n.recoveryReplacedKept,
                      style: tx(c.fg2, size: 13),
                    ),
                  ),
                  const SizedBox(width: 12),
                  FgButton(
                    key: const ValueKey('recovery-delete-replaced'),
                    text: l10n.recoveryDeleteReplaced,
                    icon: Ic.trash,
                    kind: Kind.ghost,
                    height: 30,
                    fontSize: 12,
                    onTap: busy ? null : _deleteReplaced,
                  ),
                ],
              ),
            ],
          ],
        ),
      ),
    );
  }
}

/// A piece of work that stops the core ([AppState.stopForWork]): each made anew, so that
/// only the one that stopped it starts it again.
class _Work {
  _Work.copy() : copy = true;
  _Work.forget() : copy = false;

  /// Making a copy, or deleting the account a restore replaced.
  final bool copy;
}

/// The two groups of the key asked back: never the last, which is only the check
/// character. In order.
List<int> _groupsToAsk(int groups, math.Random random) {
  final first = random.nextInt(groups - 1);
  var second = random.nextInt(groups - 2);
  if (second >= first) second++;
  return [math.min(first, second), math.max(first, second)];
}

/// A group of the key as the person may write it: any case, with the letters the key
/// never uses read as the digits they look like.
String _plainGroup(String text) => text
    .trim()
    .toUpperCase()
    .replaceAll('O', '0')
    .replaceAll(RegExp('[IL]'), '1');

/// A new recovery key, shown once, then two of its groups typed back before copies are
/// turned on with it. Pops true once they are.
class NewKeyDialog extends StatefulWidget {
  const NewKeyDialog({
    super.key,
    required this.newKey,
    required this.dataDir,
    this.random,
  });
  final api.UiNewRecoveryKey newKey;
  final String dataDir;

  /// Which groups are asked: at random, or as a test says.
  final math.Random? random;

  @override
  State<NewKeyDialog> createState() => _NewKeyDialogState();
}

class _NewKeyDialogState extends State<NewKeyDialog> {
  late final List<String> _groups = widget.newKey.text.split(' ');
  late final List<int> _asked = _groupsToAsk(
    _groups.length,
    widget.random ?? math.Random.secure(),
  );
  final _typed = [TextEditingController(), TextEditingController()];
  bool _checking = false;
  bool _busy = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    for (final t in _typed) {
      t.addListener(() => setState(() => _error = null));
    }
  }

  @override
  void dispose() {
    for (final t in _typed) {
      t.dispose();
    }
    super.dispose();
  }

  bool _right(int i) =>
      _plainGroup(_typed[i].text) == _plainGroup(_groups[_asked[i]]);

  /// Typed in full and not the group: said at once.
  bool _wrong(int i) =>
      _typed[i].text.trim().length >= _groups[_asked[i]].length && !_right(i);

  Future<void> _turnOn() async {
    if (_busy || !_right(0) || !_right(1)) return;
    setState(() => _busy = true);
    try {
      await api.recoveryTurnOn(dataDir: widget.dataDir, id: widget.newKey.id);
      if (mounted) Navigator.of(context).pop(true);
    } catch (e) {
      if (mounted) {
        setState(() {
          _busy = false;
          _error = explain(e);
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) =>
      _checking ? _check(context) : _show(context);

  Widget _show(BuildContext context) {
    final c = context.fg;
    return _dialog(
      context,
      key: const ValueKey('recovery-new-key'),
      title: l10n.recoveryKeyTitle,
      body: [
        Text(l10n.recoveryKeyText, style: tx(c.fg2, height: 1.45)),
        const SizedBox(height: 16),
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
          decoration: BoxDecoration(
            color: c.bg2,
            borderRadius: BorderRadius.circular(10),
            border: Border.all(color: c.border),
          ),
          child: Text(
            widget.newKey.text,
            key: const ValueKey('recovery-key-text'),
            textAlign: TextAlign.center,
            style: tx(
              c.fg,
              size: 20,
              weight: w500,
              monospace: true,
              height: 1.6,
              spacing: 1,
            ),
          ),
        ),
        const SizedBox(height: 8),
        Text(
          l10n.recoveryKeyName(widget.newKey.id),
          textAlign: TextAlign.center,
          style: tx(c.fg3, size: 12),
        ),
        const SizedBox(height: 14),
        _Note(icon: Ic.warning, text: l10n.recoveryKeyWarning),
      ],
      footer: [
        const Spacer(),
        FgButton(
          text: l10n.commonCancel,
          onTap: () => Navigator.of(context).pop(false),
        ),
        const SizedBox(width: 10),
        FgButton(
          key: const ValueKey('recovery-key-written'),
          text: l10n.recoveryKeyWritten,
          kind: Kind.primary,
          onTap: () => setState(() => _checking = true),
        ),
      ],
    );
  }

  Widget _check(BuildContext context) {
    final c = context.fg;
    Widget field(int i) {
      final wrong = _wrong(i);
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            l10n.recoveryCheckGroup(_asked[i] + 1),
            style: tx(c.fg2, size: 12, weight: w500),
          ),
          const SizedBox(height: 6),
          DecoratedBox(
            position: DecorationPosition.foreground,
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(8),
              border: wrong
                  ? Border.all(color: c.red.withValues(alpha: .7))
                  : null,
            ),
            child: FgField(
              key: ValueKey('recovery-group-$i'),
              controller: _typed[i],
              monospace: true,
              fontSize: 16,
              autofocus: i == 0,
              background: c.bg2,
              label: l10n.recoveryCheckGroup(_asked[i] + 1),
              onSubmitted: (_) => _turnOn(),
            ),
          ),
        ],
      );
    }

    final wrong = _wrong(0) || _wrong(1);
    final error = _error ?? (wrong ? l10n.recoveryCheckWrong : null);
    return _dialog(
      context,
      key: const ValueKey('recovery-check'),
      title: l10n.recoveryCheckTitle,
      body: [
        Text(l10n.recoveryCheckText, style: tx(c.fg2, height: 1.45)),
        const SizedBox(height: 16),
        Row(
          children: [
            Expanded(child: field(0)),
            const SizedBox(width: 12),
            Expanded(child: field(1)),
          ],
        ),
        if (error != null) ...[
          const SizedBox(height: 10),
          Text(error, style: tx(c.red, size: 13)),
        ],
      ],
      footer: [
        FgButton(
          text: l10n.commonBack,
          icon: Ic.back,
          kind: Kind.ghost,
          onTap: _busy ? null : () => setState(() => _checking = false),
        ),
        const Spacer(),
        FgButton(
          key: const ValueKey('recovery-turn-on-confirm'),
          text: l10n.recoveryTurnOnButton,
          kind: Kind.primary,
          busy: _busy,
          onTap: _right(0) && _right(1) ? _turnOn : null,
        ),
      ],
    );
  }
}

/// Where a copy goes, and whether the files sent in chats go in it.
class _CopyChoice {
  const _CopyChoice(this.path, this.attachments);
  final String path;
  final bool attachments;
}

/// Before a copy is made: whether the files sent in chats go in it (off unless chosen,
/// with what they add), then where it is saved. Never in the data folder.
class MakeCopyDialog extends StatefulWidget {
  const MakeCopyDialog({super.key, required this.dataDir, this.folder});
  final String dataDir;

  /// The folder the save dialog opens in.
  final String? folder;

  @override
  State<MakeCopyDialog> createState() => _MakeCopyDialogState();
}

class _MakeCopyDialogState extends State<MakeCopyDialog> {
  bool _attachments = false;
  int? _attachmentsBytes;
  String? _error;

  @override
  void initState() {
    super.initState();
    unawaited(_size());
  }

  Future<void> _size() async {
    try {
      final b = await api.recoveryAttachmentsBytes(dataDir: widget.dataDir);
      if (mounted) setState(() => _attachmentsBytes = b);
    } catch (_) {}
  }

  Future<void> _choose() async {
    final path = await chooseWhereToSave(api.recoveryCopyName(), widget.folder);
    if (path == null || !mounted) return;
    if (api.recoveryInDataFolder(dataDir: widget.dataDir, path: path)) {
      setState(() => _error = l10n.recoveryCopyInDataFolder);
      return;
    }
    Navigator.of(context).pop(_CopyChoice(path, _attachments));
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final size = _attachmentsBytes;
    return _dialog(
      context,
      key: const ValueKey('recovery-make-copy'),
      title: l10n.recoveryCopyTitle,
      body: [
        Text(l10n.recoveryCopyText, style: tx(c.fg2, height: 1.45)),
        const SizedBox(height: 16),
        Pressable(
          key: const ValueKey('recovery-attachments'),
          onTap: () => setState(() => _attachments = !_attachments),
          label: l10n.recoveryCopyAttachments,
          focusRadius: 8,
          builder: (context, hovered) => Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Padding(
                padding: const EdgeInsets.only(top: 2),
                child: FgCheck(_attachments),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      l10n.recoveryCopyAttachments,
                      style: tx(c.fg, weight: w500),
                    ),
                    if (size != null)
                      Text(
                        l10n.recoveryCopyAttachmentsNote(bytes(size)),
                        key: const ValueKey('recovery-attachments-size'),
                        style: tx(c.fg3, size: 12),
                      ),
                  ],
                ),
              ),
            ],
          ),
        ),
        if (_error != null) ...[
          const SizedBox(height: 12),
          Text(_error!, style: tx(c.red, size: 13)),
        ],
      ],
      footer: [
        const Spacer(),
        FgButton(
          text: l10n.commonCancel,
          onTap: () => Navigator.of(context).pop(),
        ),
        const SizedBox(width: 10),
        FgButton(
          key: const ValueKey('recovery-choose-where'),
          text: l10n.recoveryCopySave,
          icon: Ic.folder,
          kind: Kind.primary,
          onTap: _choose,
        ),
      ],
    );
  }
}

// ----- The welcome page -----

/// Under the welcome page's profile card, beside the link that brings an account from
/// another computer: for people whose computer, or its keychain, is gone.
class RestoreLink extends StatelessWidget {
  const RestoreLink({super.key});

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final state = AppScope.read(context);
    return Springy(
      key: const ValueKey('restore-link'),
      onTap: () {
        state.restoring = true;
        state.notifyListeners();
      },
      label: l10n.recoveryRestoreLink,
      focusRadius: 999,
      hoverScale: 1.03,
      pressScale: .96,
      builder: (context, hovered) => AnimatedContainer(
        duration: const Duration(milliseconds: 160),
        constraints: const BoxConstraints(minHeight: 34),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 6),
        decoration: BoxDecoration(
          color: hovered ? c.hover : Colors.transparent,
          borderRadius: BorderRadius.circular(17),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            FgIcon(Ic.history, size: 14, color: c.fg3),
            const SizedBox(width: 8),
            Flexible(
              child: Text.rich(
                textAlign: TextAlign.center,
                TextSpan(
                  children: [
                    TextSpan(
                      text: '${l10n.recoveryLostComputer}  ',
                      style: tx(c.fg3, size: 13),
                    ),
                    TextSpan(
                      text: l10n.recoveryRestoreLink,
                      style: tx(c.fg2, size: 13, weight: w600),
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

enum _Step { pick, key, who, restoring }

/// The welcome page's card for restoring a copy, in place of the profile card: the file,
/// then the recovery key, then whose account it holds and what it replaces here, with the
/// warning that only one computer may use an account. Once restored, the application
/// starts with it, as after a move.
class RestoreCard extends StatefulWidget {
  const RestoreCard({super.key});

  @override
  State<RestoreCard> createState() => _RestoreCardState();
}

class _RestoreCardState extends State<RestoreCard> {
  late final AppState _app;
  final _key = TextEditingController();
  _Step _step = _Step.pick;
  bool _busy = false;
  String? _error;

  /// The key was submitted once: from then on, any problem with it is said as it is
  /// typed.
  bool _submitted = false;

  String? _copy;
  api.UiCopyInfo? _info;
  api.UiCopySummary? _summary;
  api.UiAccountHere? _here;

  /// The restore was refused for an account an earlier one kept aside: the card offers to
  /// delete it, since Settings cannot be reached without a profile.
  bool _keptAside = false;

  @override
  void initState() {
    super.initState();
    _app = AppScope.read(context);
    _key.addListener(_typed);
  }

  void _typed() => setState(() => _error = null);

  @override
  void dispose() {
    _forgetKey();
    _key.dispose();
    super.dispose();
  }

  /// The key typed goes from the field once it is no longer needed.
  void _forgetKey() {
    _key.removeListener(_typed);
    _key.clear();
  }

  void _leave() {
    _forgetKey();
    _app.restoring = false;
    _app.notifyListeners();
  }

  void _to(_Step step) => setState(() {
    _step = step;
    _error = null;
    _busy = false;
  });

  Future<void> _pick() async {
    final path = await chooseCopyToOpen();
    if (path == null || !mounted) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final info = await api.recoveryInspect(dataDir: _app.dataDir, copy: path);
      if (!mounted) return;
      if (info.problem case final problem?) {
        setState(() {
          _busy = false;
          _error = copyProblemWords(problem);
        });
        return;
      }
      _copy = path;
      _info = info;
      _to(_Step.key);
    } catch (e) {
      if (mounted) {
        setState(() {
          _busy = false;
          _error = explain(e);
        });
      }
    }
  }

  /// Said while the key is typed: a key with a wrong character as soon as it is whole,
  /// anything else once it was submitted.
  String? get _keyProblem {
    final problem = api.recoveryKeyProblem(text: _key.text);
    if (problem == api.UiKeyProblem.mistyped || _submitted) {
      return keyProblemWords(problem);
    }
    return null;
  }

  Future<void> _open() async {
    final copy = _copy;
    if (copy == null || _busy) return;
    setState(() => _submitted = true);
    if (api.recoveryKeyProblem(text: _key.text) != null) return;
    setState(() => _busy = true);
    try {
      final opened = await api.recoveryOpen(copy: copy, key: _key.text);
      if (!mounted) return;
      final problem =
          keyProblemWords(opened.keyProblem) ??
          switch (opened.problem) {
            final p? => copyProblemWords(p),
            null => null,
          };
      if (problem != null || opened.summary == null) {
        setState(() {
          _busy = false;
          _error = problem;
        });
        return;
      }
      final here = await api.recoveryAccountIn(dataDir: _app.dataDir);
      _summary = opened.summary;
      _here = here;
      _to(_Step.who);
    } catch (e) {
      if (mounted) {
        setState(() {
          _busy = false;
          _error = explain(e);
        });
      }
    }
  }

  Future<void> _restore({bool unencrypted = false}) async {
    final copy = _copy;
    if (copy == null) return;
    _to(_Step.restoring);
    _keptAside = false;
    final api.UiRestoreOutcome outcome;
    try {
      outcome = await api.recoveryRestore(
        dataDir: _app.dataDir,
        copy: copy,
        key: _key.text,
        replace: _here != null,
        unencrypted: unencrypted,
      );
    } catch (e) {
      // A copy checked whole but not put in place (the disk failed as it went in) is
      // finished at the next start, as an account a move brought is: start again for it.
      await _app.loadMoveState();
      if (_app.moveState?.kind == 'staged') {
        _forgetKey();
        await _app.stopping.restart();
        return;
      }
      if (mounted) {
        _to(_Step.who);
        setState(() => _error = explain(e));
      }
      return;
    }
    if (!mounted) return;
    if (outcome.restored case final restored?) {
      await _started(restored);
      return;
    }
    if (outcome.refused == api.UiRestoreRefusal.noKeychain) {
      final plain = await _askKeychain(context);
      if (!mounted) return;
      if (plain == null) {
        _to(_Step.who);
      } else {
        // "Try again" asks the keychain again; unencrypted, as the person chose.
        await _restore(unencrypted: plain);
      }
      return;
    }
    final words = switch (outcome.refused) {
      api.UiRestoreRefusal.accountHere => l10n.recoveryRefusedAccountHere,
      api.UiRestoreRefusal.replacedKept => l10n.recoveryRefusedReplacedKept,
      api.UiRestoreRefusal.noRoom => l10n.recoveryRefusedNoRoom(
        bytes(outcome.need),
        bytes(outcome.free),
      ),
      api.UiRestoreRefusal.copyInFolder => l10n.recoveryRefusedInFolder,
      api.UiRestoreRefusal.noKeychain || null =>
        keyProblemWords(outcome.keyProblem) ??
            switch (outcome.problem) {
              final p? => copyProblemWords(p),
              null => l10n.errSomething,
            },
    };
    _to(_Step.who);
    setState(() {
      _error = words;
      _keptAside = outcome.refused == api.UiRestoreRefusal.replacedKept;
    });
  }

  /// Deletes the account an earlier restore kept aside, once the person said so, and
  /// restores again. No core runs on the welcome page.
  Future<void> _deleteKeptAside() async {
    if (!await _confirm(
      context,
      title: l10n.recoveryDeleteTitle,
      text: l10n.recoveryDeleteText,
      yes: l10n.recoveryDelete,
      danger: true,
    )) {
      return;
    }
    if (!mounted) return;
    setState(() => _busy = true);
    try {
      await api.recoveryForgetReplaced(dataDir: _app.dataDir);
    } catch (e) {
      if (mounted) {
        setState(() {
          _busy = false;
          _error = explain(e);
        });
      }
      return;
    }
    if (mounted) await _restore();
  }

  /// The account is here: the application starts with it, as after a move brought it.
  Future<void> _started(api.UiRestored restored) async {
    _forgetKey();
    _app.restoring = false;
    _app.hasProfile = true;
    await _app.loadMoveState();
    await _app.prefs.load();
    await _app.start();
    _app.say(l10n.recoveryRestored(restored.name));
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final body = switch (_step) {
      _Step.pick => _pickStep(),
      _Step.key => _keyStep(),
      _Step.who => _whoStep(),
      _Step.restoring => _restoringStep(),
    };
    return Container(
      key: const ValueKey('restore-card'),
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
          if (_step != _Step.restoring) ...[
            Align(
              alignment: Alignment.centerLeft,
              child: FgButton(
                key: const ValueKey('restore-back'),
                text: l10n.commonBack,
                icon: Ic.back,
                kind: Kind.ghost,
                height: 28,
                padding: const EdgeInsets.symmetric(horizontal: 8),
                onTap: _busy
                    ? null
                    : switch (_step) {
                        _Step.pick => _leave,
                        _Step.key => () => _to(_Step.pick),
                        _ => () => _to(_Step.key),
                      },
              ),
            ),
            const SizedBox(height: 10),
          ],
          AnimatedSwitcher(
            duration: springTime,
            switchInCurve: spring,
            child: KeyedSubtree(key: ValueKey(_step), child: body),
          ),
          if (_error != null && _step != _Step.restoring) ...[
            const SizedBox(height: 12),
            Text(
              _error!,
              key: const ValueKey('restore-error'),
              textAlign: TextAlign.center,
              style: tx(c.red, size: 13),
            ),
            if (_keptAside && _step == _Step.who) ...[
              const SizedBox(height: 10),
              Center(
                child: FgButton(
                  key: const ValueKey('restore-delete-kept'),
                  text: l10n.recoveryDeleteKeptAside,
                  icon: Ic.trash,
                  kind: Kind.danger,
                  height: 32,
                  fontSize: 13,
                  busy: _busy,
                  onTap: _deleteKeptAside,
                ),
              ),
            ],
          ],
        ],
      ),
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

  Widget _pickStep() => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      _title(l10n.recoveryRestoreTitle, l10n.recoveryRestoreSub),
      const SizedBox(height: 22),
      FgButton(
        key: const ValueKey('restore-choose'),
        text: l10n.recoveryChooseCopy,
        icon: Ic.folder,
        kind: Kind.primary,
        expand: true,
        busy: _busy,
        onTap: _pick,
      ),
    ],
  );

  Widget _keyStep() {
    final c = context.fg;
    final info = _info!;
    final problem = _keyProblem;
    final wrong = problem != null;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _title(
          l10n.recoveryRestoreTitle,
          l10n.recoveryMadeOn(
            day(
              DateTime.tryParse(info.madeOn) ?? DateTime.now(),
              withYear: true,
            ),
          ),
        ),
        if (info.thisKey == false) ...[
          const SizedBox(height: 12),
          _Note(icon: Ic.info, text: l10n.recoveryOtherKey),
        ],
        const SizedBox(height: 18),
        DecoratedBox(
          position: DecorationPosition.foreground,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(8),
            border: wrong
                ? Border.all(color: c.red.withValues(alpha: .7))
                : null,
          ),
          child: FgField(
            key: const ValueKey('restore-key-field'),
            controller: _key,
            placeholder: 'XXXX XXXX XXXX …',
            monospace: true,
            autofocus: true,
            label: l10n.recoveryKeyField,
            background: c.bg2,
            onSubmitted: (_) => _open(),
          ),
        ),
        if (problem != null) ...[
          const SizedBox(height: 8),
          Text(
            problem,
            key: const ValueKey('restore-key-problem'),
            style: tx(c.red, size: 12),
          ),
        ],
        const SizedBox(height: 14),
        FgButton(
          key: const ValueKey('restore-open'),
          text: l10n.commonContinue,
          kind: Kind.primary,
          expand: true,
          busy: _busy,
          onTap: _key.text.trim().isEmpty ? null : _open,
        ),
      ],
    );
  }

  Widget _whoStep() {
    final c = context.fg;
    final s = _summary!;
    final here = _here;
    final hereWords = here == null
        ? null
        : here.name != null
        ? l10n.recoveryHereNamed(here.name!)
        : here.fingerprint != null
        ? l10n.recoveryHereDevice(here.fingerprint!)
        : l10n.recoveryHereUnknown;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _title(
          l10n.recoveryWhose(s.name),
          l10n.recoveryHolds(s.files, bytes(s.bytes)),
        ),
        const SizedBox(height: 8),
        SelectableText(
          s.fingerprint,
          textAlign: TextAlign.center,
          style: tx(c.fg3, size: 12, monospace: true),
        ),
        Text(
          s.attachments
              ? l10n.recoveryWithAttachments
              : l10n.recoveryWithoutAttachments,
          textAlign: TextAlign.center,
          style: tx(c.fg3, size: 12),
        ),
        const SizedBox(height: 16),
        _Note(icon: Ic.warning, text: l10n.recoveryOnlyIf),
        const SizedBox(height: 6),
        Align(
          alignment: Alignment.centerLeft,
          child: FgButton(
            key: const ValueKey('restore-move-instead'),
            text: l10n.recoveryMoveInstead,
            icon: Ic.swap,
            kind: Kind.ghost,
            height: 30,
            fontSize: 13,
            onTap: () {
              _forgetKey();
              _app.restoring = false;
              unawaited(bringHere(_app));
            },
          ),
        ),
        if (hereWords != null) ...[
          const SizedBox(height: 8),
          _Note(
            icon: Ic.info,
            warning: false,
            text: hereWords,
            more: l10n.recoveryHereReplaced,
          ),
        ],
        const SizedBox(height: 16),
        FgButton(
          key: const ValueKey('restore-confirm'),
          text: here == null ? l10n.recoveryRestore : l10n.recoveryReplace,
          kind: Kind.primary,
          expand: true,
          onTap: _restore,
        ),
      ],
    );
  }

  Widget _restoringStep() => Padding(
    padding: const EdgeInsets.symmetric(vertical: 24),
    child: Column(
      children: [
        const Spinner(size: 18),
        const SizedBox(height: 14),
        Text(
          l10n.recoveryRestoring(_summary?.name ?? ''),
          textAlign: TextAlign.center,
          style: tx(context.fg.fg2, size: 13),
        ),
      ],
    ),
  );
}

/// The keychain cannot keep the key that encrypts what is restored: the person chooses
/// between trying again (after unlocking or starting it) and keeping the account
/// unencrypted, knowingly, as the welcome page offers. True for unencrypted, false to try
/// again, null when the question was closed.
Future<bool?> _askKeychain(BuildContext context) {
  final c = context.fg;
  return showFgDialog<bool>(
    context,
    builder: (dialog) => _dialog(
      dialog,
      key: const ValueKey('restore-keychain'),
      title: l10n.welcomeKeychainTitle,
      body: [
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
      footer: [
        const Spacer(),
        FgButton(
          key: const ValueKey('restore-keychain-plain'),
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
  );
}
