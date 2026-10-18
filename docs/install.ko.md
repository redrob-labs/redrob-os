# Redrob OS 설치

Redrob OS는 PC·노트북(x86-64)용 설치 ISO와 라즈베리 파이 5(aarch64)용 raw 디스크
이미지로 배포됩니다. 둘 다 최신
[릴리즈](https://github.com/redrob-labs/redrob-os/releases/latest)에 있고, 파일마다
`.sha256` 체크섬이 옆에 있습니다.

## PC 또는 노트북

### 1. 다운로드

- `redrob-os-<version>-x86_64.iso` — 설치 프로그램
- `redrob-os-<version>-x86_64.iso.sha256` — 체크섬

### 2. USB 메모리에 쓰기 (4 GB 이상)

**쉬운 방법(권장): [balenaEtcher](https://etcher.balena.io)** — Flash from file에서
`.iso` 선택, 대상 USB 선택, Flash. Etcher가 쓰기를 검증해 줍니다.

**명령줄(Linux/macOS)** — 먼저 USB 장치 이름을 확인하고(`lsblk` 또는 `diskutil list`):

```sh
sha256sum -c redrob-os-<version>-x86_64.iso.sha256
# /dev/sdX 를 반드시 자기 USB 장치로 바꾸고 다시 확인
sudo dd if=redrob-os-<version>-x86_64.iso of=/dev/sdX bs=4M status=progress oflag=sync
```

> **주의.** `dd`는 대상 장치 전체를 되돌릴 수 없게 지웁니다. 실행 전 `lsblk`로
> `/dev/sdX`를 확인하십시오.

### 3. USB로 부팅해 설치

USB로 부팅합니다(전원을 켠 직후 F12, F2, Esc, Del 중 하나로 펌웨어 부팅 메뉴). 안내형
설치 프로그램이 다음을 묻습니다.

- **언어와 키보드** (한국어 선택 가능. 그러면 시스템이 한국어로 부팅됩니다.)
- **설치 위치.** 디스크를 고릅니다. Windows를 남기려면 *함께 설치*를 고르면 Windows
  파티션을 줄이고 부팅 메뉴를 만들어 줍니다. 디스크를 통째로 쓰려면 *디스크 전체 사용*.
  Secure Boot는 켜 둔 채로 됩니다.
- **디스크 암호화** (선택, 노트북이면 권장). 부팅마다 암호를 입력합니다.
- **사용자 계정** — 이름, 비밀번호. 이 계정이 관리자입니다.

설치를 누르고 기다렸다가 USB를 빼고 재부팅합니다. 그 밖에 설정할 것은 없습니다.

## 라즈베리 파이 5

`redrob-os-<version>-aarch64.raw.xz`와 `.sha256`을 받아 balenaEtcher로 microSD 또는
USB/NVMe 드라이브에 쓰고(`.xz`는 Etcher가 풉니다) 전원을 켭니다. 첫 부팅(사용자 계정,
Wi‑Fi)에는 키보드·마우스·HDMI 디스플레이가 필요합니다. 마지막 파티션은 첫 부팅 때 카드
크기만큼 늘어납니다.

## 첫 부팅

데스크톱이 올라오면 짧은 첫 실행 설정(필요하면 Wi‑Fi, 그다음 Redrob 앱을 홈에 설치하는
Redrob Launcher)이 진행됩니다. 에이전트는 이미 돌고 있습니다. 폰을 페어링하려면
Launcher 또는 시스템 트레이에서 페어링 QR을 열어 스캔하십시오. 화면 없는 기기에서는 같은
코드가 시리얼 콘솔에 출력됩니다.

## 설치 뒤

- 업데이트는 자동입니다. 하루 한 번 받아 두고 다음 재부팅 때 적용하며, 새 버전이 부팅에
  실패하면 기기가 스스로 되돌립니다. [maintenance.md](./maintenance.md) 참고.
- 폰에서 기기 접속: [remote-access.ko.md](./remote-access.ko.md).
- English: [install.md](./install.md).
