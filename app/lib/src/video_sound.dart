import 'package:flutter/foundation.dart';
import 'package:fogata_video/fogata_video.dart';

import 'app_state.dart';

/// The sound of a chat video playing over the window. Deafened, the person hears no video
/// either, as they hear no call: it goes quiet when they deafen, also in the middle of the
/// video, and comes back when they hear again, unless they muted it themselves meanwhile.
class VideoSound {
  VideoSound(this._state, this._changed)
    : _deafened = _state.callState.deafened {
    _muted = _deafened;
    _byDeafen = _deafened;
    // Deafening is told to the calls' part of the state.
    _state.callState.addListener(_follow);
  }

  final AppState _state;
  final VoidCallback _changed;
  Future<void> Function(double volume)? _setVolume;
  bool _deafened;
  bool _muted = false;

  /// Muted because the person deafened, not by hand.
  bool _byDeafen = false;

  bool get muted => _muted;

  /// The player whose sound this is; it starts quiet if it should.
  Future<void> attach(VideoPlayer player) => attachVolume(player.setVolume);

  /// [attach] for whatever sets the volume.
  @visibleForTesting
  Future<void> attachVolume(Future<void> Function(double volume) set) async {
    _setVolume = set;
    if (_muted) await set(0);
  }

  /// The video's own Mute and Sound on.
  Future<void> toggle() => _set(!_muted, byDeafen: false);

  void _follow() {
    final deafened = _state.callState.deafened;
    if (deafened == _deafened) return;
    _deafened = deafened;
    if (deafened && !_muted) {
      _set(true, byDeafen: true);
    } else if (!deafened && _byDeafen) {
      _set(false, byDeafen: false);
    }
  }

  Future<void> _set(bool muted, {required bool byDeafen}) async {
    _muted = muted;
    _byDeafen = byDeafen;
    _changed();
    await _setVolume?.call(muted ? 0 : 1);
  }

  void dispose() => _state.callState.removeListener(_follow);
}
