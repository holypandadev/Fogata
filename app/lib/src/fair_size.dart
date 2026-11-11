import 'rust/api/types.dart';

/// The narrowest and the widest shape a picture is drawn in, width over height, whatever
/// it says it has: a video's first picture another device describes (its size goes on
/// as it says, and nothing checks it), or a camera or screen as it arrives. A video that
/// said it was 1 pixel wide and 4294967295 tall made its chat card about a trillion
/// pixels tall, for everyone in the room; one that said the opposite, a card no one sees.
const minAspect = .5;
const maxAspect = 3.0;

/// The shape for a picture of [width] by [height]: within [minAspect] and [maxAspect],
/// and 16:9 when they tell nothing (zero, negative, or no number at all).
double fairAspect(num width, num height) {
  if (!(width > 0 && height > 0 && width.isFinite && height.isFinite)) {
    return 16 / 9;
  }
  return (width / height).clamp(minAspect, maxAspect).toDouble();
}

/// The longest a video or a voice message is shown to last: 100 hours. Longer, or below
/// zero, is not a length another device can have recorded, and its words would not fit.
const maxShownMs = 100 * 60 * 60 * 1000;

/// A length in milliseconds as it can be shown: within 0 and [maxShownMs].
int fairLength(int ms) => ms.clamp(0, maxShownMs);

/// A video as another device describes it, as it is drawn here.
extension FairVideo on UiVideo {
  /// Its shape ([fairAspect]).
  double get aspect => fairAspect(width, height);

  /// How long it lasts ([fairLength]).
  int get lengthMs => fairLength(durationMs);
}
