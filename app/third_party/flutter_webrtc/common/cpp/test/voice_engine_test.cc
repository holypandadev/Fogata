// What FlutterWebRTCBase::KeepVoiceEngine counts on: libwebrtc stops its voice engine when
// the last connection is freed, and the audio device with it, which then lists no input
// and no output at all (-1, not initialized) until another connection starts it again; an
// idle connection kept meanwhile keeps the engine, and the device's lists, as they were.
// Real libwebrtc and the computer's sound server (the audio device reaches it when the
// factory is made, as in rejected_section_test.cc); no track is added, so nothing is
// played or recorded.

#include <cstdio>
#include <string>

#include "libwebrtc.h"
#include "rtc_audio_device.h"
#include "rtc_mediaconstraints.h"
#include "rtc_peerconnection.h"
#include "rtc_peerconnection_factory.h"

using namespace libwebrtc;

namespace {

int failures = 0;

void check(bool ok, const std::string& what) {
  if (!ok) {
    std::printf("FAIL: %s\n", what.c_str());
    failures++;
  }
}

scoped_refptr<RTCPeerConnection> Connection(
    scoped_refptr<RTCPeerConnectionFactory> factory) {
  RTCConfiguration idle;
  return factory->Create(idle, RTCMediaConstraints::Create());
}

// Closes and frees a connection, as the plugin does on peerConnectionDispose.
void Free(scoped_refptr<RTCPeerConnectionFactory> factory,
          scoped_refptr<RTCPeerConnection>& connection) {
  connection->Close();
  factory->Delete(connection);
  connection = nullptr;
}

std::string Counts(scoped_refptr<RTCAudioDevice> audio) {
  return std::to_string(audio->RecordingDevices()) + " inputs, " +
         std::to_string(audio->PlayoutDevices()) + " outputs";
}

}  // namespace

int main() {
  LibWebRTC::Initialize();
  auto factory = LibWebRTC::CreateRTCPeerConnectionFactory();
  if (factory.get() == nullptr || !factory->Initialize()) {
    std::printf("FAIL: no peer connection factory\n");
    return 1;
  }
  auto audio = factory->GetAudioDevice();
  // The system's default at least, listed first, whenever the device is initialized.
  const int inputs = audio->RecordingDevices();
  const int outputs = audio->PlayoutDevices();
  check(inputs > 0 && outputs > 0,
        "the devices listed before any connection: " + Counts(audio));

  // The last connection freed: the voice engine stops, and the device lists nothing.
  auto first = Connection(factory);
  check(audio->RecordingDevices() == inputs,
        "the devices listed with a connection: " + Counts(audio));
  Free(factory, first);
  check(audio->RecordingDevices() < 0 && audio->PlayoutDevices() < 0,
        "libwebrtc no longer stops the audio device with its last connection (" +
            Counts(audio) +
            " listed after it): KeepVoiceEngine may no longer be needed");

  // A connection starts the engine again; one kept while it is freed keeps it running.
  auto second = Connection(factory);
  check(audio->RecordingDevices() == inputs,
        "the devices listed once a connection starts the engine again: " +
            Counts(audio));
  auto keeper = Connection(factory);
  Free(factory, second);
  check(audio->RecordingDevices() == inputs && audio->PlayoutDevices() == outputs,
        "the devices listed after the last other connection went, with an idle one "
        "kept: " + Counts(audio));
  Free(factory, keeper);

  factory->Terminate();
  factory = nullptr;
  LibWebRTC::Terminate();
  if (failures == 0) std::printf("voice_engine_test: passed\n");
  return failures == 0 ? 0 : 1;
}
