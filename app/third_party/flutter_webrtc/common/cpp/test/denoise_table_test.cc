// The table of noise suppression functions is taken from the application only from
// inside a library loaded in this process, with its functions in that same library; any
// other address is refused without being read. Built and run by run_tests.sh.

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <memory>

#include "fogata_denoise_table.h"

using namespace flutter_webrtc_plugin;

namespace {

void* Create() { return nullptr; }
void Process(void*, bool, int32_t, float*, size_t) {}

const FogataDenoiseApi kTable = {kFogataDenoiseApiVersion, Create, Process};

int failures = 0;

void Expect(bool ok, const char* what) {
  if (ok) return;
  failures++;
  std::fprintf(stderr, "denoise table: %s\n", what);
}

int64_t Address(const void* p) {
  return static_cast<int64_t>(reinterpret_cast<intptr_t>(p));
}

}  // namespace

int main() {
  Expect(FogataDenoiseTable(Address(&kTable)) == &kTable,
         "the table in a loaded library is refused");
  // The same table made on the heap or on the stack: not inside any library.
  auto heap = std::make_unique<FogataDenoiseApi>(kTable);
  Expect(FogataDenoiseTable(Address(heap.get())) == nullptr,
         "a table on the heap is taken");
  FogataDenoiseApi stack = kTable;
  Expect(FogataDenoiseTable(Address(&stack)) == nullptr,
         "a table on the stack is taken");
  // A table in a library, one of whose functions is in another.
  static FogataDenoiseApi foreign = {kFogataDenoiseApiVersion, Create, Process};
  foreign.create = reinterpret_cast<void* (*)()>(&std::getenv);
  Expect(FogataDenoiseTable(Address(&foreign)) == nullptr,
         "a table with a function of another library is taken");
  static FogataDenoiseApi older = {kFogataDenoiseApiVersion + 1, Create, Process};
  Expect(FogataDenoiseTable(Address(&older)) == nullptr,
         "a table of another version is taken");
  // Nothing at all there.
  Expect(FogataDenoiseTable(0) == nullptr, "address 0 is taken");
  Expect(FogataDenoiseTable(-8) == nullptr, "a negative address is taken");
  Expect(FogataDenoiseTable(16) == nullptr, "address 16 is taken");
  if (failures > 0) return 1;
  std::printf("denoise table: ok\n");
  return 0;
}
