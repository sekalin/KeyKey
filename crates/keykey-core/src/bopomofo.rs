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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syllable_construction() {
        let mut syl = SyllableBuffer::new();
        assert!(syl.feed_key('1').unwrap()); // ㄅ
        assert!(syl.feed_key('j').unwrap()); // ㄨ
        assert_eq!(syl.to_cin_code(), "1j");
        assert!(!syl.is_complete());

        assert!(syl.feed_key('4').unwrap()); // ˋ (4th tone)
        assert!(syl.is_complete());
        assert_eq!(syl.to_cin_code(), "1j4");
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
        syl.feed_key('1').unwrap();
        syl.feed_key('j').unwrap();
        assert_eq!(syl.to_cin_code(), "1j");

        syl.backspace();
        assert_eq!(syl.to_cin_code(), "1");
        syl.backspace();
        assert!(syl.is_empty());
    }
}
