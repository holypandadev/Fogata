// A local offer with a section rejected as call.dart's onlyCallCodecs rejects one (the
// port made 0, `a=bundle-only` dropped, its mid taken out of the BUNDLE group) is taken by
// libwebrtc: SetLocalDescription succeeds and that section's transceiver stops, while the
// other sections go on. The same offer with its first section (the BUNDLE group's tag)
// rejected but still named in the group is refused, which is why onlyCallCodecs takes a
// rejected section's mid out. Real libwebrtc, with the
// peer connection set up as call.dart sets it (Unified Plan, max-bundle); no device is
// opened, as no track is added.

#include <chrono>
#include <cstdio>
#include <future>
#include <memory>
#include <sstream>
#include <string>
#include <vector>

#include "libwebrtc.h"
#include "rtc_mediaconstraints.h"
#include "rtc_peerconnection.h"
#include "rtc_peerconnection_factory.h"
#include "rtc_rtp_parameters.h"
#include "rtc_rtp_transceiver.h"

using namespace libwebrtc;

namespace {

int failures = 0;

void check(bool ok, const std::string& what) {
  if (!ok) {
    std::printf("FAIL: %s\n", what.c_str());
    failures++;
  }
}

// How long a callback of libwebrtc is waited for: long, since it only guards against a
// hang; the callbacks come as soon as libwebrtc is done.
constexpr auto kHang = std::chrono::seconds(30);

std::vector<std::string> Lines(const std::string& sdp) {
  std::vector<std::string> lines;
  std::string::size_type at = 0;
  while (at < sdp.size()) {
    auto end = sdp.find("\r\n", at);
    if (end == std::string::npos) end = sdp.size();
    lines.push_back(sdp.substr(at, end - at));
    at = end + 2;
  }
  return lines;
}

// `sdp` with the section of `mid` rejected as onlyCallCodecs rejects one: port 0 on its
// m= line, its `a=bundle-only` dropped, every other line kept; and, when `leave_bundle`,
// its mid taken out of the `a=group:BUNDLE` line.
std::string Reject(const std::string& sdp, const std::string& mid, bool leave_bundle) {
  std::vector<std::string> lines = Lines(sdp);
  std::vector<std::string> out;
  size_t i = 0;
  while (i < lines.size()) {
    if (lines[i].rfind("m=", 0) != 0) {
      out.push_back(lines[i++]);
      continue;
    }
    size_t end = i + 1;
    while (end < lines.size() && lines[end].rfind("m=", 0) != 0) end++;
    bool ours = false;
    for (size_t j = i; j < end; j++) ours = ours || lines[j] == "a=mid:" + mid;
    if (!ours) {
      for (size_t j = i; j < end; j++) out.push_back(lines[j]);
    } else {
      // "m=video 9 UDP/TLS/RTP/SAVPF 96 97" -> "m=video 0 UDP/TLS/RTP/SAVPF 96 97".
      std::istringstream words(lines[i]);
      std::string kind, port, rest;
      words >> kind >> port;
      std::getline(words, rest);
      out.push_back(kind + " 0" + rest);
      for (size_t j = i + 1; j < end; j++) {
        if (lines[j] != "a=bundle-only") out.push_back(lines[j]);
      }
    }
    i = end;
  }
  std::string result;
  for (const auto& line : out) {
    std::string kept = line;
    if (leave_bundle && line.rfind("a=group:BUNDLE", 0) == 0) {
      std::istringstream words(line);
      std::string word;
      kept.clear();
      while (words >> word) {
        if (word == mid) continue;
        kept += kept.empty() ? word : " " + word;
      }
    }
    result += kept + "\r\n";
  }
  return result;
}

scoped_refptr<RTCPeerConnection> Connection(scoped_refptr<RTCPeerConnectionFactory> factory) {
  RTCConfiguration config;
  config.bundle_policy = BundlePolicy::kBundlePolicyMaxBundle;
  config.sdp_semantics = SdpSemantics::kUnifiedPlan;
  auto connection = factory->Create(config, RTCMediaConstraints::Create());
  auto init = RTCRtpTransceiverInit::Create(RTCRtpTransceiverDirection::kSendRecv, {}, {});
  connection->AddTransceiver(RTCMediaType::AUDIO, init);
  connection->AddTransceiver(RTCMediaType::VIDEO, init);
  return connection;
}

std::string Offer(scoped_refptr<RTCPeerConnection> connection) {
  std::promise<std::string> made;
  connection->CreateOffer(
      [&](const string sdp, const string) { made.set_value(sdp.std_string()); },
      [&](const char* error) { made.set_value(std::string("error: ") + error); },
      RTCMediaConstraints::Create());
  auto future = made.get_future();
  if (future.wait_for(kHang) != std::future_status::ready) return "error: no offer";
  return future.get();
}

// The error SetLocalDescription gives, or "" when it takes the description.
std::string SetLocal(scoped_refptr<RTCPeerConnection> connection, const std::string& sdp) {
  std::promise<std::string> set;
  connection->SetLocalDescription(
      sdp, "offer", [&]() { set.set_value(""); },
      [&](const char* error) { set.set_value(error ? error : "failed"); });
  auto future = set.get_future();
  if (future.wait_for(kHang) != std::future_status::ready) return "no answer";
  return future.get();
}

// The mid of the first `kind` ("audio" or "video") section in `sdp`: a transceiver has
// none until a description is set.
std::string MidIn(const std::string& sdp, const std::string& kind) {
  bool in = false;
  for (const auto& line : Lines(sdp)) {
    if (line.rfind("m=", 0) == 0) in = line.rfind("m=" + kind + " ", 0) == 0;
    if (in && line.rfind("a=mid:", 0) == 0) return line.substr(6);
  }
  return "";
}

}  // namespace

int main() {
  LibWebRTC::Initialize();
  auto factory = LibWebRTC::CreateRTCPeerConnectionFactory();
  if (factory.get() == nullptr || !factory->Initialize()) {
    std::printf("FAIL: no peer connection factory\n");
    return 1;
  }

  for (RTCMediaType kind : {RTCMediaType::VIDEO, RTCMediaType::AUDIO}) {
    const char* name = kind == RTCMediaType::VIDEO ? "video" : "sound";
    // As onlyCallCodecs leaves it: the section rejected and out of the BUNDLE group.
    {
      auto connection = Connection(factory);
      std::string offer = Offer(connection);
      check(offer.rfind("error", 0) != 0, std::string("an offer: ") + offer);
      std::string mid = MidIn(offer, kind == RTCMediaType::VIDEO ? "video" : "audio");
      check(!mid.empty(), std::string("a mid for the ") + name);
      std::string error = SetLocal(connection, Reject(offer, mid, true));
      check(error.empty(), std::string("the offer with its ") + name +
                               " section rejected is taken; libwebrtc said: " + error);
      for (auto transceiver : connection->transceivers().std_vector()) {
        bool rejected = transceiver->media_type() == kind;
        bool stopped = transceiver->Stopped() || transceiver->Stopping();
        check(stopped == rejected,
              std::string(rejected ? "the rejected " : "the other ") + "section's " +
                  "transceiver " + (rejected ? "stops" : "goes on") + " (" + name +
                  " rejected)");
      }
      connection->Close();
      factory->Delete(connection);
    }
    // The sound section, first in the offer, is what the BUNDLE group is tagged by:
    // rejected and still named in the group, the offer is refused. onlyCallCodecs takes
    // a rejected section's mid out of the group for this. (A later section rejected but
    // still named is taken by M156; not counted on.)
    if (kind == RTCMediaType::AUDIO) {
      auto connection = Connection(factory);
      std::string offer = Offer(connection);
      std::string mid = MidIn(offer, kind == RTCMediaType::VIDEO ? "video" : "audio");
      std::string error = SetLocal(connection, Reject(offer, mid, false));
      check(!error.empty(), std::string("the offer with its ") + name +
                                " section rejected but still bundled is refused");
      connection->Close();
      factory->Delete(connection);
    }
  }

  factory->Terminate();
  factory = nullptr;
  LibWebRTC::Terminate();
  if (failures == 0) std::printf("rejected_section_test: all passed\n");
  return failures == 0 ? 0 : 1;
}
