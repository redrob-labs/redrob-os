// Redrob OS default Plasma layout (applied on a user's first login by the
// ai.redrob.desktop look-and-feel package).
// Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0.
//
// Same shape as Plasma's own org.kde.breeze layout -- one bottom panel: start
// menu, icons-only task manager, system tray, clock, show desktop -- with two
// Redrob choices: the start button is the Redrob tile (hicolor `redrob-os`, the
// design system's white symbol on a Redrob Blue tile, Favicons/README.md), and
// the task manager is icons-only, the way Windows users expect.

var panel = new Panel;
panel.location = "bottom";
panel.height = 2 * Math.floor(gridUnit * 1.6 / 2); // Plasma's default (44 px at 96 dpi)
panel.hiding = "none";
panel.floating = false;

var kickoff = panel.addWidget("org.kde.plasma.kickoff");
kickoff.currentConfigGroup = ["General"];
kickoff.writeConfig("icon", "redrob-os");
kickoff.writeConfig("favoritesPortedToKAstats", true);

panel.addWidget("org.kde.plasma.pager");

var tasks = panel.addWidget("org.kde.plasma.icontasks");
tasks.currentConfigGroup = ["General"];
tasks.writeConfig("launchers", [
  "applications:systemsettings.desktop",
  "preferred://filemanager",
  "preferred://browser",
  "applications:ai.redrob.launcher.desktop"
]);

panel.addWidget("org.kde.plasma.marginsseparator");
panel.addWidget("org.kde.plasma.systemtray");
panel.addWidget("org.kde.plasma.digitalclock");
panel.addWidget("org.kde.plasma.showdesktop");

// Wallpaper. The LnF `defaults` file (Image=Redrob) is only read by
// lookandfeeltool / System Settings, not on a first login, so without this the
// desktop falls back to Plasma's stock image (seen on the v0.3.0 install).
var allDesktops = desktops();
for (var i = 0; i < allDesktops.length; i++) {
  var d = allDesktops[i];
  d.wallpaperPlugin = "org.kde.image";
  d.currentConfigGroup = ["Wallpaper", "org.kde.image", "General"];
  d.writeConfig("Image", "file:///usr/share/wallpapers/Redrob");
  d.writeConfig("FillMode", 2);
}
