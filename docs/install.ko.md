# 레드롭 OS 설치

레드롭 OS는 USB나 디스크에 구워 부팅하는 디스크 이미지입니다. 아직 설치 마법사는
없고, 세 단계입니다.

최신 다운로드: [Releases](https://github.com/redrob-labs/redrob-os/releases/latest).
지금은 x86-64만 지원합니다.

## 1. 내려받기

최신 릴리스에서 두 파일을 받습니다.

- `redrob_generic-x86-64-<버전>.img.xz` — 이미지
- `redrob_generic-x86-64-<버전>.img.xz.sha256` — 체크섬

## 2. 굽기

**쉬운 방법(권장): [balenaEtcher](https://etcher.balena.io)**

1. Etcher를 열고 **Flash from file**로 `.img.xz`를 고릅니다(압축 풀 필요 없음).
2. **Select target**으로 USB·디스크를 고릅니다.
3. **Flash**를 누릅니다.

Etcher가 쓴 결과를 알아서 검증합니다.

**명령줄(Linux/macOS)**

먼저 대상 장치를 확인하고(Linux `lsblk`, macOS `diskutil list`):

```sh
# 받은 파일이 체크섬과 맞는지 확인
sha256sum -c redrob_generic-x86-64-<버전>.img.xz.sha256

# 굽기 — /dev/sdX 를 본인 장치로 바꾸고, 꼭 다시 확인
xz -dc redrob_generic-x86-64-<버전>.img.xz | sudo dd of=/dev/sdX bs=4M status=progress oflag=sync
```

> **주의.** `dd`는 대상 장치를 통째로 지우고 되돌릴 수 없습니다. 장치를 잘못 고르면
> 그 디스크가 사라집니다. 실행 전 `lsblk`로 `/dev/sdX`를 반드시 확인하세요.

## 3. 부팅

구운 드라이브를 기기에 꽂고 전원을 켭니다. 필요하면 부팅 메뉴에서 그 드라이브를
선택합니다.

첫 부팅은 알아서 설정됩니다 — 기기 ID, 서명 키, 호스트명 `redrob-<id6>`. 입력할
것은 없습니다.

계정에 연결하려면 화면에 뜨는 QR·페어링 코드로 폰을 연결합니다. 화면 없는 기기는
같은 코드가 시리얼 콘솔에 찍힙니다.

## 설치 이후

- 업데이트는 OTA(A/B 슬롯 + 자동 롤백)로 됩니다 — 다시 구울 필요 없음.
- English: [install.md](./install.md).
