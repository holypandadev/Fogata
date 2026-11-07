#ifndef RUNNER_DESKTOP_CHANNEL_H_
#define RUNNER_DESKTOP_CHANNEL_H_

#include <flutter_linux/flutter_linux.h>

// The channel "fogata/desktop": what the application asks the Linux desktop.
//  - "requestBackground" {autostart, commandline, reason}: asks the Background portal
//    to start the application with the session (inside a Flatpak, the only way); answers
//    whether it will, or null when the portal could not be asked.
//  - "mainWindow": the splash window becomes the main one; answers its corner radius,
//    round where the desktop composites windows, 0 for square corners.
// And it tells the application:
//  - "windowCorner" <radius>: the corners turned round or square (maximized, tiled, full
//    screen, or the desktop stopped compositing).
void desktop_channel_register(FlView* view);

#endif  // RUNNER_DESKTOP_CHANNEL_H_
