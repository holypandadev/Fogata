import 'package:flutter/foundation.dart' show listEquals;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../app_state.dart';
import '../autostart.dart';
import '../devices.dart';
import '../format.dart';
import '../host.dart';
import '../icons.dart';
import '../l10n.dart';
import '../moving.dart';
import '../names.dart';
import '../rust/api/app.dart' as api;
import '../rust/api/types.dart';
import '../theme.dart';
import '../version.dart';
import '../widgets.dart';
import '../window.dart';
import 'donate.dart';
import 'identity_preview.dart';
import 'move_account.dart';
import 'recovery.dart';
import 'report.dart';
import 'storage.dart';
import 'theme_picker.dart';
import 'user_panel.dart';

/// Narrower than this, the settings show either the list of parts or one part, with a way
/// back to the list.
const settingsNarrowBelow = 680.0;

/// What each part of the settings is called, and its icon.
(String, Ic) settingsLabel(SettingsSection section) => switch (section) {
  SettingsSection.profile => (l10n.settingsSectionProfile, Ic.user),
  SettingsSection.move => (l10n.settingsSectionMove, Ic.swap),
  SettingsSection.donate => (l10n.settingsSectionDonate, Ic.heart),
  SettingsSection.appearance => (l10n.settingsSectionAppearance, Ic.palette),
  SettingsSection.languageTime => (
    l10n.settingsSectionLanguageTime,
    Ic.translate,
  ),
  SettingsSection.voice => (l10n.settingsSectionVoice, Ic.mic),
  SettingsSection.sounds => (l10n.settingsSectionSounds, Ic.volume),
  SettingsSection.behaviour => (l10n.settingsSectionBehaviour, Ic.appWindow),
  SettingsSection.network => (l10n.settingsSectionNetwork, Ic.globe),
  SettingsSection.storage => (l10n.settingsSectionStorage, Ic.hardDrives),
  SettingsSection.report => (l10n.settingsSectionReport, Ic.bug),
};

/// The parts of the settings under their headings, as the column lists them.
List<(String, List<SettingsSection>)> get _groups => [
  (
    l10n.settingsHeadingUser,
    const [
      SettingsSection.profile,
      SettingsSection.move,
      SettingsSection.donate,
    ],
  ),
  (
    l10n.settingsHeadingApp,
    [
      SettingsSection.appearance,
      SettingsSection.languageTime,
      SettingsSection.voice,
      SettingsSection.sounds,
      SettingsSection.behaviour,
      SettingsSection.network,
      SettingsSection.storage,
    ],
  ),
  (l10n.settingsHeadingHelp, const [SettingsSection.report]),
];

/// Settings: a column lists the parts under small headings and the rest
/// shows the chosen one. Where both do not fit, the list and a part take turns.
/// The parts of the state the settings show: the person's settings and profile, where
/// the settings are, the firewall's notice, and how the secrets are kept (told to the
/// whole state). Rooms, calls, toasts and direct chats do not build them again.
const settingsParts = {Prefs, Contacts, Navigation, Notices, AppState};

class Settings extends StatefulWidget {
  const Settings({super.key});

  @override
  State<Settings> createState() => _SettingsState();
}

class _SettingsState extends State<Settings> {
  late final TextEditingController _name;
  late final TextEditingController _bio;

  /// The bio as it was saved, or as the cards show it when none was ever written.
  late String _bioSaved;
  int _preset = 0;
  Uint8List? _photo;

  /// The animation that moves in place of [_photo], when it is one.
  Uint8List? _moving;

  /// The name, picture and animation as they were saved, to tell what changed.
  late String _nameSaved;
  late int _presetSaved;
  Uint8List? _photoSaved;
  Uint8List? _movingSaved;
  List<UiFinding>? _findings;
  bool _checking = false;

  /// Whether Fogata opens at sign-in, as the system has it; null until read.
  AutostartState? _autostart;
  bool _autostartBusy = false;

  /// The system's setting could not be read: the switch stays off, and says why.
  bool _autostartUnread = false;

  late final AppState _app;

  @override
  void initState() {
    super.initState();
    final state = _app = AppScope.read(context);
    final person = state.contacts.profile?.person;
    _name = TextEditingController(text: person?.name ?? '');
    _preset = person?.avatar.preset ?? 0;
    _photo = person?.avatar.photo;
    _moving = _photo == null ? null : state.prefs.movingPhoto;
    _keepSaved();
    _name.addListener(() => setState(() {}));
    _bioSaved = state.contacts.bio ?? defaultBio;
    _bio = TextEditingController(text: _bioSaved);
    _bio.addListener(() => setState(() {}));
    autostart.read().then(
      (s) {
        if (mounted) setState(() => _autostart = s);
      },
      onError: (_) {
        if (mounted) setState(() => _autostartUnread = true);
      },
    );
    if (state.nav.networkCheckOnOpen) {
      state.nav.networkCheckOnOpen = false;
      WidgetsBinding.instance.addPostFrameCallback((_) => _check());
    }
  }

  @override
  void dispose() {
    if (_app.nav.settingsMayClose == _mayClose) {
      _app.nav.settingsMayClose = null;
    }
    _name.dispose();
    _bio.dispose();
    super.dispose();
  }

  void _keepSaved() {
    _nameSaved = tidyName(_name.text);
    _presetSaved = _preset;
    _photoSaved = _photo;
    _movingSaved = _moving;
  }

  /// The name or the picture differ from what was saved: moving the cursor in the name,
  /// or choosing the same picture again, changes nothing.
  bool get _profileChanged =>
      tidyName(_name.text) != _nameSaved ||
      _preset != _presetSaved ||
      !listEquals(_photo, _photoSaved) ||
      !listEquals(_moving, _movingSaved);

  bool get _canSave =>
      (_profileChanged || _bioChanged) &&
      tidyName(_name.text).isNotEmpty &&
      nameProblem(_name.text) == null &&
      bioProblem(_bio.text) == null;

  /// Before the settings close with changes not saved: saves them, lets them go, or keeps
  /// the settings open, as chosen. Whether they may close.
  Future<bool> _mayClose() async {
    final c = context.fg;
    final canSave = _canSave;
    final choice = await showFgDialog<String>(
      context,
      builder: (dialog) => Column(
        key: const ValueKey('settings-unsaved'),
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.all(24),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  canSave
                      ? l10n.settingsUnsavedSaveTitle
                      : l10n.settingsUnsavedDiscardTitle,
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
                  canSave
                      ? l10n.settingsUnsavedSaveText
                      : l10n.settingsUnsavedDiscardText,
                  style: tx(c.fg2),
                ),
              ],
            ),
          ),
          Footer(
            children: [
              const Spacer(),
              if (canSave) ...[
                FgButton(
                  text: l10n.commonDiscard,
                  onTap: () => Navigator.of(dialog).pop('discard'),
                ),
                const SizedBox(width: 10),
                FgButton(
                  text: l10n.commonKeepEditing,
                  onTap: () => Navigator.of(dialog).pop(),
                ),
                const SizedBox(width: 10),
                FgButton(
                  text: l10n.commonSave,
                  kind: Kind.primary,
                  onTap: () => Navigator.of(dialog).pop('save'),
                ),
              ] else ...[
                FgButton(
                  text: l10n.commonKeepEditing,
                  onTap: () => Navigator.of(dialog).pop(),
                ),
                const SizedBox(width: 10),
                FgButton(
                  text: l10n.commonDiscard,
                  kind: Kind.danger,
                  onTap: () => Navigator.of(dialog).pop('discard'),
                ),
              ],
            ],
          ),
        ],
      ),
    );
    return switch (choice) {
      'discard' => true,
      'save' => mounted && await _save(),
      _ => false,
    };
  }

  Future<void> _check() async {
    final state = AppScope.read(context);
    setState(() => _checking = true);
    final findings = await state.attempt(api.networkCheck);
    if (!mounted) return;
    setState(() {
      _checking = false;
      _findings = findings;
    });
  }

  bool get _bioChanged => tidyBio(_bio.text) != tidyBio(_bioSaved);

  /// Saves what changed; whether all of it was saved.
  Future<bool> _save() async {
    final state = AppScope.read(context);
    final profile = _profileChanged;
    // A profile that could not be saved stays as written, and the reason was said.
    if (profile &&
        !await state.updateProfile(
          tidyName(_name.text),
          _preset,
          _photo,
          moving: _moving,
        )) {
      return false;
    }
    if (profile && mounted) setState(_keepSaved);
    if (_bioChanged) {
      final bio = tidyBio(_bio.text);
      if (!await state.updateBio(bio)) return false;
      if (mounted) setState(() => _bioSaved = bio);
    }
    state.say(profile ? l10n.settingsProfileSaved : l10n.settingsBioSaved);
    return true;
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.only(context, settingsParts);
    // Closing asks first while there is something to save.
    state.nav.settingsMayClose = _profileChanged || _bioChanged
        ? _mayClose
        : null;
    final chosen = state.nav.settingsSection;
    return LayoutBuilder(
      builder: (context, box) {
        if (box.maxWidth < settingsNarrowBelow) {
          return chosen == null
              ? _Column(selected: null, narrow: true)
              : _page(chosen, back: true);
        }
        final shown = chosen ?? SettingsSection.profile;
        return Row(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            SizedBox(
              width: 224,
              child: _Column(selected: shown, narrow: false),
            ),
            Expanded(child: _page(shown, back: false)),
          ],
        );
      },
    );
  }

  /// One part of the settings: its name over what it holds, which scrolls.
  Widget _page(SettingsSection section, {required bool back}) {
    final state = AppScope.only(context, settingsParts);
    final c = context.fg;
    final own = _ownPage(section, back: back);
    return Column(
      key: ValueKey('settings-page:${section.name}'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: EdgeInsets.fromLTRB(back ? 16 : 32, 16, 16, 8),
          child: Row(
            children: [
              if (back) ...[
                FgButton(
                  icon: Ic.back,
                  height: 32,
                  tooltip: l10n.settingsAll,
                  onTap: () => state.showSettingsSection(null),
                ),
                const SizedBox(width: 12),
              ],
              Expanded(
                // The donation page shows no title: the picture and its words say it.
                child: section == SettingsSection.donate
                    ? const SizedBox(height: 32)
                    : Text(
                        settingsLabel(section).$1,
                        overflow: TextOverflow.ellipsis,
                        style: hx(c.fg, size: 20),
                      ),
              ),
              const SizedBox(width: 12),
              FgButton(
                icon: Ic.close,
                height: 32,
                tooltip: l10n.settingsCloseTooltip,
                onTap: state.closeSettings,
              ),
            ],
          ),
        ),
        Expanded(
          child:
              own ??
              PageScroll(
                key: PageStorageKey('settings-scroll:${section.name}'),
                child: Padding(
                  padding: EdgeInsets.fromLTRB(
                    back ? 20 : 32,
                    8,
                    back ? 20 : 32,
                    32,
                  ),
                  child: switch (section) {
                    SettingsSection.profile => _profile(),
                    SettingsSection.move => const MoveAccountPage(),
                    SettingsSection.appearance => _appearance(),
                    SettingsSection.languageTime => _languageTime(),
                    SettingsSection.voice => _voice(),
                    SettingsSection.sounds => _sounds(),
                    SettingsSection.behaviour => _behaviour(),
                    SettingsSection.network => _network(),
                    SettingsSection.storage => _storage(),
                    SettingsSection.donate ||
                    SettingsSection.report => const SizedBox.shrink(),
                  },
                ),
              ),
        ),
      ],
    );
  }

  /// The parts that place their content in the page themselves.
  Widget? _ownPage(SettingsSection section, {required bool back}) =>
      switch (section) {
        SettingsSection.donate => const DonatePage(),
        SettingsSection.report => ReportPage(narrow: back),
        _ => null,
      };

  /// A block of a part: a title, a line on what it does, and its controls.
  Widget _block(String title, String note, Widget body) {
    final c = context.fg;
    return Panel(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 20, 20, 0),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  style: tx(c.fg, size: 16, weight: w600, spacing: -.16),
                ),
                const SizedBox(height: 4),
                Text(note, style: tx(c.fg2, size: 13)),
              ],
            ),
          ),
          Padding(padding: const EdgeInsets.all(20), child: body),
        ],
      ),
    );
  }

  /// A switch with "On" or "Off" beside it; without [toggle] it cannot be changed.
  Widget _switch(String what, bool on, VoidCallback? toggle) {
    final c = context.fg;
    return Align(
      alignment: Alignment.centerLeft,
      child: Pressable(
        onTap: toggle,
        label: on ? l10n.settingsSwitchOn(what) : l10n.settingsSwitchOff(what),
        focusRadius: 999,
        builder: (context, hovered) => Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            FgSwitch(on: on),
            const SizedBox(width: 10),
            // Said in the label already.
            ExcludeSemantics(
              child: Text(
                on ? l10n.settingsOn : l10n.settingsOff,
                style: tx(hovered ? c.fg : c.fg2, size: 13),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _blocks(List<Widget> blocks) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      for (final (i, block) in blocks.indexed) ...[
        if (i > 0) const SizedBox(height: 16),
        block,
      ],
    ],
  );

  Future<void> _uploadPicture() async {
    final state = AppScope.read(context);
    final Picked? picked;
    try {
      picked = await pickProfilePicture();
    } on PictureProblem catch (e) {
      state.say(e.message, error: true);
      return;
    }
    if (picked != null && mounted) {
      setState(() {
        _photo = picked!.still;
        _moving = picked.moving;
      });
    }
  }

  Future<void> _uploadBanner() async {
    final state = AppScope.read(context);
    final Picked? picked;
    try {
      picked = await pickCardBanner();
    } on PictureProblem catch (e) {
      state.say(e.message, error: true);
      return;
    }
    if (picked != null) {
      await state.setBannerPicture(picked.still, moving: picked.moving);
    }
  }

  Widget _profile() {
    final state = AppScope.only(context, settingsParts);
    final c = context.fg;
    final profile = state.contacts.profile;
    final name = tidyName(_name.text);
    Widget label(String text) => Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Text(text, style: tx(c.fg2, size: 13, weight: w500)),
    );
    return _blocks([
      // The card as friends see it, with what is being written.
      IdentityPreview(
        banner: PanelBanner(
          color: state.prefs.bannerColor,
          picture: state.prefs.bannerPicture,
          id: profile?.person.id,
        ),
        picture: Avatar(
          avatar: UiAvatar(preset: _preset, photo: _photo),
          size: IdentityPreview.pictureSize,
          moving: _moving,
        ),
        title: name.isEmpty ? _nameSaved : name,
        below: CardBio(tidyBio(_bio.text), top: 10),
        onPicture: _uploadPicture,
        pictureTooltip: l10n.settingsUploadPicture,
        bannerActions: [
          OverPill(
            icon: Ic.image,
            text: state.prefs.bannerPicture == null
                ? l10n.settingsPicture
                : l10n.settingsBannerAnother,
            tooltip: l10n.settingsBannerUpload,
            onTap: _uploadBanner,
          ),
        ],
      ),
      _block(
        l10n.settingsIdentity,
        l10n.settingsIdentityNote,
        Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            label(l10n.settingsName),
            FgField(controller: _name, maxLength: 40, label: l10n.settingsName),
            NameNote(_name.text),
            const SizedBox(height: 20),
            label(l10n.settingsBio),
            _BioField(controller: _bio),
            const SizedBox(height: 6),
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  child: Text(
                    bioProblem(_bio.text) ??
                        (tidyBio(_bio.text).isEmpty
                            ? l10n.settingsNoBio
                            : l10n.settingsBioWhere(maxBioLines)),
                    style: tx(
                      bioProblem(_bio.text) == null ? c.fg3 : c.red,
                      size: 12,
                    ),
                  ),
                ),
                const SizedBox(width: 12),
                Text(
                  '${_bio.text.runes.length}/$maxBioChars',
                  style: tx(c.fg3, size: 12, monospace: true),
                ),
              ],
            ),
            const SizedBox(height: 20),
            label(l10n.settingsPicture),
            Wrap(
              spacing: 10,
              runSpacing: 10,
              children: [
                for (var i = 0; i < avatarPresets.length; i++)
                  Pressable(
                    onTap: () => setState(() {
                      _preset = i;
                      _photo = null;
                      _moving = null;
                    }),
                    tooltip: avatarPresets[i].name,
                    selected: _photo == null && _preset == i,
                    focusRadius: 20,
                    builder: (context, hovered) => Container(
                      decoration: BoxDecoration(
                        shape: BoxShape.circle,
                        boxShadow: _photo == null && _preset == i
                            ? [
                                BoxShadow(color: c.fg, spreadRadius: 4),
                                BoxShadow(color: c.bg1, spreadRadius: 2),
                              ]
                            : null,
                      ),
                      child: Avatar.preset(i, size: 40),
                    ),
                  ),
                FgButton(
                  icon: Ic.plus,
                  height: 40,
                  radius: 20,
                  tooltip: l10n.settingsUploadPicture,
                  onTap: _uploadPicture,
                ),
              ],
            ),
            const SizedBox(height: 24),
            Align(
              alignment: Alignment.centerRight,
              child: FgButton(
                text: l10n.commonSave,
                kind: Kind.primary,
                onTap: _canSave ? _save : null,
              ),
            ),
          ],
        ),
      ),
      _block(
        l10n.settingsBannerTitle,
        l10n.settingsBannerNote,
        _BannerPicker(state: state),
      ),
      Panel(
        padding: const EdgeInsets.all(20),
        child: Row(
          children: [
            FgIcon(Ic.shield, size: 16, color: c.green),
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    l10n.settingsFingerprint,
                    style: tx(c.fg, size: 13, weight: w500),
                  ),
                  SelectableText(
                    profile?.fingerprint ?? '',
                    style: tx(c.fg3, size: 12, monospace: true),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    ]);
  }

  Widget _appearance() {
    return _blocks([
      _block(l10n.settingsTheme, l10n.settingsThemeNote, const ThemePicker()),
    ]);
  }

  Widget _languageTime() {
    final state = AppScope.only(context, settingsParts);
    // A clock time to show each way of writing them: 14:52.
    final example = DateTime(2026, 1, 1, 14, 52).millisecondsSinceEpoch;
    String shown(bool twelve) {
      final was = clock12;
      clock12 = twelve;
      final text = clock(example);
      clock12 = was;
      return text;
    }

    return _blocks([
      _block(
        l10n.settingsLanguage,
        l10n.settingsLanguageNote,
        Align(
          alignment: Alignment.centerLeft,
          child: SizedBox(
            width: 280,
            child: _Select(
              label: l10n.settingsLanguageChoose,
              options: [
                for (final MapEntry(:key, :value) in languages.entries)
                  (key, value),
              ],
              chosen: state.prefs.language,
              onChoose: state.setLanguage,
            ),
          ),
        ),
      ),
      _block(
        l10n.settingsTime,
        l10n.settingsTimeNote,
        Align(
          alignment: Alignment.centerLeft,
          child: Segmented(
            tabs: [
              (l10n.settingsTime24(shown(false)), null, null),
              (l10n.settingsTime12(shown(true)), null, null),
            ],
            icons: const [Ic.clock, Ic.clock],
            selected: clock12 ? 1 : 0,
            onSelect: (i) => state.setClock12(i == 1),
          ),
        ),
      ),
    ]);
  }

  Widget _voice() => _blocks([
    _block(
      l10n.settingsMicSpeaker,
      l10n.settingsMicSpeakerNote,
      const DeviceChoices(camera: false, testMic: true),
    ),
    _block(
      l10n.settingsCamera,
      l10n.settingsCameraNote,
      const DeviceChoices(audio: false, testVideo: true),
    ),
  ]);

  Widget _sounds() {
    final state = AppScope.only(context, settingsParts);
    return _blocks([
      _block(
        l10n.settingsSoundEffects,
        l10n.settingsSoundEffectsNote,
        _switch(
          l10n.settingsSounds,
          state.prefs.sounds,
          () => state.setSounds(!state.prefs.sounds),
        ),
      ),
    ]);
  }

  Future<void> _setAutostart(bool on) async {
    final state = AppScope.read(context);
    setState(() => _autostartBusy = true);
    AutostartState? now;
    try {
      now = await autostart.write(on);
    } catch (e) {
      state.say(
        l10n.settingsChangeFailed('${e is PlatformException ? e.message : e}'),
        error: true,
      );
      try {
        now = await autostart.read();
      } catch (_) {}
    }
    if (!mounted) return;
    setState(() {
      _autostartBusy = false;
      if (now != null) _autostart = now;
    });
  }

  Widget _behaviour() {
    final state = AppScope.only(context, settingsParts);
    final c = context.fg;
    final start = _autostart;
    // Asked for and waiting for macOS's approval counts as on: turning it off takes it out.
    final on =
        start == AutostartState.on || start == AutostartState.needsApproval;
    final note = switch (start) {
      AutostartState.needsApproval => l10n.settingsAutostartApprove,
      AutostartState.unsupported =>
        onMacOS
            ? l10n.settingsAutostartOldMac
            : l10n.settingsAutostartUnsupported,
      null when _autostartUnread => l10n.settingsAutostartUnread,
      _ => null,
    };
    return _blocks([
      _block(
        l10n.settingsAutostart,
        l10n.settingsAutostartNote,
        Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Opacity(
              opacity: start == AutostartState.unsupported ? .45 : 1,
              child: _switch(
                l10n.settingsAutostart,
                on,
                start == null ||
                        start == AutostartState.unsupported ||
                        _autostartBusy
                    ? null
                    : () => _setAutostart(!on),
              ),
            ),
            if (note != null) ...[
              const SizedBox(height: 8),
              Text(note, style: tx(c.fg3, size: 12)),
            ],
          ],
        ),
      ),
      // Keeping Fogata running needs a tray icon to come back from.
      if (hasTray)
        _block(
          l10n.settingsKeepRunning,
          l10n.settingsKeepRunningNote,
          _switch(
            l10n.settingsKeepRunning,
            state.prefs.tray,
            () => state.setTray(!state.prefs.tray),
          ),
        )
      else if (trayNotShown)
        _block(
          l10n.settingsKeepRunning,
          l10n.settingsKeepRunningNote,
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Opacity(
                opacity: .45,
                child: _switch(l10n.settingsKeepRunning, false, null),
              ),
              const SizedBox(height: 8),
              Text(l10n.settingsNoTray, style: tx(c.fg3, size: 12)),
            ],
          ),
        ),
    ]);
  }

  Widget _network() {
    final state = AppScope.only(context, settingsParts);
    final c = context.fg;
    return _blocks([
      _block(
        l10n.settingsHelping,
        l10n.settingsHelpingNote,
        Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Segmented(
              tabs: [
                (l10n.settingsHelpRelay, null, null),
                (l10n.settingsHelpConnect, null, null),
              ],
              selected: state.prefs.bridge ? 0 : 1,
              onSelect: (i) => state.setBridge(i == 0),
            ),
            const SizedBox(height: 10),
            Text(
              state.prefs.bridge
                  ? l10n.settingsHelpRelayNote
                  : l10n.settingsHelpConnectNote,
              style: tx(c.fg2, size: 13),
            ),
            const SizedBox(height: 4),
            Text(
              // Where closing the window leaves Fogata running in the tray, quitting is
              // done from there.
              hasTray && state.prefs.tray
                  ? l10n.settingsRestartTray
                  : l10n.settingsRestart,
              style: tx(c.fg3, size: 12),
            ),
          ],
        ),
      ),
      _block(
        l10n.settingsNetworkCheck,
        l10n.settingsNetworkCheckNote,
        Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (_findings != null) ...[
              for (final f in _findings!) ...[
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Padding(
                      padding: const EdgeInsets.only(top: 7),
                      child: Dot(switch (f.level) {
                        UiLevel.good => c.green,
                        UiLevel.note => c.fg3,
                        UiLevel.bad => c.red,
                      }),
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(findingText(f), style: tx(c.fg, size: 13)),
                          if (f.topic == UiTopic.firewall &&
                              state.notices.firewallCommand != null) ...[
                            const SizedBox(height: 6),
                            _Command(state.notices.firewallCommand!),
                          ],
                        ],
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 10),
              ],
              const SizedBox(height: 6),
            ],
            Row(
              children: [
                FgButton(
                  text: _findings == null
                      ? l10n.settingsCheckNetwork
                      : l10n.settingsCheckAgain,
                  icon: Ic.retry,
                  busy: _checking,
                  onTap: _check,
                ),
              ],
            ),
          ],
        ),
      ),
    ]);
  }

  Widget _storage() {
    final state = AppScope.only(context, settingsParts);
    final c = context.fg;
    return _blocks([
      // First, where it cannot be missed.
      if (state.atRest == UiAtRest.noKeychain) const NotEncrypted(),
      if (state.atRest == UiAtRest.unfinished)
        NotEncrypted(
          unfinished: true,
          damaged: state.damagedSecret,
          onSetAside: state.setDamagedSecretAside,
        ),
      _block(
        l10n.settingsDownloads,
        l10n.settingsDownloadsNote,
        Text(
          state.prefs.downloadsDir ?? '',
          style: tx(c.fg2, size: 13, monospace: true),
        ),
      ),
      const StorageSettings(),
      const RecoveryCopyPanel(),
      Text(
        l10n.settingsVersionData(appVersion, state.dataDir),
        style: tx(c.fg3, size: 12),
      ),
    ]);
  }
}

/// The column of the settings: the parts under their headings. Alone (narrow), it is the
/// whole page and has the close button.
class _Column extends StatelessWidget {
  const _Column({required this.selected, required this.narrow});
  final SettingsSection? selected;
  final bool narrow;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.only(context, settingsParts);
    final c = context.fg;
    return Container(
      decoration: BoxDecoration(
        color: narrow ? null : c.bg1,
        border: narrow ? null : Border(right: BorderSide(color: c.border)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: EdgeInsets.fromLTRB(narrow ? 24 : 18, 18, 16, 6),
            child: Row(
              children: [
                Expanded(
                  child: Text(l10n.settingsTitle, style: hx(c.fg, size: 20)),
                ),
                if (narrow)
                  FgButton(
                    icon: Ic.close,
                    height: 32,
                    tooltip: l10n.settingsCloseTooltip,
                    onTap: state.closeSettings,
                  ),
              ],
            ),
          ),
          Expanded(
            child: ListView(
              padding: EdgeInsets.fromLTRB(
                narrow ? 16 : 12,
                8,
                narrow ? 16 : 12,
                12,
              ),
              children: [
                for (final (i, (heading, sections)) in _groups.indexed) ...[
                  Padding(
                    padding: EdgeInsets.fromLTRB(10, i == 0 ? 4 : 18, 10, 6),
                    child: Text(
                      heading.toUpperCase(),
                      style: tx(c.fg3, size: 11, weight: w600, spacing: .6),
                    ),
                  ),
                  for (final section in sections)
                    NavItem(
                      key: ValueKey('settings-nav:${section.name}'),
                      text: settingsLabel(section).$1,
                      icon: settingsLabel(section).$2,
                      iconColor: section == SettingsSection.donate
                          ? c.red
                          : null,
                      selected: section == selected,
                      onTap: () => state.showSettingsSection(section),
                    ),
                ],
              ],
            ),
          ),
          Padding(
            padding: EdgeInsets.fromLTRB(narrow ? 26 : 22, 0, 16, 16),
            child: Wrap(
              spacing: 8,
              runSpacing: 4,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                Text('Fogata $appVersion', style: tx(c.fg3, size: 12)),
                const _PreReleasePill(),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// The red label beside the version: this copy is a pre-release.
class _PreReleasePill extends StatelessWidget {
  const _PreReleasePill();

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Container(
      key: const ValueKey('pre-release'),
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 1),
      decoration: BoxDecoration(
        color: c.redSoft,
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: c.redLine),
      ),
      child: Text(
        l10n.settingsPreRelease.toUpperCase(),
        style: tx(c.red, size: 10.5, weight: w600, height: 1.45, spacing: .63),
      ),
    );
  }
}

/// The box a bio is written in: a few lines, as many as a bio may have, and no more
/// characters than one may take.
class _BioField extends StatefulWidget {
  const _BioField({required this.controller});
  final TextEditingController controller;

  @override
  State<_BioField> createState() => _BioFieldState();
}

class _BioFieldState extends State<_BioField> {
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
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        color: c.bg1,
        borderRadius: BorderRadius.circular(8),
        // Focused, a stronger border; no halo around it.
        border: Border.all(color: _focus.hasFocus ? c.fg2 : c.edge),
      ),
      // The label goes on the field's own node: one marked as a text field here would be
      // a second field around it.
      child: Semantics(
        label: l10n.settingsBio,
        child: TextField(
          key: const ValueKey('bio-field'),
          controller: widget.controller,
          focusNode: _focus,
          minLines: 3,
          maxLines: 6,
          keyboardType: TextInputType.multiline,
          inputFormatters: const [BioLines(), CharacterLimit(maxBioChars)],
          cursorColor: c.fg,
          cursorWidth: 1.5,
          style: tx(c.fg, size: 14, height: 1.4),
          decoration: InputDecoration(
            isCollapsed: true,
            border: InputBorder.none,
            counterText: '',
            hintText: l10n.settingsBioHint,
            hintStyle: tx(c.fg3, size: 14, height: 1.4),
          ),
        ),
      ),
    );
  }
}

/// Keeps what is typed or pasted in a bio to its lines: Windows and old Mac line ends
/// become plain ones, tabs become spaces, and an edit that would make more lines than a
/// bio may have is not taken.
class BioLines extends TextInputFormatter {
  const BioLines();

  @override
  TextEditingValue formatEditUpdate(
    TextEditingValue oldValue,
    TextEditingValue newValue,
  ) {
    final text = newValue.text
        .replaceAll('\r\n', '\n')
        .replaceAll('\r', '\n')
        .replaceAll('\t', ' ');
    if ('\n'.allMatches(text).length >= maxBioLines) return oldValue;
    if (text == newValue.text) return newValue;
    final end = (newValue.selection.end - (newValue.text.length - text.length))
        .clamp(0, text.length);
    return TextEditingValue(
      text: text,
      selection: TextSelection.collapsed(offset: end),
    );
  }
}

/// The colours for the top of the person's panel, and a picture of their own.
class _BannerPicker extends StatelessWidget {
  const _BannerPicker({required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final picture = state.prefs.bannerPicture;
    BoxDecoration ring(bool on, {BoxShape shape = BoxShape.circle}) =>
        BoxDecoration(
          shape: shape,
          borderRadius: shape == BoxShape.circle
              ? null
              : BorderRadius.circular(10),
          boxShadow: on
              ? [
                  BoxShadow(color: c.fg, spreadRadius: 4),
                  BoxShadow(color: c.bg1, spreadRadius: 2),
                ]
              : null,
        );
    return Wrap(
      spacing: 12,
      runSpacing: 12,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        for (final (color, name) in bannerColors)
          Springy(
            onTap: () => state.setBannerColor(color),
            tooltip: name,
            selected:
                picture == null &&
                color.toARGB32() == state.prefs.bannerColor.toARGB32(),
            builder: (context, hovered) => AnimatedContainer(
              duration: springTime,
              curve: spring,
              decoration: ring(
                picture == null &&
                    color.toARGB32() == state.prefs.bannerColor.toARGB32(),
              ),
              child: Container(
                width: 28,
                height: 28,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: color,
                  border: Border.all(color: c.border),
                ),
              ),
            ),
          ),
        if (picture != null)
          AnimatedContainer(
            duration: springTime,
            curve: spring,
            decoration: ring(true, shape: BoxShape.rectangle),
            child: ClipRRect(
              borderRadius: BorderRadius.circular(10),
              child: SizedBox(
                width: 84,
                height: 28,
                child: MovingPicture(
                  key: ValueKey(state.prefs.movingBanner),
                  bytes: state.prefs.movingBanner ?? picture,
                  maxPixels: maxMovingBannerPixels,
                  still: Image.memory(
                    picture,
                    width: 84,
                    height: 28,
                    // Decoded at the size it shows, not the banner's 900 pixels.
                    cacheWidth: (84 * MediaQuery.devicePixelRatioOf(context))
                        .round(),
                    fit: BoxFit.cover,
                    gaplessPlayback: true,
                  ),
                ),
              ),
            ),
          ),
      ],
    );
  }
}

/// The firewall's command, with a button that copies it.
class _Command extends StatelessWidget {
  const _Command(this.command);
  final String command;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Container(
      padding: const EdgeInsets.fromLTRB(12, 6, 6, 6),
      decoration: BoxDecoration(
        color: c.bg2,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: c.border),
      ),
      child: Row(
        children: [
          Expanded(
            child: SelectableText(
              command,
              style: tx(c.fg, size: 12, monospace: true),
            ),
          ),
          const SizedBox(width: 8),
          FgButton(
            text: l10n.commonCopy,
            icon: Ic.copy,
            height: 28,
            fontSize: 12,
            onTap: () {
              Clipboard.setData(ClipboardData(text: command));
              AppScope.read(context).say(l10n.firewallCopied(firewallSystem));
            },
          ),
        ],
      ),
    );
  }
}

/// A choice among a few options, as a field that opens their list, like the devices'.
class _Select extends StatelessWidget {
  const _Select({
    required this.label,
    required this.options,
    required this.chosen,
    required this.onChoose,
  });

  /// What the field chooses, for its tooltip.
  final String label;

  /// Each option's value and its text.
  final List<(String, String)> options;
  final String chosen;
  final ValueChanged<String> onChoose;

  Future<void> _open(BuildContext anchor) async {
    final c = anchor.fg;
    final box = anchor.findRenderObject() as RenderBox?;
    final overlay = Overlay.of(anchor).context.findRenderObject() as RenderBox?;
    if (box == null || overlay == null) return;
    final corner = box.localToGlobal(
      Offset(0, box.size.height + 4),
      ancestor: overlay,
    );
    final picked = await showMenu<String>(
      context: anchor,
      position: RelativeRect.fromRect(
        corner & Size(box.size.width, 0),
        Offset.zero & overlay.size,
      ),
      color: c.bg1,
      elevation: 0,
      constraints: BoxConstraints(
        minWidth: box.size.width,
        maxWidth: box.size.width,
      ),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(8),
        side: BorderSide(color: c.border),
      ),
      items: [
        for (final (value, text) in options)
          PopupMenuItem<String>(
            value: value,
            height: 34,
            child: Semantics(
              selected: value == chosen,
              child: Row(
                children: [
                  Expanded(
                    child: Text(
                      text,
                      overflow: TextOverflow.ellipsis,
                      style: tx(
                        value == chosen ? c.fg : c.fg2,
                        size: 13,
                        weight: value == chosen ? w500 : FontWeight.w400,
                      ),
                    ),
                  ),
                  if (value == chosen) FgIcon(Ic.check, size: 12, color: c.fg),
                ],
              ),
            ),
          ),
      ],
    );
    if (picked != null) onChoose(picked);
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final text = options.where((o) => o.$1 == chosen).firstOrNull?.$2 ?? '';
    return Builder(
      builder: (anchor) => Pressable(
        onTap: () => _open(anchor),
        tooltip: label,
        focusRadius: 6,
        builder: (context, hovered) => Container(
          height: 36,
          padding: const EdgeInsets.symmetric(horizontal: 12),
          decoration: BoxDecoration(
            color: hovered ? c.hover : c.bg1,
            borderRadius: BorderRadius.circular(6),
            border: Border.all(color: c.borderStrong),
          ),
          child: Row(
            children: [
              Expanded(
                child: Text(
                  text,
                  overflow: TextOverflow.ellipsis,
                  style: tx(c.fg, size: 13),
                ),
              ),
              const SizedBox(width: 8),
              FgIcon(Ic.chevronDown, size: 14, color: c.fg3),
            ],
          ),
        ),
      ),
    );
  }
}

/// Said in the storage settings when no system keychain kept the key that seals this
/// device's secrets: they are on the disk unencrypted, and Fogata tries again at each start.
/// Or, [unfinished], when sealing them began and some are not sealed yet.
class NotEncrypted extends StatelessWidget {
  const NotEncrypted({
    super.key,
    this.unfinished = false,
    this.damaged,
    this.onSetAside,
  });

  /// Encrypting began and did not end, rather than no keychain keeping a key.
  final bool unfinished;

  /// The name of a secret damaged on the disk that keeps encrypting from its end, with
  /// [onSetAside] to set it aside and finish.
  final String? damaged;
  final Future<void> Function()? onSetAside;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Container(
      key: const ValueKey('not-encrypted'),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: c.amberSoft,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: c.amberLine),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(top: 1),
            child: FgIcon(Ic.warning, size: 16, color: c.amber),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  unfinished
                      ? l10n.settingsNotAllEncrypted
                      : l10n.settingsNotEncrypted,
                  style: tx(c.fg, weight: w600),
                ),
                const SizedBox(height: 4),
                Text(
                  unfinished
                      ? l10n.settingsNotAllEncryptedText
                      : l10n.settingsNotEncryptedText(
                          onLinux ? 'linux' : 'other',
                        ),
                  style: tx(c.fg2, size: 13, height: 1.45),
                ),
                if (unfinished && damaged != null) ...[
                  const SizedBox(height: 8),
                  Text(
                    l10n.settingsDamagedSecret(damaged!),
                    style: tx(c.fg2, size: 13, height: 1.45),
                  ),
                  if (onSetAside != null) ...[
                    const SizedBox(height: 10),
                    FgButton(
                      key: const ValueKey('set-damaged-aside'),
                      text: l10n.settingsSetDamagedAside,
                      onTap: onSetAside,
                    ),
                  ],
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}
