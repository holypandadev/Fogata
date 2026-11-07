// Fogata: the size a screen or window is captured at, in pixels, from its size in pixels:
// the same shape, made smaller only as far as the others take it (8K: 7680 by 4320 pixels,
// no side over 8192, as lib/src/call.dart's maxScreenSide and maxScreenPixels, and the
// renderers), in even numbers as the encoders take them, at least 2 by 2.
#ifndef FOGATA_CAPTURE_SIZE_H
#define FOGATA_CAPTURE_SIZE_H

#include <math.h>
#include <stddef.h>

static const double kFogataCaptureMaxSide = 8192;
static const double kFogataCaptureMaxPixels = 7680.0 * 4320.0;

static inline void FogataCaptureSize(double width, double height, size_t* outWidth,
                                     size_t* outHeight) {
  if (!(width > 0) || !(height > 0) || isinf(width) || isinf(height)) {
    *outWidth = 2;
    *outHeight = 2;
    return;
  }
  double scale = 1;
  if (width > kFogataCaptureMaxSide) scale = fmin(scale, kFogataCaptureMaxSide / width);
  if (height > kFogataCaptureMaxSide) scale = fmin(scale, kFogataCaptureMaxSide / height);
  if (width * height > kFogataCaptureMaxPixels) {
    scale = fmin(scale, sqrt(kFogataCaptureMaxPixels / (width * height)));
  }
  // Rounded down to even numbers, so the limits hold after rounding too.
  *outWidth = (size_t)fmax(2, floor(width * scale / 2) * 2);
  *outHeight = (size_t)fmax(2, floor(height * scale / 2) * 2);
}

#endif
