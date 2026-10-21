//! Hangul composition for the shell's own text fields (the app finder):
//! the standard two-set (두벌식) layout on a US keyboard, composed into
//! syllables as you type. Apps get Korean through Fcitx 5 instead.

/// Jamo for each key in the two-set layout (shifted letters in upper case).
fn jamo(c: char) -> Option<char> {
    Some(match c {
        'q' => 'ㅂ', 'w' => 'ㅈ', 'e' => 'ㄷ', 'r' => 'ㄱ', 't' => 'ㅅ',
        'y' => 'ㅛ', 'u' => 'ㅕ', 'i' => 'ㅑ', 'o' => 'ㅐ', 'p' => 'ㅔ',
        'a' => 'ㅁ', 's' => 'ㄴ', 'd' => 'ㅇ', 'f' => 'ㄹ', 'g' => 'ㅎ',
        'h' => 'ㅗ', 'j' => 'ㅓ', 'k' => 'ㅏ', 'l' => 'ㅣ',
        'z' => 'ㅋ', 'x' => 'ㅌ', 'c' => 'ㅊ', 'v' => 'ㅍ', 'b' => 'ㅠ', 'n' => 'ㅜ', 'm' => 'ㅡ',
        'Q' => 'ㅃ', 'W' => 'ㅉ', 'E' => 'ㄸ', 'R' => 'ㄲ', 'T' => 'ㅆ', 'O' => 'ㅒ', 'P' => 'ㅖ',
        c if c.is_ascii_uppercase() => return jamo(c.to_ascii_lowercase()),
        _ => return None,
    })
}

const CHO: &str = "ㄱㄲㄴㄷㄸㄹㅁㅂㅃㅅㅆㅇㅈㅉㅊㅋㅌㅍㅎ";
const JUNG: &str = "ㅏㅐㅑㅒㅓㅔㅕㅖㅗㅘㅙㅚㅛㅜㅝㅞㅟㅠㅡㅢㅣ";
const JONG: &str = " ㄱㄲㄳㄴㄵㄶㄷㄹㄺㄻㄼㄽㄾㄿㅀㅁㅂㅄㅅㅆㅇㅈㅊㅋㅌㅍㅎ";

fn idx(set: &str, c: char) -> Option<usize> {
    set.chars().position(|x| x == c)
}

fn is_vowel(c: char) -> bool {
    idx(JUNG, c).is_some()
}

fn join_vowel(a: char, b: char) -> Option<char> {
    Some(match (a, b) {
        ('ㅗ', 'ㅏ') => 'ㅘ', ('ㅗ', 'ㅐ') => 'ㅙ', ('ㅗ', 'ㅣ') => 'ㅚ',
        ('ㅜ', 'ㅓ') => 'ㅝ', ('ㅜ', 'ㅔ') => 'ㅞ', ('ㅜ', 'ㅣ') => 'ㅟ', ('ㅡ', 'ㅣ') => 'ㅢ',
        _ => return None,
    })
}

fn join_final(a: char, b: char) -> Option<char> {
    Some(match (a, b) {
        ('ㄱ', 'ㅅ') => 'ㄳ', ('ㄴ', 'ㅈ') => 'ㄵ', ('ㄴ', 'ㅎ') => 'ㄶ', ('ㄹ', 'ㄱ') => 'ㄺ',
        ('ㄹ', 'ㅁ') => 'ㄻ', ('ㄹ', 'ㅂ') => 'ㄼ', ('ㄹ', 'ㅅ') => 'ㄽ', ('ㄹ', 'ㅌ') => 'ㄾ',
        ('ㄹ', 'ㅍ') => 'ㄿ', ('ㄹ', 'ㅎ') => 'ㅀ', ('ㅂ', 'ㅅ') => 'ㅄ',
        _ => return None,
    })
}

fn split_final(c: char) -> Option<(char, char)> {
    Some(match c {
        'ㄳ' => ('ㄱ', 'ㅅ'), 'ㄵ' => ('ㄴ', 'ㅈ'), 'ㄶ' => ('ㄴ', 'ㅎ'), 'ㄺ' => ('ㄹ', 'ㄱ'),
        'ㄻ' => ('ㄹ', 'ㅁ'), 'ㄼ' => ('ㄹ', 'ㅂ'), 'ㄽ' => ('ㄹ', 'ㅅ'), 'ㄾ' => ('ㄹ', 'ㅌ'),
        'ㄿ' => ('ㄹ', 'ㅍ'), 'ㅀ' => ('ㄹ', 'ㅎ'), 'ㅄ' => ('ㅂ', 'ㅅ'),
        _ => return None,
    })
}

fn compose(cho: char, jung: char, jong: Option<char>) -> char {
    let (l, v) = (idx(CHO, cho).unwrap_or(0), idx(JUNG, jung).unwrap_or(0));
    let t = jong.and_then(|j| idx(JONG, j)).unwrap_or(0);
    char::from_u32(0xAC00 + ((l * 21 + v) * 28 + t) as u32).unwrap_or(cho)
}

/// The syllable being composed.
#[derive(Clone, Debug, Default)]
pub struct Composer {
    cho: Option<char>,
    jung: Option<char>,
    jong: Option<char>,
}

impl Composer {
    /// The in-progress syllable as text (empty when nothing is pending).
    pub fn preedit(&self) -> String {
        match (self.cho, self.jung, self.jong) {
            (Some(c), Some(v), j) => compose(c, v, j).to_string(),
            (Some(c), None, _) => c.to_string(),
            (None, Some(v), _) => v.to_string(),
            _ => String::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.cho.is_none() && self.jung.is_none()
    }

    /// Finish the pending syllable, returning its text.
    pub fn flush(&mut self) -> String {
        let s = self.preedit();
        *self = Self::default();
        s
    }

    /// Feed a key. Returns committed text (possibly empty); `None` when the
    /// key is not a Hangul key (the caller handles it after a flush).
    pub fn feed(&mut self, key: char) -> Option<String> {
        let j = jamo(key)?;
        let mut out = String::new();
        if is_vowel(j) {
            match (self.cho, self.jung, self.jong) {
                (_, Some(v), None) if join_vowel(v, j).is_some() => self.jung = join_vowel(v, j),
                (Some(_), None, None) => self.jung = Some(j),
                // A final consonant moves to start the next syllable.
                (Some(_), Some(_), Some(f)) => {
                    let (keep, moved) = match split_final(f) {
                        Some((a, b)) => (Some(a), b),
                        None => (None, f),
                    };
                    self.jong = keep;
                    out = self.flush();
                    self.cho = Some(moved);
                    self.jung = Some(j);
                }
                _ => {
                    out = self.flush();
                    self.jung = Some(j);
                }
            }
        } else {
            match (self.cho, self.jung, self.jong) {
                (Some(_), Some(_), None) if idx(JONG, j).is_some() => self.jong = Some(j),
                (Some(_), Some(_), Some(f)) if join_final(f, j).is_some() => self.jong = join_final(f, j),
                _ => {
                    out = self.flush();
                    self.cho = Some(j);
                }
            }
        }
        Some(out)
    }

    /// Backspace inside the syllable: true when it removed something.
    pub fn back(&mut self) -> bool {
        if let Some(f) = self.jong {
            self.jong = split_final(f).map(|(a, _)| a);
            return true;
        }
        if self.jung.is_some() {
            self.jung = None;
            return true;
        }
        if self.cho.is_some() {
            self.cho = None;
            return true;
        }
        false
    }
}
