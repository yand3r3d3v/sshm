//! State for the in-TUI key-generation modal.
//!
//! Backs `keygen_form.rs`. Replaces the previous `inquire`-based flow, which
//! had to leave the alternate screen and confused users into thinking the
//! prompts hadn't appeared.

use std::path::PathBuf;

pub const KEY_TYPES: &[&str] = &["ed25519", "ed25519-sk", "ecdsa", "rsa"];

pub struct KeygenFormState {
    /// Index into [`KEY_TYPES`].
    pub key_type_idx: usize,
    pub path: String,
    pub comment: String,
    pub passphrase: String,
    pub selected_field: usize,
    pub error: Option<String>,
    pub vim_mode: crate::tui::vim_mode::VimMode,
    pub pending_g: bool,
}

impl KeygenFormState {
    pub fn new() -> Self {
        let user = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
        let host = super::super::app::key_flows::hostname_best_effort();
        let default_path = dirs::home_dir()
            .map(|h| h.join(".ssh").join("id_ed25519"))
            .unwrap_or_else(|| PathBuf::from("id_ed25519"));
        KeygenFormState {
            key_type_idx: 0,
            path: default_path.display().to_string(),
            comment: format!("{}@{}", user, host),
            passphrase: String::new(),
            selected_field: 0,
            error: None,
            vim_mode: crate::tui::vim_mode::VimMode::default(),
            pending_g: false,
        }
    }

    pub const KEY_TYPE_FIELD: usize = 0;
    pub const PATH_FIELD: usize = 1;
    pub const COMMENT_FIELD: usize = 2;
    pub const PASSPHRASE_FIELD: usize = 3;

    pub fn fields_count() -> usize { 4 }

    pub fn key_type(&self) -> &'static str {
        KEY_TYPES[self.key_type_idx]
    }

    /// When the user changes the key type, snap the default file name to match
    /// (only if the user hasn't customized it away from the previous default).
    pub fn sync_default_path(&mut self, previous_type_idx: usize) {
        if previous_type_idx == self.key_type_idx {
            return;
        }
        let prev_default = Self::default_filename_for(KEY_TYPES[previous_type_idx]);
        let new_default = Self::default_filename_for(KEY_TYPES[self.key_type_idx]);
        if let Some(home) = dirs::home_dir() {
            let prev_full = home.join(".ssh").join(prev_default).display().to_string();
            if self.path == prev_full {
                self.path = home.join(".ssh").join(new_default).display().to_string();
            }
        }
    }

    fn default_filename_for(key_type: &str) -> &'static str {
        match key_type {
            "rsa" => "id_rsa",
            "ecdsa" => "id_ecdsa",
            "ed25519-sk" => "id_ed25519_sk",
            _ => "id_ed25519",
        }
    }

    pub fn next_field(&mut self) {
        self.selected_field = (self.selected_field + 1) % (Self::fields_count() + 1);
    }

    pub fn prev_field(&mut self) {
        if self.selected_field == 0 {
            self.selected_field = Self::fields_count();
        } else {
            self.selected_field -= 1;
        }
    }

    pub fn cycle_key_type(&mut self, forward: bool) {
        let prev = self.key_type_idx;
        let n = KEY_TYPES.len();
        if forward {
            self.key_type_idx = (self.key_type_idx + 1) % n;
        } else {
            self.key_type_idx = (self.key_type_idx + n - 1) % n;
        }
        self.sync_default_path(prev);
    }

    pub fn active_value_mut(&mut self) -> Option<&mut String> {
        match self.selected_field {
            Self::PATH_FIELD => Some(&mut self.path),
            Self::COMMENT_FIELD => Some(&mut self.comment),
            Self::PASSPHRASE_FIELD => Some(&mut self.passphrase),
            _ => None,
        }
    }

    pub fn push_char(&mut self, c: char) {
        self.error = None;
        if self.selected_field == Self::KEY_TYPE_FIELD {
            // Space cycles through key types; arrows would shadow field nav.
            if c == ' ' { self.cycle_key_type(true); }
            return;
        }
        if let Some(field) = self.active_value_mut() {
            field.push(c);
        }
    }

    pub fn pop_char(&mut self) {
        self.error = None;
        if let Some(field) = self.active_value_mut() {
            field.pop();
        }
    }
}
