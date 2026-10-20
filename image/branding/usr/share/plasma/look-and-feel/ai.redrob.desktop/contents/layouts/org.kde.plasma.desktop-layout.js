// Redrob OS default Plasma layout (applied on a user's first login by the
// ai.redrob.desktop look-and-feel package). This is the SAFE DEFAULT (Windows
// shape); the first-login style chooser (/usr/libexec/redrob/style-chooser)
// rebuilds the panels for whichever style the user picks (macOS or Windows),
// so this only governs the brief window before the chooser runs, and the case
// where the chooser is cancelled.
// Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0.

var panel = new Panel;
panel.location = "bottom";
panel.height = 2 * Math.floor(gridUnit * 1.6 / 2);
panel.hiding = "none";
panel.floating = false;

var kickoff = panel.addWidget("org.kde.plasma.kickoff");
kickoff.currentConfigGroup = ["General"];
kickoff.writeConfig("icon", "redrob-os");
kickoff.writeConfig("favoritesPortedToKAstats", true);

var tasks = panel.addWidget("org.kde.plasma.icontasks");
tasks.currentConfigGroup = ["General"];
tasks.writeConfig("launchers", [
  "applications:redrob-agent-sidebar.desktop",
  "applications:systemsettings.desktop",
  "preferred://filemanager",
  "preferred://browser",
  "applications:ai.redrob.launcher.desktop"
]);

panel.addWidget("org.kde.plasma.marginsseparator");
panel.addWidget("org.kde.plasma.systemtray");
panel.addWidget("org.kde.plasma.digitalclock");
panel.addWidget("org.kde.plasma.showdesktop");

var allDesktops = desktops();
for (var i = 0; i < allDesktops.length; i++) {
  var d = allDesktops[i];
  d.wallpaperPlugin = "org.kde.image";
  d.currentConfigGroup = ["Wallpaper", "org.kde.image", "General"];
  d.writeConfig("Image", "file:///usr/share/wallpapers/Redrob");
  d.writeConfig("FillMode", 2);
}
