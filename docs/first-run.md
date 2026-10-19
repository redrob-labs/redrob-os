# First run

What happens between the end of the installer and a working desktop, and the
three things the welcome dialog offers. 한국어 설명은 각 절 끝에 있습니다.

## Boot and login

- **First boot** takes a little longer than usual (about a minute). Redrob OS
  creates the device identity, its signing key and the host name `redrob-<id>`,
  then shows the login screen. Nothing to type.
- On a **Raspberry Pi** (raw image) a first-boot wizard runs *before* the login
  screen: language, keyboard, time zone, and your first user account. The
  installer ISO already asked these, so PC installs skip it.
- Log in with the user you created. The desktop is a bottom taskbar with a
  start menu on the left, system tray and clock on the right. Double-click
  opens files; the **Meta** (Windows) key opens the start menu; `Meta+E`
  opens the file manager; `Meta+L` locks the screen.

> 첫 부팅은 1분쯤 걸립니다(기기 ID·서명 키·호스트 이름 생성). 파이에서는 로그인
> 전에 첫 부팅 마법사가 언어·키보드·시간대·사용자를 묻습니다. 바탕화면은 Windows와
> 비슷합니다: 아래 작업 표시줄, 왼쪽 시작 메뉴, 더블 클릭으로 열기, **Meta**(윈도)
> 키로 시작 메뉴.

## The welcome dialog

Right after the first login a small window appears:

<!-- TODO screenshot: welcome dialog (en) / 환영 창 (ko) -->

| Button | What it does |
|--------|--------------|
| **Connect phone** / 폰 연결 | Shows the QR code and pairing code for the Redrob phone app (see below). |
| **Install apps** / 앱 설치 | Opens **Redrob Launcher** (see below). |
| **Later** / 나중에 | Closes the dialog. |

It shows once per user. Both actions stay available in *Start menu → Redrob*.
The dialog follows the language you picked in the installer; to see it again,
delete `~/.config/redrob/welcomed` and log in again.

> 첫 로그인 직후 작은 환영 창이 뜹니다. **폰 연결**은 QR·페어링 코드를, **앱 설치**는
> Redrob Launcher를 열고, **나중에**는 닫습니다. 사용자마다 한 번만 보이고, 두 기능은
> *시작 메뉴 → Redrob*에 그대로 있습니다. 다시 보려면 `~/.config/redrob/welcomed`를
> 지우고 다시 로그인하세요.

## Pairing a phone

Pairing links this computer to your Redrob account so the agent can be reached
from the phone app and from the channels you connect (Slack, Discord, Google).

1. Press **Connect phone** (or *Start menu → Redrob → Connect phone*). A window
   shows a QR code and a short one-time code.
2. In the Redrob phone app choose *Add device* and scan the QR, or type the
   code. The code changes every few minutes; if it expires, open the window
   again.
3. The phone confirms the device name `redrob-<id>`. Done — the agent now
   accepts tasks from your account.

On a headless machine the same code is printed on the serial console and sent
over any channel the agent is already connected to. Remote access from outside
your network goes through the relay: [remote-access.md](./remote-access.md).

> **폰 연결**(페어링)은 이 컴퓨터를 내 레드롭 계정에 묶어 폰 앱과 연결한 채널(Slack,
> Discord, Google)에서 에이전트를 쓰게 합니다. 폰 앱의 *기기 추가*에서 QR을 찍거나
> 코드를 입력하면 끝입니다. 코드는 몇 분마다 바뀌니 만료되면 창을 다시 엽니다.

## Redrob Launcher

**Redrob Launcher** is where Redrob's own apps come from — **Redrob Browser**,
**Redrob Office**, **Redrob Canvas**, **Redrob Code**. It starts with the
desktop (tray icon) and also lives in *Start menu → Redrob*.

- Pick an app → **Install**. Apps install into your home folder, so no
  password is asked and each user has their own copy.
- Installed apps appear in the start menu like any other program.
- Launcher keeps its apps up to date on its own.

> **Redrob Launcher**에서 Redrob Browser·Office·Canvas·Code를 설치합니다. 홈 폴더에
> 설치되므로 비밀번호를 묻지 않고, 설치된 앱은 시작 메뉴에 나옵니다. 업데이트는 자동입니다.

## Everything else: Discover

For all other software — browsers, games, office suites, media players — open
**Discover** (*Start menu → Discover*, the shopping-bag icon). It installs
Flatpak apps from Flathub; search, press *Install*, done. Those apps update in
the background too.

System updates do not appear in Discover: they are automatic. If you want to
see them, *Start menu → System Settings → About this System* shows the current
version, and a terminal `bootc status` shows the staged one.

> 그 밖의 프로그램은 **Discover**(시작 메뉴 → Discover)에서 설치합니다. Flathub의
> Flatpak 앱을 검색 → *설치*로 받고, 역시 뒤에서 자동 업데이트됩니다. 시스템 업데이트는
> Discover에 안 나옵니다 — 자동이고, 터미널에서 `bootc status`로 볼 수 있습니다.

## Korean input

Korean (Hangul) input is ready on every account, whatever language you picked in
the installer: press **Hangul** (`한/영`), `Shift+Space` or `Super+Space` to switch
between English and Korean. Nothing to install or enable.

> 설치 언어와 상관없이 모든 계정에서 바로 `한/영`(또는 `Shift+Space`, `Super+Space`)으로
> 한글을 칩니다. 따로 켤 것이 없습니다.
