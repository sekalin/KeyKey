#[derive(Debug, Clone, Default)]
pub struct CandidateList {
    candidates: Vec<String>,
    page_size: usize,
    current_page: usize,
    selkey: String,
}

impl CandidateList {
    pub fn new(candidates: Vec<String>, selkey: &str) -> Self {
        let selkey_str = if selkey.is_empty() {
            "123456789".to_string()
        } else {
            selkey.to_string()
        };
        let page_size = selkey_str.len();
        Self {
            candidates,
            page_size,
            current_page: 0,
            selkey: selkey_str,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    pub fn total_candidates(&self) -> usize {
        self.candidates.len()
    }

    pub fn total_pages(&self) -> usize {
        if self.candidates.is_empty() {
            0
        } else {
            (self.candidates.len() + self.page_size - 1) / self.page_size
        }
    }

    pub fn current_page(&self) -> usize {
        self.current_page
    }

    pub fn next_page(&mut self) -> bool {
        let pages = self.total_pages();
        if self.current_page + 1 < pages {
            self.current_page += 1;
            true
        } else {
            false
        }
    }

    pub fn prev_page(&mut self) -> bool {
        if self.current_page > 0 {
            self.current_page -= 1;
            true
        } else {
            false
        }
    }

    /// Returns a list of (SelectionKey, CandidateString) for current page
    pub fn current_page_items(&self) -> Vec<(char, &str)> {
        let start = self.current_page * self.page_size;
        let end = (start + self.page_size).min(self.candidates.len());
        let selkeys: Vec<char> = self.selkey.chars().collect();

        self.candidates[start..end]
            .iter()
            .enumerate()
            .map(|(i, s)| (selkeys.get(i).copied().unwrap_or('?'), s.as_str()))
            .collect()
    }

    /// Select candidate by selection key (e.g. '1', '2', ...)
    pub fn select_by_key(&self, key: char) -> Option<&str> {
        if let Some(pos) = self.selkey.find(key) {
            let global_index = self.current_page * self.page_size + pos;
            self.candidates.get(global_index).map(|s| s.as_str())
        } else {
            None
        }
    }

    /// Select candidate by page index (0 to page_size - 1)
    pub fn select_by_index(&self, index: usize) -> Option<&str> {
        let global_index = self.current_page * self.page_size + index;
        self.candidates.get(global_index).map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candidate_pagination() {
        let items: Vec<String> = (1..=20).map(|i| format!("候選_{}", i)).collect();
        let mut list = CandidateList::new(items, "123456789");

        assert_eq!(list.total_pages(), 3);
        assert_eq!(list.current_page(), 0);

        let p1 = list.current_page_items();
        assert_eq!(p1.len(), 9);
        assert_eq!(p1[0], ('1', "候選_1"));
        assert_eq!(p1[8], ('9', "候選_9"));

        assert_eq!(list.select_by_key('2'), Some("候選_2"));

        assert!(list.next_page());
        assert_eq!(list.current_page(), 1);
        let p2 = list.current_page_items();
        assert_eq!(p2[0], ('1', "候選_10"));

        assert!(list.next_page());
        assert_eq!(list.current_page(), 2);
        let p3 = list.current_page_items();
        assert_eq!(p3.len(), 2); // 20 - 18 = 2 items
        assert_eq!(p3[0], ('1', "候選_19"));
        assert_eq!(p3[1], ('2', "候選_20"));

        assert!(!list.next_page()); // no more pages
        assert!(list.prev_page());
        assert_eq!(list.current_page(), 1);
    }
}
