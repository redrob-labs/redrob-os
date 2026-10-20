// Redrob OS -- Windows-style desktop (applied by style-chooser via plasmashell
// evaluateScript). Bottom taskbar: start menu, icon tasks, tray, clock.
// Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0.

var ex = panels();
for (var i = 0; i < ex.length; i++) { ex[i].remove(); }

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

var ds = desktops();
for (var j = 0; j < ds.length; j++) {
  ds[j].wallpaperPlugin = "org.kde.image";
  ds[j].currentConfigGroup = ["Wallpaper", "org.kde.image", "General"];
  ds[j].writeConfig("Image", "file:///usr/share/wallpapers/Redrob");
  ds[j].writeConfig("FillMode", 2);
}
