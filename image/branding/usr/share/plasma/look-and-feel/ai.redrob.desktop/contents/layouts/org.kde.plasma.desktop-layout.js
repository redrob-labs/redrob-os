// Redrob OS default Plasma layout (applied on a user's first login by the
// ai.redrob.desktop look-and-feel package).
// Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0.
//
// macOS-shaped, not Windows-shaped: a thin TOP menu bar (Redrob menu + the
// global application menu + status tray + clock) and a floating BOTTOM dock
// (icon-only, auto-hiding) for apps. Window buttons move to the left and the
// per-window menu is lifted into the top bar (see /etc/xdg/kwinrc, kdeglobals).

// --- Top menu bar -------------------------------------------------------------
var top = new Panel;
top.location = "top";
top.height = Math.round(gridUnit * 1.4);
top.hiding = "none";
top.floating = false;

// Redrob menu: the start/launcher menu sits where macOS puts the Apple menu,
// as the Redrob tile (hicolor `redrob-os`, white symbol on Redrob Blue).
var kickoff = top.addWidget("org.kde.plasma.kickoff");
kickoff.currentConfigGroup = ["General"];
kickoff.writeConfig("icon", "redrob-os");
kickoff.writeConfig("favoritesPortedToKAstats", true);

// Global application menu: the focused window's menu bar, macOS-style.
top.addWidget("org.kde.plasma.appmenu");

// Push status items to the right edge.
top.addWidget("org.kde.plasma.panelspacer");

top.addWidget("org.kde.plasma.systemtray");
top.addWidget("org.kde.plasma.digitalclock");

// --- Bottom dock --------------------------------------------------------------
var dock = new Panel;
dock.location = "bottom";
dock.height = Math.round(gridUnit * 3.2);
dock.floating = true;             // detached, rounded -- macOS dock
dock.hiding = "dodgewindows";     // slides away when a window would overlap it

var dtasks = dock.addWidget("org.kde.plasma.icontasks");
dtasks.currentConfigGroup = ["General"];
dtasks.writeConfig("launchers", [
  "applications:org.kde.dolphin.desktop",
  "preferred://browser",
  "applications:ai.redrob.launcher.desktop",
  "applications:systemsettings.desktop"
]);
dtasks.writeConfig("showOnlyCurrentDesktop", false);
dtasks.writeConfig("showOnlyCurrentActivity", false);

// --- Wallpaper ----------------------------------------------------------------
// The LnF `defaults` Image= is only read by lookandfeeltool / System Settings,
// not on a first login, so set it here (seen falling back to stock on v0.3.0).
var allDesktops = desktops();
for (var i = 0; i < allDesktops.length; i++) {
  var d = allDesktops[i];
  d.wallpaperPlugin = "org.kde.image";
  d.currentConfigGroup = ["Wallpaper", "org.kde.image", "General"];
  d.writeConfig("Image", "file:///usr/share/wallpapers/Redrob");
  d.writeConfig("FillMode", 2);
}
