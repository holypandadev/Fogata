// Videos on Linux with GStreamer, the system's media framework. It is loaded when first
// needed, so the application starts on systems without it; videos then stay plain files.
#include "include/fogata_video/fogata_video_plugin.h"

#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <flutter_linux/flutter_linux.h>
#include <gdk-pixbuf/gdk-pixbuf.h>
#include <gtk/gtk.h>
#include <sys/stat.h>
#include <unistd.h>

#include <atomic>
#include <cstdint>
#include <cstring>
#include <map>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

#include "../common/video_head.h"
#include "open_video.h"
#include "rows.h"
#include "tone_map.h"

using fogata_video::Opened;
using fogata_video::open_video;

namespace {

// GStreamer's values used here, from its headers.
constexpr int kStateNull = 1;
constexpr int kStatePaused = 3;
constexpr int kStatePlaying = 4;
constexpr int kFormatTime = 3;
constexpr int kSeekFlush = 1 << 0;
constexpr int kSeekKeyUnit = 1 << 2;
constexpr int kMessageEos = 1 << 0;
constexpr int kMessageError = 1 << 1;
// playbin's flags: video, audio, soft volume and deinterlacing; never subtitles.
constexpr int kPlayVideo = 1 << 0;
constexpr int kPlayAudio = 1 << 1;
constexpr int kPlaySoftVolume = 1 << 4;
constexpr int kPlayDeinterlace = 1 << 9;
// GST_VIDEO_ORIENTATION_AUTO: videoflip follows the stream's image-orientation tag.
constexpr int kVideoDirectionAuto = 8;
constexpr guint64 kSecond = 1000000000ULL;
// The largest picture played: 8K (7680 by 4320 pixels), no side longer than 8192. Each
// picture is copied as width * height * 4 bytes, and a video can claim far more.
constexpr int64_t kMaxSide = 8192;
constexpr int64_t kMaxPixels = int64_t{7680} * 4320;

// GstMapInfo, as GStreamer 1.x lays it out.
struct MapInfo {
  void* memory;
  int flags;
  guint8* data;
  gsize size;
  gsize maxsize;
  gpointer user_data[4];
  gpointer reserved[4];
};
constexpr int kMapRead = 1;

// The part of GStreamer this plugin calls, found with dlsym.
struct Gst {
  gboolean (*init_check)(int*, char***, GError**);
  void* (*element_factory_make)(const char*, const char*);
  int (*element_set_state)(void*, int);
  int (*element_get_state)(void*, int*, int*, guint64);
  gboolean (*element_seek_simple)(void*, int, int, gint64);
  gboolean (*element_query_position)(void*, int, gint64*);
  gboolean (*element_query_duration)(void*, int, gint64*);
  void* (*element_get_bus)(void*);
  void* (*bus_timed_pop_filtered)(void*, guint64, int);
  void (*message_parse_error)(void*, GError**, gchar**);
  void* (*message_get_structure)(void*);
  void (*mini_object_unref)(void*);
  void* (*caps_from_string)(const char*);
  void (*object_unref)(void*);
  void* (*sample_get_buffer)(void*);
  void* (*sample_get_caps)(void*);
  void* (*caps_get_structure)(void*, unsigned);
  gboolean (*structure_get_int)(void*, const char*, int*);
  const gchar* (*structure_get_string)(void*, const char*);
  gsize (*buffer_get_size)(void*);
  gsize (*buffer_extract)(void*, gsize, void*, gsize);
  gboolean (*buffer_map)(void*, MapInfo*, int);
  void (*buffer_unmap)(void*, MapInfo*);
  void* (*app_sink_try_pull_sample)(void*, guint64);
  void* (*app_sink_try_pull_preroll)(void*, guint64);
  void* (*registry_get)();
  GList* (*registry_get_feature_list)(void*, GType);
  GType (*element_factory_get_type)();
  const gchar* (*element_factory_get_metadata)(void*, const gchar*);
  gchar* (*object_get_name)(void*);
  void (*plugin_feature_set_rank)(void*, guint);
  void (*plugin_feature_list_free)(GList*);
};

// Files to play come from other people. Only the demuxers of the formats the chat offers
// (MP4 and QuickTime, Matroska and WebM, AVI) may open them, and nothing that reaches the
// network: a file that is really an HLS or DASH playlist would otherwise make GStreamer
// fetch whatever addresses it lists, and rarely used demuxers would parse untrusted bytes.
// Those formats can name any codec, so only the decoders and parsers of the codecs such
// videos carry may decode them (VP8, VP9, AV1, H.264, H.265, MPEG-4 Part 2, MJPEG; Opus,
// Vorbis, AAC, MP3; plain PCM needs none), not every one installed: decoders are the most
// error-prone code a file reaches. Hardware decoders are left out too. Ranks only change
// in this process.
void keep_to_known_formats(const Gst* g) {
  static const char* const kDemuxers[] = {"qtdemux", "matroskademux", "avidemux"};
  static const char* const kCodecs[] = {
      // Pictures.
      "vp8dec", "vp9dec", "vp8alphadecodebin", "vp9alphadecodebin", "av1dec",
      "dav1ddec", "avdec_h264", "openh264dec", "avdec_h265", "libde265dec",
      "avdec_mpeg4", "jpegdec", "h264parse", "h265parse", "vp9parse", "av1parse",
      "mpeg4videoparse", "jpegparse",
      // Sound.
      "opusdec", "vorbisdec", "avdec_aac", "faad", "fdkaacdec", "mpg123audiodec",
      "avdec_mp3", "avdec_mp3float", "aacparse", "mpegaudioparse", "opusparse",
      "vorbisparse"};
  auto named = [g](void* feature, const char* const* names, size_t count) {
    gchar* name = g->object_get_name(feature);
    bool found = false;
    for (size_t i = 0; i < count && name != nullptr; i++) {
      found |= strcmp(name, names[i]) == 0;
    }
    g_free(name);
    return found;
  };
  GList* features = g->registry_get_feature_list(g->registry_get(),
                                                 g->element_factory_get_type());
  for (GList* l = features; l != nullptr; l = l->next) {
    const gchar* klass = g->element_factory_get_metadata(l->data, "klass");
    if (klass == nullptr) continue;
    const bool network = strstr(klass, "Network") != nullptr;
    const bool demuxer = strstr(klass, "Demux") != nullptr;
    const bool codec = strstr(klass, "Decoder") != nullptr ||
                       strstr(klass, "Parser") != nullptr ||
                       strstr(klass, "Decryptor") != nullptr;
    bool allowed = true;
    if (network) {
      allowed = false;
    } else if (demuxer) {
      allowed = named(l->data, kDemuxers, G_N_ELEMENTS(kDemuxers));
    } else if (codec) {
      allowed = named(l->data, kCodecs, G_N_ELEMENTS(kCodecs));
    }
    if (!allowed) g->plugin_feature_set_rank(l->data, 0);
  }
  g->plugin_feature_list_free(features);
}

// GStreamer once loaded and started, or null when the system has none.
const Gst* gst() {
  static Gst* loaded = []() -> Gst* {
    void* core = dlopen("libgstreamer-1.0.so.0", RTLD_NOW | RTLD_GLOBAL);
    void* app = dlopen("libgstapp-1.0.so.0", RTLD_NOW | RTLD_GLOBAL);
    if (core == nullptr || app == nullptr) return nullptr;
    auto* g = new Gst();
    bool ok = true;
    auto find = [&](void* lib, const char* name, auto& slot) {
      void* f = dlsym(lib, name);
      if (f == nullptr) ok = false;
      slot = reinterpret_cast<std::remove_reference_t<decltype(slot)>>(f);
    };
    find(core, "gst_init_check", g->init_check);
    find(core, "gst_element_factory_make", g->element_factory_make);
    find(core, "gst_element_set_state", g->element_set_state);
    find(core, "gst_element_get_state", g->element_get_state);
    find(core, "gst_element_seek_simple", g->element_seek_simple);
    find(core, "gst_element_query_position", g->element_query_position);
    find(core, "gst_element_query_duration", g->element_query_duration);
    find(core, "gst_element_get_bus", g->element_get_bus);
    find(core, "gst_bus_timed_pop_filtered", g->bus_timed_pop_filtered);
    find(core, "gst_message_parse_error", g->message_parse_error);
    find(core, "gst_message_get_structure", g->message_get_structure);
    find(core, "gst_mini_object_unref", g->mini_object_unref);
    find(core, "gst_caps_from_string", g->caps_from_string);
    find(core, "gst_object_unref", g->object_unref);
    find(core, "gst_sample_get_buffer", g->sample_get_buffer);
    find(core, "gst_sample_get_caps", g->sample_get_caps);
    find(core, "gst_caps_get_structure", g->caps_get_structure);
    find(core, "gst_structure_get_int", g->structure_get_int);
    find(core, "gst_structure_get_string", g->structure_get_string);
    find(core, "gst_buffer_get_size", g->buffer_get_size);
    find(core, "gst_buffer_extract", g->buffer_extract);
    find(core, "gst_buffer_map", g->buffer_map);
    find(core, "gst_buffer_unmap", g->buffer_unmap);
    find(app, "gst_app_sink_try_pull_sample", g->app_sink_try_pull_sample);
    find(app, "gst_app_sink_try_pull_preroll", g->app_sink_try_pull_preroll);
    find(core, "gst_registry_get", g->registry_get);
    find(core, "gst_registry_get_feature_list", g->registry_get_feature_list);
    find(core, "gst_element_factory_get_type", g->element_factory_get_type);
    find(core, "gst_element_factory_get_metadata",
         g->element_factory_get_metadata);
    find(core, "gst_object_get_name", g->object_get_name);
    find(core, "gst_plugin_feature_set_rank", g->plugin_feature_set_rank);
    find(core, "gst_plugin_feature_list_free", g->plugin_feature_list_free);
    if (!ok || !g->init_check(nullptr, nullptr, nullptr)) {
      delete g;
      return nullptr;
    }
    keep_to_known_formats(g);
    return g;
  }();
  return loaded;
}

// The pictures as the appsink takes them: RGBA, 8 bits a value, or 16 for videos of more
// than 8 bits (HDR ones are), which the conversion picks for them by itself, so an HDR
// picture is brought down to 8 bits here, after its tone mapping, not before.
constexpr char kPictureCaps[] = "video/x-raw,format={RGBA64_LE,RGBA}";

// A playbin for `uri` whose pictures go to an appsink as kPictureCaps. With `quiet` its
// sound is not decoded at all. Subtitles never are. Returns the playbin; `sink` is the
// appsink.
void* make_pipeline(const Gst* g, const char* uri, bool quiet, void** sink) {
  void* playbin = g->element_factory_make("playbin", nullptr);
  void* appsink = g->element_factory_make("appsink", nullptr);
  if (playbin == nullptr || appsink == nullptr) {
    if (playbin != nullptr) g->object_unref(playbin);
    if (appsink != nullptr) g->object_unref(appsink);
    return nullptr;
  }
  void* caps = g->caps_from_string(kPictureCaps);
  g_object_set(G_OBJECT(appsink), "caps", caps, "max-buffers", 2, "drop", TRUE,
               nullptr);
  g->mini_object_unref(caps);
  int flags = kPlayVideo | kPlayDeinterlace;
  if (!quiet) flags |= kPlayAudio | kPlaySoftVolume;
  g_object_set(G_OBJECT(playbin), "uri", uri, "video-sink", appsink, "flags",
               flags, nullptr);
  // Turns the pictures as the file's orientation says: phones film upright videos as
  // sideways pictures with a rotation tag. Without videoflip (plugins-good) they stay as
  // they are.
  void* flip = g->element_factory_make("videoflip", nullptr);
  if (flip != nullptr) {
    g_object_set(G_OBJECT(flip), "video-direction", kVideoDirectionAuto,
                 nullptr);
    g_object_set(G_OBJECT(playbin), "video-filter", flip, nullptr);
  }
  *sink = appsink;
  return playbin;
}

// Bytes a row of a picture may carry past its pixels, as some decoders align rows.
constexpr int64_t kMaxRowPadding = 256;

// A sample's picture as packed 8-bit RGBA, and its size, unless it is larger than a
// picture played (then `too_large` is set), its bytes are more than a pixel's plus row
// padding, or they are not whole rows. HDR pictures are tone mapped on the way.
bool sample_picture(const Gst* g, void* sample, int* width, int* height,
                    std::vector<uint8_t>* rgba, bool* too_large) {
  void* caps = g->sample_get_caps(sample);
  void* buffer = g->sample_get_buffer(sample);
  if (caps == nullptr || buffer == nullptr) return false;
  void* s = g->caps_get_structure(caps, 0);
  if (s == nullptr || !g->structure_get_int(s, "width", width) ||
      !g->structure_get_int(s, "height", height) || *width <= 0 ||
      *height <= 0) {
    return false;
  }
  const int64_t w = *width;
  const int64_t h = *height;
  if (w > kMaxSide || h > kMaxSide || w * h > kMaxPixels) {
    *too_large = true;
    return false;
  }
  const gchar* format = g->structure_get_string(s, "format");
  const bool sixteen = format != nullptr && strcmp(format, "RGBA64_LE") == 0;
  const int64_t pixel = sixteen ? 8 : 4;
  const fogata_video::Colours colours =
      fogata_video::colours_of(g->structure_get_string(s, "colorimetry"));
  const gsize size = g->buffer_get_size(buffer);
  if (static_cast<int64_t>(size) > (w * pixel + kMaxRowPadding) * h) return false;
  if (!sixteen && colours.transfer == fogata_video::Transfer::kStandard) {
    rgba->resize(size);
    g->buffer_extract(buffer, 0, rgba->data(), rgba->size());
    return fogata_video::pack_rows(rgba, *width, *height);
  }
  // Read where it is, without a copy: these pictures are the largest.
  MapInfo map = {};
  if (!g->buffer_map(buffer, &map, kMapRead)) return false;
  const size_t stride = map.size / h;
  const bool whole = map.size == size && map.size % h == 0 &&
                     static_cast<int64_t>(stride) >= w * pixel;
  if (whole) {
    rgba->resize(static_cast<size_t>(w * h * 4));
    if (colours.transfer == fogata_video::Transfer::kStandard) {
      // More than 8 bits, but not HDR: the top byte of each value.
      for (int64_t y = 0; y < h; y++) {
        const uint8_t* from = map.data + y * stride;
        uint8_t* to = rgba->data() + y * w * 4;
        for (int64_t i = 0; i < w * 4; i++) to[i] = from[i * 2 + 1];
      }
    } else {
      fogata_video::tone_map_picture(fogata_video::tone_map(colours), map.data,
                                     stride, sixteen, *width, *height,
                                     rgba->data());
    }
  }
  g->buffer_unmap(buffer, &map);
  return whole;
}

// Pipelines being taken down, each on a thread of its own: going to NULL waits for the
// streaming threads, and one stuck reading a file that stopped coming (a network share that
// hangs) holds it for as long as that read lasts. Neither a reading's answer nor the main
// thread waits for it; past so many still going, no new reading starts.
std::atomic<int> teardowns{0};
constexpr int kTeardownsMost = 8;

// Takes `playbin` down, then closes `fd`, the file it read (-1 for none), once it no longer
// reads it.
void tear_down(const Gst* g, void* playbin, int fd) {
  teardowns++;
  std::thread([g, playbin, fd]() {
    if (playbin != nullptr) {
      g->element_set_state(playbin, kStateNull);
      g->object_unref(playbin);
    }
    if (fd >= 0) close(fd);
    teardowns--;
  }).detach();
}

struct Probe {
  std::vector<uint8_t> thumb;
  int width = 0;
  int height = 0;
  int64_t duration_ms = 0;
};

// Reads the first picture of a video opened and checked (see open_video) as a JPEG at
// most `max_width` wide, its size and length.
// `opened` is closed once read.
bool probe(const Opened& opened, int max_width, Probe* out) {
  const Gst* g = gst();
  if (g == nullptr) {
    close(opened.fd);
    return false;
  }
  void* sink = nullptr;
  void* playbin = make_pipeline(g, opened.uri.c_str(), true, &sink);
  if (playbin == nullptr) {
    close(opened.fd);
    return false;
  }
  bool ok = false;
  g->element_set_state(playbin, kStatePaused);
  g->element_get_state(playbin, nullptr, nullptr, 5 * kSecond);
  void* sample = g->app_sink_try_pull_preroll(sink, 3 * kSecond);
  int width = 0;
  int height = 0;
  bool too_large = false;
  std::vector<uint8_t> rgba;
  if (sample != nullptr &&
      sample_picture(g, sample, &width, &height, &rgba, &too_large)) {
    // JPEG takes no alpha: packed RGB, in place.
    const size_t pixels = static_cast<size_t>(width) * height;
    for (size_t i = 0; i < pixels; i++) {
      memmove(rgba.data() + i * 3, rgba.data() + i * 4, 3);
    }
    g_autoptr(GdkPixbuf) full = gdk_pixbuf_new_from_data(
        rgba.data(), GDK_COLORSPACE_RGB, FALSE, 8, width, height, width * 3,
        nullptr, nullptr);
    const int w = width > max_width ? max_width : width;
    const int h = static_cast<int>(
        std::max<int64_t>(1, int64_t{height} * w / width));
    g_autoptr(GdkPixbuf) small =
        gdk_pixbuf_scale_simple(full, w, h, GDK_INTERP_BILINEAR);
    gchar* jpeg = nullptr;
    gsize size = 0;
    if (small != nullptr &&
        gdk_pixbuf_save_to_buffer(small, &jpeg, &size, "jpeg", nullptr,
                                  "quality", "75", nullptr)) {
      out->thumb.assign(jpeg, jpeg + size);
      g_free(jpeg);
      out->width = width;
      out->height = height;
      gint64 duration = 0;
      if (g->element_query_duration(playbin, kFormatTime, &duration)) {
        out->duration_ms = duration / 1000000;
      }
      ok = true;
    }
  }
  if (sample != nullptr) g->mini_object_unref(sample);
  tear_down(g, playbin, opened.fd);
  return ok;
}

}  // namespace

// ----- The texture a player draws into -----

struct Player;

G_DECLARE_FINAL_TYPE(VideoTexture, video_texture, FOGATA, VIDEO_TEXTURE,
                     FlPixelBufferTexture)

struct _VideoTexture {
  FlPixelBufferTexture parent_instance;
  // Owned by the texture: deleted with it, once the engine can no longer be copying the
  // player's pictures (see `retire`).
  Player* player;
};

struct Player {
  void* playbin = nullptr;
  // The file it plays, open since it was checked (see open_video).
  int fd = -1;
  void* sink = nullptr;
  FlTextureRegistrar* registrar = nullptr;
  VideoTexture* texture = nullptr;
  std::thread worker;
  std::atomic<bool> stop{false};
  std::atomic<bool> ended{false};
  std::atomic<bool> playing{false};
  std::mutex mutex;
  // Filled by the worker; handed to Flutter by copy_pixels.
  std::vector<uint8_t> latest;
  std::vector<uint8_t> shown;
  // The size of `latest`, and of `shown`.
  int width = 0;
  int height = 0;
  int shown_width = 0;
  int shown_height = 0;
  std::string error;
  // Set when the video cannot go on (an error, or pictures too large or not readable): it
  // does not play again.
  std::atomic<bool> refused{false};
};

G_DEFINE_TYPE(VideoTexture, video_texture, fl_pixel_buffer_texture_get_type())

// Closed players' textures, kept until the engine is surely done with them. The engine
// looks a texture up and copies its pixels on its raster thread without holding a
// reference, and uses the pixels after `copy_pixels` returns, so a texture unregistered on
// the main thread may still be in use there. That thread copies one texture at a time:
// once it starts copying another texture after the unregistering, the closed one is no
// longer in use and never will be.
namespace {
struct Retired {
  VideoTexture* texture;
  // `copies` when it was closed.
  uint64_t after;
};
std::mutex retired_mutex;
std::vector<Retired> retired;
// How many times the raster thread has started copying pixels.
uint64_t copies = 0;
}  // namespace

// Hands the plugin's reference to `texture` over to the list above.
static void retire(VideoTexture* texture) {
  std::lock_guard<std::mutex> lock(retired_mutex);
  retired.push_back({texture, copies});
}

static gboolean video_texture_copy_pixels(FlPixelBufferTexture* texture,
                                          const uint8_t** out_buffer,
                                          uint32_t* width, uint32_t* height,
                                          GError** error) {
  {
    // Frees the closed textures this thread has moved past. Their last unref runs here, on
    // the thread whose GL context made their GL textures.
    std::vector<VideoTexture*> done;
    {
      std::lock_guard<std::mutex> lock(retired_mutex);
      copies++;
      for (auto it = retired.begin(); it != retired.end();) {
        if (copies > it->after &&
            it->texture != FOGATA_VIDEO_TEXTURE(texture)) {
          done.push_back(it->texture);
          it = retired.erase(it);
        } else {
          ++it;
        }
      }
    }
    for (VideoTexture* gone : done) g_object_unref(gone);
  }
  Player* player = FOGATA_VIDEO_TEXTURE(texture)->player;
  std::lock_guard<std::mutex> lock(player->mutex);
  // Before the first picture, one clear pixel: an answer of FALSE needs an error set, and
  // the engine reads it.
  static const uint8_t clear[4] = {0, 0, 0, 0};
  if (player->latest.empty() && player->shown.empty()) {
    *out_buffer = clear;
    *width = 1;
    *height = 1;
    return TRUE;
  }
  if (!player->latest.empty()) {
    player->shown.swap(player->latest);
    player->shown_width = player->width;
    player->shown_height = player->height;
  }
  player->latest.clear();
  *out_buffer = player->shown.data();
  *width = player->shown_width;
  *height = player->shown_height;
  return TRUE;
}

static void video_texture_finalize(GObject* object) {
  delete FOGATA_VIDEO_TEXTURE(object)->player;
  G_OBJECT_CLASS(video_texture_parent_class)->finalize(object);
}

static void video_texture_class_init(VideoTextureClass* klass) {
  FL_PIXEL_BUFFER_TEXTURE_CLASS(klass)->copy_pixels = video_texture_copy_pixels;
  G_OBJECT_CLASS(klass)->finalize = video_texture_finalize;
}

static void video_texture_init(VideoTexture* self) {}

// Takes the pictures as they come, and notices the end and errors. After an error, or
// pictures too large or not readable, the player stops for good with its error.
static void run_player(Player* p) {
  const Gst* g = gst();
  void* bus = g->element_get_bus(p->playbin);
  // Pictures in a row whose bytes could not be read.
  int unreadable = 0;
  auto stop_for_good = [&](const std::string& error) {
    {
      std::lock_guard<std::mutex> lock(p->mutex);
      p->error = error;
    }
    p->refused = true;
    p->ended = true;
    p->playing = false;
    // Left as it is: going to NULL here would wait for a streaming thread stuck in a read,
    // and closing the player waits for this thread. close_player takes it down apart.
  };
  while (!p->stop) {
    void* sample = g->app_sink_try_pull_sample(p->sink, 20 * 1000000ULL);
    if (sample != nullptr) {
      int width = 0;
      int height = 0;
      bool too_large = false;
      std::vector<uint8_t> rgba;
      if (sample_picture(g, sample, &width, &height, &rgba, &too_large)) {
        unreadable = 0;
        {
          std::lock_guard<std::mutex> lock(p->mutex);
          p->latest.swap(rgba);
          p->width = width;
          p->height = height;
        }
        fl_texture_registrar_mark_texture_frame_available(p->registrar,
                                                          FL_TEXTURE(p->texture));
      } else if (!too_large) {
        unreadable++;
      }
      g->mini_object_unref(sample);
      if (too_large) {
        stop_for_good("its pictures are too large to play");
        break;
      }
      if (unreadable >= 30) {
        stop_for_good("its pictures cannot be read");
        break;
      }
    }
    void* message =
        g->bus_timed_pop_filtered(bus, 0, kMessageEos | kMessageError);
    if (message == nullptr) continue;
    // Of the two, only an error carries a structure, which holds the error.
    if (g->message_get_structure(message) != nullptr) {
      GError* err = nullptr;
      g->message_parse_error(message, &err, nullptr);
      g->mini_object_unref(message);
      stop_for_good(err != nullptr ? err->message : "it cannot be played");
      if (err != nullptr) g_error_free(err);
      break;
    }
    p->ended = true;
    p->playing = false;
    g->mini_object_unref(message);
  }
  g->object_unref(bus);
}

// ----- The plugin -----

#define FOGATA_VIDEO_PLUGIN(obj)                                     \
  (G_TYPE_CHECK_INSTANCE_CAST((obj), fogata_video_plugin_get_type(), \
                              FogataVideoPlugin))

struct _FogataVideoPlugin {
  GObject parent_instance;
  FlTextureRegistrar* registrar;
  std::map<int64_t, Player*>* players;
};

G_DEFINE_TYPE(FogataVideoPlugin, fogata_video_plugin, g_object_get_type())

static void close_player(Player* p) {
  const Gst* g = gst();
  p->stop = true;
  if (p->worker.joinable()) p->worker.join();
  tear_down(g, p->playbin, p->fd);
  p->playbin = nullptr;
  p->fd = -1;
  fl_texture_registrar_unregister_texture(p->registrar, FL_TEXTURE(p->texture));
  {
    // The picture not yet handed to the engine can go now.
    std::lock_guard<std::mutex> lock(p->mutex);
    std::vector<uint8_t>().swap(p->latest);
  }
  // The engine may still be copying the last picture on its own thread: the texture, the
  // player and that picture go once it has moved on.
  retire(p->texture);
}

static FlMethodResponse* success(FlValue* value) {
  return FL_METHOD_RESPONSE(fl_method_success_response_new(value));
}

static FlMethodResponse* failure(const char* code, const char* message) {
  return FL_METHOD_RESPONSE(
      fl_method_error_response_new(code, message, nullptr));
}

static int64_t int_arg(FlValue* args, const char* key, int64_t fallback) {
  FlValue* v = fl_value_lookup_string(args, key);
  return v != nullptr && fl_value_get_type(v) == FL_VALUE_TYPE_INT
             ? fl_value_get_int(v)
             : fallback;
}

// The heads the application gives for a video (see video_head.h): a list of maps of
// "bytes" and "mask". None when they are missing or not of that shape.
static std::vector<fogata_video::Head> heads_arg(FlValue* args) {
  std::vector<fogata_video::Head> heads;
  FlValue* list = fl_value_lookup_string(args, "heads");
  if (list == nullptr || fl_value_get_type(list) != FL_VALUE_TYPE_LIST) {
    return heads;
  }
  for (size_t i = 0; i < fl_value_get_length(list); i++) {
    FlValue* head = fl_value_get_list_value(list, i);
    if (fl_value_get_type(head) != FL_VALUE_TYPE_MAP) continue;
    FlValue* bytes = fl_value_lookup_string(head, "bytes");
    FlValue* mask = fl_value_lookup_string(head, "mask");
    if (bytes == nullptr || mask == nullptr ||
        fl_value_get_type(bytes) != FL_VALUE_TYPE_UINT8_LIST ||
        fl_value_get_type(mask) != FL_VALUE_TYPE_UINT8_LIST) {
      continue;
    }
    const uint8_t* b = fl_value_get_uint8_list(bytes);
    const uint8_t* m = fl_value_get_uint8_list(mask);
    heads.push_back({std::vector<uint8_t>(b, b + fl_value_get_length(bytes)),
                     std::vector<uint8_t>(m, m + fl_value_get_length(mask))});
  }
  return heads;
}

// Probes reading now, and the most at once: one more is answered at once as unread. A
// probe counts once its file is opened and checked, so one whose file never opens (on a
// network share that hangs) does not keep the others waiting.
static std::atomic<int> probes_running{0};
constexpr int kProbesAtOnce = 2;

// Openings of a file under way (open_video, for a probe or a player), and the most at once:
// a file that never opens (on a network share that hangs) holds its thread, and past so
// many no more is tried. Each is answered as unread, or with no player, at once.
static std::atomic<int> opens_running{0};
constexpr int kOpensAtOnce = 8;

// Counts one more probe reading, unless as many as allowed already are.
static bool take_probe_turn() {
  int now = probes_running.load();
  while (now < kProbesAtOnce) {
    if (probes_running.compare_exchange_weak(now, now + 1)) return true;
  }
  return false;
}

// A probe runs away from the main thread and answers on it.
struct ProbeJob {
  FlMethodCall* call;
  std::string path;
  std::vector<fogata_video::Head> heads;
  int max_width;
  Probe result;
  bool ok = false;
  // It took a turn (take_probe_turn), given back once answered.
  bool counted = false;
};

static gboolean answer_probe(gpointer data) {
  auto* job = static_cast<ProbeJob*>(data);
  g_autoptr(FlMethodResponse) response = nullptr;
  if (job->ok) {
    g_autoptr(FlValue) map = fl_value_new_map();
    fl_value_set_string_take(
        map, "thumb",
        fl_value_new_uint8_list(job->result.thumb.data(),
                                job->result.thumb.size()));
    fl_value_set_string_take(map, "width", fl_value_new_int(job->result.width));
    fl_value_set_string_take(map, "height",
                             fl_value_new_int(job->result.height));
    fl_value_set_string_take(map, "durationMs",
                             fl_value_new_int(job->result.duration_ms));
    response = success(map);
  } else {
    response = success(fl_value_new_null());
  }
  fl_method_call_respond(job->call, response, nullptr);
  g_object_unref(job->call);
  if (job->counted) probes_running--;
  delete job;
  return G_SOURCE_REMOVE;
}

// An opening runs away from the main thread and answers on it.
struct OpenJob {
  FlMethodCall* call;
  FogataVideoPlugin* plugin;
  std::string path;
  std::vector<fogata_video::Head> heads;
  Opened opened;
  bool ok = false;
};

// Makes the player of a file opened and checked (see open_video), or answers that there is
// none: null when it is not the kind of video its name says, or cannot be opened.
static gboolean answer_open(gpointer data) {
  auto* job = static_cast<OpenJob*>(data);
  FogataVideoPlugin* self = job->plugin;
  const Gst* g = gst();
  g_autoptr(FlMethodResponse) response = nullptr;
  if (!job->ok) {
    response = success(fl_value_new_null());
  } else if (self->players == nullptr) {
    // The plugin went away meanwhile.
    close(job->opened.fd);
    response = failure("open", "cannot play it");
  } else {
    auto* p = new Player();
    p->fd = job->opened.fd;
    p->playbin =
        make_pipeline(g, job->opened.uri.c_str(), false, &p->sink);
    if (p->playbin == nullptr) {
      close(p->fd);
      delete p;
      response = failure("open", "cannot play it");
    } else {
      p->registrar = self->registrar;
      p->texture =
          FOGATA_VIDEO_TEXTURE(g_object_new(video_texture_get_type(), nullptr));
      p->texture->player = p;
      fl_texture_registrar_register_texture(p->registrar,
                                            FL_TEXTURE(p->texture));
      g->element_set_state(p->playbin, kStatePaused);
      p->worker = std::thread(run_player, p);
      const int64_t id = fl_texture_get_id(FL_TEXTURE(p->texture));
      (*self->players)[id] = p;
      response = success(fl_value_new_int(id));
    }
  }
  fl_method_call_respond(job->call, response, nullptr);
  g_object_unref(job->call);
  g_object_unref(job->plugin);
  delete job;
  return G_SOURCE_REMOVE;
}

static void handle(FogataVideoPlugin* self, FlMethodCall* call) {
  const gchar* method = fl_method_call_get_name(call);
  FlValue* args = fl_method_call_get_args(call);
  g_autoptr(FlMethodResponse) response = nullptr;
  if (strcmp(method, "probe") == 0) {
    FlValue* path = fl_value_lookup_string(args, "path");
    if (path == nullptr || fl_value_get_type(path) != FL_VALUE_TYPE_STRING) {
      response = failure("args", "no path");
    } else if (probes_running >= kProbesAtOnce ||
               teardowns >= kTeardownsMost || opens_running >= kOpensAtOnce) {
      response = success(fl_value_new_null());
    } else {
      opens_running++;
      auto* job = new ProbeJob{FL_METHOD_CALL(g_object_ref(call)),
                               fl_value_get_string(path), heads_arg(args),
                               static_cast<int>(int_arg(args, "maxWidth", 480))};
      std::thread([job]() {
        Opened opened;
        const bool checked = open_video(job->path.c_str(), job->heads, &opened);
        opens_running--;
        if (checked) {
          job->counted = take_probe_turn();
          if (job->counted) {
            job->ok = probe(opened, job->max_width, &job->result);
          } else {
            close(opened.fd);
          }
        }
        g_idle_add(answer_probe, job);
      }).detach();
      return;
    }
  } else if (strcmp(method, "open") == 0) {
    FlValue* path = fl_value_lookup_string(args, "path");
    if (gst() == nullptr) {
      response = failure("unavailable", "GStreamer is not installed");
    } else if (path == nullptr ||
               fl_value_get_type(path) != FL_VALUE_TYPE_STRING) {
      response = failure("args", "no path");
    } else if (opens_running >= kOpensAtOnce) {
      response = success(fl_value_new_null());
    } else {
      opens_running++;
      // The file is opened and checked away from the main thread, which a file that is
      // slow to open (on a network share) would otherwise hold up.
      auto* job = new OpenJob{FL_METHOD_CALL(g_object_ref(call)),
                              FOGATA_VIDEO_PLUGIN(g_object_ref(self)),
                              fl_value_get_string(path), heads_arg(args)};
      std::thread([job]() {
        job->ok = open_video(job->path.c_str(), job->heads, &job->opened);
        opens_running--;
        g_idle_add(answer_open, job);
      }).detach();
      return;
    }
  } else {
    const int64_t id = int_arg(args, "id", -1);
    auto found = self->players->find(id);
    if (found == self->players->end()) {
      response = failure("player", "no such player");
    } else {
      Player* p = found->second;
      const Gst* g = gst();
      const bool moves = strcmp(method, "play") == 0 ||
                         strcmp(method, "pause") == 0 ||
                         strcmp(method, "seek") == 0;
      if (moves && p->refused) {
        // A video that cannot go on stays stopped.
        response = success(nullptr);
      } else if (strcmp(method, "play") == 0) {
        if (p->ended) {
          g->element_seek_simple(p->playbin, kFormatTime,
                                 kSeekFlush | kSeekKeyUnit, 0);
          p->ended = false;
        }
        g->element_set_state(p->playbin, kStatePlaying);
        p->playing = true;
        response = success(nullptr);
      } else if (strcmp(method, "pause") == 0) {
        g->element_set_state(p->playbin, kStatePaused);
        p->playing = false;
        response = success(nullptr);
      } else if (strcmp(method, "seek") == 0) {
        const int64_t ms = int_arg(args, "ms", 0);
        g->element_seek_simple(p->playbin, kFormatTime, kSeekFlush,
                               ms * 1000000);
        p->ended = false;
        response = success(nullptr);
      } else if (strcmp(method, "volume") == 0) {
        FlValue* v = fl_value_lookup_string(args, "volume");
        const double volume =
            v != nullptr && fl_value_get_type(v) == FL_VALUE_TYPE_FLOAT
                ? fl_value_get_float(v)
                : 1.0;
        g_object_set(G_OBJECT(p->playbin), "volume", volume, nullptr);
        response = success(nullptr);
      } else if (strcmp(method, "state") == 0) {
        gint64 position = 0;
        gint64 duration = 0;
        g->element_query_position(p->playbin, kFormatTime, &position);
        g->element_query_duration(p->playbin, kFormatTime, &duration);
        g_autoptr(FlValue) map = fl_value_new_map();
        fl_value_set_string_take(map, "positionMs",
                                 fl_value_new_int(position / 1000000));
        fl_value_set_string_take(map, "durationMs",
                                 fl_value_new_int(duration / 1000000));
        fl_value_set_string_take(map, "playing",
                                 fl_value_new_bool(p->playing));
        fl_value_set_string_take(map, "ended", fl_value_new_bool(p->ended));
        std::lock_guard<std::mutex> lock(p->mutex);
        fl_value_set_string_take(map, "width", fl_value_new_int(p->width));
        fl_value_set_string_take(map, "height", fl_value_new_int(p->height));
        if (!p->error.empty()) {
          fl_value_set_string_take(map, "error",
                                   fl_value_new_string(p->error.c_str()));
        }
        response = success(map);
      } else if (strcmp(method, "close") == 0) {
        self->players->erase(found);
        close_player(p);
        response = success(nullptr);
      } else {
        response = FL_METHOD_RESPONSE(fl_method_not_implemented_response_new());
      }
    }
  }
  fl_method_call_respond(call, response, nullptr);
}

static void fogata_video_plugin_dispose(GObject* object) {
  FogataVideoPlugin* self = FOGATA_VIDEO_PLUGIN(object);
  if (self->players != nullptr) {
    for (auto& [id, player] : *self->players) close_player(player);
    delete self->players;
    self->players = nullptr;
  }
  G_OBJECT_CLASS(fogata_video_plugin_parent_class)->dispose(object);
}

static void fogata_video_plugin_class_init(FogataVideoPluginClass* klass) {
  G_OBJECT_CLASS(klass)->dispose = fogata_video_plugin_dispose;
}

static void fogata_video_plugin_init(FogataVideoPlugin* self) {
  self->players = new std::map<int64_t, Player*>();
}

static void method_call_cb(FlMethodChannel* channel, FlMethodCall* call,
                           gpointer user_data) {
  handle(FOGATA_VIDEO_PLUGIN(user_data), call);
}

void fogata_video_plugin_register_with_registrar(FlPluginRegistrar* registrar) {
  FogataVideoPlugin* plugin = FOGATA_VIDEO_PLUGIN(
      g_object_new(fogata_video_plugin_get_type(), nullptr));
  plugin->registrar = fl_plugin_registrar_get_texture_registrar(registrar);
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  g_autoptr(FlMethodChannel) channel =
      fl_method_channel_new(fl_plugin_registrar_get_messenger(registrar),
                            "fogata/video", FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(channel, method_call_cb,
                                            g_object_ref(plugin), g_object_unref);
  g_object_unref(plugin);
}
