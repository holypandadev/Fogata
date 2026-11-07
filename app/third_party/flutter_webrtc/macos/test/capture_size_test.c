// Tests of FogataCaptureSize: cc -I ../flutter_webrtc/Sources/flutter_webrtc capture_size_test.c
#include <stdio.h>
#include <stdlib.h>

#include "FogataCaptureSize.h"

static int failed = 0;

static void check(double w, double h, size_t ew, size_t eh) {
  size_t gw, gh;
  FogataCaptureSize(w, h, &gw, &gh);
  const int fits = gw <= 8192 && gh <= 8192 && (double)gw * gh <= 7680.0 * 4320.0 &&
                   gw % 2 == 0 && gh % 2 == 0 && gw >= 2 && gh >= 2;
  if (!fits || (ew != 0 && (gw != ew || gh != eh))) {
    printf("FAIL: %.0f x %.0f gave %zu x %zu, wanted %zu x %zu\n", w, h, gw, gh, ew, eh);
    failed = 1;
  }
}

int main(void) {
  check(1512, 982, 1512, 982);   // a laptop's screen: as it is
  check(3025, 1965, 3024, 1964); // odd: made even
  check(7680, 4320, 7680, 4320); // 8K: as it is
  check(18000, 900, 8192, 408);  // a window wider than any side taken
  check(8192, 8192, 0, 0);       // sides allowed, but more pixels than 8K
  check(10240, 2880, 0, 0);      // across two 5K screens
  check(1, 1, 2, 2);
  check(0, 0, 2, 2);
  check(NAN, 100, 2, 2);
  check(INFINITY, 100, 2, 2);
  if (!failed) printf("capture_size_test: all passed\n");
  return failed;
}
