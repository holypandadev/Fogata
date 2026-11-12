// The system's default microphone and speaker listed first (fogata_default_device.h): the
// place of the default in libwebrtc's list, and the order the list is given to the
// application in. Plain C++; finding Windows' default endpoint itself needs Windows.

#include <cstdio>
#include <string>
#include <vector>

#include "fogata_default_device.h"

using namespace flutter_webrtc_plugin;

namespace {

int failures = 0;

void check(bool ok, const char* what) {
  if (!ok) {
    std::printf("FAIL: %s\n", what);
    failures++;
  }
}

}  // namespace

int main() {
  // Windows lists its active endpoints with no default first: a virtual cable's
  // 16-channel input may come before the speakers the system uses for calls.
  const std::vector<std::string> outputs = {
      "{0.0.0.00000000}.{16ch}", "{0.0.0.00000000}.{cable}",
      "{0.0.0.00000000}.{speakers}"};
  check(DefaultDevicePlace(outputs, "{0.0.0.00000000}.{speakers}") == 2,
        "the default found at its place");
  check(DefaultDevicePlace(outputs, "{0.0.0.00000000}.{16ch}") == 0,
        "a default listed first stays first");
  // Not known (any system but Windows, or Windows could not tell) or not listed (it went
  // away meanwhile): the first, as before.
  check(DefaultDevicePlace(outputs, "") == 0, "no default known: the first");
  check(DefaultDevicePlace(outputs, "{gone}") == 0, "a default not listed: the first");
  check(DefaultDevicePlace({}, "{0.0.0.00000000}.{speakers}") == 0,
        "an empty list: the first");
  // A device whose guid could not be read is passed over.
  check(DefaultDevicePlace({"", "{x}"}, "{x}") == 1,
        "an unread guid before the default");

  check(ListingOrder(3, 2) == std::vector<uint16_t>({2, 0, 1}),
        "the default first, then the others in their order");
  check(ListingOrder(3, 1) == std::vector<uint16_t>({1, 0, 2}),
        "the default first, from the middle");
  // Elsewhere the order is libwebrtc's, unchanged.
  check(ListingOrder(3, 0) == std::vector<uint16_t>({0, 1, 2}),
        "the first already the default: the list as it is");
  check(ListingOrder(1, 0) == std::vector<uint16_t>({0}), "one device");
  check(ListingOrder(0, 0).empty(), "no device");
  check(ListingOrder(-1, 0).empty(), "a list that cannot be read");
  check(ListingOrder(2, 5) == std::vector<uint16_t>({0, 1}),
        "a place past the end: the list as it is");

  if (failures == 0) std::printf("default_device_test: passed\n");
  return failures == 0 ? 0 : 1;
}
