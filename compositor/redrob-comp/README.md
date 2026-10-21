# redrob-comp (experiment)

Redrob OS's own compositor + shell, written on [Smithay](https://github.com/Smithay/smithay)
instead of KDE Plasma or COSMIC. The shell chrome (bar, dock) is drawn by the compositor
itself in Redrob Design System colours.

Started from Smithay's `smallvil` example (MIT, Copyright (c) 2017 Victor Berger and
Victoria Brekenfeld -- see `LICENSE-smithay.txt`); Redrob changes are MIT too.

Milestone 1 (now): nested window (winit backend), Redrob ground, top bar with the Redrob
tile, centred dock with five slots. Next: text/clock, real icons, click-to-launch, then a
DRM/udev backend so it runs as the login session on real hardware.

Run nested (from a desktop session):

    cargo build --jobs 14
    ./target/debug/redrob-comp -c <wayland-app>
