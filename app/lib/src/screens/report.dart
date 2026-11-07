import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../app_state.dart';
import '../format.dart' show explain;
import '../icons.dart';
import '../l10n.dart';
import '../private_file.dart';
import '../report.dart';
import '../reveal.dart';
import '../theme.dart';
import '../typing.dart' show styledSpans;
import '../widgets.dart';

/// The most a report's text takes.
const _maxWhat = 5000;

/// Settings › Report a problem: what happened, whether the log goes too, the whole report
/// to read before it goes, and two ways to send it: an e-mail ready in the mail
/// application with the file beside it, or a file saved where the person chooses. Nothing
/// leaves the computer from here.
class ReportPage extends StatefulWidget {
  const ReportPage({
    super.key,
    this.narrow = false,
    @visibleForTesting this.sent,
    @visibleForTesting this.listShown = false,
    @visibleForTesting this.preview,
    @visibleForTesting this.deliver,
  });

  /// The settings show the list of parts or this page, not both: less padding.
  final bool narrow;

  /// For tests: the page as after "Email it" wrote this report, and with the list of
  /// what goes shown.
  final String? sent;
  final bool listShown;

  /// For tests: the page with the report shown, made of these files.
  final Map<String, String>? preview;

  /// For tests: what "Email it" does with the report, instead of writing it and opening
  /// the mail application.
  final Future<void> Function(PreparedReport report)? deliver;

  @override
  State<ReportPage> createState() => _ReportPageState();
}

class _ReportPageState extends State<ReportPage> {
  final _what = TextEditingController();
  bool _withLog = true;
  late bool _showList = widget.listShown;
  bool _busy = false;

  /// Whether the whole report shows. While it does, what goes is the report shown, and
  /// nothing goes while the report shown is not yet that of what the page holds.
  late bool _showPreview = widget.preview != null;

  /// The parts of the report with what is personal taken out ([ReportParts]): read and
  /// scrubbed once while the page shows, and again when the log is turned on or off.
  ReportParts? _parts;
  Cancellable<ReportParts>? _scrubbing;

  /// The report made of [_parts] and what the person wrote, and the one being made.
  PreparedReport? _prepared;
  Cancellable<PreparedReport>? _assembling;

  /// Making the report again waits for typing to pause.
  Timer? _again;

  /// The report written for an e-mail, once it is: the page then says what to do next.
  late String? _sent = widget.sent;

  @override
  void initState() {
    super.initState();
    _what.addListener(_changed);
  }

  @override
  void dispose() {
    _again?.cancel();
    _scrubbing?.cancel();
    _assembling?.cancel();
    _what.dispose();
    super.dispose();
  }

  /// Whether the report shown is that of what the page holds, nothing being made.
  bool get _shownIsCurrent =>
      widget.preview != null ||
      (_current() != null && _scrubbing == null && _assembling == null);

  bool get _canSend =>
      _what.text.trim().isNotEmpty &&
      !_busy &&
      (!_showPreview || _shownIsCurrent);

  /// What the person wrote changed: the report shown is made again a moment later.
  void _changed() {
    setState(() {});
    if (!_showPreview) return;
    _again?.cancel();
    _again = Timer(const Duration(milliseconds: 400), _refresh);
  }

  void _setWithLog(bool withLog) {
    setState(() {
      _withLog = withLog;
      // The log is read again, as it is now.
      _parts = null;
    });
    if (_showPreview) _refresh();
  }

  void _togglePreview() {
    setState(() => _showPreview = !_showPreview);
    if (_showPreview) _refresh();
  }

  /// The report made, when it is still that of what the page holds.
  PreparedReport? _current() {
    final prepared = _prepared;
    return prepared != null &&
            _parts != null &&
            prepared.isFor(_what.text, _withLog)
        ? prepared
        : null;
  }

  /// Makes the report shown again: its parts when they are not those of the page's
  /// choice, then the report of them. What was being made for an older state stops.
  Future<void> _refresh() async {
    _again?.cancel();
    if (_current() != null) return;
    final state = AppScope.read(context);
    try {
      final parts = await _scrubbed(state);
      if (!mounted) return;
      _assembling?.cancel();
      final what = _what.text;
      final facts = _facts(state);
      final job = assembleReportApart(facts: facts, what: what, parts: parts);
      setState(() => _assembling = job);
      final report = await job.future;
      if (!mounted || !identical(_assembling, job)) return;
      setState(() {
        _prepared = report;
        _assembling = null;
      });
    } on Cancelled {
      // A newer one is being made.
    } catch (e) {
      state.say(l10n.reportFailed(explain(e)), error: true);
      if (mounted) setState(() => _assembling = _scrubbing = null);
    }
  }

  /// The parts of the report for the page's choice of the log: those read before, or
  /// read now.
  Future<ReportParts> _scrubbed(AppState state) async {
    final parts = _parts;
    if (parts != null && parts.withLog == _withLog) return parts;
    _scrubbing?.cancel();
    final job = scrubReportPartsApart(
      dataDir: state.dataDir,
      withLog: _withLog,
      now: DateTime.now(),
      names: reportNames(state).toList(),
    );
    setState(() => _scrubbing = job);
    final scrubbed = await job.future;
    if (mounted && identical(_scrubbing, job)) {
      setState(() {
        _parts = scrubbed;
        _scrubbing = null;
      });
    }
    return scrubbed;
  }

  /// What goes: the report shown when it shows (sending waits for it to be current, see
  /// [_canSend]), or one made now.
  Future<PreparedReport> _report(AppState state) async {
    final current = _current();
    if (current != null) return current;
    return prepareReport(
      dataDir: state.dataDir,
      what: _what.text,
      withLog: _withLog,
      facts: _facts(state),
      names: reportNames(state),
    );
  }

  ReportFacts _facts(AppState state) => ReportFacts(
    build: state.updates.status?.thisBuild.toInt(),
    language: currentLanguage,
    at: DateTime.now(),
  );

  Future<void> _email() async {
    final state = AppScope.read(context);
    setState(() => _busy = true);
    try {
      final report = await _report(state);
      final deliver = widget.deliver;
      if (deliver != null) {
        await deliver(report);
        return;
      }
      final path = await writeReport(
        folder: state.prefs.downloadsDir ?? state.dataDir,
        report: report,
      );
      final name = File(path).uri.pathSegments.last;
      if (!await openExternal(reportMail(name))) {
        state.say(l10n.reportNoMailApp(reportAddress), error: true);
      }
      await showInFolderOrSay(path, state.say);
      if (mounted) setState(() => _sent = path);
    } catch (e) {
      state.say(l10n.reportFailed(explain(e)), error: true);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _save() async {
    final state = AppScope.read(context);
    final where = await getSaveLocation(
      suggestedName: (_current()?.facts ?? _facts(state)).fileName,
      initialDirectory: state.prefs.downloadsDir,
      acceptedTypeGroups: const [
        XTypeGroup(label: 'ZIP', extensions: ['zip']),
      ],
    );
    if (where == null || !mounted) return;
    setState(() => _busy = true);
    try {
      final report = await _report(state);
      final file = File(where.path);
      await writePrivate(file.path, report.bytes);
      state.say(l10n.reportSaved(file.uri.pathSegments.last));
    } catch (e) {
      state.say(l10n.reportFailed(explain(e)), error: true);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final side = widget.narrow ? 20.0 : 32.0;
    final sent = _sent;
    if (sent != null) {
      return LayoutBuilder(
        builder: (context, box) => SingleChildScrollView(
          child: ConstrainedBox(
            constraints: BoxConstraints(minHeight: box.maxHeight),
            child: Align(
              alignment: const Alignment(0, -.1),
              child: Padding(
                padding: EdgeInsets.fromLTRB(side, 8, side, 32),
                child: _Ready(path: sent),
              ),
            ),
          ),
        ),
      );
    }
    return PageScroll(
      key: const PageStorageKey('settings-scroll:report'),
      child: Padding(
        padding: EdgeInsets.fromLTRB(side, 8, side, 32),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [_form(), const SizedBox(height: 16), _howNote()],
        ),
      ),
    );
  }

  Widget _form() {
    final c = context.fg;
    return Panel(
      padding: const EdgeInsets.all(20),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            l10n.reportWhatTitle,
            style: tx(c.fg, size: 15, weight: w600, height: 1.4),
          ),
          const SizedBox(height: 4),
          Text(l10n.reportWhatNote, style: tx(c.fg2, size: 13)),
          const SizedBox(height: 14),
          _WhatField(controller: _what),
          const SizedBox(height: 14),
          _logChoice(),
          const SizedBox(height: 12),
          _preview(),
          const SizedBox(height: 16),
          Wrap(
            spacing: 10,
            runSpacing: 10,
            children: [
              FgButton(
                key: const ValueKey('report-email'),
                text: l10n.reportEmail,
                icon: Ic.paperPlane,
                kind: Kind.primary,
                busy: _busy,
                onTap: _canSend ? _email : null,
              ),
              FgButton(
                key: const ValueKey('report-save'),
                text: l10n.reportSave,
                icon: Ic.fileText,
                kind: Kind.ghost,
                onTap: _canSend ? _save : null,
              ),
            ],
          ),
        ],
      ),
    );
  }

  /// The log's checkbox, what the log holds, and the list of what goes.
  Widget _logChoice() {
    final c = context.fg;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Pressable(
              onTap: () => _setWithLog(!_withLog),
              label: l10n.reportIncludeLog,
              selected: _withLog,
              focusRadius: 5,
              builder: (context, hovered) => Padding(
                padding: const EdgeInsets.only(top: 1),
                child: FgCheck(_withLog, size: 18),
              ),
            ),
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  ExcludeSemantics(
                    child: GestureDetector(
                      onTap: () => _setWithLog(!_withLog),
                      child: Text(
                        l10n.reportIncludeLog,
                        style: tx(c.fg, size: 13.5, height: 1.45),
                      ),
                    ),
                  ),
                  Text.rich(
                    TextSpan(
                      children: [
                        TextSpan(text: '${l10n.reportLogNote} '),
                        WidgetSpan(
                          alignment: PlaceholderAlignment.baseline,
                          baseline: TextBaseline.alphabetic,
                          child: Pressable(
                            key: const ValueKey('report-see-sent'),
                            onTap: () => setState(() => _showList = !_showList),
                            expanded: _showList,
                            label: _showList
                                ? l10n.reportHideSent
                                : l10n.reportSeeSent,
                            builder: (context, hovered) => Text(
                              _showList
                                  ? l10n.reportHideSent
                                  : l10n.reportSeeSent,
                              style: tx(
                                hovered ? c.fg : c.fg2,
                                size: 12.5,
                                height: 1.45,
                                decoration: TextDecoration.underline,
                              ),
                            ),
                          ),
                        ),
                      ],
                    ),
                    style: tx(c.fg3, size: 12.5, height: 1.45),
                  ),
                  AnimatedSize(
                    duration: springTime,
                    curve: spring,
                    alignment: Alignment.topLeft,
                    child: _showList
                        ? Padding(
                            key: const ValueKey('report-sent-list'),
                            padding: const EdgeInsets.only(top: 8),
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                for (final item in reportContents(
                                  withLog: _withLog,
                                ))
                                  Padding(
                                    padding: const EdgeInsets.only(bottom: 4),
                                    child: Row(
                                      crossAxisAlignment:
                                          CrossAxisAlignment.start,
                                      children: [
                                        Text(
                                          '·  ',
                                          style: tx(c.fg3, size: 12.5),
                                        ),
                                        Expanded(
                                          child: Text(
                                            item,
                                            style: tx(
                                              c.fg2,
                                              size: 12.5,
                                              height: 1.45,
                                            ),
                                          ),
                                        ),
                                      ],
                                    ),
                                  ),
                              ],
                            ),
                          )
                        : const SizedBox(width: double.infinity),
                  ),
                ],
              ),
            ),
          ],
        ),
      ],
    );
  }

  /// The link that shows the whole report, and the report under it once it shows.
  Widget _preview() {
    final c = context.fg;
    // While what the page holds changes, the last report made stays until the next one
    // is ready, a moment after typing pauses: that one is what goes.
    final files = widget.preview ?? _prepared?.files;
    final stale = !_shownIsCurrent;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Align(
          alignment: Alignment.centerLeft,
          child: Pressable(
            key: const ValueKey('report-preview'),
            onTap: _togglePreview,
            expanded: _showPreview,
            label: _showPreview ? l10n.reportHidePreview : l10n.reportPreview,
            builder: (context, hovered) => Text(
              _showPreview ? l10n.reportHidePreview : l10n.reportPreview,
              style: tx(
                hovered ? c.fg : c.fg2,
                size: 13,
                height: 1.45,
                decoration: TextDecoration.underline,
              ),
            ),
          ),
        ),
        AnimatedSize(
          duration: springTime,
          curve: spring,
          alignment: Alignment.topLeft,
          child: !_showPreview
              ? const SizedBox(width: double.infinity)
              : Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      Text(
                        l10n.reportPreviewNote,
                        style: tx(c.fg3, size: 12.5, height: 1.45),
                      ),
                      const SizedBox(height: 8),
                      // While the report is made again, the one before shows faded,
                      // and nothing goes until the new one shows.
                      if (stale)
                        Padding(
                          padding: const EdgeInsets.only(bottom: 6),
                          child: Text(
                            l10n.reportPreviewWorking,
                            key: const ValueKey('report-preview-working'),
                            style: tx(c.fg2, size: 12.5, height: 1.45),
                          ),
                        ),
                      if (files != null)
                        AnimatedOpacity(
                          duration: const Duration(milliseconds: 160),
                          opacity: stale ? .45 : 1,
                          child: _ReportText(files: files),
                        ),
                    ],
                  ),
                ),
        ),
      ],
    );
  }

  /// What "Email it" does, under the box.
  Widget _howNote() {
    final c = context.fg;
    const mark = '\u{E000}';
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        color: c.bg2,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: c.border),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(top: 2),
            child: FgIcon(Ic.info, size: 15, color: c.fg2),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Text.rich(
              TextSpan(
                children: styledSpans(l10n.reportHowEmail(mark), {
                  mark: TextSpan(
                    text: reportAddress,
                    style: tx(c.fg, size: 13, weight: w600, height: 1.45),
                  ),
                }),
              ),
              style: tx(c.fg2, size: 13, height: 1.45),
            ),
          ),
        ],
      ),
    );
  }
}

/// After "Email it": the e-mail waits in the mail application, and the report's file in
/// its folder.
class _Ready extends StatelessWidget {
  const _Ready({required this.path});
  final String path;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    const file = '\u{E000}', address = '\u{E001}';
    final name = File(path).uri.pathSegments.last;
    return ConstrainedBox(
      key: const ValueKey('report-ready'),
      constraints: const BoxConstraints(maxWidth: 440),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
            width: 64,
            height: 64,
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              color: c.greenSoft,
              border: Border.all(color: c.greenLine),
            ),
            alignment: Alignment.center,
            child: FgIcon(Ic.checkCircle, size: 28, color: c.green),
          ),
          const SizedBox(height: 18),
          Text(
            l10n.reportReadyTitle,
            textAlign: TextAlign.center,
            style: tx(c.fg, size: 20, weight: FontWeight.w700, height: 1.3),
          ),
          const SizedBox(height: 6),
          Text.rich(
            TextSpan(
              children: styledSpans(l10n.reportReadyText(file, address), {
                file: TextSpan(
                  text: name,
                  style: tx(c.fg2, size: 14, monospace: true, height: 1.45),
                ),
                address: TextSpan(
                  text: reportAddress,
                  style: tx(c.fg, weight: w600, height: 1.45),
                ),
              }),
            ),
            textAlign: TextAlign.center,
            style: tx(c.fg2, height: 1.45),
          ),
          const SizedBox(height: 18),
          Wrap(
            alignment: WrapAlignment.center,
            spacing: 10,
            runSpacing: 10,
            children: [
              FgButton(
                text: l10n.reportShowFile,
                icon: Ic.folder,
                onTap: () => showInFolderOrSay(path, state.say),
              ),
              FgButton(
                text: l10n.reportCopyAddress,
                icon: Ic.copy,
                kind: Kind.ghost,
                onTap: () async {
                  await Clipboard.setData(
                    const ClipboardData(text: reportAddress),
                  );
                  state.say(l10n.reportAddressCopied);
                },
              ),
            ],
          ),
        ],
      ),
    );
  }
}

/// The report's files as they go, each under its name, in a box that scrolls: every line
/// can be read and copied.
class _ReportText extends StatefulWidget {
  const _ReportText({required this.files});
  final Map<String, String> files;

  @override
  State<_ReportText> createState() => _ReportTextState();
}

class _ReportTextState extends State<_ReportText> {
  final _scroll = ScrollController();

  /// The lines shown, each with whether it is a file's name: a log of two days has
  /// thousands, split once for each report and built only as they scroll into the box.
  late List<(String, bool)> _lines = _split(widget.files);

  static List<(String, bool)> _split(Map<String, String> files) => [
    for (final MapEntry(key: name, value: text) in files.entries) ...[
      (name, true),
      for (final line in const LineSplitter().convert(text)) (line, false),
      ('', false),
    ],
  ];

  @override
  void didUpdateWidget(_ReportText old) {
    super.didUpdateWidget(old);
    if (!identical(old.files, widget.files)) _lines = _split(widget.files);
  }

  @override
  void dispose() {
    _scroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final lines = _lines;
    return Container(
      key: const ValueKey('report-preview-text'),
      height: 320,
      decoration: BoxDecoration(
        color: c.bg2,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: c.border),
      ),
      child: SelectionArea(
        child: Scrollbar(
          controller: _scroll,
          child: ListView.builder(
            controller: _scroll,
            padding: const EdgeInsets.all(12),
            itemCount: lines.length,
            itemBuilder: (context, i) {
              final (line, isName) = lines[i];
              return Text(
                line,
                style: tx(
                  isName ? c.fg : c.fg2,
                  size: 12,
                  weight: isName ? w600 : FontWeight.w400,
                  monospace: true,
                  height: 1.45,
                ),
              );
            },
          ),
        ),
      ),
    );
  }
}

/// The box the problem is written in: about five lines, scrolling past them.
class _WhatField extends StatefulWidget {
  const _WhatField({required this.controller});
  final TextEditingController controller;

  @override
  State<_WhatField> createState() => _WhatFieldState();
}

class _WhatFieldState extends State<_WhatField> {
  final _focus = FocusNode();

  @override
  void initState() {
    super.initState();
    _focus.addListener(_changed);
  }

  void _changed() => setState(() {});

  @override
  void dispose() {
    _focus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return AnimatedContainer(
      duration: const Duration(milliseconds: 160),
      height: 132,
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: c.bg2,
        borderRadius: BorderRadius.circular(12),
        // Focused, a stronger border; no halo around it.
        border: Border.all(color: _focus.hasFocus ? c.fg2 : c.edge),
      ),
      child: Semantics(
        label: l10n.reportWhatTitle,
        child: TextField(
          key: const ValueKey('report-what'),
          controller: widget.controller,
          focusNode: _focus,
          expands: true,
          maxLines: null,
          textAlignVertical: TextAlignVertical.top,
          keyboardType: TextInputType.multiline,
          inputFormatters: const [CharacterLimit(_maxWhat)],
          cursorColor: c.fg,
          cursorWidth: 1.5,
          style: tx(c.fg, size: 14, height: 1.45),
          decoration: const InputDecoration(
            isCollapsed: true,
            border: InputBorder.none,
            counterText: '',
          ),
        ),
      ),
    );
  }
}

/// What a report holds, item by item, as the page lists it.
List<String> reportContents({required bool withLog}) => [
  l10n.reportSentWhat,
  l10n.reportSentVersion,
  if (withLog) l10n.reportSentLog,
];

/// The names of people and groups known here, and those known before while Fogata runs
/// ([rememberNames]), which a report's log never carries.
Set<String> reportNames(AppState state) {
  final contacts = state.contacts;
  return rememberNames({
    ?contacts.profile?.person.name,
    for (final f in contacts.friends) f.person.name,
    for (final g in contacts.groups) ...[
      g.name,
      g.localName,
      for (final v in g.voiceRooms) v.name,
    ],
    for (final members in contacts.members.values)
      for (final m in members) m.person.name,
    for (final a in contacts.joinAsks) ...[a.name, a.groupName],
    for (final i in contacts.invitations) ...[i.name, i.by.name],
    for (final r in contacts.requests.incoming) r.person.name,
    for (final r in contacts.requests.outgoing) r.person.name,
  });
}
