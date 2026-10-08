# 확인 필요 사항 조사 결과 (요구사항 9절)

조사일 2026-10-08. L0~L5 개발 머신 사실은 레포에서 직접 확인했고, 외부 사실은
공개 1차 출처로 확정했다. 각 항목에 근거를 단다.

---

## Q1. HAOS 원본 레포 라이선스와 브랜딩/상표 제약

**결론: Apache-2.0. 코드 복제·수정·재배포·판매 모두 허용. 이름만 못 쓴다.**

- `operating-system` 레포 `LICENSE`는 Apache License 2.0, Copyright 2017 Pascal
  Vizeli. (출처: github.com/home-assistant/operating-system `dev` 브랜치 LICENSE)
- 지켜야 할 조건:
  - §4(b): 수정한 파일에 "변경했다"는 고지를 남긴다. (우리 외부 트리 수정에 커밋
    메시지로 기록 중)
  - §4(c): Source 배포 시 원본의 저작권·특허·**상표**·귀속 고지를 유지한다.
  - §4(d): NOTICE 파일이 있으면 파생물에도 읽을 수 있게 포함한다. (우리 `NOTICE`에
    기록 완료)
  - §6 Trademarks: 라이선스는 Licensor의 **상표/상호/제품명 사용 권한을 주지 않는다**.
    즉 "Home Assistant" 이름·로고를 제품명으로 쓸 수 없다. 우리는 전부 Redrob로
    리브랜딩하므로 해당 없음(스플래시/호스트명/os-release 이미 Redrob).
- 함의: 포크·판매에 라이선스 장벽 없음. 할 일은 NOTICE 귀속 유지와 리브랜딩 철저.

## Q2. 파이5 커널 16K 페이지와 gVisor, arm64 바이너리 호환 — **치명적 제약**

**결론: gVisor(runsc)는 arm64에서 4K 페이지만 지원한다. 파이5 기본 커널(16K)에서
F4 샌드박스가 뜨지 않는다. 해결책이 F6과 상충하므로 설계 결정이 필요하다.**

- gVisor는 x86_64/ARM64 지원, Linux 5.6+ 필요. (출처: gvisor.dev/docs/user_guide/install)
- **그러나 arm64에서 16K 페이지면 설치부터 패닉**: `runsc install -> panic: Only 4K
  page size is supported on arm64!` (출처: github.com/google/gvisor issue #8196)
- 파이5는 기본 16K 페이지(`getconf PAGESIZE = 16384`). (출처: raspberrypi 커널,
  anthropics/claude-code issue #9462, hangover wiki Page-Size)
- 4K로 되돌리는 공식 방법: `config.txt`에 `kernel=kernel8` 지정 → 4K 페이지 커널로
  부팅. (출처: raspiblitz issue #4346, hangover wiki)
- **상충**: 16K 페이지는 메모리·I/O 성능 이점이 있고 일부 arm64 바이너리(jemalloc을
  16K로 빌드한 ripgrep 등)는 16K를 전제한다. 4K로 내리면 그런 바이너리는 반대로
  "host page size does not match compiled page size" 류 문제가 날 수 있다.
  (출처: claude-code #9462의 jemalloc 사례)

**설계 결정 (이 조사로 내가 정함, 되돌릴 수 있음):**
파이5 이미지는 **4K 페이지 커널(`kernel=kernel8`)로 고정**한다. 이유: F4 샌드박스
(gVisor)가 제품의 핵심 안전장치이고 16K에선 아예 못 뜬다. 성능 이점보다 샌드박스
동작이 우선. 대신 이미지에 들어가는 모든 arm64 사용자 바이너리(redrob-code,
redrob-agent, llama.cpp)는 4K 페이지 전제로 빌드/검증한다. 실기기(L4) 단계에서
`getconf PAGESIZE`가 4096인지, gVisor `runsc install`이 통과하는지 가장 먼저 측정한다.
→ 사용자 확인 필요: 16K 성능을 포기하는 트레이드오프에 동의하는지.

## Q3. AI HAT+ 2(Hailo)에 자체 모델을 올릴 수 있는지

**결론: 비전/CNN 모델(YOLO 류)은 변환해 올릴 수 있다. 변환 툴체인은 x86 전용이고,
Hailo-8은 LLM 추론 가속기가 아니다. 그래서 F6 라우터 모델은 Hailo로 안 돌린다.**

- 자체 모델 변환 경로는 성립: 커스텀 학습 YOLOv8 → HEF 변환 → 파이5+Hailo 배포
  워크플로가 공개로 다수 존재. (출처: hailo_model_generator, mi0iou gist, RasPi_YOLO)
- **변환은 x86_64 Linux 전용**: Hailo Dataflow Compiler(DFC)는 파이(arm64)에서 못
  돌린다. "The Hailo DFC is only available on x86 platforms"; "full model conversion
  (parsing, quantizing, compiling) is only supported on x86_64 Linux".
  (출처: seapanda0/hailo-yolo-guide, ridgerun.ai Hailo SDK 가이드)
- Hailo-8 26 TOPS / 8L 13 TOPS, 정수 양자화 CNN 가속기. 공개 사례는 전부 비전
  (객체 탐지)이고 생성형 LLM 추론 사례는 없다. (출처: doleron medium)

**함의 (F6 설계에 반영):**
- F6 라우터(1B급 LLM, 인텐트 분류/툴 선택)는 **Hailo로 가속하지 않는다**. 백엔드는
  CPU(llama.cpp) / 원격 GPU 박스(tailnet) 두 경로 유지. "AI HAT+ 2 백엔드 전환"은
  LLM이 아니라 **비전 모듈(local-inference의 비전 서브경로)** 전용으로 범위를 좁힌다.
- Hailo용 모델 변환이 필요하면 개발 머신(x86, 이 호스트가 자격 충족)에서 DFC로 HEF를
  만들어 `/data`에 넣고 OTA와 분리 업데이트. 파이에서는 추론(HailoRT)만 한다.

## Q4. redrob-code의 linux-arm64 빌드와 헤드리스 서버 모드 — **둘 다 지원**

**결론: linux-arm64 바이너리를 실제로 릴리스하고, `redrob serve` 헤드리스 모드가 있다.
파이5(aarch64)에서 샌드박스 안 헤드리스 실행(F3) 가능.**

- linux-arm64 산출물 생성·배포 확인: `packages/redrob/script/publish.ts`가
  `dist/redrob-linux-arm64.tar.gz`를 만들고 sha256을 찍어 GitHub Release에 올리며
  AUR aarch64 소스와 Homebrew url까지 생성한다.
- 헤드리스 서버: `packages/redrob/src/cli/cmd/serve.ts` — `command: "serve"`,
  "starts a headless redrob server", 리스닝 로그 프리픽스 "redrob server listening".
  `attach`/`run --server`로 붙는다. 엔진이 자기 툴을 실행하고 `POST /session/:id
  /message` + `GET /event` 스트림 구조(기존 조사와 일치).
- 함의: F3 그대로 성립. 샌드박스 컨테이너 안에서 `redrob serve`로 띄우고 브로커가
  앞단에서 승인·반출을 게이트한다.

## Q5. zeroclaw 채널 어댑터의 Slack BYO 앱 호환 — **호환**

**결론: Socket Mode 기반 BYO 앱. 사용자가 자기 Slack 앱을 만들어 토큰 2개를 넣는다.
공개 수신 URL이 필요 없어 파이5 헤드리스/NAT 뒤에서도 동작한다.**

- 구조: `agent/crates/zeroclaw-channels/src/slack.rs`의 `SlackChannel`은
  `bot_token: String` + `app_token: Option<String>`. app_token이 있으면 **Socket
  Mode**(아웃바운드 연결)로 동작. (slack.rs L124-126, L1417 `configured_app_token`)
- 셋업 문서 `agent/docs/book/src/channels/slack.md`: 사용자가 (1) 자기 Slack 앱 생성,
  (2) Bot Token Scopes 부여, (3) Socket Mode 켜고 `xapp-` app-level 토큰 생성,
  (4) 설치 후 `xoxb-` bot 토큰 복사 — 정확히 "BYO 앱" 흐름.
- 함의: 요구사항 F2의 "Slack(BYO 앱 구조와 호환)" 충족. Socket Mode라 인바운드 웹훅
  공개 엔드포인트 불필요 → 가정용 기기에 적합. 토큰은 F5 크리덴셜 브로커가 보관.

## Q6. 개발 레포를 redrob-labs private로 둘지, 일부 공개할지

**결론 (내가 1단계 기준으로 정함, 되돌릴 수 있음): 전부 redrob-labs **private** 유지.**

- 현재 `redrob-labs/redrob-os`는 private 모노레포(Stage 1). 그대로 둔다.
- 근거: (1) 1단계는 개발·테스트 단계라 공개 이득이 없다. (2) HAOS·zeroclaw 상류가
  Apache-2.0이라 **지금 공개 의무는 없다** — Apache는 배포 시점에만 소스 제공·귀속
  의무가 생기고, 사내 사용/비배포에는 공개 의무가 없다. (3) 판매 결정 전까지
  브랜딩·구성이 유동적이다.
- 판매·배포 단계에서 바뀌는 것: 바이너리를 외부에 배포하는 순간 Apache §4가 발동하므로
  그 시점에 NOTICE·변경고지·(요청 시)대응 소스 제공 절차를 갖춰야 한다. 지금 NOTICE를
  미리 정비해 둔 이유.
- → 사용자 확인 필요: 공개 의도가 있는 하위 컴포넌트(예: redrob-os-modules를 커뮤니티
  모듈로 공개)가 있으면 그것만 분리 공개. 없으면 전부 private 유지.
