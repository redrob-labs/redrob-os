# Redrob OS

라즈베리 파이 5용 에이전트 OS입니다. 상주 에이전트가 사용자를 대신해 샌드박스 안에서
작업(코딩 포함)을 수행하고, Slack, Discord, Google 같은 채널을 연결합니다. GPU/가속기,
디스플레이, 키보드, USB는 붙였다 뗄 수 있는 모듈입니다.

English: [README.md](./README.md)

모노레포 구성:

| 경로 | 역할 | 상류 | 라이선스 |
|---|---|---|---|
| `os/` | Buildroot 기반 읽기 전용 OS, A/B OTA, 모듈(애드온) 구조 | [home-assistant/operating-system](https://github.com/home-assistant/operating-system) (`git subtree`) | Apache-2.0 |
| `agent/` | 상주 에이전트 런타임, 채널, 크론, 시크릿 | [zeroclaw-labs/zeroclaw](https://github.com/zeroclaw-labs/zeroclaw) (`git subtree`) | MIT OR Apache-2.0 (Apache-2.0 선택) |
| `modules/` | display, usb-broker, credential-broker, local-inference | 신규 | Apache-2.0 |
| `docs/` | 요구사항, 결정 기록, 테스트 계획 | 신규 | Apache-2.0 |

Redrob Code(샌드박스 안에서 도는 코딩 툴)는 별도 저장소
[redrob-labs/redrob-code](https://github.com/redrob-labs/redrob-code) 에 그대로 둡니다.

## 상류 추적

각 외부 트리는 전체 히스토리를 가진 `git subtree` 입니다. 원격:

```sh
git remote add upstream-os    https://github.com/home-assistant/operating-system.git
git remote add upstream-agent https://github.com/zeroclaw-labs/zeroclaw.git
```

상류 변경 가져오기:

```sh
git fetch upstream-os    && git subtree pull --prefix=os    upstream-os    dev
git fetch upstream-agent && git subtree pull --prefix=agent upstream-agent master
```

`os/buildroot` 는 서브모듈입니다(루트 `.gitmodules` 에 선언):

```sh
git submodule update --init --depth 1 os/buildroot
```

현재 상류 지점은 [UPSTREAM.md](./UPSTREAM.md) 에 기록합니다.

## 라이선스

Apache-2.0. `LICENSE` 와 `NOTICE` 를 보십시오. 서드파티 고지는 `NOTICE` 에 있습니다.
