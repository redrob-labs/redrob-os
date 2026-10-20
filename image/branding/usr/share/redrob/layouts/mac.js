// Redrob OS -- macOS-style desktop (applied by style-chooser via plasmashell
// evaluateScript). Top menu bar (Redrob menu + global app menu + tray + clock)
// and a floating bottom dock. Pair with window buttons on the left (style-chooser
// sets kwinrc ButtonsOnLeft for this style).
// Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0.

var ex = panels();
for (var i = 0; i < ex.length; i++) { ex[i].remove(); }

var top = new Panel;
top.location = "top";
top.height = Math.round(gridUnit * 1.4);
top.hiding = "none";
top.floating = false;

var kickoff = top.addWidget("org.kde.plasma.kickoff");
kickoff.currentConfigGroup = ["General"];
kickoff.writeConfig("icon", "redrob-os");
kickoff.writeConfig("favoritesPortedToKAstats", true);

top.addWidget("org.kde.plasma.appmenu");
top.addWidget("org.kde.plasma.panelspacer");
top.addWidget("org.kde.plasma.systemtray");
top.addWidget("org.kde.plasma.digitalclock");

var dock = new Panel;
dock.location = "bottom";
dock.height = Math.round(gridUnit * 3.2);
dock.floating = true;
dock.hiding = "dodgewindows";

var dtasks = dock.addWidget("org.kde.plasma.icontasks");
dtasks.currentConfigGroup = ["General"];
dtasks.writeConfig("launchers", [
  "applications:redrob-agent-sidebar.desktop",
  "applications:org.kde.dolphin.desktop",
  "preferred://browser",
  "applications:ai.redrob.launcher.desktop",
  "applications:systemsettings.desktop"
]);
dtasks.writeConfig("showOnlyCurrentDesktop", false);

var ds = desktops();
for (var j = 0; j < ds.length; j++) {
  ds[j].wallpaperPlugin = "org.kde.image";
  ds[j].currentConfigGroup = ["Wallpaper", "org.kde.image", "General"];
  ds[j].writeConfig("Image", "file:///usr/share/wallpapers/Redrob");
  ds[j].writeConfig("FillMode", 2);
}
