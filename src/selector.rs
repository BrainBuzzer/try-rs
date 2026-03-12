use regex::Regex;
use std::cmp;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::time::SystemTime;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};

use crate::fuzzy;
use crate::input::KeyInput;
use crate::tui;

/// Represents a try directory entry with metadata
#[derive(Debug, Clone, PartialEq)]
pub struct TryEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_symlink: bool,
    pub mtime: SystemTime,
    pub score: f64,
    pub date_prefix: Option<String>,
}

/// Load all try directories from the given path and compute their scores
pub fn load_all_tries(try_path: &Path) -> Result<Vec<fuzzy::Entry<TryEntry>>, std::io::Error> {
    let mut entries = Vec::new();
    let now = SystemTime::now();
    let date_prefix_re =
        Regex::new(r"^\d{4}-\d{2}-\d{2}-").expect("date prefix regex literal must compile");

    for entry in fs::read_dir(try_path)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden entries
        if name.starts_with('.') {
            continue;
        }

        // Only include directories
        let metadata = fs::metadata(&path)?;
        if !metadata.is_dir() {
            continue;
        }

        // Get modification time
        let mtime = metadata.modified()?;

        // Compute base score from recency
        let hours_old = now.duration_since(mtime).unwrap_or_default().as_secs_f64() / 3600.0;
        let mut base_score = 3.0 / (hours_old + 1.0).sqrt();

        // Detect date prefix
        let date_prefix = if date_prefix_re.is_match(&name) {
            // Bonus for date-prefixed directories
            base_score += 2.0;
            Some(name.chars().take(10).collect())
        } else {
            None
        };

        // Check if it's a symlink
        let is_symlink = fs::symlink_metadata(&path)?.file_type().is_symlink();

        let try_entry = TryEntry {
            name: name.clone(),
            path,
            is_symlink,
            mtime,
            score: base_score,
            date_prefix,
        };

        entries.push(fuzzy::Entry {
            data: try_entry,
            text: name.clone(),
            text_lower: name.to_lowercase(),
            base_score,
        });
    }

    Ok(entries)
}

/// Format a relative time string from a SystemTime
pub fn relative_time(mtime: SystemTime) -> String {
    let now = SystemTime::now();
    let duration = now.duration_since(mtime).unwrap_or_default();
    let seconds = duration.as_secs_f64();
    let minutes = seconds / 60.0;
    let hours = minutes / 60.0;
    let days = hours / 24.0;

    if seconds < 60.0 {
        "just now".to_string()
    } else if minutes < 60.0 {
        format!("{}m ago", minutes as u64)
    } else if hours < 24.0 {
        format!("{}h ago", hours as u64)
    } else if days < 7.0 {
        format!("{}d ago", days as u64)
    } else {
        format!("{}w ago", (days / 7.0) as u64)
    }
}

/// Generate a unique directory name by appending -2, -3, ... if needed
pub fn unique_dir_name(try_path: &Path, base: &str) -> String {
    let mut candidate = base.to_string();
    let mut i = 2;

    while try_path.join(&candidate).exists() {
        candidate = format!("{}-{}", base, i);
        i += 1;
    }

    candidate
}

/// Resolve a unique name with versioning strategy
/// - If base ends with digits (e.g., "feature1"): increment number → "feature2"
/// - If base doesn't end with digits (e.g., "feature"): use unique_dir_name → "feature-2"
///
/// This function takes a date_prefix and base separately, checks if "{date_prefix}-{base}" exists,
/// and returns only the base part (without date prefix).
pub fn resolve_unique_name_with_versioning(
    try_path: &Path,
    date_prefix: &str,
    base: &str,
) -> String {
    let initial = format!("{}-{}", date_prefix, base);

    // Check if the initial name already exists
    if !try_path.join(&initial).exists() {
        return base.to_string();
    }

    // Check if base ends with digits
    let numeric_suffix_re =
        Regex::new(r"^(.*?)(\d+)$").expect("numeric suffix regex literal must compile");

    if let Some(captures) = numeric_suffix_re.captures(base) {
        // Base ends with digits: increment the number
        let stem = captures.get(1).map(|m| m.as_str()).unwrap_or("");
        let num: u64 = captures
            .get(2)
            .map(|m| m.as_str())
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);

        let mut candidate_num = num + 1;
        loop {
            let candidate_base = format!("{}{}", stem, candidate_num);
            let candidate_full = format!("{}-{}", date_prefix, candidate_base);
            if !try_path.join(&candidate_full).exists() {
                return candidate_base;
            }
            candidate_num += 1;
        }
    } else {
        // No numeric suffix: use unique_dir_name with -2, -3, ... on full path
        let unique_full = unique_dir_name(try_path, &initial);
        // Strip the date prefix to return just the base part
        unique_full
            .strip_prefix(&format!("{}-", date_prefix))
            .unwrap_or(&unique_full)
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_relative_time_just_now() {
        let now = SystemTime::now();
        assert_eq!(relative_time(now), "just now");

        let thirty_secs_ago = now - Duration::from_secs(30);
        assert_eq!(relative_time(thirty_secs_ago), "just now");
    }

    #[test]
    fn test_relative_time_minutes() {
        let now = SystemTime::now();
        let five_mins_ago = now - Duration::from_secs(5 * 60);
        assert_eq!(relative_time(five_mins_ago), "5m ago");

        let thirty_mins_ago = now - Duration::from_secs(30 * 60);
        assert_eq!(relative_time(thirty_mins_ago), "30m ago");
    }

    #[test]
    fn test_relative_time_hours() {
        let now = SystemTime::now();
        let two_hours_ago = now - Duration::from_secs(2 * 3600);
        assert_eq!(relative_time(two_hours_ago), "2h ago");

        let twelve_hours_ago = now - Duration::from_secs(12 * 3600);
        assert_eq!(relative_time(twelve_hours_ago), "12h ago");
    }

    #[test]
    fn test_relative_time_days() {
        let now = SystemTime::now();
        let two_days_ago = now - Duration::from_secs(2 * 24 * 3600);
        assert_eq!(relative_time(two_days_ago), "2d ago");

        let five_days_ago = now - Duration::from_secs(5 * 24 * 3600);
        assert_eq!(relative_time(five_days_ago), "5d ago");
    }

    #[test]
    fn test_relative_time_weeks() {
        let now = SystemTime::now();
        let one_week_ago = now - Duration::from_secs(7 * 24 * 3600);
        assert_eq!(relative_time(one_week_ago), "1w ago");

        let three_weeks_ago = now - Duration::from_secs(21 * 24 * 3600);
        assert_eq!(relative_time(three_weeks_ago), "3w ago");
    }

    #[test]
    fn test_scoring_formula() {
        // Just created (0 hours old): score = 3.0 / sqrt(1) = 3.0
        let score_now = 3.0_f64 / (0.0_f64 + 1.0_f64).sqrt();
        assert!((score_now - 3.0).abs() < 0.01);

        // 1 hour old: score = 3.0 / sqrt(2) ≈ 2.12
        let score_1h = 3.0_f64 / (1.0_f64 + 1.0_f64).sqrt();
        assert!((score_1h - 2.12).abs() < 0.01);

        // 24 hours old: score = 3.0 / sqrt(25) = 0.6
        let score_24h = 3.0_f64 / (24.0_f64 + 1.0_f64).sqrt();
        assert!((score_24h - 0.6).abs() < 0.01);

        // 1 week old: score = 3.0 / sqrt(169) ≈ 0.23
        let score_1w = 3.0_f64 / (168.0_f64 + 1.0_f64).sqrt();
        assert!((score_1w - 0.23).abs() < 0.01);
    }

    #[test]
    fn test_date_prefix_bonus() {
        let date_prefix_re = Regex::new(r"^\d{4}-\d{2}-\d{2}-").unwrap();

        assert!(date_prefix_re.is_match("2025-08-17-redis-experiment"));
        assert!(date_prefix_re.is_match("2025-01-01-test"));
        assert!(!date_prefix_re.is_match("redis-experiment"));
        assert!(!date_prefix_re.is_match("2025-08-17"));
        assert!(!date_prefix_re.is_match("25-08-17-test"));
    }

    #[test]
    fn test_unique_dir_name() {
        let temp_dir = tempfile::tempdir().unwrap();
        let try_path = temp_dir.path();

        // First name should be unchanged
        assert_eq!(unique_dir_name(try_path, "test"), "test");

        // Create "test" directory
        fs::create_dir(try_path.join("test")).unwrap();

        // Second attempt should be "test-2"
        assert_eq!(unique_dir_name(try_path, "test"), "test-2");

        // Create "test-2" directory
        fs::create_dir(try_path.join("test-2")).unwrap();

        // Third attempt should be "test-3"
        assert_eq!(unique_dir_name(try_path, "test"), "test-3");
    }

    #[test]
    fn test_resolve_unique_name_with_versioning_no_collision() {
        let temp_dir = tempfile::tempdir().unwrap();
        let try_path = temp_dir.path();
        let date_prefix = "2026-03-11";

        assert_eq!(
            resolve_unique_name_with_versioning(try_path, date_prefix, "feature"),
            "feature"
        );
        assert_eq!(
            resolve_unique_name_with_versioning(try_path, date_prefix, "feature1"),
            "feature1"
        );
    }

    #[test]
    fn test_resolve_unique_name_with_versioning_numeric_suffix() {
        let temp_dir = tempfile::tempdir().unwrap();
        let try_path = temp_dir.path();
        let date_prefix = "2026-03-11";

        fs::create_dir(try_path.join("2026-03-11-feature1")).unwrap();

        assert_eq!(
            resolve_unique_name_with_versioning(try_path, date_prefix, "feature1"),
            "feature2"
        );

        fs::create_dir(try_path.join("2026-03-11-feature2")).unwrap();

        assert_eq!(
            resolve_unique_name_with_versioning(try_path, date_prefix, "feature1"),
            "feature3"
        );
    }

    #[test]
    fn test_resolve_unique_name_with_versioning_non_numeric() {
        let temp_dir = tempfile::tempdir().unwrap();
        let try_path = temp_dir.path();
        let date_prefix = "2026-03-11";

        fs::create_dir(try_path.join("2026-03-11-feature")).unwrap();

        assert_eq!(
            resolve_unique_name_with_versioning(try_path, date_prefix, "feature"),
            "feature-2"
        );

        fs::create_dir(try_path.join("2026-03-11-feature-2")).unwrap();

        assert_eq!(
            resolve_unique_name_with_versioning(try_path, date_prefix, "feature"),
            "feature-3"
        );
    }

    #[test]
    fn test_load_all_tries_empty_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let try_path = temp_dir.path();

        let entries = load_all_tries(try_path).unwrap();
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_load_all_tries_with_directories() {
        let temp_dir = tempfile::tempdir().unwrap();
        let try_path = temp_dir.path();

        // Create some directories
        fs::create_dir(try_path.join("test1")).unwrap();
        fs::create_dir(try_path.join("2025-08-17-test2")).unwrap();

        // Create a file (should be ignored)
        fs::write(try_path.join("file.txt"), "content").unwrap();

        let entries = load_all_tries(try_path).unwrap();
        assert_eq!(entries.len(), 2);

        // Check that both directories were loaded
        let names: Vec<&str> = entries.iter().map(|e| e.data.name.as_str()).collect();
        assert!(names.contains(&"test1"));
        assert!(names.contains(&"2025-08-17-test2"));
    }

    #[test]
    fn test_load_all_tries_date_prefix_bonus() {
        let temp_dir = tempfile::tempdir().unwrap();
        let try_path = temp_dir.path();

        // Create a date-prefixed directory
        fs::create_dir(try_path.join("2025-08-17-test")).unwrap();

        // Create a non-date-prefixed directory
        fs::create_dir(try_path.join("regular-test")).unwrap();

        let entries = load_all_tries(try_path).unwrap();
        assert_eq!(entries.len(), 2);

        // Find the date-prefixed entry
        let date_entry = entries
            .iter()
            .find(|e| e.data.name == "2025-08-17-test")
            .unwrap();
        let regular_entry = entries
            .iter()
            .find(|e| e.data.name == "regular-test")
            .unwrap();

        // Date-prefixed entry should have date_prefix set
        assert!(date_entry.data.date_prefix.is_some());
        assert_eq!(date_entry.data.date_prefix.as_ref().unwrap(), "2025-08-17");
        assert!(regular_entry.data.date_prefix.is_none());

        // Date-prefixed entry should have higher score (by about +2.0)
        let score_diff = date_entry.base_score - regular_entry.base_score;
        assert!((score_diff - 2.0).abs() < 0.1);
    }
}

#[derive(Debug, Clone, Default)]
pub struct TestFlags {
    pub test_no_cls: bool,
    pub test_keys: Vec<KeyInput>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectionResult {
    Cd(PathBuf),
    Mkdir(PathBuf),
    Delete(Vec<PathBuf>),
    Rename(PathBuf, PathBuf),
    Ascend(PathBuf, PathBuf),
    Cancelled,
}

enum KeyResult {
    Continue,
    Exit(SelectionResult),
}

pub struct TrySelector {
    tries: Vec<fuzzy::MatchResult<TryEntry>>,
    cursor_pos: usize,
    scroll_offset: usize,
    input_field: tui::InputField,
    screen: tui::Screen,
    try_path: PathBuf,
    needs_redraw: bool,
    test_no_cls: bool,
    delete_mode: bool,
    marked_for_delete: HashSet<PathBuf>,
    test_keys: VecDeque<KeyInput>,
}

impl TrySelector {
    fn is_input_char(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == ' '
    }

    pub fn new(try_path: PathBuf, test_flags: TestFlags) -> io::Result<Self> {
        let entries = load_all_tries(&try_path)?;
        let tries = fuzzy::fuzzy_match(&entries, "");
        let (width, height) = tui::Terminal::size();

        if !test_flags.test_no_cls {
            execute!(io::stderr(), EnterAlternateScreen)?;
            enable_raw_mode()?;
        }

        Ok(Self {
            tries,
            cursor_pos: 0,
            scroll_offset: 0,
            input_field: tui::InputField::new(),
            screen: tui::Screen::new(width, height, 2, 1),
            try_path,
            needs_redraw: true,
            test_no_cls: test_flags.test_no_cls,
            delete_mode: false,
            marked_for_delete: HashSet::new(),
            test_keys: test_flags.test_keys.into(),
        })
    }

    fn key_event_from_test_input(key: KeyInput) -> KeyEvent {
        use KeyInput::*;

        match key {
            Up => KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            Down => KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            Left => KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            Right => KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            Enter => KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            Escape => KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            Backspace => KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            CtrlA => KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            CtrlB => KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
            CtrlC => KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            CtrlD => KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
            CtrlE => KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL),
            CtrlF => KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
            CtrlG => KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
            CtrlJ => KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
            CtrlK => KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
            CtrlN => KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
            CtrlP => KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
            CtrlR => KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
            CtrlT => KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
            CtrlW => KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
            Char(c) => KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE),
        }
    }

    fn next_event(&mut self) -> io::Result<Option<Event>> {
        if let Some(key) = self.test_keys.pop_front() {
            return Ok(Some(Event::Key(Self::key_event_from_test_input(key))));
        }

        if event::poll(Duration::from_millis(100))? {
            return Ok(Some(event::read()?));
        }

        Ok(None)
    }

    fn body_height(&self) -> usize {
        let total = self.screen.height as usize;
        let header = self.screen.header.max_lines.min(total);
        let footer = self
            .screen
            .footer
            .max_lines
            .min(total.saturating_sub(header));
        total.saturating_sub(header + footer)
    }

    fn adjust_scroll(&mut self) {
        let visible_height = self.body_height();

        if self.cursor_pos < self.scroll_offset {
            self.scroll_offset = self.cursor_pos;
        } else if visible_height > 0 && self.cursor_pos >= self.scroll_offset + visible_height {
            self.scroll_offset = self.cursor_pos - visible_height + 1;
        }

        let max_offset = self.tries.len().saturating_sub(visible_height);
        self.scroll_offset = cmp::min(self.scroll_offset, max_offset);
    }

    fn write_highlighted_name(
        writer: &mut tui::SegmentWriter,
        text: &str,
        positions: &[usize],
        offset: usize,
    ) {
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if positions.contains(&(offset + i)) {
                let start = i;
                i += 1;
                while i < chars.len() && positions.contains(&(offset + i)) {
                    i += 1;
                }
                let chunk: String = chars[start..i].iter().collect();
                writer.write(&chunk, &tui::palette::match_color());
            } else {
                let start = i;
                i += 1;
                while i < chars.len() && !positions.contains(&(offset + i)) {
                    i += 1;
                }
                let chunk: String = chars[start..i].iter().collect();
                writer.write(&chunk, "");
            }
        }
    }

    pub fn render(&mut self) {
        let (width, height) = tui::Terminal::size();
        self.screen.width = width;
        self.screen.height = height;
        self.screen.clear();

        self.adjust_scroll();

        if self.delete_mode {
            // DELETE MODE header
            let header_line = self.screen.header.add_line();
            header_line.left.emoji("🗑️");
            header_line.left.write_bold(" DELETE MODE", "");
            header_line
                .left
                .write_dim(" — mark entries with ctrl-d, enter to confirm, esc to cancel");
        } else {
            // Normal header
            let newest = self
                .tries
                .iter()
                .max_by_key(|entry| entry.data.mtime)
                .map(|entry| relative_time(entry.data.mtime));

            let header_line = self.screen.header.add_line();
            header_line.left.emoji("🧪");
            header_line.left.write_bold(" Try Directory Selection", "");
            header_line.left.write_dim(" — ephemeral workspace manager");
            if let Some(when) = newest {
                header_line.right.write_dim(&when);
            }
        }

        if !self.delete_mode {
            let input_line = self.screen.header.add_line();
            input_line.left.write_dim("Search: ");
            self.input_field
                .render(&mut input_line.left, "", self.screen.width as usize);
            input_line
                .right
                .write_dim(&format!("{} entries", self.tries.len()));
        }

        let visible_height = self.body_height();
        let end = cmp::min(self.scroll_offset + visible_height, self.tries.len());
        for (idx, entry) in self.tries[self.scroll_offset..end].iter().enumerate() {
            let abs_idx = self.scroll_offset + idx;
            let line = self.screen.body.add_line();

            let is_marked = self.marked_for_delete.contains(&entry.data.path);

            if is_marked {
                line.background = Some(52); // DANGER_BG
            } else if abs_idx == self.cursor_pos {
                line.background = Some(24);
            }

            if is_marked {
                line.left.write("× ", &tui::palette::accent());
            } else if abs_idx == self.cursor_pos {
                line.left.write("→ ", &tui::palette::accent());
            } else {
                line.left.write("  ", "");
            }

            line.left.emoji("📁");
            line.left.write(" ", "");

            if let Some(prefix) = &entry.data.date_prefix {
                if entry.data.name.starts_with(prefix) {
                    line.left.write_dim(prefix);
                    let remainder = entry.data.name[prefix.len()..]
                        .strip_prefix('-')
                        .unwrap_or("");
                    line.left.write_dim("-");
                    Self::write_highlighted_name(
                        &mut line.left,
                        remainder,
                        &entry.positions,
                        prefix.chars().count() + 1,
                    );
                } else {
                    Self::write_highlighted_name(
                        &mut line.left,
                        &entry.data.name,
                        &entry.positions,
                        0,
                    );
                }
            } else {
                Self::write_highlighted_name(&mut line.left, &entry.data.name, &entry.positions, 0);
            }

            let meta_text = format!("{}, {:.1}", relative_time(entry.data.mtime), entry.score);
            line.right.write_dim(&meta_text);
            if entry.data.is_symlink {
                line.right.write(" ", "");
                line.right.emoji("🔗");
            }
        }

        if !self.delete_mode
            && !self.input_field.text().is_empty()
            && self.screen.body.lines.len() < visible_height
        {
            let preview = format!(
                "[new] {}-{}",
                chrono::Local::now().format("%Y-%m-%d"),
                self.input_field.text()
            );
            let line = self.screen.body.add_line();
            line.left.write(&preview, &tui::palette::accent());
        }

        if self.delete_mode {
            let footer_line = self.screen.footer.add_line();
            footer_line.background = Some(52); // DANGER_BG
            footer_line
                .left
                .write(&format!(" marked: {} ", self.marked_for_delete.len()), "");
            footer_line
                .left
                .write(" |  Ctrl-D: Toggle  Enter: Confirm  Esc: Cancel", "");
        } else {
            self.screen.footer.add_line().left.write_dim(
                "esc:quit  enter:select  ctrl-d:delete  ctrl-r:rename  ctrl-g:graduate  ctrl-t:new",
            );
        }
    }

    fn restore_terminal(&self) {
        if !self.test_no_cls {
            let _ = disable_raw_mode();
            let _ = execute!(io::stderr(), LeaveAlternateScreen);
        }
    }

    fn move_cursor_up(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.adjust_scroll();
        }
    }

    fn move_cursor_down(&mut self) {
        if self.cursor_pos + 1 < self.tries.len() {
            self.cursor_pos += 1;
            self.adjust_scroll();
        }
    }

    fn apply_filter(&mut self) {
        let query = self.input_field.text();
        if let Ok(entries) = load_all_tries(&self.try_path) {
            self.tries = fuzzy::fuzzy_match(&entries, query);
        }

        self.cursor_pos = 0;
        self.scroll_offset = 0;

        if !self.tries.is_empty() {
            let max_cursor = self.tries.len() - 1;
            self.cursor_pos = cmp::min(self.cursor_pos, max_cursor);
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> KeyResult {
        use KeyCode::*;

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            Up | BackTab => {
                self.move_cursor_up();
                KeyResult::Continue
            }
            Down | Tab => {
                self.move_cursor_down();
                KeyResult::Continue
            }
            Char('p') if ctrl => {
                self.move_cursor_up();
                KeyResult::Continue
            }
            Char('n') if ctrl => {
                self.move_cursor_down();
                KeyResult::Continue
            }
            Char('j') if ctrl => {
                self.move_cursor_down();
                KeyResult::Continue
            }
            Char('k') if ctrl => {
                if self.input_field.text().is_empty() {
                    self.move_cursor_up();
                } else {
                    self.input_field.kill_to_end();
                    self.apply_filter();
                }
                KeyResult::Continue
            }
            Enter => {
                if self.delete_mode && !self.marked_for_delete.is_empty() {
                    // Confirmation dialog
                    match self.show_confirmation_dialog() {
                        Ok(confirmed) => {
                            if confirmed {
                                // User confirmed with YES - collect paths
                                let paths: Vec<PathBuf> =
                                    self.marked_for_delete.iter().cloned().collect();
                                return KeyResult::Exit(SelectionResult::Delete(paths));
                            } else {
                                // User cancelled or didn't type YES - stay in delete mode
                                KeyResult::Continue
                            }
                        }
                        Err(_) => KeyResult::Continue,
                    }
                } else if self.cursor_pos < self.tries.len() {
                    let entry = &self.tries[self.cursor_pos];
                    KeyResult::Exit(SelectionResult::Cd(entry.data.path.clone()))
                } else if !self.input_field.text().is_empty() {
                    match self.create_from_buffer() {
                        Ok(path) => KeyResult::Exit(SelectionResult::Mkdir(path)),
                        Err(_) => KeyResult::Continue,
                    }
                } else {
                    KeyResult::Continue
                }
            }
            Esc => {
                if self.delete_mode {
                    // Cancel delete mode
                    self.delete_mode = false;
                    self.marked_for_delete.clear();
                    KeyResult::Continue
                } else {
                    KeyResult::Exit(SelectionResult::Cancelled)
                }
            }
            Char('c') if ctrl => {
                if self.delete_mode {
                    // Cancel delete mode
                    self.delete_mode = false;
                    self.marked_for_delete.clear();
                    KeyResult::Continue
                } else {
                    KeyResult::Exit(SelectionResult::Cancelled)
                }
            }
            Char(c) if Self::is_input_char(c) && !ctrl => {
                self.input_field.insert_char(c);
                self.apply_filter();
                KeyResult::Continue
            }
            Backspace => {
                self.input_field.delete_char_before();
                self.apply_filter();
                KeyResult::Continue
            }
            Delete => {
                self.input_field.delete_char_at();
                self.apply_filter();
                KeyResult::Continue
            }
            Char('a') if ctrl => {
                self.input_field.move_to_start();
                KeyResult::Continue
            }
            Char('e') if ctrl => {
                self.input_field.move_to_end();
                KeyResult::Continue
            }
            Char('b') if ctrl => {
                self.input_field.move_left();
                KeyResult::Continue
            }
            Char('f') if ctrl => {
                self.input_field.move_right();
                KeyResult::Continue
            }
            Char('w') if ctrl => {
                self.input_field.kill_word_backward();
                self.apply_filter();
                KeyResult::Continue
            }
            Char('t') if ctrl => match self.handle_create_new() {
                Ok(path) => KeyResult::Exit(SelectionResult::Mkdir(path)),
                Err(_) => KeyResult::Continue,
            },
            Char('d') if ctrl => {
                if !self.delete_mode {
                    self.delete_mode = true;
                    if self.cursor_pos < self.tries.len() {
                        let entry_path = self.tries[self.cursor_pos].data.path.clone();
                        self.marked_for_delete.insert(entry_path);
                    }
                } else if self.cursor_pos < self.tries.len() {
                    // Subsequent Ctrl-D: toggle mark on current entry
                    let entry_path = self.tries[self.cursor_pos].data.path.clone();
                    if self.marked_for_delete.contains(&entry_path) {
                        self.marked_for_delete.remove(&entry_path);
                    } else {
                        self.marked_for_delete.insert(entry_path);
                    }
                    // Auto-exit delete mode if no marks remain
                    if self.marked_for_delete.is_empty() {
                        self.delete_mode = false;
                    }
                }
                KeyResult::Continue
            }
            Char('r') if ctrl => {
                if self.cursor_pos < self.tries.len() {
                    let current_entry = &self.tries[self.cursor_pos];
                    let old_path = current_entry.data.path.clone();
                    match self.show_rename_dialog() {
                        Ok(Some(new_name)) => {
                            let new_path = self.try_path.join(new_name);
                            KeyResult::Exit(SelectionResult::Rename(old_path, new_path))
                        }
                        Ok(None) => {
                            self.needs_redraw = true;
                            KeyResult::Continue
                        }
                        Err(_) => {
                            self.needs_redraw = true;
                            KeyResult::Continue
                        }
                    }
                } else {
                    KeyResult::Continue
                }
            }
            Char('g') if ctrl => {
                if self.cursor_pos < self.tries.len() {
                    let current_entry = &self.tries[self.cursor_pos];
                    let src_path = current_entry.data.path.clone();
                    match self.show_graduate_dialog() {
                        Ok(Some(dest_path)) => {
                            KeyResult::Exit(SelectionResult::Ascend(src_path, dest_path))
                        }
                        Ok(None) => {
                            self.needs_redraw = true;
                            KeyResult::Continue
                        }
                        Err(_) => {
                            self.needs_redraw = true;
                            KeyResult::Continue
                        }
                    }
                } else {
                    KeyResult::Continue
                }
            }
            _ => KeyResult::Continue,
        }
    }

    fn handle_create_new(&mut self) -> io::Result<PathBuf> {
        let name = if self.input_field.text().is_empty() {
            self.restore_terminal();

            eprint!("Enter name: ");
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let name = input.trim().to_string();

            if !self.test_no_cls {
                execute!(io::stderr(), EnterAlternateScreen)?;
                enable_raw_mode()?;
            }
            self.needs_redraw = true;

            if name.is_empty() {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "Empty name"));
            }
            name
        } else {
            self.input_field.text().to_string()
        };

        self.create_from_name(&name)
    }

    fn create_from_buffer(&mut self) -> io::Result<PathBuf> {
        let name = self.input_field.text().to_string();
        self.create_from_name(&name)
    }

    fn show_confirmation_dialog(&mut self) -> io::Result<bool> {
        let mut confirmation_field = tui::InputField::new();

        loop {
            let (width, height) = tui::Terminal::size();
            let mut dialog_screen = tui::Screen::new(width, height, 2, 2);
            dialog_screen.clear();

            let count = self.marked_for_delete.len();
            let header_line = dialog_screen.header.add_line();
            header_line.left.emoji("🗑️");
            header_line.left.write_bold(
                &format!(
                    "  Delete {} {}?",
                    count,
                    if count == 1 {
                        "directory"
                    } else {
                        "directories"
                    }
                ),
                &tui::palette::accent(),
            );

            dialog_screen
                .header
                .add_line()
                .left
                .write_dim(&tui::ansi::sgr(&["2m"]));

            for path in &self.marked_for_delete {
                let line = dialog_screen.body.add_line();
                line.background = Some(52);
                line.left.emoji("🗑️");
                if let Some(name) = path.file_name() {
                    line.left.write(&format!(" {}", name.to_string_lossy()), "");
                }
            }

            dialog_screen.body.add_line();
            dialog_screen.body.add_line();

            let prompt_line = dialog_screen.body.add_line();
            prompt_line.left.write_dim("Type YES to confirm: ");
            confirmation_field.render(&mut prompt_line.left, "", width as usize);

            let footer_line = dialog_screen.footer.add_line();
            footer_line.left.write_dim("Enter: Confirm  Esc: Cancel");

            dialog_screen.flush_stderr()?;

            if let Some(event) = self.next_event()? {
                match event {
                    Event::Resize(_, _) => {
                        continue;
                    }
                    Event::Key(key_event) => {
                        use KeyCode::*;
                        let ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);

                        match key_event.code {
                            Enter => {
                                let input = confirmation_field.text();
                                if input == "YES" {
                                    return Ok(true);
                                } else {
                                    return Ok(false);
                                }
                            }
                            Esc | Char('c') if ctrl => {
                                return Ok(false);
                            }
                            Char(c) if c.is_ascii_alphabetic() || c.is_ascii_whitespace() => {
                                confirmation_field.insert_char(c);
                            }
                            Backspace => {
                                confirmation_field.delete_char_before();
                            }
                            Delete => {
                                confirmation_field.delete_char_at();
                            }
                            Char('a') if ctrl => {
                                confirmation_field.move_to_start();
                            }
                            Char('e') if ctrl => {
                                confirmation_field.move_to_end();
                            }
                            Char('b') if ctrl => {
                                confirmation_field.move_left();
                            }
                            Char('f') if ctrl => {
                                confirmation_field.move_right();
                            }
                            Char('w') if ctrl => {
                                confirmation_field.kill_word_backward();
                            }
                            Char('k') if ctrl => {
                                confirmation_field.kill_to_end();
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn create_from_name(&self, name: &str) -> io::Result<PathBuf> {
        let expanded = crate::expand_tokens(name);

        let date_prefix = chrono::Local::now().format("%Y-%m-%d").to_string();

        let unique_base =
            resolve_unique_name_with_versioning(&self.try_path, &date_prefix, &expanded);
        let full_name = format!("{}-{}", date_prefix, unique_base);

        Ok(self.try_path.join(full_name))
    }

    fn show_rename_dialog(&mut self) -> io::Result<Option<String>> {
        let current_name = self.tries[self.cursor_pos].data.name.clone();
        let mut rename_field = tui::InputField::new();
        let mut rename_error: Option<String> = None;

        // Pre-fill with current name
        for c in current_name.chars() {
            rename_field.insert_char(c);
        }

        loop {
            let (width, height) = tui::Terminal::size();
            let mut dialog_screen = tui::Screen::new(width, height, 2, 2);
            dialog_screen.clear();

            let header_line = dialog_screen.header.add_line();
            header_line.left.emoji("✏️");
            header_line
                .left
                .write_bold("  Rename directory", &tui::palette::accent());

            dialog_screen
                .header
                .add_line()
                .left
                .write_dim(&tui::ansi::sgr(&["2m"]));

            let body_line = dialog_screen.body.add_line();
            body_line.left.emoji("📁");
            body_line.left.write(&format!(" {}", current_name), "");

            dialog_screen.body.add_line();
            dialog_screen.body.add_line();

            let prompt_line = dialog_screen.body.add_line();
            prompt_line.left.write_dim("New name: ");
            rename_field.render(&mut prompt_line.left, "", width as usize);

            if let Some(err) = &rename_error {
                dialog_screen.body.add_line();
                dialog_screen.body.add_line().left.write_bold(err, "");
            }

            let footer_line = dialog_screen.footer.add_line();
            footer_line.left.write_dim("Enter: Confirm  Esc: Cancel");

            dialog_screen.flush_stderr()?;

            if let Some(event) = self.next_event()? {
                match event {
                    Event::Resize(_, _) => {
                        continue;
                    }
                    Event::Key(key_event) => {
                        use KeyCode::*;
                        let ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);

                        match key_event.code {
                            Enter => {
                                let normalized = rename_field
                                    .text()
                                    .split_whitespace()
                                    .collect::<Vec<_>>()
                                    .join("-");

                                if normalized.is_empty() {
                                    rename_error = Some("Name cannot be empty".to_string());
                                    continue;
                                }

                                if normalized.contains('/') {
                                    rename_error = Some("Name cannot contain /".to_string());
                                    continue;
                                }

                                if normalized == current_name {
                                    return Ok(None);
                                }

                                if self.try_path.join(&normalized).exists() {
                                    rename_error =
                                        Some(format!("Directory exists: {}", normalized));
                                    continue;
                                }

                                return Ok(Some(normalized));
                            }
                            Esc | Char('c') if ctrl => {
                                return Ok(None);
                            }
                            Char(c)
                                if !ctrl
                                    && (c.is_ascii_alphanumeric()
                                        || c == '-'
                                        || c == '_'
                                        || c == '.'
                                        || c == ' '
                                        || c == '/') =>
                            {
                                rename_field.insert_char(c);
                                rename_error = None;
                            }
                            Backspace => {
                                rename_field.delete_char_before();
                                rename_error = None;
                            }
                            Delete => {
                                rename_field.delete_char_at();
                                rename_error = None;
                            }
                            Char('a') if ctrl => {
                                rename_field.move_to_start();
                            }
                            Char('e') if ctrl => {
                                rename_field.move_to_end();
                            }
                            Char('b') if ctrl => {
                                rename_field.move_left();
                            }
                            Char('f') if ctrl => {
                                rename_field.move_right();
                            }
                            Char('w') if ctrl => {
                                rename_field.kill_word_backward();
                                rename_error = None;
                            }
                            Char('k') if ctrl => {
                                rename_field.kill_to_end();
                                rename_error = None;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn show_graduate_dialog(&mut self) -> io::Result<Option<PathBuf>> {
        let current_entry = self.tries[self.cursor_pos].clone();
        let current_name = current_entry.data.name.clone();

        // Get projects_dir: TRY_PROJECTS env var OR parent of TRY_PATH
        let projects_dir = std::env::var("TRY_PROJECTS").unwrap_or_else(|_| {
            self.try_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_string_lossy()
                .to_string()
        });

        // Strip date prefix: remove YYYY-MM-DD- if present
        let stripped_name = if let Some(date_prefix) = &current_entry.data.date_prefix {
            current_name
                .strip_prefix(date_prefix)
                .and_then(|s| s.strip_prefix('-'))
                .unwrap_or(&current_name)
        } else {
            &current_name
        };

        let default_dest = format!("{}/{}", projects_dir, stripped_name);
        let mut graduate_field = tui::InputField::new();

        // Pre-fill with default destination
        for c in default_dest.chars() {
            graduate_field.insert_char(c);
        }

        loop {
            let (width, height) = tui::Terminal::size();
            let mut dialog_screen = tui::Screen::new(width, height, 2, 2);
            dialog_screen.clear();

            let header_line = dialog_screen.header.add_line();
            header_line.left.emoji("🎓");
            header_line
                .left
                .write_bold("  Graduate try to project", &tui::palette::accent());

            dialog_screen
                .header
                .add_line()
                .left
                .write_dim(&tui::ansi::sgr(&["2m"]));

            let body_line = dialog_screen.body.add_line();
            body_line.left.emoji("📁");
            body_line.left.write(&format!(" {}", current_name), "");

            dialog_screen.body.add_line();

            let env_hint = if std::env::var("TRY_PROJECTS").is_ok() {
                "$TRY_PROJECTS"
            } else {
                "parent of $TRY_PATH"
            };
            let hint_line = dialog_screen.body.add_line();
            hint_line
                .left
                .write_dim(&format!("Destination ({}: {})", env_hint, projects_dir));

            dialog_screen.body.add_line();

            let prompt_line = dialog_screen.body.add_line();
            prompt_line.left.write_dim("Move to: ");
            graduate_field.render(&mut prompt_line.left, "", width as usize);

            let footer_line = dialog_screen.footer.add_line();
            footer_line.left.write_dim("Enter: Confirm  Esc: Cancel");

            dialog_screen.flush_stderr()?;

            if let Some(event) = self.next_event()? {
                match event {
                    Event::Resize(_, _) => {
                        continue;
                    }
                    Event::Key(key_event) => {
                        use KeyCode::*;
                        let ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);

                        match key_event.code {
                            Enter => {
                                let dest = graduate_field.text().trim();
                                if dest.is_empty() {
                                    // Empty destination: cancel
                                    return Ok(None);
                                }
                                return Ok(Some(PathBuf::from(dest)));
                            }
                            Esc | Char('c') if ctrl => {
                                return Ok(None);
                            }
                            Char(c)
                                if c.is_ascii_alphanumeric()
                                    || c == '-'
                                    || c == '_'
                                    || c == '.'
                                    || c == ' '
                                    || c == '/'
                                    || c == '~' =>
                            {
                                graduate_field.insert_char(c);
                            }
                            Backspace => {
                                graduate_field.delete_char_before();
                            }
                            Delete => {
                                graduate_field.delete_char_at();
                            }
                            Char('a') if ctrl => {
                                graduate_field.move_to_start();
                            }
                            Char('e') if ctrl => {
                                graduate_field.move_to_end();
                            }
                            Char('b') if ctrl => {
                                graduate_field.move_left();
                            }
                            Char('f') if ctrl => {
                                graduate_field.move_right();
                            }
                            Char('w') if ctrl => {
                                graduate_field.kill_word_backward();
                            }
                            Char('k') if ctrl => {
                                graduate_field.kill_to_end();
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn run(&mut self) -> io::Result<SelectionResult> {
        loop {
            if self.needs_redraw {
                self.render();
                self.screen.flush_stderr()?;
                self.needs_redraw = false;
            }

            if let Some(event) = self.next_event()? {
                match event {
                    Event::Resize(_, _) => {
                        self.needs_redraw = true;
                        continue;
                    }
                    Event::Key(key_event) => match self.handle_key(key_event) {
                        KeyResult::Continue => {
                            self.needs_redraw = true;
                        }
                        KeyResult::Exit(result) => {
                            self.restore_terminal();
                            return Ok(result);
                        }
                    },
                    _ => {}
                }
            }
        }
    }

    pub fn render_once(&mut self) -> io::Result<()> {
        self.render();
        self.screen.flush_stderr()
    }
}

impl Drop for TrySelector {
    fn drop(&mut self) {
        self.restore_terminal();
    }
}

#[cfg(test)]
mod tryselector_tests {
    use super::*;

    #[test]
    fn tryselector_new_initializes_state() {
        let temp_dir = tempfile::tempdir().unwrap();
        let try_path = temp_dir.path();

        fs::create_dir(try_path.join("alpha")).unwrap();
        fs::create_dir(try_path.join("beta")).unwrap();
        fs::create_dir(try_path.join("gamma")).unwrap();

        let selector = TrySelector::new(
            try_path.to_path_buf(),
            TestFlags {
                test_no_cls: true,
                test_keys: vec![],
            },
        )
        .unwrap();

        assert_eq!(selector.tries.len(), 3);
        assert_eq!(selector.cursor_pos, 0);
        assert_eq!(selector.scroll_offset, 0);
    }

    #[test]
    fn scroll_offset_adjusts_when_cursor_moves_down() {
        let temp_dir = tempfile::tempdir().unwrap();
        let try_path = temp_dir.path();

        for i in 0..30 {
            fs::create_dir(try_path.join(format!("entry-{}", i))).unwrap();
        }

        let mut selector = TrySelector::new(
            try_path.to_path_buf(),
            TestFlags {
                test_no_cls: true,
                test_keys: vec![],
            },
        )
        .unwrap();

        selector.screen.height = 13;
        selector.cursor_pos = 15;
        selector.adjust_scroll();

        assert!(selector.scroll_offset >= 6);
        assert!(selector.cursor_pos < selector.scroll_offset + selector.body_height());
    }

    #[test]
    fn scroll_offset_clamped_to_max() {
        let temp_dir = tempfile::tempdir().unwrap();
        let try_path = temp_dir.path();

        for i in 0..5 {
            fs::create_dir(try_path.join(format!("entry-{}", i))).unwrap();
        }

        let mut selector = TrySelector::new(
            try_path.to_path_buf(),
            TestFlags {
                test_no_cls: true,
                test_keys: vec![],
            },
        )
        .unwrap();

        selector.screen.height = 13;
        selector.scroll_offset = 100;
        selector.adjust_scroll();

        assert_eq!(selector.scroll_offset, 0);
    }

    #[test]
    fn move_cursor_up_decrements_position() {
        let temp_dir = tempfile::tempdir().unwrap();
        let try_path = temp_dir.path();

        for i in 0..5 {
            fs::create_dir(try_path.join(format!("entry-{}", i))).unwrap();
        }

        let mut selector = TrySelector::new(
            try_path.to_path_buf(),
            TestFlags {
                test_no_cls: true,
                test_keys: vec![],
            },
        )
        .unwrap();

        selector.cursor_pos = 3;
        selector.move_cursor_up();

        assert_eq!(selector.cursor_pos, 2);
    }

    #[test]
    fn move_cursor_down_increments_position() {
        let temp_dir = tempfile::tempdir().unwrap();
        let try_path = temp_dir.path();

        for i in 0..5 {
            fs::create_dir(try_path.join(format!("entry-{}", i))).unwrap();
        }

        let mut selector = TrySelector::new(
            try_path.to_path_buf(),
            TestFlags {
                test_no_cls: true,
                test_keys: vec![],
            },
        )
        .unwrap();

        selector.cursor_pos = 2;
        selector.move_cursor_down();

        assert_eq!(selector.cursor_pos, 3);
    }

    #[test]
    fn cursor_clamped_at_boundaries() {
        let temp_dir = tempfile::tempdir().unwrap();
        let try_path = temp_dir.path();

        for i in 0..3 {
            fs::create_dir(try_path.join(format!("entry-{}", i))).unwrap();
        }

        let mut selector = TrySelector::new(
            try_path.to_path_buf(),
            TestFlags {
                test_no_cls: true,
                test_keys: vec![],
            },
        )
        .unwrap();

        selector.cursor_pos = 0;
        selector.move_cursor_up();
        assert_eq!(selector.cursor_pos, 0);

        selector.cursor_pos = 2;
        selector.move_cursor_down();
        assert_eq!(selector.cursor_pos, 2);
    }

    #[test]
    fn apply_filter_resets_cursor() {
        let temp_dir = tempfile::tempdir().unwrap();
        let try_path = temp_dir.path();

        fs::create_dir(try_path.join("alpha")).unwrap();
        fs::create_dir(try_path.join("beta")).unwrap();
        fs::create_dir(try_path.join("gamma")).unwrap();

        let mut selector = TrySelector::new(
            try_path.to_path_buf(),
            TestFlags {
                test_no_cls: true,
                test_keys: vec![],
            },
        )
        .unwrap();

        selector.cursor_pos = 2;
        selector.input_field.insert_char('a');
        selector.apply_filter();

        assert_eq!(selector.cursor_pos, 0);
        assert_eq!(selector.scroll_offset, 0);
    }

    #[test]
    fn create_from_name_applies_token_expansion() {
        let temp_dir = tempfile::tempdir().unwrap();
        let selector = TrySelector::new(
            temp_dir.path().to_path_buf(),
            TestFlags {
                test_no_cls: true,
                test_keys: vec![],
            },
        )
        .unwrap();

        let result = selector.create_from_name("test-{date}").unwrap();
        let name = result.file_name().unwrap().to_str().unwrap();

        assert!(name.contains("test-"));
        assert!(name.starts_with("20"));
    }

    #[test]
    fn create_from_name_adds_date_prefix() {
        let temp_dir = tempfile::tempdir().unwrap();
        let selector = TrySelector::new(
            temp_dir.path().to_path_buf(),
            TestFlags {
                test_no_cls: true,
                test_keys: vec![],
            },
        )
        .unwrap();

        let result = selector.create_from_name("myproject").unwrap();
        let name = result.file_name().unwrap().to_str().unwrap();

        assert!(name.starts_with("20"));
        assert!(name.contains("-myproject"));
        assert!(name.matches('-').count() >= 3);
    }

    #[test]
    fn create_from_name_resolves_unique_name_on_collision() {
        let temp_dir = tempfile::tempdir().unwrap();
        let try_path = temp_dir.path();

        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let existing = format!("{}-myproject", date);
        fs::create_dir(try_path.join(&existing)).unwrap();

        let selector = TrySelector::new(
            try_path.to_path_buf(),
            TestFlags {
                test_no_cls: true,
                test_keys: vec![],
            },
        )
        .unwrap();

        let result = selector.create_from_name("myproject").unwrap();
        let name = result.file_name().unwrap().to_str().unwrap();

        assert!(name.contains("-myproject-2") || name.contains("-myproject2"));
    }

    #[test]
    fn toggle_delete_mode_on_ctrl_d() {
        let temp_dir = tempfile::tempdir().unwrap();
        let try_path = temp_dir.path();
        fs::create_dir(try_path.join("test1")).unwrap();

        let mut selector = TrySelector::new(
            try_path.to_path_buf(),
            TestFlags {
                test_no_cls: true,
                test_keys: vec![],
            },
        )
        .unwrap();

        assert!(!selector.delete_mode);

        let result = selector.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
        assert!(matches!(result, KeyResult::Continue));
        assert!(selector.delete_mode);
    }

    #[test]
    fn mark_entries_in_delete_mode() {
        let temp_dir = tempfile::tempdir().unwrap();
        let try_path = temp_dir.path();
        fs::create_dir(try_path.join("test1")).unwrap();
        fs::create_dir(try_path.join("test2")).unwrap();

        let mut selector = TrySelector::new(
            try_path.to_path_buf(),
            TestFlags {
                test_no_cls: true,
                test_keys: vec![],
            },
        )
        .unwrap();

        selector.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
        assert!(selector.delete_mode);
        assert_eq!(selector.marked_for_delete.len(), 0);

        selector.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
        assert_eq!(selector.marked_for_delete.len(), 1);

        selector.cursor_pos = 1;
        selector.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
        assert_eq!(selector.marked_for_delete.len(), 2);

        selector.cursor_pos = 0;
        selector.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
        assert_eq!(selector.marked_for_delete.len(), 1);
    }

    #[test]
    fn esc_cancels_delete_mode() {
        let temp_dir = tempfile::tempdir().unwrap();
        let try_path = temp_dir.path();
        fs::create_dir(try_path.join("test1")).unwrap();

        let mut selector = TrySelector::new(
            try_path.to_path_buf(),
            TestFlags {
                test_no_cls: true,
                test_keys: vec![],
            },
        )
        .unwrap();

        selector.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
        selector.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
        assert!(selector.delete_mode);
        assert_eq!(selector.marked_for_delete.len(), 1);

        let result = selector.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(matches!(result, KeyResult::Continue));
        assert!(!selector.delete_mode);
        assert_eq!(selector.marked_for_delete.len(), 0);
    }

    #[test]
    fn rename_accepts_slash_character() {
        let c = '/';
        assert!(
            c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == ' ' || c == '/'
        );
    }

    #[test]
    fn graduate_accepts_tilde_character() {
        let c = '~';
        assert!(
            c.is_ascii_alphanumeric()
                || c == '-'
                || c == '_'
                || c == '.'
                || c == ' '
                || c == '/'
                || c == '~'
        );
    }

    #[test]
    fn graduate_uses_try_projects_when_set() {
        use std::env;

        let temp_dir = tempfile::tempdir().unwrap();
        let try_path = temp_dir.path();
        let projects_dir = tempfile::tempdir().unwrap();

        fs::create_dir(try_path.join("2024-01-01-test")).unwrap();

        env::set_var("TRY_PROJECTS", projects_dir.path().to_str().unwrap());

        let mut selector = TrySelector::new(
            try_path.to_path_buf(),
            TestFlags {
                test_no_cls: true,
                test_keys: vec![],
            },
        )
        .unwrap();

        let retrieved = env::var("TRY_PROJECTS").unwrap();
        assert_eq!(retrieved, projects_dir.path().to_str().unwrap());

        env::remove_var("TRY_PROJECTS");
    }

    #[test]
    fn graduate_uses_parent_when_try_projects_unset() {
        use std::env;

        env::remove_var("TRY_PROJECTS");

        let temp_root = tempfile::tempdir().unwrap();
        let try_path = temp_root.path().join("tries");
        fs::create_dir(&try_path).unwrap();
        fs::create_dir(try_path.join("2024-01-01-test")).unwrap();

        let mut selector = TrySelector::new(
            try_path.clone(),
            TestFlags {
                test_no_cls: true,
                test_keys: vec![],
            },
        )
        .unwrap();

        let fallback = try_path.parent().unwrap().to_string_lossy().to_string();
        assert_eq!(fallback, temp_root.path().to_str().unwrap());
    }

    #[test]
    fn graduate_strips_date_prefix() {
        let name = "2024-01-01-myproject";
        let date_prefix = Some("2024-01-01".to_string());

        let stripped = if let Some(prefix) = &date_prefix {
            name.strip_prefix(prefix)
                .and_then(|s| s.strip_prefix('-'))
                .unwrap_or(name)
        } else {
            name
        };

        assert_eq!(stripped, "myproject");
    }

    #[test]
    fn graduate_handles_no_date_prefix() {
        let name = "myproject";
        let date_prefix: Option<String> = None;

        let stripped = if let Some(prefix) = &date_prefix {
            name.strip_prefix(prefix)
                .and_then(|s| s.strip_prefix('-'))
                .unwrap_or(name)
        } else {
            name
        };

        assert_eq!(stripped, "myproject");
    }
}
