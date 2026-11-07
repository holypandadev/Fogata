// A camera for tests on a computer without one: a PipeWire video source that WirePlumber
// and the Camera portal take for a camera (media.role Camera), offering YUY2 at 640x480
// and 1280x720, 30 frames a second, as a webcam does. Its pictures are a gradient that
// moves, so a receiver can tell they change. It runs until it is ended (SIGTERM, SIGINT);
// ending it is a camera unplugged.
//
//     camera_source <description> [<V4L2 bus>]
//
// <V4L2 bus> gives the node the property PipeWire's V4L2 nodes carry
// (api.v4l2.cap.bus_info), so a test can check that a camera keeps the id V4L2 gives it.
//
//     cc -O2 -o camera_source camera_source.c $(pkg-config --cflags --libs libpipewire-0.3)

#include <pipewire/pipewire.h>
#include <signal.h>
#include <spa/param/buffers.h>
#include <spa/param/video/format-utils.h>
#include <spa/utils/result.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

struct camera {
  struct pw_main_loop* loop;
  struct pw_stream* stream;
  struct spa_hook listener;
  struct spa_source* timer;
  struct spa_video_info_raw format;
  int stride;
  unsigned frame;
};

static void on_timeout(void* data, uint64_t expirations) {
  struct camera* camera = data;
  pw_stream_trigger_process(camera->stream);
}

static void on_process(void* data) {
  struct camera* camera = data;
  struct pw_buffer* b = pw_stream_dequeue_buffer(camera->stream);
  if (!b) return;
  struct spa_buffer* buf = b->buffer;
  uint8_t* p = buf->datas[0].data;
  if (p && camera->stride > 0) {
    const int w = camera->format.size.width;
    const int h = camera->format.size.height;
    for (int y = 0; y < h; y++) {
      uint8_t* row = p + y * camera->stride;
      for (int x = 0; x < w; x += 2) {
        const uint8_t luma = (uint8_t)(x / 4 + y / 4 + camera->frame * 3);
        row[x * 2 + 0] = luma;
        row[x * 2 + 1] = (uint8_t)(64 + y % 128);
        row[x * 2 + 2] = luma;
        row[x * 2 + 3] = (uint8_t)(64 + x % 128);
      }
    }
    buf->datas[0].chunk->offset = 0;
    buf->datas[0].chunk->stride = camera->stride;
    buf->datas[0].chunk->size = camera->stride * h;
    camera->frame++;
  }
  pw_stream_queue_buffer(camera->stream, b);
}

static void on_state_changed(void* data, enum pw_stream_state old,
                             enum pw_stream_state state, const char* error) {
  struct camera* camera = data;
  struct pw_loop* loop = pw_main_loop_get_loop(camera->loop);
  fprintf(stderr, "camera_source: %s%s%s\n", pw_stream_state_as_string(state),
          error ? ": " : "", error ? error : "");
  if (state == PW_STREAM_STATE_STREAMING) {
    struct timespec first = {0, 1}, every = {0, 1000000000 / 30};
    pw_loop_update_timer(loop, camera->timer, &first, &every, false);
  } else {
    pw_loop_update_timer(loop, camera->timer, NULL, NULL, false);
  }
}

static void on_param_changed(void* data, uint32_t id, const struct spa_pod* param) {
  struct camera* camera = data;
  if (param == NULL || id != SPA_PARAM_Format) return;
  if (spa_format_video_raw_parse(param, &camera->format) < 0) return;
  camera->stride = SPA_ROUND_UP_N(camera->format.size.width * 2, 4);
  fprintf(stderr, "camera_source: format %ux%u\n", camera->format.size.width,
          camera->format.size.height);
  uint8_t buffer[1024];
  struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
  const struct spa_pod* params[1];
  params[0] = spa_pod_builder_add_object(
      &b, SPA_TYPE_OBJECT_ParamBuffers, SPA_PARAM_Buffers,
      SPA_PARAM_BUFFERS_buffers, SPA_POD_CHOICE_RANGE_Int(8, 2, 16),
      SPA_PARAM_BUFFERS_blocks, SPA_POD_Int(1),
      SPA_PARAM_BUFFERS_size, SPA_POD_Int(camera->stride * camera->format.size.height),
      SPA_PARAM_BUFFERS_stride, SPA_POD_Int(camera->stride),
      SPA_PARAM_BUFFERS_dataType,
      SPA_POD_CHOICE_FLAGS_Int((1 << SPA_DATA_MemFd) | (1 << SPA_DATA_MemPtr)));
  pw_stream_update_params(camera->stream, params, 1);
}

static const struct pw_stream_events events = {
    PW_VERSION_STREAM_EVENTS,
    .state_changed = on_state_changed,
    .param_changed = on_param_changed,
    .process = on_process,
};

static void quit(void* data, int signal) {
  pw_main_loop_quit(((struct camera*)data)->loop);
}

// One format the camera offers: YUY2 at one size, 30 frames a second, nothing to choose.
static const struct spa_pod* format(struct spa_pod_builder* b, uint32_t width,
                                    uint32_t height) {
  return spa_pod_builder_add_object(
      b, SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat,
      SPA_FORMAT_mediaType, SPA_POD_Id(SPA_MEDIA_TYPE_video),
      SPA_FORMAT_mediaSubtype, SPA_POD_Id(SPA_MEDIA_SUBTYPE_raw),
      SPA_FORMAT_VIDEO_format, SPA_POD_Id(SPA_VIDEO_FORMAT_YUY2),
      SPA_FORMAT_VIDEO_size, SPA_POD_Rectangle(&SPA_RECTANGLE(width, height)),
      SPA_FORMAT_VIDEO_framerate, SPA_POD_Fraction(&SPA_FRACTION(30, 1)));
}

int main(int argc, char* argv[]) {
  if (argc < 2) {
    fprintf(stderr, "usage: camera_source <description> [<V4L2 bus>]\n");
    return 2;
  }
  struct camera camera = {0};
  pw_init(&argc, &argv);
  camera.loop = pw_main_loop_new(NULL);
  struct pw_loop* loop = pw_main_loop_get_loop(camera.loop);
  pw_loop_add_signal(loop, SIGINT, quit, &camera);
  pw_loop_add_signal(loop, SIGTERM, quit, &camera);
  camera.timer = pw_loop_add_timer(loop, on_timeout, &camera);

  struct pw_properties* props = pw_properties_new(
      PW_KEY_MEDIA_CLASS, "Video/Source", PW_KEY_MEDIA_ROLE, "Camera",
      PW_KEY_MEDIA_TYPE, "Video", PW_KEY_MEDIA_CATEGORY, "Capture",
      PW_KEY_NODE_DESCRIPTION, argv[1], NULL);
  pw_properties_setf(props, PW_KEY_NODE_NAME, "fogata-test-camera.%d", (int)getpid());
  if (argc > 2) pw_properties_set(props, "api.v4l2.cap.bus_info", argv[2]);
  camera.stream = pw_stream_new_simple(loop, argv[1], props, &events, &camera);

  uint8_t buffer[1024];
  struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
  const struct spa_pod* params[2] = {format(&b, 640, 480), format(&b, 1280, 720)};
  int res = pw_stream_connect(
      camera.stream, PW_DIRECTION_OUTPUT, PW_ID_ANY,
      PW_STREAM_FLAG_DRIVER | PW_STREAM_FLAG_MAP_BUFFERS, params, 2);
  if (res < 0) {
    fprintf(stderr, "camera_source: cannot connect: %s\n", spa_strerror(res));
    return 1;
  }
  pw_main_loop_run(camera.loop);
  pw_stream_destroy(camera.stream);
  pw_main_loop_destroy(camera.loop);
  pw_deinit();
  return 0;
}
