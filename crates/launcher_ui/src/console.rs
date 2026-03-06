use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

const MAX_CONSOLE_LINES: usize = 4000;

static CONSOLE_LINES: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();

fn store() -> &'static Mutex<VecDeque<String>> {
    CONSOLE_LINES.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub fn push_line(line: impl Into<String>) {
    let Ok(mut lines) = store().lock() else {
        return;
    };
    lines.push_back(line.into());
    while lines.len() > MAX_CONSOLE_LINES {
        let _ = lines.pop_front();
    }
}

pub fn snapshot() -> Vec<String> {
    let Ok(lines) = store().lock() else {
        return Vec::new();
    };
    lines.iter().cloned().collect()
}
