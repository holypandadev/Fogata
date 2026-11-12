import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';

import 'app_state.dart';
import 'call.dart'
    show
        CameraAccess,
        cameraAccess,
        cameraFailure,
        defaultAlias,
        noCameraNote,
        silenceLabel;
import 'camera_preview.dart';
import 'mic_test.dart';
import 'host.dart';
import 'icons.dart';
import 'l10n.dart';
import 'popover.dart';
import 'shown.dart' show windowShown;
import 'theme.dart';
import 'widgets.dart';

/// The microphone, speakers and camera the calls use. A change applies at once, also in
/// the middle of a call. The settings show the sound and the camera apart, the camera
/// with a preview over its choice.
class DeviceChoices extends StatefulWidget {
  const DeviceChoices({
    super.key,
    this.audio = true,
    this.camera = true,
    this.testVideo = false,
    this.testMic = false,
  });

  /// The microphone, with noise suppression, and the speaker.
  final bool audio;
  final bool camera;

  /// The microphone's test, under the speaker's choice.
  final bool testMic;

  /// The camera's preview, above its choice.
  final bool testVideo;

  @override
  State<DeviceChoices> createState() => _DeviceChoicesState();
}

class _DeviceChoicesState extends State<DeviceChoices> {
  List<MediaDeviceInfo> _devices = const [];
  bool _loaded = false;

  /// How the cameras are reached (see [askCameras]).
  CameraAccess _camera = CameraAccess.direct;

  /// Lists the devices again now and then while shown: not every system tells when a
  /// device is plugged in or out (a camera never on Linux), and the choices and the
  /// camera's preview should follow.
  Timer? _again;
  static const _every = Duration(seconds: 4);

  @override
  void initState() {
    super.initState();
    _load();
    _again = Timer.periodic(_every, (_) {
      if (windowShown.value) _load();
    });
    if (widget.camera) {
      askCameras().then((access) {
        if (!mounted) return;
        setState(() => _camera = access);
        if (access == CameraAccess.granted) _load();
      });
    }
  }

  @override
  void dispose() {
    _again?.cancel();
    super.dispose();
  }

  /// Lists the devices again: one may have been plugged in or out. The call's own silent
  /// input is left out, as are the system's default under its own names: neither is a
  /// device to choose ("System default" is).
  Future<List<MediaDeviceInfo>> _load() async {
    List<MediaDeviceInfo> devices;
    try {
      devices = await navigator.mediaDevices.enumerateDevices();
    } catch (_) {
      devices = const [];
    }
    final listed = [
      for (final d in devices)
        if (choosable(d)) d,
    ];
    if (mounted && (!_loaded || !_sameDevices(listed, _devices))) {
      setState(() {
        _devices = listed;
        _loaded = true;
      });
    }
    return listed;
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    Widget choice(
      String label,
      String tooltip,
      String kind,
      String? chosen,
      Future<void> Function(String?) choose,
    ) => _Choice(
      label: label,
      tooltip: tooltip,
      kind: kind,
      chosen: chosen,
      devices: _devices,
      loaded: _loaded,
      reload: _load,
      onChoose: choose,
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (widget.audio) ...[
          choice(
            l10n.devicesMicrophone,
            l10n.devicesChooseMicrophone,
            'audioinput',
            state.prefs.audioInput,
            state.setAudioInput,
          ),
          const SizedBox(height: 10),
          const NoiseSuppressionSwitch(),
          const SizedBox(height: 12),
          choice(
            l10n.devicesSpeaker,
            l10n.devicesChooseSpeaker,
            'audiooutput',
            state.prefs.audioOutput,
            state.setAudioOutput,
          ),
          if (widget.testMic) ...[
            const SizedBox(height: 16),
            TestMic(
              loaded: _loaded,
              hasMic: _devices.any((d) => d.kind == 'audioinput'),
            ),
          ],
        ],
        if (widget.audio && widget.camera) const SizedBox(height: 12),
        if (widget.testVideo) ...[
          TestVideo(
            loaded: _loaded,
            hasCamera: _devices.any((d) => d.kind == 'videoinput'),
            camera: _camera,
          ),
          const SizedBox(height: 16),
        ],
        if (widget.camera)
          choice(
            l10n.devicesCamera,
            l10n.devicesChooseCamera,
            'videoinput',
            state.prefs.videoInput,
            state.setVideoInput,
          ),
        if (widget.camera && widget.testVideo) ...[
          const SizedBox(height: 10),
          const MirrorCameraSwitch(),
        ],
      ],
    );
  }
}

/// Where the cameras come through the system's Camera portal (Linux, in a Flatpak), asks
/// for them when their choice shows, unless asked already: the system asks the person
/// once (a "no" is not asked again here; opening the camera asks again), and they are
/// listed after. Elsewhere it changes nothing.
Future<CameraAccess> askCameras() async {
  final access = await cameraAccess();
  if (access != CameraAccess.notAsked) return access;
  return cameraAccess(request: true);
}

/// Whether a listed device is one to choose: not the call's own silent input, not the
/// system's default under one of its names (see [defaultAlias]), which "System default"
/// stands for, and not a nameless entry (macOS lists one for the default input on a
/// computer without any).
@visibleForTesting
bool choosable(MediaDeviceInfo d) =>
    !d.label.contains(silenceLabel) &&
    !defaultAlias(d.deviceId) &&
    d.label.trim().isNotEmpty;

bool _sameDevices(List<MediaDeviceInfo> a, List<MediaDeviceInfo> b) =>
    a.length == b.length &&
    [for (var i = 0; i < a.length; i++) i].every(
      (i) =>
          a[i].kind == b[i].kind &&
          a[i].deviceId == b[i].deviceId &&
          a[i].label == b[i].label,
    );

/// A preview of the chosen camera, for this device only, with a button that starts and
/// stops it. The camera opens only when asked, and closes when the preview goes away
/// (another part of the settings, the settings closed), the window is hidden or a call
/// opens the camera.
class TestVideo extends StatefulWidget {
  const TestVideo({
    super.key,
    required this.loaded,
    required this.hasCamera,
    this.camera = CameraAccess.direct,
    this.preview,
  });

  /// The devices were listed.
  final bool loaded;
  final bool hasCamera;

  /// How the cameras are reached: without any listed, it tells why.
  final CameraAccess camera;

  /// The settings' own, unless a test gives one.
  final CameraPreview? preview;

  @override
  State<TestVideo> createState() => _TestVideoState();
}

class _TestVideoState extends State<TestVideo> {
  CameraPreview get _preview => widget.preview ?? cameraPreview;

  /// The camera the preview was opened with, to open the new one when it changes.
  String? _openedFor;

  /// Changing to another camera is under way: builds meanwhile ask for nothing more.
  bool _switching = false;

  @override
  void initState() {
    super.initState();
    _preview.addListener(_changed);
    windowShown.addListener(_window);
  }

  @override
  void dispose() {
    windowShown.removeListener(_window);
    _preview.removeListener(_changed);
    _preview.stop();
    super.dispose();
  }

  /// Nobody looks at a hidden or minimized window: the camera closes.
  void _window() {
    if (!windowShown.value) _preview.stop();
  }

  void _changed() {
    if (mounted) setState(() {});
  }

  void _start() {
    final state = AppScope.read(context);
    // The call has the camera.
    if (state.callState.call?.camera ?? false) return;
    _openedFor = state.prefs.videoInput;
    _preview.start(state.prefs.videoInput);
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final preview = _preview;
    final inCall = state.callState.call?.camera ?? false;
    final feed = preview.feed;
    if (preview.on &&
        !preview.opening &&
        !_switching &&
        _openedFor != state.prefs.videoInput) {
      // Another camera was chosen: it shows instead.
      _switching = true;
      WidgetsBinding.instance.addPostFrameCallback((_) async {
        try {
          if (!mounted) return;
          await preview.stop();
          if (mounted) _start();
        } finally {
          _switching = false;
        }
      });
    }
    final note = !widget.loaded
        ? ''
        : !widget.hasCamera
        ? noCameraNote(widget.camera)
        : inCall
        ? l10n.devicesCameraInCall
        : preview.failed
        ? cameraFailure(preview.failure)
        : l10n.devicesPreviewOnlyYou;
    final canStart =
        widget.loaded && widget.hasCamera && !inCall && !preview.on;
    // The preview in the middle of the camera's box, its button and note under it.
    return Column(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        LayoutBuilder(
          builder: (context, box) {
            final width = box.maxWidth.clamp(0.0, 320.0);
            return Container(
              key: const ValueKey('test-video'),
              width: width,
              height: width * 9 / 16,
              clipBehavior: Clip.antiAlias,
              // Inside the outline, as when it was drawn under what is inside.
              padding: const EdgeInsets.all(1),
              decoration: BoxDecoration(
                color: feed != null ? Colors.black : c.bg2,
                borderRadius: BorderRadius.circular(10),
              ),
              // Over what is inside, which would cover it at the rounded corners.
              foregroundDecoration: BoxDecoration(
                borderRadius: BorderRadius.circular(10),
                border: Border.all(color: c.border),
              ),
              child: feed != null
                  ? feed.view(mirror: state.prefs.mirrorCamera)
                  : Center(
                      child: FgIcon(
                        preview.opening ? Ic.camera : Ic.cameraOff,
                        size: 22,
                        color: c.fg3,
                      ),
                    ),
            );
          },
        ),
        const SizedBox(height: 12),
        FgButton(
          key: const ValueKey('test-video-button'),
          text: preview.on ? l10n.devicesStopVideo : l10n.devicesTestVideo,
          icon: preview.on ? Ic.cameraOff : Ic.camera,
          height: 32,
          fontSize: 13,
          busy: preview.opening,
          onTap: preview.on
              ? preview.stop
              : canStart
              ? _start
              : null,
        ),
        const SizedBox(height: 8),
        // Empty until the devices are listed, it keeps its line: nothing moves then.
        Text(
          note,
          textAlign: TextAlign.center,
          style: tx(c.fg3, size: 12, height: 1.4),
        ),
      ],
    );
  }
}

/// The microphone's test, for this device only: a button that opens the chosen
/// microphone and closes it, how loud it is as a bar, and a switch that plays it through the
/// chosen speaker (off until turned on). Nothing is sent. The microphone opens only when
/// asked, and closes when the test goes away (another part of the settings, the settings
/// closed), the window is hidden or a call starts; in a call the call has it.
class TestMic extends StatefulWidget {
  const TestMic({
    super.key,
    required this.loaded,
    required this.hasMic,
    this.test,
  });

  /// The devices were listed.
  final bool loaded;
  final bool hasMic;

  /// The settings' own, unless a test gives one.
  final MicTest? test;

  @override
  State<TestMic> createState() => _TestMicState();
}

class _TestMicState extends State<TestMic> {
  MicTest get _test => widget.test ?? micTest;

  /// The devices the test was opened with, to open it again when another is chosen.
  (String?, String?)? _openedFor;

  /// Opening it again is under way: builds meanwhile ask for nothing more.
  bool _switching = false;

  @override
  void initState() {
    super.initState();
    _test.addListener(_changed);
    windowShown.addListener(_window);
  }

  @override
  void dispose() {
    windowShown.removeListener(_window);
    _test.removeListener(_changed);
    _test.stop();
    // The next test starts without playing anything back.
    _test.hearing = false;
    super.dispose();
  }

  void _window() {
    if (!windowShown.value) _test.stop();
  }

  void _changed() {
    if (mounted) setState(() {});
  }

  void _start() {
    final state = AppScope.read(context);
    if (state.callState.call != null) return;
    final prefs = state.prefs;
    _openedFor = (prefs.audioInput, prefs.audioOutput);
    _test.start(
      input: prefs.audioInput,
      output: prefs.audioOutput,
      noiseSuppression: prefs.noiseSuppression,
    );
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final test = _test;
    final inCall = state.callState.call != null || test.blocked;
    final chosen = (state.prefs.audioInput, state.prefs.audioOutput);
    if (test.on && !test.opening && !_switching && _openedFor != chosen) {
      // Another microphone or speaker was chosen: the test goes on with it.
      _switching = true;
      WidgetsBinding.instance.addPostFrameCallback((_) async {
        try {
          if (!mounted) return;
          await test.stop();
          if (mounted) _start();
        } finally {
          _switching = false;
        }
      });
    }
    final note = !widget.loaded
        ? ''
        : !widget.hasMic
        ? l10n.devicesNoMic
        : inCall
        ? l10n.devicesMicInCall
        : test.failed
        ? l10n.callMicFailed(onMacOS ? 'macos' : 'other')
        : l10n.devicesMicTestOnlyYou;
    final canStart = widget.loaded && widget.hasMic && !inCall && !test.on;
    return Padding(
      padding: const EdgeInsets.only(left: _Choice.labelWidth),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              FgButton(
                key: const ValueKey('test-mic-button'),
                text: test.on ? l10n.devicesStopMicTest : l10n.devicesTestMic,
                icon: test.on ? Ic.micOff : Ic.mic,
                height: 32,
                fontSize: 13,
                busy: test.opening,
                onTap: test.on
                    ? test.stop
                    : canStart
                    ? _start
                    : null,
              ),
              const SizedBox(width: 12),
              Expanded(
                child: _LevelBar(level: test.level, on: test.on),
              ),
            ],
          ),
          const SizedBox(height: 10),
          _ChoiceSwitch(
            key: const ValueKey('hear-myself'),
            indent: false,
            on: test.hearing,
            text: l10n.devicesHearMyself,
            note: l10n.devicesHearMyselfNote,
            label: test.hearing
                ? l10n.devicesHearMyselfOn
                : l10n.devicesHearMyselfOff,
            onTap: () => test.hearing = !test.hearing,
          ),
          const SizedBox(height: 8),
          // Empty until the devices are listed, it keeps its line: nothing moves then.
          Text(note, style: tx(c.fg3, size: 12, height: 1.4)),
        ],
      ),
    );
  }
}

/// How loud the microphone is, as a bar that fills from the left.
class _LevelBar extends StatelessWidget {
  const _LevelBar({required this.level, required this.on});
  final ValueListenable<double> level;
  final bool on;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Semantics(
      label: l10n.devicesMicLevel,
      child: ValueListenableBuilder<double>(
        valueListenable: level,
        builder: (context, now, _) => Container(
          key: const ValueKey('mic-level'),
          height: 8,
          clipBehavior: Clip.antiAlias,
          // Inside the outline, as when it was drawn under what is inside.
          padding: const EdgeInsets.all(1),
          decoration: BoxDecoration(
            color: c.bg2,
            borderRadius: BorderRadius.circular(4),
          ),
          // Over what is inside, which would cover it at the rounded corners.
          foregroundDecoration: BoxDecoration(
            borderRadius: BorderRadius.circular(4),
            border: Border.all(color: c.border),
          ),
          alignment: Alignment.centerLeft,
          child: AnimatedFractionallySizedBox(
            key: const ValueKey('mic-level-fill'),
            duration: MicTest.every,
            widthFactor: on ? meterShare(now) : 0,
            heightFactor: 1,
            child: ColoredBox(color: c.green),
          ),
        ),
      ),
    );
  }
}

/// A switch lined up under a device's choice, with a line under it on what it does.
class _ChoiceSwitch extends StatelessWidget {
  const _ChoiceSwitch({
    super.key,
    this.indent = true,
    required this.on,
    required this.text,
    required this.note,
    required this.label,
    required this.onTap,
  });

  /// Lined up with the choices, past their labels.
  final bool indent;
  final bool on;
  final String text;
  final String note;

  /// What a screen reader says: the switch and whether it is on.
  final String label;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Padding(
      padding: EdgeInsets.only(left: indent ? _Choice.labelWidth : 0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Pressable(
            onTap: onTap,
            label: label,
            focusRadius: 999,
            builder: (context, hovered) => Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                FgSwitch(on: on, small: true),
                const SizedBox(width: 10),
                Text(text, style: tx(hovered ? c.fg : c.fg2, size: 13)),
              ],
            ),
          ),
          const SizedBox(height: 4),
          Text(note, style: tx(c.fg3, size: 12)),
        ],
      ),
    );
  }
}

/// Whether this device's own camera is drawn as in a mirror here, lined up under the
/// camera's choice. The others always get the camera's picture as it is.
class MirrorCameraSwitch extends StatelessWidget {
  const MirrorCameraSwitch({super.key});

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final on = state.prefs.mirrorCamera;
    return _ChoiceSwitch(
      key: const ValueKey('mirror-camera'),
      on: on,
      text: l10n.devicesMirror,
      note: l10n.devicesMirrorNote,
      label: on ? l10n.devicesMirrorOn : l10n.devicesMirrorOff,
      onTap: () => state.setMirrorCamera(!on),
    );
  }
}

/// The noise suppression switch, lined up under the microphone's choice.
class NoiseSuppressionSwitch extends StatelessWidget {
  const NoiseSuppressionSwitch({super.key});

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final on = state.prefs.noiseSuppression;
    return _ChoiceSwitch(
      on: on,
      text: l10n.devicesNoise,
      note: l10n.devicesNoiseNote,
      label: on ? l10n.devicesNoiseOn : l10n.devicesNoiseOff,
      onTap: () => state.setNoiseSuppression(!on),
    );
  }
}

class _Choice extends StatelessWidget {
  const _Choice({
    required this.label,
    required this.tooltip,
    required this.kind,
    required this.chosen,
    required this.devices,
    required this.loaded,
    required this.reload,
    required this.onChoose,
  });

  final String label;
  final String tooltip;
  final String kind;
  final String? chosen;
  final List<MediaDeviceInfo> devices;
  final bool loaded;
  final Future<List<MediaDeviceInfo>> Function() reload;
  final Future<void> Function(String?) onChoose;

  static String get _default => l10n.devicesSystemDefault;

  /// The width of the label in front of each choice.
  static const labelWidth = 96.0;

  static String _name(MediaDeviceInfo d) => d.label.trim();

  String get _current {
    final id = chosen;
    if (id == null) return _default;
    final device = devices.where((d) => d.kind == kind && d.deviceId == id);
    if (device.isNotEmpty) return _name(device.first);
    return loaded ? l10n.devicesNotConnected : '';
  }

  Future<void> _open(BuildContext anchor) async {
    final c = anchor.fg;
    final box = anchor.findRenderObject() as RenderBox?;
    final overlay = Overlay.of(anchor).context.findRenderObject() as RenderBox?;
    if (box == null || overlay == null) return;
    final listed = (await reload()).where((d) => d.kind == kind).toList();
    if (!anchor.mounted) return;
    final corner = box.localToGlobal(
      Offset(0, box.size.height + 4),
      ancestor: overlay,
    );
    final options = [
      if (kind != 'videoinput' || listed.isNotEmpty) ('', _default),
      for (final d in listed) (d.deviceId, _name(d)),
    ];
    Widget row(String text, bool selected, {bool muted = false}) => Row(
      children: [
        Expanded(
          child: Text(
            text,
            overflow: TextOverflow.ellipsis,
            style: tx(
              muted ? c.fg3 : (selected ? c.fg : c.fg2),
              size: 13,
              weight: selected ? w500 : FontWeight.w400,
            ),
          ),
        ),
        if (selected) FgIcon(Ic.check, size: 12, color: c.fg),
      ],
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
        if (options.isEmpty)
          PopupMenuItem<String>(
            enabled: false,
            height: 34,
            child: row(l10n.devicesNoCameraShort, false, muted: true),
          ),
        for (final (id, text) in options)
          PopupMenuItem<String>(
            value: id,
            height: 34,
            child: row(text, (chosen ?? '') == id),
          ),
      ],
    );
    if (picked != null) await onChoose(picked.isEmpty ? null : picked);
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Row(
      children: [
        SizedBox(
          width: labelWidth,
          child: Text(label, style: tx(c.fg2, size: 13, weight: w500)),
        ),
        Expanded(
          child: Builder(
            builder: (anchor) => Pressable(
              onTap: () => _open(anchor),
              tooltip: tooltip,
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
                        _current,
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
          ),
        ),
      ],
    );
  }
}

/// What a device panel chooses: the microphone, the speaker or the camera.
enum DeviceKind { input, output, camera }

/// The small arrow beside a call's microphone, deafen or camera button: it opens that
/// device's panel attached to [anchor] (the button and the arrow together) and points up
/// while the panel is open.
class DeviceArrow extends StatefulWidget {
  const DeviceArrow({
    super.key,
    required this.kind,
    required this.anchor,
    this.width = 24,
    this.height = 40,
    this.size = 13,
    this.gap = 8,
  });

  final DeviceKind kind;

  /// What the panel lines up with.
  final BuildContext anchor;
  final double width;
  final double height;

  /// The arrow's own size.
  final double size;

  /// How far the panel keeps from [anchor].
  final double gap;

  static String tooltip(DeviceKind kind) => switch (kind) {
    DeviceKind.input => l10n.devicesMicOptions,
    DeviceKind.output => l10n.devicesSpeakerOptions,
    DeviceKind.camera => l10n.devicesCameraOptions,
  };

  @override
  State<DeviceArrow> createState() => _DeviceArrowState();
}

class _DeviceArrowState extends State<DeviceArrow> {
  bool _open = false;

  Future<void> _toggle() async {
    // Open, a click on the arrow lands beside the panel and closes it.
    if (_open) return;
    setState(() => _open = true);
    await showPopover<void>(
      widget.anchor.mounted ? widget.anchor : context,
      label: DevicePanel.heading(widget.kind),
      gap: widget.gap,
      builder: (context) => DevicePanel(kind: widget.kind),
    );
    if (mounted) setState(() => _open = false);
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Springy(
      onTap: _toggle,
      tooltip: DeviceArrow.tooltip(widget.kind),
      expanded: _open,
      hoverScale: 1.1,
      builder: (context, hovered) => Container(
        width: widget.width,
        height: widget.height,
        alignment: Alignment.centerLeft,
        child: AnimatedRotation(
          key: const ValueKey('device-arrow'),
          turns: _open ? .5 : 0,
          duration: springTime,
          curve: spring,
          child: FgIcon(
            Ic.chevronDown,
            size: widget.size,
            color: hovered || _open ? c.fg : c.fg2,
          ),
        ),
      ),
    );
  }
}

/// One device's panel: the devices of its kind to choose from (the system's default
/// first), noise suppression under the microphones, and the way to the voice and video
/// settings. A choice applies at once, also in the middle of a call, and is kept.
class DevicePanel extends StatefulWidget {
  const DevicePanel({super.key, required this.kind});
  final DeviceKind kind;

  static const width = 300.0;

  /// What it chooses, over its list.
  static String heading(DeviceKind kind) => switch (kind) {
    DeviceKind.input => l10n.devicesInputDevice,
    DeviceKind.output => l10n.devicesOutputDevice,
    DeviceKind.camera => l10n.devicesCamera,
  };

  @override
  State<DevicePanel> createState() => _DevicePanelState();
}

class _DevicePanelState extends State<DevicePanel> {
  List<MediaDeviceInfo> _devices = const [];
  bool _loaded = false;

  /// How the cameras are reached (see [askCameras]).
  CameraAccess _camera = CameraAccess.direct;

  String get _kind => switch (widget.kind) {
    DeviceKind.input => 'audioinput',
    DeviceKind.output => 'audiooutput',
    DeviceKind.camera => 'videoinput',
  };

  /// Lists the devices again now and then while open, as [DeviceChoices] does: a device
  /// plugged in or out shows without opening the panel again.
  Timer? _again;

  @override
  void initState() {
    super.initState();
    _load();
    _again = Timer.periodic(_DeviceChoicesState._every, (_) {
      if (windowShown.value) _load();
    });
    if (widget.kind == DeviceKind.camera) {
      askCameras().then((access) {
        if (!mounted) return;
        setState(() => _camera = access);
        if (access == CameraAccess.granted) _load();
      });
    }
  }

  @override
  void dispose() {
    _again?.cancel();
    super.dispose();
  }

  /// Lists the devices of its kind that can be chosen (see [choosable]).
  Future<void> _load() async {
    List<MediaDeviceInfo> devices;
    try {
      devices = await navigator.mediaDevices.enumerateDevices();
    } catch (_) {
      devices = const [];
    }
    final listed = [
      for (final d in devices)
        if (d.kind == _kind && choosable(d)) d,
    ];
    if (!mounted || (_loaded && _sameDevices(listed, _devices))) return;
    setState(() {
      _loaded = true;
      _devices = listed;
    });
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final (chosen, choose) = switch (widget.kind) {
      DeviceKind.input => (state.prefs.audioInput, state.setAudioInput),
      DeviceKind.output => (state.prefs.audioOutput, state.setAudioOutput),
      DeviceKind.camera => (state.prefs.videoInput, state.setVideoInput),
    };
    // A chosen device that is not plugged in: the default is used meanwhile.
    final missing =
        _loaded && chosen != null && !_devices.any((d) => d.deviceId == chosen);
    Widget row(String text, String? id) {
      final selected = chosen == id;
      return Pressable(
        key: ValueKey('device:${id ?? ''}'),
        onTap: () => choose(id),
        label: text,
        selected: selected,
        builder: (context, hovered) => Container(
          height: 36,
          padding: const EdgeInsets.symmetric(horizontal: 10),
          decoration: BoxDecoration(
            color: hovered ? c.hover : null,
            borderRadius: BorderRadius.circular(6),
          ),
          child: Row(
            children: [
              Expanded(
                child: Text(
                  text,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(
                    selected ? c.fg : c.fg2,
                    weight: selected ? w500 : FontWeight.w400,
                  ),
                ),
              ),
              if (selected) FgIcon(Ic.check, size: 14, color: c.fg),
            ],
          ),
        ),
      );
    }

    Widget line() => Container(
      height: 1,
      margin: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      color: c.border,
    );
    final noCamera =
        _loaded && widget.kind == DeviceKind.camera && _devices.isEmpty;
    return Container(
      key: ValueKey('device-panel:${widget.kind.name}'),
      width: DevicePanel.width,
      padding: const EdgeInsets.all(6),
      decoration: BoxDecoration(
        color: c.bg2,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: c.borderStrong),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(10, 8, 10, 4),
            child: Text(
              DevicePanel.heading(widget.kind).toUpperCase(),
              style: tx(c.fg3, size: 11, weight: w500, spacing: .3),
            ),
          ),
          // Many devices scroll here; the rest stays.
          Flexible(
            child: SingleChildScrollView(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  if (noCamera)
                    Padding(
                      padding: const EdgeInsets.fromLTRB(10, 6, 10, 8),
                      child: Text(
                        noCameraNote(_camera),
                        style: tx(c.fg3, size: 13),
                      ),
                    )
                  else if (_loaded) ...[
                    row(l10n.devicesSystemDefault, null),
                    for (final d in _devices) row(d.label.trim(), d.deviceId),
                    if (missing)
                      Padding(
                        padding: const EdgeInsets.fromLTRB(10, 4, 10, 6),
                        child: Text(
                          l10n.devicesChosenMissing,
                          style: tx(c.fg3, size: 12),
                        ),
                      ),
                  ],
                ],
              ),
            ),
          ),
          if (widget.kind == DeviceKind.input) ...[
            line(),
            Pressable(
              onTap: () =>
                  state.setNoiseSuppression(!state.prefs.noiseSuppression),
              label: state.prefs.noiseSuppression
                  ? l10n.devicesNoiseOn
                  : l10n.devicesNoiseOff,
              builder: (context, hovered) => Container(
                padding: const EdgeInsets.fromLTRB(10, 8, 10, 8),
                decoration: BoxDecoration(
                  color: hovered ? c.hover : null,
                  borderRadius: BorderRadius.circular(6),
                ),
                child: Row(
                  children: [
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(l10n.devicesNoise, style: tx(c.fg)),
                          const SizedBox(height: 2),
                          Text(
                            l10n.devicesNoiseShort,
                            style: tx(c.fg3, size: 12),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(width: 10),
                    FgSwitch(on: state.prefs.noiseSuppression, small: true),
                  ],
                ),
              ),
            ),
          ],
          line(),
          Pressable(
            onTap: () {
              Navigator.of(context).pop();
              state.openSettings(SettingsSection.voice);
            },
            label: l10n.devicesVoiceSettings,
            builder: (context, hovered) => Container(
              // Taller only when larger text needs it.
              constraints: const BoxConstraints(minHeight: 36),
              padding: const EdgeInsets.symmetric(horizontal: 10),
              decoration: BoxDecoration(
                color: hovered ? c.hover : null,
                borderRadius: BorderRadius.circular(6),
              ),
              child: Row(
                children: [
                  FgIcon(Ic.settings, size: 16, color: c.fg2),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text(l10n.devicesVoiceSettings, style: tx(c.fg2)),
                  ),
                  FgIcon(Ic.chevronRight, size: 13, color: c.fg3),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}
