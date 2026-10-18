# Redrob OS

Redrob OS는 PC·노트북·라즈베리 파이 5용 에이전트 데스크톱 운영체제입니다. 상주
에이전트가 샌드박스 안에서 사용자를 대신해 일하고, 이미 쓰고 있는 채널(Slack, Discord,
Google)에 연결됩니다. 데스크톱은 사용자를 방해하지 않습니다. 스스로 백그라운드에서
업데이트하고, 업데이트가 부팅에 실패하면 스스로 이전 버전으로 되돌아갑니다.

English: [README.md](./README.md)

## 설치

[Releases](https://github.com/redrob-labs/redrob-os/releases/latest)에서 설치 ISO를 받아
USB에 쓰고 부팅한 뒤 안내에 따라 설치합니다(디스크 선택, Windows와 듀얼 부팅, 디스크
암호화, 사용자 계정). 라즈베리 파이 5는 ISO 대신 raw 디스크 이미지를 씁니다.
전체 단계: [docs/install.ko.md](./docs/install.ko.md).

## 데스크톱

익숙한 데스크톱입니다. 작업 표시줄, 시작 메뉴, 시스템 트레이, 설정 앱. 앱은
**Discover** 스토어(Flathub)에서 설치하고 스스로 업데이트됩니다. **Redrob Launcher**가
로그인 때 시작되어 Redrob 앱(Browser, Office, Canvas, Code)을 홈 디렉터리에 설치합니다.
한국어 입력이 기본 내장되어 있습니다(Shift+Space 또는 한/영 키).
둘러보기: [docs/desktop.md](./docs/desktop.md).

## 업데이트와 롤백

OS 전체가 서명된 이미지 하나입니다. 기기는 하루 한 번 새 이미지를 확인해 백그라운드에서
준비해 두고, 다음 재부팅 때 전환합니다. 업데이트 뒤마다 부팅 상태 검사가 돌고, 실패하면
기기가 스스로 이전 이미지로 부팅합니다. 부팅 메뉴에서 이전 버전을 직접 고를 수도
있습니다. 자세히: [docs/maintenance.md](./docs/maintenance.md),
[docs/decisions/0001-bootc-base.md](./docs/decisions/0001-bootc-base.md).

## 에이전트

에이전트는 시스템 서비스로 돌면서 작업(코딩 포함)을 샌드박스 안에서 수행합니다.
샌드박스는 사용자의 자격증명도, 기기의 나머지 부분도 볼 수 없습니다. 첫 부팅 때 폰
페어링용 QR 코드가 뜨고, 그 뒤로는 채널에서 대화합니다. 이미지에 어떻게 들어가는지:
[docs/agent-on-bootc.md](./docs/agent-on-bootc.md). 폰에서 기기 접속(같은 Wi‑Fi, 또는
직접 띄운 릴레이로 어디서든): [docs/remote-access.ko.md](./docs/remote-access.ko.md).

## 모노레포 구성

| 경로 | 역할 |
|---|---|
| `image/` | OS 이미지: `Containerfile`, 베이스 핀(`base.env`), 산출물 핀(`pins.env`), 파일 오버레이(`files/`, `agent/`, `branding/`), 빌드 스크립트, 설치 프로그램 설정(`installer/`) |
| `agent/` | 상주 에이전트 런타임: 채널, 크론, 시크릿, 도구 샌드박스 클라이언트 (외부 트리, 아래 참고) |
| `tools/` | 기기 안의 작은 도우미 (`redrob-pairing`: 첫 부팅 페어링 QR) |
| `modules/` | 선택형 기기 서비스: `credential-broker`, `usb-broker`, `local-inference` |
| `docs/` | 설치·데스크톱 안내, 요구사항, 설계 노트, 결정 기록(ADR), 유지보수 |
| `.github/` | CI: 이미지 빌드·서명·게시, 릴리즈 태그에서 설치 프로그램 빌드, 의존성 범프 |

Redrob Code(샌드박스 안에서 도는 코딩 툴)는 별도 저장소
[redrob-labs/redrob-code](https://github.com/redrob-labs/redrob-code)에 있습니다.

## 상류 추적

상류는 둘이며 각각 한 곳에 핀하고 [UPSTREAM.md](./UPSTREAM.md)와
[`upstream-base.json`](./upstream-base.json)에 기록합니다.

| 대상 | 추적 방식 |
|---|---|
| `agent/` | [zeroclaw-labs/zeroclaw](https://github.com/zeroclaw-labs/zeroclaw)를 전체 히스토리와 함께 `git subtree`로 가져오고 태그 단위로 동기화 |
| 베이스 OS 이미지 | `image/base.env`의 OCI 이미지 참조: [Fedora Kinoite](https://fedoraproject.org/atomic-desktops/kinoite/) (`quay.io/fedora-ostree-desktops/kinoite`). 커널, KDE Plasma 데스크톱, bootc를 공급하며 태그는 Renovate PR이 올립니다 |

그 밖에 이미지에 들어가는 것은 전부 베이스 배포판의 패키지 저장소에서 오며 `NOTICE`에
적혀 있습니다.

## 라이선스

Apache-2.0. `LICENSE`와 `NOTICE`를 보십시오. 서드파티 고지와 상표 관련 문구는 `NOTICE`에
있습니다.
