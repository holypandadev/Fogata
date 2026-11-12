#ifndef FLUTTER_SCRREN_CAPTURE_HXX
#define FLUTTER_SCRREN_CAPTURE_HXX

#include "flutter_common.h"
#include "flutter_webrtc_base.h"

#include <memory>

#include "fogata_sound_start.h"
#include "loopback_capturer.h"
#include "rtc_audio_source.h"
#include "rtc_audio_track.h"
#include "rtc_desktop_capturer.h"
#include "rtc_desktop_media_list.h"

namespace flutter_webrtc_plugin {

class FlutterScreenCapture : public MediaListObserver,
                             public DesktopCapturerObserver {
 public:
  FlutterScreenCapture(FlutterWebRTCBase* base);
  ~FlutterScreenCapture();

  void GetDisplayMedia(const EncodableMap& constraints,
                       std::unique_ptr<MethodResultProxy> result);

#ifdef __linux__
  // Fogata: under Wayland, the screen or window chosen in the system's own
  // dialog, captured through PipeWire.
  void GetDisplayMediaFromPortal(double fps,
                                 bool show_cursor,
                                 bool capture_audio,
                                 RTCAudioOptions sound_options,
                                 std::unique_ptr<MethodResultProxy> result);
#endif

  void GetDesktopSources(const EncodableList& types,
                         std::unique_ptr<MethodResultProxy> result);

  void UpdateDesktopSources(const EncodableList& types,
                            std::unique_ptr<MethodResultProxy> result);

  void GetDesktopSourceThumbnail(std::string source_id,
                                 int width,
                                 int height,
                                 std::unique_ptr<MethodResultProxy> result);

 protected:
  void OnMediaSourceAdded(scoped_refptr<MediaSource> source) override;

  void OnMediaSourceRemoved(scoped_refptr<MediaSource> source) override;

  void OnMediaSourceNameChanged(scoped_refptr<MediaSource> source) override;

  void OnMediaSourceThumbnailChanged(
      scoped_refptr<MediaSource> source) override;

  void OnStart(scoped_refptr<RTCDesktopCapturer> capturer) override;

  void OnPaused(scoped_refptr<RTCDesktopCapturer> capturer) override;

  void OnStop(scoped_refptr<RTCDesktopCapturer> capturer) override;

  void OnError(scoped_refptr<RTCDesktopCapturer> capturer) override;

 private:
  bool BuildDesktopSourcesList(const EncodableList& types, bool force_reload);

 private:
  FlutterWebRTCBase* base_;
  std::map<DesktopType, scoped_refptr<RTCDesktopMediaList>> medialist_;
  std::vector<scoped_refptr<MediaSource>> sources_;

  // Loopback audio capturer active during a screen-share session.
  // Null when not capturing or on platforms without loopback support.
  std::unique_ptr<LoopbackCapturer> loopback_capturer_;
  // The custom audio source fed by the loopback capturer.
  scoped_refptr<RTCAudioSource> loopback_audio_source_;
  // Fogata: the desktop capturer the loopback capturer was started with, whose end
  // stops it (only compared, never used).
  RTCDesktopCapturer* loopback_owner_ = nullptr;

  // Fogata: the video track of each desktop capture going on, by its capturer, so
  // a capture that fails (a shared window that closed) tells Dart its track ended.
  // Like the loopback members above, only touched on the platform thread.
  std::map<RTCDesktopCapturer*, std::string> capture_tracks_;

  // Fogata: stops the loopback capturer if it was started with |capturer|.
  void StopSoundOf(RTCDesktopCapturer* capturer);

  // Fogata: runs |task| on the platform thread. The desktop capturer's observer is
  // called on libwebrtc's capture thread, while the platform thread starts and stops
  // captures and their sound.
  void OnPlatformThread(std::function<void()> task);

#ifdef _WIN32
  // Fogata: a share whose sound is still starting, off the platform thread
  // (fogata_sound_start.h): getDisplayMedia answers once it has started, could not or
  // took too long. At most one; only touched on the platform thread.
  struct PendingSound;
  std::shared_ptr<PendingSound> pending_sound_;
  // Tasks from the sound's own threads reach the platform thread through this, closed
  // as this goes (before the plugin's task runner does).
  std::shared_ptr<PlatformQueue> sound_queue_;

  // Starts the sound of the share |pending| describes, and answers it once that is
  // known.
  void BeginSound(std::shared_ptr<PendingSound> pending,
                  std::unique_ptr<LoopbackCapturer> loopback);
  // How the start of |pending|'s sound ended: the answer, with or without its sound.
  void SoundSettled(const std::shared_ptr<PendingSound>& pending,
                    SoundOutcome outcome);
  // Answers |pending| with its picture alone, as a share whose sound could not start.
  void AnswerWithoutSound(PendingSound& pending);
  // A share with sound asked for while |pending_sound_|'s still starts: that one goes
  // without its sound, as a running one is stopped by the new share.
  void SupersedePendingSound();
  // The capture of the share whose sound still starts ended (|capturer|'s OnStop or
  // OnError): it is answered with the error of a capture that could not start, and
  // nothing of it is kept. False if that share is not |capturer|'s.
  bool FailPendingSound(RTCDesktopCapturer* capturer);
#endif
};

}  // namespace flutter_webrtc_plugin

#endif  // FLUTTER_SCRREN_CAPTURE_HXX