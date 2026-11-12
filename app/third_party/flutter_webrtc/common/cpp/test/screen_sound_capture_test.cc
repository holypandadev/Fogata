// A shared screen's sound on Linux (linux/pipewire_apps_capturer.cc), the plugin's own code
// against a PipeWire server played by the test (pipewire_fake.h): which outputs it links as
// the server tells of them, in the orders a server tells them, and, built with
// ThreadSanitizer by run_tests.sh, the server's events on PipeWire's thread while the
// platform thread stops the capture (any data race fails the run).
//
// Among them the case of a person who heard their own voice in a screen shared with sound:
// a participant who leaves and comes back makes the sharer's call play anew, in a new
// output of the sharer's own, told of before whose it is; it must never be linked.

#include <unistd.h>

#include <atomic>
#include <chrono>
#include <climits>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <set>
#include <string>
#include <thread>
#include <utility>

#include "pipewire_apps_capturer.h"
#include "pipewire_fake.h"

using flutter_webrtc_plugin::PipeWireAppsCapturer;
using pwfake::Props;
using Links = std::set<std::pair<uint32_t, uint32_t>>;

namespace {

int failures = 0;

void Check(bool ok, const std::string& what) {
  if (!ok) {
    std::printf("FAIL: %s\n", what.c_str());
    failures++;
  }
}

std::string Text(const Links& links) {
  std::string text;
  for (const auto& [out, in] : links) {
    text += " " + std::to_string(out) + "->" + std::to_string(in);
  }
  return text.empty() ? " none" : text;
}

void Expect(pw_core* core, const Links& wanted, const std::string& what) {
  const Links got = pwfake::Links(core);
  if (got != wanted) {
    std::printf("FAIL: %s: links%s, wanted%s\n", what.c_str(), Text(got).c_str(),
                Text(wanted).c_str());
    failures++;
  }
}

// This test's own executable, as the capture takes this application's.
std::string OwnBinary() {
  char path[PATH_MAX];
  const ssize_t n = readlink("/proc/self/exe", path, sizeof(path) - 1);
  if (n <= 0) return "";
  path[n] = 0;
  const char* slash = std::strrchr(path, '/');
  return slash != nullptr ? slash + 1 : path;
}

const std::string kPid = std::to_string(getpid());

Props Output(uint32_t client) {
  return {{PW_KEY_MEDIA_CLASS, "Stream/Output/Audio"},
          {PW_KEY_CLIENT_ID, std::to_string(client)}};
}

Props Port(uint32_t node, bool output, const char* channel) {
  return {{PW_KEY_NODE_ID, std::to_string(node)},
          {PW_KEY_PORT_DIRECTION, output ? "out" : "in"},
          {PW_KEY_AUDIO_CHANNEL, channel}};
}

// The capture's own input (its stream's node and two ports, FL then FR) and the
// pipewire-pulse client every PulseAudio program's stream has, at the server.
uint32_t Inputs(pw_core* core) {
  const uint32_t node = pwfake::StreamNode(core);
  pwfake::Post(core, [core, node] {
    pwfake::Global(core, 30, PW_TYPE_INTERFACE_Client, {{PW_KEY_SEC_PID, "999"}});
    pwfake::Global(core, node, PW_TYPE_INTERFACE_Node,
                   {{PW_KEY_MEDIA_CLASS, "Stream/Input/Audio"}});
    pwfake::Global(core, node + 1, PW_TYPE_INTERFACE_Port, Port(node, false, "FL"));
    pwfake::Global(core, node + 2, PW_TYPE_INTERFACE_Port, Port(node, false, "FR"));
  });
  return node;
}

// An output `node` with ports node + 1 (FL) and node + 2 (FR), of `client`.
void Stereo(pw_core* core, uint32_t node, uint32_t client) {
  pwfake::Global(core, node, PW_TYPE_INTERFACE_Node, Output(client));
  pwfake::Global(core, node + 1, PW_TYPE_INTERFACE_Port, Port(node, true, "FL"));
  pwfake::Global(core, node + 2, PW_TYPE_INTERFACE_Port, Port(node, true, "FR"));
}

void Outputs() {
  PipeWireAppsCapturer capture;
  Check(capture.Start(nullptr), "the capture starts");
  pw_core* core = pwfake::NewestCore();
  const uint32_t in = Inputs(core);
  const Links player = {{41, in + 1}, {42, in + 2}};

  // A player, told of before it says whose it is: linked only then, channel to channel.
  pwfake::Post(core, [core] { Stereo(core, 40, 30); });
  pwfake::Drain(core);
  Expect(core, {}, "an output that has not said whose it is");
  pwfake::Post(core, [core] {
    pwfake::NodeInfo(core, 40,
                     {{PW_KEY_APP_PROCESS_ID, "5151"},
                      {PW_KEY_APP_PROCESS_BINARY, "player"},
                      {PW_KEY_CLIENT_ID, "30"}});
  });
  pwfake::Drain(core);
  Expect(core, player, "a player");

  // The call's own sound made again (someone left and came back), its ports told
  // before its properties: never linked, not even for a moment.
  const int made = pwfake::LinksMade(core);
  pwfake::Post(core, [core] { Stereo(core, 50, 30); });
  pwfake::Drain(core);
  pwfake::Post(core, [core] {
    pwfake::NodeInfo(core, 50,
                     {{PW_KEY_APP_PROCESS_ID, kPid},
                      {PW_KEY_APP_PROCESS_BINARY, OwnBinary()},
                      {PW_KEY_CLIENT_ID, "30"}});
  });
  pwfake::Drain(core);
  Expect(core, player, "the call's own sound made again");
  Check(pwfake::LinksMade(core) == made, "no link ever asked for the call's own sound");

  // Another copy of the application, by its executable alone.
  pwfake::Post(core, [core] {
    Stereo(core, 55, 30);
    pwfake::NodeInfo(core, 55, {{PW_KEY_APP_PROCESS_BINARY, OwnBinary()}});
  });
  pwfake::Drain(core);
  Expect(core, player, "another copy of the application");

  // A stream of this process that says nothing of itself (PipeWire's own client of
  // the application), whose client is told of after it: never linked.
  pwfake::Post(core, [core] {
    Stereo(core, 60, 31);
    pwfake::NodeInfo(core, 60, {{PW_KEY_CLIENT_ID, "31"}});
    pwfake::Global(core, 31, PW_TYPE_INTERFACE_Client, {{PW_KEY_SEC_PID, kPid}});
  });
  pwfake::Drain(core);
  Expect(core, player, "this process's client");

  // A loopback that may carry the call played into a sink it reads.
  pwfake::Post(core, [core] {
    Stereo(core, 70, 30);
    pwfake::NodeInfo(core, 70,
                     {{PW_KEY_APP_PROCESS_ID, "998"},
                      {PW_KEY_APP_PROCESS_BINARY, "pipewire-pulse"},
                      {"node.link-group", "loopback-998-1"}});
  });
  pwfake::Drain(core);
  Expect(core, player, "a loopback");

  // Known first by its executable, then told to be this process's: its links go.
  pwfake::Post(core, [core] {
    Stereo(core, 80, 30);
    pwfake::NodeInfo(core, 80, {{PW_KEY_APP_PROCESS_BINARY, "helper"}});
  });
  pwfake::Drain(core);
  Expect(core, {{41, in + 1}, {42, in + 2}, {81, in + 1}, {82, in + 2}},
         "an output known by its executable");
  pwfake::Post(core, [core] {
    pwfake::NodeInfo(core, 80, {{PW_KEY_APP_PROCESS_ID, kPid}});
  });
  pwfake::Drain(core);
  Expect(core, player, "an output found to be this process's later");

  // One channel goes to both; a port gone takes its link.
  pwfake::Post(core, [core] {
    pwfake::Global(core, 90, PW_TYPE_INTERFACE_Node, Output(30));
    pwfake::Global(core, 91, PW_TYPE_INTERFACE_Port, Port(90, true, "MONO"));
    pwfake::NodeInfo(core, 90, {{PW_KEY_APP_PROCESS_ID, "6000"}});
  });
  pwfake::Drain(core);
  Expect(core, {{41, in + 1}, {42, in + 2}, {91, in + 1}, {91, in + 2}}, "a mono output");
  pwfake::Post(core, [core] {
    pwfake::GlobalRemove(core, 91);
    pwfake::GlobalRemove(core, 90);
    pwfake::GlobalRemove(core, 42);
  });
  pwfake::Drain(core);
  Expect(core, {{41, in + 1}}, "ports gone");

  capture.Stop();
  Expect(core, {}, "every link let go once stopped");
  Check(pwfake::Disconnected(core), "disconnected once stopped");
  capture.Stop();
}

// For tests, the sound of one program alone (FOGATA_SCREEN_SOUND_FROM, debug builds): one
// whose client was told of before its properties is still taken once it says it is that
// program; another is not.
void OneProgram() {
  setenv("FOGATA_SCREEN_SOUND_FROM", "pw-cat", 1);
  PipeWireAppsCapturer capture;
  unsetenv("FOGATA_SCREEN_SOUND_FROM");
  Check(capture.Start(nullptr), "the capture starts for one program");
  pw_core* core = pwfake::NewestCore();
  const uint32_t in = Inputs(core);
  pwfake::Post(core, [core] {
    Stereo(core, 40, 32);
    pwfake::Global(core, 32, PW_TYPE_INTERFACE_Client, {{PW_KEY_SEC_PID, "4444"}});
    Stereo(core, 50, 30);
  });
  pwfake::Drain(core);
  Expect(core, {}, "one program, before it says which");
  pwfake::Post(core, [core] {
    pwfake::NodeInfo(core, 40,
                     {{PW_KEY_APP_PROCESS_ID, "4444"}, {PW_KEY_APP_PROCESS_BINARY, "pw-cat"}});
    pwfake::NodeInfo(core, 50,
                     {{PW_KEY_APP_PROCESS_ID, "4445"}, {PW_KEY_APP_PROCESS_BINARY, "pacat"}});
  });
  pwfake::Drain(core);
  Expect(core, {{41, in + 1}, {42, in + 2}}, "one program");
  capture.Stop();
}

// Without a server: no capture, and nothing left running.
void NoServer() {
  pwfake::SetReachable(false);
  PipeWireAppsCapturer capture;
  Check(!capture.Start(nullptr), "no capture without a server");
  capture.Stop();
  pwfake::SetReachable(true);
}

// The server's events keep coming on PipeWire's thread (outputs, their properties and
// ports, outputs going) while the platform thread stops the capture at any moment of
// them, as a share that ends while applications come and go.
void Races() {
  for (int round = 0; round < 200; round++) {
    PipeWireAppsCapturer capture;
    if (!capture.Start(nullptr)) {
      Check(false, "the capture starts, round " + std::to_string(round));
      return;
    }
    pw_core* core = pwfake::NewestCore();
    Inputs(core);
    std::atomic<bool> done{false};
    std::thread server([core, &done, round] {
      for (uint32_t i = 0; i < 40 && !done; i++) {
        const uint32_t node = 100 + 10 * (i % 8);
        pwfake::Post(core, [core, node, i, round] {
          if (i % 9 == 8) {
            pwfake::GlobalRemove(core, node + 1);
            pwfake::GlobalRemove(core, node);
            return;
          }
          Stereo(core, node, 30);
          pwfake::NodeInfo(
              core, node,
              {{PW_KEY_APP_PROCESS_ID, (i + round) % 3 == 0 ? kPid : "7000"},
               {PW_KEY_APP_PROCESS_BINARY, i % 4 == 0 ? "pipewire" : "player"}});
        });
        std::this_thread::yield();
      }
    });
    std::this_thread::sleep_for(std::chrono::microseconds(50 * (round % 11)));
    capture.Stop();
    done = true;
    server.join();
    Expect(core, {}, "every link let go once stopped, round " + std::to_string(round));
  }
}

}  // namespace

int main() {
  Outputs();
  OneProgram();
  NoServer();
  Races();
  if (failures == 0) std::printf("screen_sound_capture_test: ok\n");
  return failures == 0 ? 0 : 1;
}
