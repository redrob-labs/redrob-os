# 레드롭 OS 설치

레드롭 OS는 Windows나 macOS처럼 설치합니다. 설치 파일을 받아 USB에 굽고, 그 USB로
부팅해서 몇 개 화면에 답하면 끝입니다. 디스크, Windows와 함께 쓰기(듀얼 부트),
암호화, 사용자 계정은 설치 프로그램이 처리합니다.

최신 다운로드: [redrob-os-linux-x86_64.iso](https://cdn.redrob.ai/os/latest/redrob-os-linux-x86_64.iso)
(파일 하나). 이전 버전과 라즈베리 파이 이미지: [Releases](https://github.com/redrob-labs/redrob-os/releases/latest).

| 대상 | 파일 | 방법 |
|------|------|------|
| PC·노트북(x86-64, UEFI) | `redrob-os-<버전>-linux-x86_64.iso` | 설치 USB(이 문서 1~4절) |
| 라즈베리 파이 5 | `redrob-os-<버전>-linux-aarch64.img.xz` | 원본 이미지(6절) |

요구 사항: 64비트 UEFI PC, RAM 4 GB(8 GB 권장), 빈 디스크 20 GB(64 GB 권장),
4 GB 이상 USB 메모리. 보안 부팅(Secure Boot)은 켜 둔 채로 됩니다.

English: [install.md](./install.md).

## 1. 내려받기

설치 파일은 `https://cdn.redrob.ai/os/latest/redrob-os-linux-x86_64.iso` 에서 파일
하나로 받습니다. 체크섬은 같은 주소 뒤에 `.sha256`을 붙이면 됩니다. 특정 버전은
`https://cdn.redrob.ai/os/<버전>/redrob-os-<버전>-linux-x86_64.iso` 에 있습니다.

같은 ISO가 GitHub 릴리스에도 있지만, GitHub는 릴리스 파일을 2 GiB까지만 받기 때문에
거기서는 `.iso.part0`, `.iso.part1`, ... 로 나뉘어 있습니다. 조각을 모두 한 폴더에
받고 하나로 합칩니다.

```sh
# Linux / macOS
cat redrob-os-<버전>-linux-x86_64.iso.part* > redrob-os-<버전>-linux-x86_64.iso
```

```powershell
# Windows (PowerShell) -- 받은 조각을 모두 적습니다
cmd /c copy /b redrob-os-<버전>-linux-x86_64.iso.part0+redrob-os-<버전>-linux-x86_64.iso.part1+redrob-os-<버전>-linux-x86_64.iso.part2 redrob-os-<버전>-linux-x86_64.iso
```

체크섬 확인은 선택 사항이지만 금방 끝납니다.

```sh
sha256sum -c --ignore-missing SHA256SUMS
```

## 2. USB에 설치 프로그램 굽기

아래 중 하나면 됩니다. USB 내용은 지워집니다.

- **[balenaEtcher](https://etcher.balena.io)** (Windows·macOS·Linux): *Flash from
  file* → `.iso` 선택 → *Select target* → USB 선택 → *Flash*.
- **[Rufus](https://rufus.ie)** (Windows): USB와 `.iso`를 고르고 *GPT*·*UEFI*는 그대로
  두고 *시작*. 묻는 창이 나오면 **DD 이미지 모드**를 고릅니다.
- **[Ventoy](https://www.ventoy.net)**: USB에 Ventoy를 한 번 설치한 뒤 `.iso`를 그냥
  복사해 넣습니다. 설치 프로그램 여러 개를 한 USB에 둘 수 있습니다.

명령줄(Linux/macOS) — 먼저 `lsblk` / `diskutil list`로 장치를 확인하세요. `dd`는
되돌릴 수 없습니다.

```sh
sudo dd if=redrob-os-<버전>-linux-x86_64.iso of=/dev/sdX bs=4M status=progress oflag=sync
```

## 3. USB로 부팅

USB를 꽂고 전원을 켠 뒤 제조사 로고가 보일 때 부팅 메뉴를 엽니다(보통 `F12`,
`F2`, `Esc`, `Del`; Windows에서는 **Shift**를 누른 채 *다시 시작* → *장치 사용*도
됩니다). USB를 고릅니다.

메뉴가 나옵니다: **Install Redrob OS**(기본) 또는 **Test this media & install**.
설치 프로그램은 1분쯤 뒤에 뜹니다.

<!-- TODO 스크린샷: 부팅 메뉴 -->

## 4. 설치 프로그램

화면마다 질문 하나입니다. 언제든 뒤로 갈 수 있고, **설치 시작**을 누르기 전까지
디스크에는 아무것도 쓰지 않습니다.

1. **환영 / 언어.** 언어를 고르고(한국어가 있습니다) *계속*을 누릅니다. 설치
   프로그램과 설치된 시스템이 이 언어를 씁니다.
   <!-- TODO 스크린샷: 언어 화면 -->
2. **설치 요약.** 남은 항목이 모인 허브입니다. 경고 아이콘이 붙은 항목은 아직
   입력이 필요합니다.

   ![설치 요약](./images/installer-summary.png)
3. **키보드**, **시간 및 날짜**는 언어와 네트워크 위치에서 미리 채워집니다. 확인하고
   필요하면 고칩니다.
4. **네트워크 및 호스트 이름.** 유선은 자동으로 연결됩니다. 설치 직후부터 업데이트를
   받으려면 여기서 Wi-Fi에 접속하세요. 호스트 이름은 그대로 두세요 — 첫 부팅 때
   레드롭 OS가 `redrob-<id>`로 정합니다.
5. **설치 대상.** 설치할 디스크를 체크합니다.
   - *자동*(기본 선택): 빈 공간을 쓰고, 부족하면 다음 화면에서 공간을 확보하게
     합니다. 권장.
   - *데이터 암호화*: 디스크 전체 암호화(LUKS2). 부팅할 때마다 묻는 암호문을
     정합니다. 노트북에 권장하며, 잊으면 복구 방법이 없습니다.
   - *사용자 정의*: 직접 파티션 나누기. 아래 *파일 유지하며 재설치* 참고.
   <!-- TODO 스크린샷: 설치 대상 화면 -->
6. **사용자 생성.** 이름, 사용자 이름, 비밀번호. 이 계정은 관리자입니다(소프트웨어
   설치·설정 변경 가능). 별도의 `root` 비밀번호는 없습니다.
7. **설치 시작.** 5~10분 걸립니다. 끝나면 USB가 빠지고 레드롭 OS로 다시 부팅합니다.
   USB를 빼라고 하면 뺍니다.

그다음 일은 [first-run.md](./first-run.md)에 있습니다.

### Windows와 함께 쓰기(듀얼 부트)

레드롭 OS는 Windows와 한 디스크를 나눠 쓸 수 있습니다. 공간을 만드는 방법은 둘입니다.

- **Windows에서 먼저 줄이기(권장).** Windows에서 *디스크 관리*(시작 버튼 오른쪽
  클릭 → 디스크 관리)를 열고 `C:` 볼륨을 오른쪽 클릭 → *볼륨 축소…* 로 최소 20 GB
  (64 GB면 넉넉)를 비웁니다. 그 공간은 *할당되지 않음* 상태로 둡니다. 그다음 설치
  프로그램을 부팅하면 *자동* 파티션이 그 빈 공간을 쓰고 Windows는 건드리지 않습니다.
- **설치 프로그램이 공간 확보.** *자동*을 고른 채 빈 공간이 부족하면 *디스크 공간
  확보* 화면이 나옵니다. Windows 파티션 선택 → *축소* → 슬라이더 조절 → *공간 확보*.
  NTFS를 제자리에서 줄입니다. 그 전에 Windows를 정상 종료해야 하며(최대 절전 아님,
  Windows 전원 옵션에서 *빠른 시작* 끄기), 아니면 파티션이 사용 중으로 보입니다.

설치 뒤 부팅 메뉴에 **Redrob OS**와 **Windows**가 나옵니다. 기본은 레드롭 OS이고
화살표 키로 Windows를 고릅니다. 부팅 순서가 바뀌면 Windows의 BitLocker가 복구 키를
한 번 물을 수 있으니 준비해 두세요([aka.ms/myrecoverykey](https://aka.ms/myrecoverykey)).

### 파일 유지하며 재설치

레드롭 OS는 시스템을 `/`와 `/usr`(읽기 전용, 업데이트마다 통째로 교체)에 두고,
사용자의 것은 전부 `/var`에 둡니다 — 홈 폴더는 `/var/home/<사용자>`, Flatpak 앱은
`/var/lib/flatpak`. 그래서 파일을 유지하는 재설치는 **`/`는 지우고 `/var`는 남기기**
입니다.

1. 설치 대상 → 같은 디스크 체크 → *저장소 구성: 사용자 정의* → *완료*.
2. *수동 파티션 설정*에서 왼쪽 목록의 기존 레드롭 OS 설치를 펼칩니다.
3. `/`(루트) 항목 선택 → **다시 포맷** 체크 → 마운트 지점 `/`.
4. `/boot`, `/boot/efi` 선택 → 둘 다 **다시 포맷**.
5. `/var` 선택 → **다시 포맷은 체크하지 않음** → 마운트 지점 `/var`. 기존 파일
   시스템이라는 경고가 나오면 수락합니다.
6. *완료* → *변경 사항 적용*.

사용자 생성에서 *같은 사용자 이름*을 쓰면 예전 홈 폴더를 그대로 잇습니다. 설정, 문서,
Launcher 앱, Flatpak이 살아남습니다. 예전 시스템의 `/etc`에 있던 것(예: Wi-Fi
비밀번호)은 초기화됩니다.

예전 설치가 암호화돼 있었다면 *수동 파티션 설정*에서 펼칠 때 암호문을 묻습니다.

## 5. 첫 부팅 뒤

- **업데이트는 자동입니다.** 레드롭 OS는 매일 확인해서 다음 버전을 뒤에서 내려받고
  다음 재부팅 때 바꿔 탑니다. 업데이트 창도, 누를 것도 없습니다. Redrob Launcher 앱과
  Discover 앱도 같은 방식으로 업데이트됩니다.
- **업데이트로 망가지지 않습니다.** 모든 업데이트는 완전한 새 시스템 이미지입니다.
  새 버전이 부팅에 실패하면 부팅 시 상태 검사가 알아서 이전 버전으로 되돌립니다.
- **이전 버전을 직접 고르기.** 제조사 로고 직후에 **Shift**를 누르고 있으면(또는
  `Esc`를 여러 번) 부팅 메뉴가 나오고, 거기서 이전 항목을 고릅니다. 그 버전에
  머물려면: `sudo bootc rollback`.

## 6. 라즈베리 파이 5(원본 이미지)

파이에는 설치 프로그램이 없습니다. 완성된 디스크 이미지를 굽고, 첫 부팅 때 계정을
만듭니다.

1. `redrob-os-<버전>-linux-aarch64.img.xz`(와 `SHA256SUMS`)를 받습니다.
2. microSD(A2 등급, 32 GB 이상) 또는 USB/NVMe SSD에 [Raspberry Pi
   Imager](https://www.raspberrypi.com/software/)(*사용자 지정 이미지* → `.img.xz`
   선택)나 balenaEtcher로 굽습니다. `.xz`를 풀 필요는 없습니다.
3. 부팅합니다. 이미지가 카드 크기로 늘어난 뒤 **첫 부팅 마법사**가 언어, 키보드,
   시간대를 묻고 사용자를 만듭니다. 그다음 바탕화면이 뜨고 [환영 창](./first-run.md)이
   나옵니다.

모니터 없는 파이: 페어링 코드는 시리얼 콘솔(디버그 UART, 115200)에도 찍히고 에이전트
채널로도 전달됩니다 — [remote-access.ko.md](./remote-access.ko.md).

## 문제 해결

알려진 겉모습 문제: 설치 중 텍스트 콘솔(Ctrl+Alt+F2)의 인사말은 "Fedora Linux 44"입니다.
설치기 환경을 bootc-image-builder가 Fedora 패키지로 조립하기 때문이며, 설치되는
시스템이 아닙니다. 그래픽 설치기와 설치된 시스템은 Redrob OS로 표시됩니다.


- *부팅 메뉴에 USB가 안 보임.* 다시 굽고(Rufus: DD 모드), UEFI 부팅이 켜져 있는지
  확인합니다. Legacy/CSM 전용 펌웨어는 지원하지 않습니다.
- *"Install Redrob OS" 뒤 검은 화면.* `Esc`를 눌러 메시지를 봅니다. 하이브리드
  그래픽 노트북 일부는 `nomodeset`을 한 번 붙입니다: 부팅 메뉴에서 `e`, `linux` 줄
  끝에 `nomodeset` 추가, `Ctrl-X`.
- *설치 뒤 부팅 메뉴에 Windows가 없음.* 펌웨어 자체 부팅 메뉴(`F12` 등)에서 *Windows
  Boot Manager*를 고릅니다. 레드롭 OS 메뉴의 Windows 항목은 두 시스템이 같은
  디스크에 있을 때 추가되며 업데이트 한 번 뒤에 나타날 수 있습니다.
