//! Lossless `options.txt` model: unknown keys, ordering and untouched lines survive edits.

use std::fs;
use std::io;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Line {
    Entry {
        key: String,
        value: String,
    },
    /// Blank or unparseable line, kept verbatim.
    Other(String),
}

/// An `options.txt` file. Editing changes only the touched lines.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OptionsFile {
    lines: Vec<Line>,
    crlf: bool,
}

impl OptionsFile {
    pub fn parse(text: &str) -> Self {
        let crlf = text.contains("\r\n");
        let lines = text
            .lines()
            .map(|line| match line.split_once(':') {
                Some((key, value)) if !key.is_empty() => Line::Entry {
                    key: key.to_owned(),
                    value: value.to_owned(),
                },
                _ => Line::Other(line.to_owned()),
            })
            .collect();
        Self { lines, crlf }
    }

    pub fn read(path: &Path) -> io::Result<Self> {
        Ok(Self::parse(&fs::read_to_string(path)?))
    }

    pub fn serialize(&self) -> String {
        let newline = if self.crlf { "\r\n" } else { "\n" };
        let mut out = String::new();
        for line in &self.lines {
            match line {
                Line::Entry { key, value } => {
                    out.push_str(key);
                    out.push(':');
                    out.push_str(value);
                }
                Line::Other(text) => out.push_str(text),
            }
            out.push_str(newline);
        }
        out
    }

    /// Writes via a temp file and rename so the game never reads a half-written file.
    pub fn write_atomic(&self, path: &Path) -> io::Result<()> {
        let mut temp_name = path
            .file_name()
            .map(|n| n.to_os_string())
            .unwrap_or_default();
        temp_name.push(".vertex-tmp");
        let temp = path.with_file_name(temp_name);
        fs::write(&temp, self.serialize())?;
        fs::rename(&temp, path)
    }

    /// Raw value for `key` (the last occurrence wins, like the game).
    pub fn get(&self, key: &str) -> Option<&str> {
        self.lines.iter().rev().find_map(|line| match line {
            Line::Entry { key: k, value } if k == key => Some(value.as_str()),
            _ => None,
        })
    }

    pub fn contains(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Sets `key`, replacing it in place or appending. Returns true if the file changed.
    pub fn set(&mut self, key: &str, value: &str) -> bool {
        let mut found = false;
        let mut changed = false;
        for line in &mut self.lines {
            if let Line::Entry { key: k, value: v } = line
                && k == key
            {
                found = true;
                if v != value {
                    *v = value.to_owned();
                    changed = true;
                }
            }
        }
        if !found {
            self.lines.push(Line::Entry {
                key: key.to_owned(),
                value: value.to_owned(),
            });
            changed = true;
        }
        changed
    }

    pub fn remove(&mut self, key: &str) -> bool {
        let before = self.lines.len();
        self.lines
            .retain(|line| !matches!(line, Line::Entry { key: k, .. } if k == key));
        self.lines.len() != before
    }

    /// Keys in file order (duplicates collapsed to first position).
    pub fn keys(&self) -> Vec<&str> {
        let mut seen = std::collections::HashSet::new();
        self.lines
            .iter()
            .filter_map(|line| match line {
                Line::Entry { key, .. } if seen.insert(key.as_str()) => Some(key.as_str()),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "version:5023\nao:true\nlastServer:\nkey_key.attack:key.mouse.left\n\nresourcePacks:[\"vanilla\"]\n";

    #[test]
    fn round_trips_exactly_and_edits_only_touched_lines() {
        let mut file = OptionsFile::parse(SAMPLE);
        assert_eq!(file.serialize(), SAMPLE);
        assert_eq!(file.get("lastServer"), Some(""));
        assert!(file.set("ao", "false"));
        assert!(!file.set("ao", "false"));
        assert!(file.set("newKey", "1"));
        assert_eq!(
            file.serialize(),
            "version:5023\nao:false\nlastServer:\nkey_key.attack:key.mouse.left\n\nresourcePacks:[\"vanilla\"]\nnewKey:1\n"
        );
    }

    #[test]
    fn preserves_crlf_and_values_containing_colons() {
        let text = "a:b:c\r\nx:1\r\n";
        let file = OptionsFile::parse(text);
        assert_eq!(file.get("a"), Some("b:c"));
        assert_eq!(file.serialize(), text);
    }
}
