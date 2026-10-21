# The desktop

A tour of what you see after installing Redrob OS. The desktop is **redrob-comp**,
Redrob's own Wayland compositor and shell. It can look and behave like a Mac or like
Windows; pick one at first login or in Redrob Settings.

## Login

The login screen shows the clock and your account. Type the password and press Enter.
With more than one account, **Tab** (or the arrow keys) picks the next one.

## Mac layout

- **Menu bar** (top): the Redrob menu, the app in front, status icons (network,
  sound, battery) and the clock.
- **Dock** (bottom, floating): pinned and running apps, the app finder and Settings.
- Window buttons are the coloured dots on the left of the title bar.

## Windows layout

- **Taskbar** (bottom): Start button, search box, pinned and running apps, status
  icons and the clock.
- Window buttons (`─ □ ✕`) are on the right of the title bar.

## Shortcuts

| | Mac layout | Windows layout |
|---|---|---|
| Find an app | ⌘ Space | Win, Win+S |
| Switch windows | ⌘ Tab | Alt+Tab |
| All windows | Ctrl+↑, F3 | Win+Tab |
| Close / minimise | ⌘Q, ⌘W / ⌘M | Alt+F4 / Win+M |
| Maximise, snap | Ctrl+⌘F, Ctrl+⌘←/→ | Win+↑, Win+←/→ |
| Lock | Ctrl+⌘Q | Win+L |
| Screenshot: screen / area / window | ⌘⇧3 / ⌘⇧4 / ⌘⇧4 then Space | Print / Win+Shift+S / Alt+Print |
| Settings | ⌘, | Win+I |
| Terminal | Ctrl+Alt+T | Ctrl+Alt+T |

Every shortcut can be changed in Settings → 단축키 (press the new keys).
Screenshots are saved to `Pictures/Screenshots` and copied to the clipboard.

## Redrob Settings

| Page | What it does |
|---|---|
| 화면 구성 | Mac or Windows layout |
| 테마 | dark or light (apps follow) |
| 단축키 | change or reset shortcuts |
| 입력 | Korean input settings |
| 소리 | output and input volume, mute, which device |
| 디스플레이 | resolution (asks to keep it, reverts after 15 s), scale 100–200%; every connected screen is used, side by side |

## Apps

Ptyxis (terminal), Files, Text Editor, Image Viewer, Document Viewer and Calculator are
installed. **Software** installs anything else from Flathub (browsers, office, media,
games, developer tools); installed apps update themselves in the background.

## Redrob Launcher

Starts at login. It installs and updates the Redrob apps —
**Redrob Browser**, **Redrob Office**, **Redrob Canvas**, **Redrob Code** — in your home
directory and puts them in the start menu. The Launcher is also where you open the
**pairing QR** for your phone and see whether the agent is running. Closing the Launcher
window leaves it in the tray; the agent keeps running regardless.

## Korean input

Korean (Hangul) input is Fcitx 5, started with the session, for every account. No
setup step, whatever language was chosen at install time.

- Toggle Korean/English: **right Alt** (한/영), **Shift+Space** or **Ctrl+Space**.
- Hanja: **right Ctrl** (한자) after a syllable; the candidates show under the cursor.

## Redrob Agent sidebar

Find **Redrob Agent** in the app finder: the agent's dashboard docks on the right edge
of the screen (Redrob Browser in app mode). Open it again to close it.

## Hardware, codecs, phone

Things a PC user expects that stock Fedora leaves out are in the image:

- Video: the patented codecs (libavcodec-freeworld), so H.264/HEVC/AAC
  play in browsers and mpv; hardware decode on Intel and NVIDIA (AMD decodes
  patented formats in software for now).
- NVIDIA: the proprietary driver is built in (x86-64). Nothing to install. With
  Secure Boot ON the module is not signed and the card falls back to the open
  driver; turn Secure Boot off in firmware or accept the fallback.
- Fingerprint: `fprintd` is installed (enrol with `fprintd-enroll`).
- Phone: pair it with the agent; the pairing code shows as a notification after
  login until the device is paired.

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
