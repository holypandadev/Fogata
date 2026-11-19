part of '../call.dart';

/// A question to a participant whose signal named another call of theirs than the one
/// known: which call of theirs is the one now ([Call._askWhich]).
class _Asked {
  _Asked(this.token, this.call, this.message);

  /// Made for this question, so only an answer made after it was asked can carry it.
  final String token;

  /// The call the latest such signal named, and that signal, held until the answer.
  String call;
  Map<String, dynamic> message;

  /// Times asked, and the wait before the next time; none once the tries are over.
  int tries = 0;
  Timer? wait;
}

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
  /// who watch it. Set through [Call._setWatches].
  String? get watches => _watchesNow;
  String? _watchesNow;

  /// Sets [watches] without the call knowing, for tests of a participant alone.
  @visibleForTesting
  set watchesForTests(String? id) => _watchesNow = id;

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

  @visibleForTesting
  ScreenFit get screenFit => _fit;

  /// The pictures of this device's shared screen encoded for them on connection `on`
  /// by the last reading before they watched it, and whether they watch it; and the
  /// times it was sent again since ([Call._followScreenOut]).
  ({RTCPeerConnection on, num frames, bool watched})? _screenSent;
  int _screenNudges = 0;

  /// How this device's camera goes out to them, and the connection it was judged on.
  final _cameraFit = CameraFit();
  RTCPeerConnection? _cameraFitOn;

  /// Network candidates kept until the connection has its descriptions.
  static const earlyCandidates = 16;
  final List<RTCIceCandidate> _early = [];
  bool _described = false;
  int _attempt = 0;

  /// When [connection] was made, and when it first had both descriptions.
  DateTime? _madeAt;
  DateTime? _describedAt;

  /// The first offer made for [connection], sent again as it is while it waits for its
  /// answer ([Call._watch]).
  ({RTCPeerConnection on, Map<String, dynamic> message})? _offerOut;

  /// The call of theirs this connection is with, as their signals name it, and the ones
  /// of theirs that ended since this participant came in (see [Call._theirCall]).
  String? _theirCall;
  final List<String> _endedCalls = [];
  static const endedCallsKept = 8;

  void _ended(String call) {
    _endedCalls.add(call);
    if (_endedCalls.length > endedCallsKept) _endedCalls.removeAt(0);
  }

  /// Their signals say they answer a `which` ([Call._askWhich]), and a copy of an offer
  /// with the answer it had ([Call._watch]); those of copies before it say nothing, their
  /// signals of another call are taken as they come, and they get no copies. Null while
  /// none of their signals that name a call came.
  bool? _answersWhich;

  /// The question asked of them, while its answer is awaited ([Call._askWhich]); kept once
  /// no longer asked, as its answer is still taken, until the call known changes.
  _Asked? _asked;

  /// Their call when they last left the room during this call ([Call._left]): a signal of
  /// another call of theirs says it ended, and a late one of it is then dropped.
  String? _callWhenLeft;

  /// The newest state taken from them: the call of theirs it named (null from a copy that
  /// names none) and its number. Null before one, after their signals named a new call of
  /// theirs ([Call._theirCall]), or from a copy that numbers none (see 'state' in
  /// [Call._handle]).
  ({String? call, int number})? _stateIn;

  /// The first offer [connection] was made for, as their call and its number name it, and
  /// the answer sent to it: a copy of that offer gets the same answer again (see
  /// [Call._answeredAlready]). Kept while [connection] is that one.
  ({
    String call,
    int number,
    RTCPeerConnection on,
    Map<String, dynamic> answer,
  })?
  _answered;

  /// Their call, as their signals name it, and those of theirs that ended, for tests.
  @visibleForTesting
  String? get theirCall => _theirCall;
  @visibleForTesting
  List<String> get endedCalls => List.unmodifiable(_endedCalls);

  /// A `which` is being asked of them, for tests.
  @visibleForTesting
  bool get asking => _asked?.wait != null;

  /// [connection]'s offer or answer went while the link to them was down, or it was left
  /// waiting to be agreed when the link went ([Call.memberGone]): its signals went nowhere,
  /// and it is tried anew once the link is back ([Call.memberBack]).
  bool _blind = false;

  /// Times the first offer and answer were tried again.
  int _waits = 0;

  /// The round of [Call._watch]'s tries going on for them; an older one stops.
  int _watching = 0;

  /// The rounds of tries started for them, for tests.
  @visibleForTesting
  int get watchRounds => _watching;

  /// The next of those tries, and the time [connection] has to come up
  /// ([Call.connectWithin]): cancelled once they are let go of, so that nothing of theirs
  /// waits on after they left or the call ended.
  Timer? _watchWait;
  Timer? _connectWait;

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

  /// The track their voice arrives on, with the connection that receives it, so who talks
  /// is read from that receiver's statistics alone ([Call._readLevelsNow]).
  ({RTCPeerConnection on, MediaStreamTrack track})? _voice;
  @visibleForTesting
  set voiceForTests(MediaStreamTrack track) =>
      _voice = (on: connection!, track: track);

  /// A reading found nothing of their voice while their microphone was on; the next five
  /// look for voice that waited in the network and came all at once.
  int _voiceGap = 0;

  /// Their voice is not taken for a moment, to drop what was held up (see
  /// [Call._flushLateVoice]); [_agreed] completes when a description is agreed.
  bool get _voicePaused => _voicePausedNow;

  /// Their voice is being dropped for a moment ([Call._pauseVoiceIn]), one at a time.
  bool _pausingVoice = false;
  bool _voicePausedNow = false;
  Completer<void>? _agreed;

  /// [Call._voiceTo] waits in their turn and has not started yet.
  bool _voiceDue = false;

  /// [connection] has been up: WebRTC prepared the microphone for it if its voice channel
  /// was agreed to send, and does not again. Set through [Call._comeUp].
  bool get _upOnce => _upOnceNow;
  bool _upOnceNow = false;

  /// The connection whose encoding of each channel of sound is being tried again
  /// ([Call._startLater]).
  final _starting = <int, RTCPeerConnection>{};

  /// Times their connection was made anew in this call for a voice encoding not taken
  /// ([Call._connectAnew]): for the participant, not one connection, so never reset.
  int _voiceAnew = 0;

  /// Takes [connection] for one that is up, and has been, for tests.
  @visibleForTesting
  void upForTests() {
    connected = true;
    _upOnceNow = true;
  }

  /// The change of the microphone [Call._voiceTo] last brought [connection]'s voice
  /// channel to; null for none yet on it. The microphone check judges a connection only
  /// once its voice channel follows the change there is ([Call._micAsked]).
  int? _voiceDone;

  /// Runs [step] in their turn, as a signal from them is handled, for tests.
  @visibleForTesting
  Future<void> inTurnForTests(Future<void> Function() step) => _inTurn(step);

  /// Takes [connection] for one agreed once, its candidates gathered, for tests.
  @visibleForTesting
  void describedForTests() {
    _described = true;
    if (!_gathered.isCompleted) _gathered.complete();
  }

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

  /// The offer [_soundIn] was made for and the answer sent to it: a copy of that offer
  /// gets the same answer again. Kept while [_soundIn] is that connection.
  ({String offer, RTCPeerConnection on, String answer})? _soundAnswered;

  Future<void> _closeSoundIn() async {
    final c = _soundIn;
    _soundIn = null;
    _soundAnswered = null;
    if (c != null) await closeConnection(c);
  }

  /// How fast the pictures of [connection] arrive, reading after reading ([VideoFlood]).
  VideoFlood _pictures = VideoFlood();

  /// It sent pictures larger than any participant sends (see [fairPicture]), or far more
  /// of them ([VideoFlood]): its
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

  /// The streams made to show the pictures of [connection] ([Call._onTrack]).
  final _shown = <MediaStream>[];

  Future<void> _close() async {
    final c = connection;
    connection = null;
    final shown = [..._shown];
    _shown.clear();
    _voice = null;
    _answered = null;
    _offerOut = null;
    _describedAt = null;
    _pictures = VideoFlood();
    _described = false;
    _candidates = 0;
    _offering = false;
    _again = false;
    _voiceDone = null;
    _upOnceNow = false;
    _starting.clear();
    _early.clear();
    connected = false;
    if (c != null) await closeConnection(c);
    // After the connection: its tracks end with it. A renderer still showing one is
    // given another stream when pictures come again, and shows nothing meanwhile.
    for (final stream in shown) {
      await _letGo('a stream of $id', stream.dispose);
    }
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
    _watchWait?.cancel();
    _connectWait?.cancel();
    _asked?.wait?.cancel();
    _asked = null;
    await _letGo('the connection with $id', _close);
    // A description still waiting for its candidates goes on now, on the closed
    // connection, as it would once its wait ran out.
    if (!_gathered.isCompleted) _gathered.complete();
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
