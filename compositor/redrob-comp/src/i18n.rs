//! UI language for the shell. Korean is the source text; English is the
//! other language. The choice follows the session's `LANG` (set from the
//! language picked in the installer), falling back to `/etc/locale.conf`.
//! `language = ko|en` in desktop.conf overrides both.

use std::sync::OnceLock;

static KO: OnceLock<bool> = OnceLock::new();

/// Decide once at startup. `pref` is desktop.conf's `language` (or empty).
pub fn init(pref: &str) {
    let _ = KO.set(match pref.trim() {
        "ko" => true,
        "en" => false,
        _ => system_is_ko(),
    });
}

fn system_is_ko() -> bool {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                return v.starts_with("ko");
            }
        }
    }
    // greetd starts the greeter without LANG; read the system default.
    std::fs::read_to_string("/etc/locale.conf")
        .ok()
        .and_then(|t| t.lines().find_map(|l| l.strip_prefix("LANG=").map(|v| v.trim_matches('"').starts_with("ko"))))
        .unwrap_or(true)
}

/// True when the shell speaks Korean.
pub fn ko() -> bool {
    *KO.get_or_init(system_is_ko)
}

/// Korean source text in the user's language.
pub fn tr(ko_text: &'static str) -> &'static str {
    if ko() {
        return ko_text;
    }
    EN.iter().find(|(k, _)| *k == ko_text).map(|(_, e)| *e).unwrap_or(ko_text)
}

/// Short weekday ("화" / "Tue"), Monday = 0.
pub fn weekday_short(i: usize) -> &'static str {
    if ko() { ["월", "화", "수", "목", "금", "토", "일"][i] } else { ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"][i] }
}

/// Long weekday ("화요일" / "Tuesday"), Monday = 0.
pub fn weekday_long(i: usize) -> &'static str {
    if ko() {
        ["월요일", "화요일", "수요일", "목요일", "금요일", "토요일", "일요일"][i]
    } else {
        ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"][i]
    }
}

/// Month name for English dates (1 = January).
pub fn month_en(m: u32, long: bool) -> &'static str {
    const L: [&str; 12] =
        ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let n = L[(m.clamp(1, 12) - 1) as usize];
    if long { n } else { &n[..3] }
}

const EN: &[(&str, &str)] = &[
    // Bar, dock, taskbar
    ("검색", "Search"),
    ("파일", "Files"),
    ("터미널", "Terminal"),
    ("설정", "Settings"),
    ("레드롭", "Redrob"),
    // Shortcut actions
    ("창 닫기", "Close window"),
    ("창 전환", "Switch windows"),
    ("창 최소화", "Minimize window"),
    ("모든 창 보기", "Show all windows"),
    ("화면 잠금", "Lock screen"),
    ("스크린샷 (전체)", "Screenshot (full)"),
    ("스크린샷 (영역)", "Screenshot (area)"),
    ("스크린샷 (창)", "Screenshot (window)"),
    ("앱 찾기", "Find apps"),
    ("바탕화면 보기", "Show desktop"),
    ("최대화", "Maximize"),
    ("원래 크기", "Restore size"),
    ("왼쪽 반에 붙이기", "Snap to left half"),
    ("오른쪽 반에 붙이기", "Snap to right half"),
    ("터미널 열기", "Open terminal"),
    ("파일 열기", "Open files"),
    ("브라우저 열기", "Open browser"),
    ("설정 열기", "Open settings"),
    ("라이트/다크 전환", "Switch light/dark"),
    // Overlays
    ("이 해상도를 유지할까요?", "Keep this resolution?"),
    ("유지", "Keep"),
    ("되돌리기", "Revert"),
    ("드래그해서 영역을 고르세요 · Esc 취소", "Drag to pick an area · Esc to cancel"),
    ("레드롭 검색", "Redrob Search"),
    ("앱, 설정 검색", "Search apps and settings"),
    ("결과 없음", "No results"),
    ("모든 앱", "All apps"),
    ("앱", "Apps"),
    ("↵ 열기", "↵ Open"),
    ("확인 중…", "Checking…"),
    ("암호 입력", "Enter password"),
    ("Tab 키로 사용자 바꾸기", "Press Tab to switch user"),
    ("암호가 맞지 않아요", "Wrong password"),
    ("지금", "now"),
    ("스크린샷을 저장했어요", "Screenshot saved"),
    ("스크린샷을 찍지 못했어요", "Couldn't take a screenshot"),
    // Settings
    ("화면 구성", "Layout"),
    ("테마", "Theme"),
    ("단축키", "Shortcuts"),
    ("입력", "Input"),
    ("소리", "Sound"),
    ("디스플레이", "Display"),
    ("익숙한 배치와 단축키를 고르세요. 바로 바뀌어요.", "Pick a familiar layout and shortcuts. It changes right away."),
    ("맥 스타일", "Mac style"),
    ("위 메뉴 바 · 떠 있는 도크 · ⌘ 단축키", "Top menu bar · floating dock · ⌘ shortcuts"),
    ("윈도우 스타일", "Windows style"),
    ("아래 작업 표시줄 · 시작 버튼 · Win 단축키", "Bottom taskbar · Start button · Win shortcuts"),
    ("앱 창까지 함께 바뀌어요.", "Open app windows change too."),
    ("라이트", "Light"),
    ("다크", "Dark"),
    ("바꿀 항목을 누른 뒤 새 키를 누르세요.", "Click an item, then press the new key."),
    ("없음", "None"),
    ("기본값으로", "Reset to defaults"),
    ("한/영 전환", "Korean/English"),
    ("한/영 키 · 오른쪽 Alt · Shift+Space", "Han/Eng key · Right Alt · Shift+Space"),
    ("한자 변환", "Hanja"),
    ("한자 키 · 오른쪽 Ctrl", "Hanja key · Right Ctrl"),
    ("입력기", "Input method"),
    ("Fcitx 5 · 한국어 두벌식", "Fcitx 5 · Korean Dubeolsik"),
    ("입력기 세부 설정", "Input method settings"),
    ("출력", "Output"),
    ("음소거 해제", "Unmute"),
    ("음소거", "Mute"),
    ("장치 없음", "No devices"),
    ("해상도", "Resolution"),
    ("이 화면에서는 바꿀 수 없어요", "Can't be changed on this screen"),
    ("배율", "Scale"),
];
