# The desktop

A tour of what you see after installing Redrob OS. If you have used Windows, nothing
here needs learning; the differences are called out where they exist.

## Taskbar

Along the bottom of the screen:

- **Start button** (left) opens the start menu.
- **Pinned apps** next to it; running apps show up here too. Right-click an icon to pin
  or unpin it.
- **System tray** (right): network (Wi‑Fi), sound, battery, Bluetooth, the input-method
  indicator (`한`/`A`), notifications, and the **Redrob** icon for the agent and pairing.
- **Clock** at the far right; click it for the calendar.

## Start menu

Click the start button or press the Windows (Meta) key. Type to search apps, files and
settings; the categories on the left list all installed apps. Pin a favourite by
right-clicking it. Power options (sleep, restart, shut down, log out) are at the bottom.

## Settings

Start menu → **System Settings**. The parts you are most likely to need:

| Section | What it does |
|---|---|
| Wi‑Fi & Networking | join networks, VPN |
| Display & Monitor | resolution, scaling (for high-DPI screens), second monitor layout |
| Sound | output device, volume per app |
| Keyboard → Virtual Keyboard / Input Method | Korean input (see below) |
| Appearance | light/dark theme, wallpaper, fonts |
| Users | add or change accounts |
| Updates | shows the staged OS update and the last rollback, if any |

## Discover (app store)

Start menu → **Discover**. Search and install apps from the Flathub catalogue (browsers,
office, media, games, developer tools). Installed apps update themselves in the
background; the **Updates** page in Discover shows what changed. Removing an app is the
**Remove** button on its page. Apps are sandboxed per app; the first launch may ask
permission for your camera or files.

## Redrob Launcher

Starts at login (the Redrob icon in the tray). It installs and updates the Redrob apps —
**Redrob Browser**, **Redrob Office**, **Redrob Canvas**, **Redrob Code** — in your home
directory and puts them in the start menu. The Launcher is also where you open the
**pairing QR** for your phone and see whether the agent is running. Closing the Launcher
window leaves it in the tray; the agent keeps running regardless.

## Korean input

Korean (Hangul) input is installed and enabled when Korean is chosen as the system
language at install time, and can be added later in Settings → Keyboard → Input Method.

- Toggle Korean/English: **Shift+Space**, or the **한/영** key on a Korean keyboard.
- The tray indicator shows `한` when Korean is active and `A` for English.
- Hanja conversion: **F9** (or the 한자 key) while a Korean syllable is selected.

## Updates and rollback

You do not have to do anything. Once a day the device checks for a new OS image,
downloads it in the background and prepares it; the next time you restart, you boot into
the new version. Flatpak apps and the Redrob apps update on their own schedule.

If a new version fails its boot health check, the device goes back to the previous
version by itself, and Settings → Updates says so.

To go back on purpose: restart, and at the **boot menu** (hold **Shift**, or press
**Esc** repeatedly, as the machine starts if the menu is hidden) pick the entry marked
as the previous version. Both the current and the previous version are always kept; the
one you boot stays until the next update replaces the other.

## Agent

The resident agent runs in the background as a system service, not as a window. You talk
to it through the channels you connect (Slack, Discord, Google) or from your phone after
pairing; see `remote-access.md`. Its tasks run in a sandbox that cannot see your files
outside the task folder or any of your credentials; anything that would send data out
(a message, a push, an email) asks you first.
