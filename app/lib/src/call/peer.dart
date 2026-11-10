part of '../call.dart';

/// Another participant of the call.
class CallPeer {
  CallPeer(this.id);

  /// Device id in hex.
  final String id;
  RTCPeerConnection? connection;
  // Not final: one whose channel failed is replaced ([Call._renew]).
  RTCVideoRenderer camera = RTCVideoRenderer();
  RTCVideoRenderer screen = RTCVideoRenderer();
  bool rendering = false;

  /// Times a failed renderer was replaced; see [Call.maxRenewals].
  int _renewed = 0;

  /// What the participant says it sends.
  bool mic = false;
  bool cam = false;
  bool sharing = false;

  /// Their shared screen comes with sound.
  bool sound = false;

  /// The participant whose shared screen this one watches; a screen goes only to those
  /// who watch it.
  String? watches;

  /// Their voice is heard now.
  bool speaking = false;

  /// They are in the room without a call: their computer has no sound server (see
  /// [Call.noVoice]). Nobody connects to them until they say otherwise.
  bool voiceless = false;
  RTCPeerConnectionState state =
      RTCPeerConnectionState.RTCPeerConnectionStateNew;
  bool connected = false;

  /// Packets lost and received at the previous quality reading, by channel.
  final Map<int, (int, int)> _packets = {};

  /// How this device's shared screen goes out to them.
  final _fit = ScreenFit();

  /// How this device's camera goes out to them, and the connection it was judged on.
  final _cameraFit = CameraFit();
  RTCPeerConnection? _cameraFitOn;

  /// Network candidates kept until the connection has its descriptions.
  static const earlyCandidates = 16;
  final List<RTCIceCandidate> _early = [];
  bool _described = false;
  int _attempt = 0;

  /// When [connection] was made.
  DateTime? _madeAt;

  /// Times the first offer and answer were tried again.
  int _waits = 0;

  /// The round of [Call._watch]'s tries going on for them; an older one stops.
  int _watching = 0;

  /// The rounds of tries started for them, for tests.
  @visibleForTesting
  int get watchRounds => _watching;

  /// The connection has its network candidates.
  Completer<void> _gathered = Completer<void>();

  /// Signals from them and this side's first offers, one after another: two at once would
  /// each make a connection, and the first would stay open with nothing to close it.
  Future<void> _turn = Future<void>.value();

  Future<void> _inTurn(Future<void> Function() step) {
    _queued++;
    final next = _turn.then((_) => step()).whenComplete(() => _queued--);
    _turn = next.catchError((Object _) {});
    return next;
  }

  /// Steps waiting in [_inTurn]; past [maxQueued], signals from them other than answers
  /// are dropped, so a participant sending signal after signal cannot pile up work.
  int _queued = 0;
  static const maxQueued = 32;

  /// Offers (of the call, or of their screen's sound) received from them: only the newest
  /// waiting is taken, as it replaces those before it.
  int _offersIn = 0;
  int _soundOffersIn = 0;

  /// When an offer of each kind was last taken. Each makes a new connection, or agrees
  /// again on the one there is (a new connection on this device's media bridge, if it
  /// asks to start over): one taken every moment, as fast as signals go, kept this device
  /// doing nothing else and used up the requests the bridge takes from connections it
  /// does not know yet, so that a connection with someone else could not start. A copy
  /// offers again only after an answer, or seconds after a failure.
  DateTime _offerTaken = DateTime(0);
  DateTime _soundOfferTaken = DateTime(0);
  static const offerEvery = Duration(seconds: 1);

  /// Network candidates added to [connection] after it had its descriptions; a connection
  /// gathers a few dozen at most.
  int _candidates = 0;
  static const maxCandidates = 64;

  /// This side's offer waits for its answer; another round is due after it.
  bool _offering = false;
  bool _again = false;

  /// The number of this side's latest offer. An answer names the offer it answers, and
  /// one to an earlier offer is dropped: when two offers crossed (each side tried again
  /// after a failure), the answer to the first would otherwise be set on the connection
  /// made for the second, which then waited for minutes with the other side's closed one.
  /// Starts anywhere, so a participant met again never matches an answer meant before.
  int _offerNumber = math.Random().nextInt(1 << 30);

  /// Their voice as last read (see [Call._flushLateVoice]).
  ({num packets, num delay, num target, num emitted})? _voiceIn;

  /// A reading found nothing of their voice while their microphone was on; the next five
  /// look for voice that waited in the network and came all at once.
  int _voiceGap = 0;

  /// Their voice is not taken for a moment, to drop what was held up (see
  /// [Call._flushLateVoice]); [_agreed] completes when a description is agreed.
  bool _voicePaused = false;
  Completer<void>? _agreed;

  /// The sound of this device's shared screen to them, and of theirs to this device, when
  /// it goes on a connection of its own.
  RTCPeerConnection? _soundOut;
  RTCPeerConnection? _soundIn;

  /// Changes to [_soundOut], one after the other.
  Future<void> _soundTurn = Future.value();

  /// The connection that brings the sound of their shared screen, if it comes apart.
  @visibleForTesting
  RTCPeerConnection? get soundConnection => _soundIn;
  @visibleForTesting
  set soundConnection(RTCPeerConnection? connection) => _soundIn = connection;

  Future<void> _closeSoundIn() async {
    final c = _soundIn;
    _soundIn = null;
    if (c != null) await closeConnection(c);
  }

  /// It sent pictures larger than any participant sends (see [fairPicture]): its
  /// connection was closed, and no other is made with it while it stays in the call.
  bool _keptOut = false;
  @visibleForTesting
  bool get keptOut => _keptOut;

  /// Let go of: it is no longer in the call, or the call ended.
  bool _gone = false;
  Future<void>? _disposing;

  Future<void>? _preparing;

  /// Makes its pictures ready, once; a track that arrives meanwhile waits for it.
  Future<void> _prepare() => _preparing ??= _prepareNow();

  Future<void> _prepareNow() async {
    await camera.initialize();
    await screen.initialize();
    rendering = true;
    // Let go of while its pictures were made ready: nothing freed them then.
    if (_gone) await _freeRenderers();
  }

  Future<void> _close() async {
    final c = connection;
    connection = null;
    _described = false;
    _candidates = 0;
    _offering = false;
    _again = false;
    _early.clear();
    connected = false;
    if (c != null) await closeConnection(c);
  }

  /// Closes its connections and frees its pictures, once however many ask: a renderer
  /// freed twice at once is used after it is gone.
  Future<void> dispose() => _disposing ??= _dispose();

  /// The steps of [dispose] that each have a limit of their own ([_letGo]): the
  /// connection, the sound in and out, and the pictures (whose own four steps go on
  /// within that one's limit).
  static const disposeSteps = 4;

  /// Each step goes on whatever the one before did: one that fails or hangs leaves the
  /// others to be let go of all the same ([_letGo]).
  Future<void> _dispose() async {
    _gone = true;
    await _letGo('the connection with $id', _close);
    await _letGo('the screen sound from $id', _closeSoundIn);
    final out = _soundOut;
    _soundOut = null;
    if (out != null) {
      await _letGo('the screen sound to $id', () => closeConnection(out));
    }
    await _letGo('the pictures of $id', _freeRenderers);
  }

  Future<void> _freeRenderers() async {
    if (!rendering) return;
    rendering = false;
    await _letGo('a camera of $id', () => camera.srcObject = null);
    await _letGo('a screen of $id', () => screen.srcObject = null);
    await _letGo('a camera of $id', camera.dispose);
    await _letGo('a screen of $id', screen.dispose);
  }
}
