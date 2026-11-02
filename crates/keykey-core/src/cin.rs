use std::collections::HashMap;
use std::io::{self, BufRead};
use std::path::Path;

/// Represents a parsed CIN (Character Information / Table) file.
#[derive(Debug, Clone, Default)]
pub struct CinTable {
    pub ename: String,
    pub cname: String,
    pub selkey: String,
    pub endkey: String,
    /// Maps input key (e.g., '1') to display symbol (e.g., "ㄅ")
    pub key_names: HashMap<char, String>,
    /// Maps display symbol (e.g., "ㄅ") to input key (e.g., '1')
    pub symbol_to_key: HashMap<String, char>,
    /// Maps key sequence (e.g., "1j4") to candidates (e.g., ["不", "布", "部"])
    pub char_defs: HashMap<String, Vec<String>>,
}

impl CinTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load and parse a `.cin` file from the given file path.
    /// Automatically detects UTF-8 vs Big5/CP950 encoding.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let bytes = std::fs::read(path)?;
        let text = match String::from_utf8(bytes.clone()) {
            Ok(s) => s,
            Err(_) => {
                let (decoded, _, _) = encoding_rs::BIG5.decode(&bytes);
                decoded.into_owned()
            }
        };
        Self::load_from_reader(text.as_bytes())
    }

    /// Load and parse a `.cin` stream from any reader.
    pub fn load_from_reader<R: BufRead>(reader: R) -> io::Result<Self> {
        let mut table = CinTable::new();
        let mut in_keyname = false;
        let mut in_chardef = false;

        for line_res in reader.lines() {
            let line = line_res?;
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let mut parts = trimmed.split_whitespace();
            let first = parts.next();
            let second = parts.next();

            if first == Some("%keyname") && second == Some("begin") {
                in_keyname = true;
                in_chardef = false;
                continue;
            } else if first == Some("%keyname") && second == Some("end") {
                in_keyname = false;
                continue;
            } else if first == Some("%chardef") && second == Some("begin") {
                in_chardef = true;
                in_keyname = false;
                continue;
            } else if first == Some("%chardef") && second == Some("end") {
                in_chardef = false;
                continue;
            }

            if in_keyname {
                let mut parts = trimmed.split_whitespace();
                if let (Some(k), Some(sym)) = (parts.next(), parts.next()) {
                    if let Some(ch) = k.chars().next() {
                        table.key_names.insert(ch, sym.to_string());
                        table.symbol_to_key.insert(sym.to_string(), ch);
                    }
                }
            } else if in_chardef {
                let mut parts = trimmed.split_whitespace();
                if let (Some(code), Some(char_or_phrase)) = (parts.next(), parts.next()) {
                    let candidates = table.char_defs.entry(code.to_string()).or_default();
                    let candidate_str = char_or_phrase.to_string();
                    if !candidates.contains(&candidate_str) {
                        candidates.push(candidate_str);
                    }
                }
            } else if trimmed.starts_with("%ename") {
                table.ename = trimmed.strip_prefix("%ename").unwrap_or("").trim().to_string();
            } else if trimmed.starts_with("%cname") {
                table.cname = trimmed.strip_prefix("%cname").unwrap_or("").trim().to_string();
            } else if trimmed.starts_with("%selkey") {
                table.selkey = trimmed.strip_prefix("%selkey").unwrap_or("").trim().to_string();
            } else if trimmed.starts_with("%endkey") {
                table.endkey = trimmed.strip_prefix("%endkey").unwrap_or("").trim().to_string();
            }
        }

        if table.selkey.is_empty() {
            table.selkey = "123456789".to_string();
        }

        Ok(table)
    }

    /// Lookup candidates by exact keycode sequence (e.g. "1j4").
    pub fn lookup(&self, code: &str) -> Option<&[String]> {
        self.char_defs.get(code).map(|v| v.as_slice())
    }

    /// Convert input key sequence to display phonetic symbols (e.g., "1j4" -> "ㄅㄨˋ")
    pub fn keys_to_symbols(&self, code: &str) -> String {
        let mut result = String::new();
        for ch in code.chars() {
            if let Some(sym) = self.key_names.get(&ch) {
                result.push_str(sym);
            } else {
                result.push(ch);
            }
        }
        result
    }

    /// Merge another CinTable (e.g., supplementary or user extensions) into this table.
    pub fn merge(&mut self, other: CinTable) {
        for (k, v) in other.key_names {
            self.key_names.entry(k).or_insert(v);
        }
        for (sym, k) in other.symbol_to_key {
            self.symbol_to_key.entry(sym).or_insert(k);
        }
        for (code, candidates) in other.char_defs {
            let list = self.char_defs.entry(code).or_default();
            for c in candidates {
                if !list.contains(&c) {
                    list.push(c);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CIN: &str = r#"
# Test CIN Header
%ename Phonetic
%cname 注音
%selkey 123456789
%endkey 3467
%keyname begin
1 ㄅ
j ㄨ
4 ˋ
%keyname end
%chardef begin
1j4 不
1j4 布
1j4 部
%chardef end
"#;

    const EXTRA_CIN: &str = r#"
%keyname begin
5 ㄓ
%keyname end
%chardef begin
1j4 佈
5 ㄓ
%chardef end
"#;

    #[test]
    fn test_parse_cin() {
        let table = CinTable::load_from_reader(SAMPLE_CIN.as_bytes()).unwrap();
        assert_eq!(table.ename, "Phonetic");
        assert_eq!(table.cname, "注音");
        assert_eq!(table.selkey, "123456789");
        assert_eq!(table.endkey, "3467");
        assert_eq!(table.keys_to_symbols("1j4?"), "ㄅㄨˋ?"); // '?' has no mapping, keeps '?'

        let candidates = table.lookup("1j4").unwrap();
        assert_eq!(candidates, &["不", "布", "部"]);
        assert_eq!(table.lookup("nonexistent"), None);
    }

    #[test]
    fn test_merge_cin() {
        let mut table1 = CinTable::load_from_reader(SAMPLE_CIN.as_bytes()).unwrap();
        let table2 = CinTable::load_from_reader(EXTRA_CIN.as_bytes()).unwrap();

        table1.merge(table2);

        assert_eq!(table1.keys_to_symbols("5"), "ㄓ");
        let candidates = table1.lookup("1j4").unwrap();
        assert_eq!(candidates, &["不", "布", "部", "佈"]);
        assert_eq!(table1.lookup("5").unwrap(), &["ㄓ"]);
    }
}
