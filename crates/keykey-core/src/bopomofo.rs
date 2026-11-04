/// Bopomofo syllable components
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentType {
    Consonant, // 聲母: ㄅㄆㄇㄈ ㄉㄊㄋㄌ ㄍㄎㄏ ㄐㄑㄒ ㄓㄔㄕㄖ ㄗㄘㄙ
    Medial,    // 介母: ㄧㄨㄩ
    Rhyme,     // 韻母: ㄚㄛㄜㄝ ㄞㄟㄠㄡ ㄢㄣㄤㄥ ㄦ
    Tone,      // 聲調: ˊ(6), ˇ(3), ˋ(4), ˙(7), 空白鍵 (1聲)
    Unknown,
}

/// Standard (大千) layout mapping
pub fn classify_key(ch: char) -> ComponentType {
    match ch {
        '1' | 'q' | 'a' | 'z' | '2' | 'w' | 's' | 'x' | 'e' | 'd' | 'c' | 'r' | 'f' | 'v'
        | '5' | 't' | 'g' | 'b' | 'y' | 'h' | 'n' => ComponentType::Consonant,
        'u' | 'j' | 'm' => ComponentType::Medial,
        '8' | 'i' | 'k' | ',' | '9' | 'o' | 'l' | '.' | '0' | 'p' | ';' | '/' | '-' => {
            ComponentType::Rhyme
        }
        '3' | '4' | '6' | '7' | ' ' => ComponentType::Tone,
        _ => ComponentType::Unknown,
    }
}

/// Single Bopomofo syllable buffer (e.g. ㄅ + ㄨ + ˋ)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyllableBuffer {
    pub consonant: Option<char>,
    pub medial: Option<char>,
    pub rhyme: Option<char>,
    pub tone: Option<char>,
}

impl SyllableBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.consonant.is_none()
            && self.medial.is_none()
            && self.rhyme.is_none()
            && self.tone.is_none()
    }

    pub fn clear(&mut self) {
        self.consonant = None;
        self.medial = None;
        self.rhyme = None;
        self.tone = None;
    }

    /// Feed an input key into the syllable.
    /// Returns true if the key was accepted as part of the syllable.
    /// If a tone is fed or space is pressed, the syllable is considered complete.
    pub fn feed_key(&mut self, ch: char) -> Result<bool, ()> {
        let comp = classify_key(ch);
        match comp {
            ComponentType::Consonant => {
                // If user presses consonant, replace or set
                self.consonant = Some(ch);
                Ok(true)
            }
            ComponentType::Medial => {
                self.medial = Some(ch);
                Ok(true)
            }
            ComponentType::Rhyme => {
                self.rhyme = Some(ch);
                Ok(true)
            }
            ComponentType::Tone => {
                if ch == ' ' {
                    // Space represents tone 1 (no tone suffix in CIN keycode)
                    // If buffer is empty, space shouldn't be consumed as tone
                    if self.is_empty() {
                        return Ok(false);
                    }
                    self.tone = Some(' ');
                } else {
                    self.tone = Some(ch);
                }
                Ok(true)
            }
            ComponentType::Unknown => Ok(false),
        }
    }

    /// Backspace action: removes tone first, then rhyme, then medial, then consonant.
    pub fn backspace(&mut self) -> bool {
        if self.tone.is_some() {
            self.tone = None;
            true
        } else if self.rhyme.is_some() {
            self.rhyme = None;
            true
        } else if self.medial.is_some() {
            self.medial = None;
            true
        } else if self.consonant.is_some() {
            self.consonant = None;
            true
        } else {
            false
        }
    }

    /// Is this syllable complete (i.e. has a tone)?
    pub fn is_complete(&self) -> bool {
        self.tone.is_some()
    }

    /// Produce the lookup code for the CIN table (e.g. "1j4", or "1j" for 1st tone)
    pub fn to_cin_code(&self) -> String {
        let mut code = String::new();
        if let Some(c) = self.consonant {
            code.push(c);
        }
        if let Some(m) = self.medial {
            code.push(m);
        }
        if let Some(r) = self.rhyme {
            code.push(r);
        }
        if let Some(t) = self.tone {
            if t != ' ' {
                // Tone 1 (' ') has no suffix in CIN code
                code.push(t);
            }
        }
        code
    }
}

/// Convert a Bopomofo symbol string (e.g. "ㄐㄧㄝˋ", "ㄅㄨˋ", or multi-syllable "ㄐㄧㄝˋ ㄇㄧㄢˋ")
/// into the corresponding CIN Daqian key sequence (e.g. "ru,4", "1j4", "ru,4au04").
pub fn bopomofo_to_cin_code(bpmf: &str) -> String {
    let mut result = String::new();
    for token in bpmf.split(|c: char| c.is_whitespace() || c == ',' || c == '-') {
        if token.is_empty() {
            continue;
        }
        for ch in token.chars() {
            let key = match ch {
                'ㄅ' => '1', 'ㄆ' => 'q', 'ㄇ' => 'a', 'ㄈ' => 'z',
                'ㄉ' => '2', 'ㄊ' => 'w', 'ㄋ' => 's', 'ㄌ' => 'x',
                'ㄍ' => 'e', 'ㄎ' => 'd', 'ㄏ' => 'c',
                'ㄐ' => 'r', 'ㄑ' => 'f', 'ㄒ' => 'v',
                'ㄓ' => '5', 'ㄔ' => 't', 'ㄕ' => 'g', 'ㄖ' => 'b',
                'ㄗ' => 'y', 'ㄘ' => 'h', 'ㄙ' => 'n',
                'ㄧ' => 'u', 'ㄨ' => 'j', 'ㄩ' => 'm',
                'ㄚ' => '8', 'ㄛ' => 'i', 'ㄜ' => 'k', 'ㄝ' => ',',
                'ㄞ' => '9', 'ㄟ' => 'o', 'ㄠ' => 'l', 'ㄡ' => '.',
                'ㄢ' => '0', 'ㄣ' => 'p', 'ㄤ' => ';', 'ㄥ' => '/',
                'ㄦ' => '-',
                'ˊ' => '6', 'ˇ' => '3', 'ˋ' => '4', '˙' => '7',
                _ => continue,
            };
            result.push(key);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_key_all() {
        // Consonants
        for ch in ['1', 'q', 'a', 'z', '2', 'w', 's', 'x', 'e', 'd', 'c', 'r', 'f', 'v', '5', 't', 'g', 'b', 'y', 'h', 'n'] {
            assert_eq!(classify_key(ch), ComponentType::Consonant);
        }
        // Medials
        for ch in ['u', 'j', 'm'] {
            assert_eq!(classify_key(ch), ComponentType::Medial);
        }
        // Rhymes
        for ch in ['8', 'i', 'k', ',', '9', 'o', 'l', '.', '0', 'p', ';', '/', '-'] {
            assert_eq!(classify_key(ch), ComponentType::Rhyme);
        }
        // Tones
        for ch in ['3', '4', '6', '7', ' '] {
            assert_eq!(classify_key(ch), ComponentType::Tone);
        }
        // Unknown
        for ch in ['!', '@', '#', 'A', 'Z', 'k' /* wait, k is rhyme */, '~'] {
            if ch != 'k' {
                assert_eq!(classify_key(ch), ComponentType::Unknown);
            }
        }
    }

    #[test]
    fn test_syllable_construction() {
        let mut syl = SyllableBuffer::new();
        assert!(syl.is_empty());
        assert!(!syl.is_complete());

        // Unknown key
        assert_eq!(syl.feed_key('!'), Ok(false));

        // Space on empty buffer does not consume
        assert_eq!(syl.feed_key(' '), Ok(false));

        // Consonant replacement
        assert!(syl.feed_key('1').unwrap()); // ㄅ
        assert_eq!(syl.consonant, Some('1'));
        assert!(syl.feed_key('2').unwrap()); // Replace with ㄉ
        assert_eq!(syl.consonant, Some('2'));

        // Medial replacement
        assert!(syl.feed_key('u').unwrap()); // ㄧ
        assert_eq!(syl.medial, Some('u'));
        assert!(syl.feed_key('j').unwrap()); // Replace with ㄨ
        assert_eq!(syl.medial, Some('j'));

        // Rhyme replacement
        assert!(syl.feed_key('8').unwrap()); // ㄚ
        assert_eq!(syl.rhyme, Some('8'));
        assert!(syl.feed_key('i').unwrap()); // Replace with ㄛ
        assert_eq!(syl.rhyme, Some('i'));

        assert!(!syl.is_complete());
        assert_eq!(syl.to_cin_code(), "2ji");

        // Tone 4
        assert!(syl.feed_key('4').unwrap()); // ˋ
        assert!(syl.is_complete());
        assert_eq!(syl.to_cin_code(), "2ji4");

        // Clear
        syl.clear();
        assert!(syl.is_empty());
    }

    #[test]
    fn test_syllable_tone1_space() {
        let mut syl = SyllableBuffer::new();
        syl.feed_key('1').unwrap(); // ㄅ
        syl.feed_key('j').unwrap(); // ㄨ
        syl.feed_key(' ').unwrap(); // Space (1st tone)
        assert!(syl.is_complete());
        assert_eq!(syl.to_cin_code(), "1j"); // tone 1 has no suffix
    }

    #[test]
    fn test_syllable_backspace() {
        let mut syl = SyllableBuffer::new();
        assert!(!syl.backspace()); // Empty backspace returns false

        syl.feed_key('1').unwrap();
        syl.feed_key('u').unwrap();
        syl.feed_key('8').unwrap();
        syl.feed_key('3').unwrap();
        assert_eq!(syl.to_cin_code(), "1u83");

        // Backspace tone
        assert!(syl.backspace());
        assert_eq!(syl.to_cin_code(), "1u8");

        // Backspace rhyme
        assert!(syl.backspace());
        assert_eq!(syl.to_cin_code(), "1u");

        // Backspace medial
        assert!(syl.backspace());
        assert_eq!(syl.to_cin_code(), "1");

        // Backspace consonant
        assert!(syl.backspace());
        assert!(syl.is_empty());
        assert!(!syl.backspace());
    }

    #[test]
    fn test_bopomofo_to_cin_code() {
        assert_eq!(bopomofo_to_cin_code("ㄅㄨˋ"), "1j4");
        assert_eq!(bopomofo_to_cin_code("ㄐㄧㄝˋ ㄇㄧㄢˋ"), "ru,4au04");
        assert_eq!(bopomofo_to_cin_code("ㄕˋ,ㄐㄧㄢˋ"), "g4ru04");
        assert_eq!(bopomofo_to_cin_code("ㄖㄣˊ ㄍㄨㄥ ㄓˋ ㄏㄨㄟˋ"), "bp6ej/54cjo4");
        assert_eq!(bopomofo_to_cin_code("ㄉㄗㄦ?"), "2y-");
        assert_eq!(bopomofo_to_cin_code(""), "");
    }
}
