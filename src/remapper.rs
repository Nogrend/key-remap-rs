//! The remap state machine — pure logic, no I/O.
//!
//! `Mapper::process` takes one input event and returns the events to emit (an empty
//! result means "swallow this one", e.g. a buffered Command press). Keeping this free
//! of device access is what lets it be unit-tested on any platform, including where
//! `evdev` can't run a device.

use evdev::{EventType, InputEvent, Key};

use crate::config;

fn key_ev(code: u16, value: i32) -> InputEvent {
    InputEvent::new(EventType::KEY, code, value)
}

/// Tracks physical modifier state and rewrites `Cmd`+key combos into `Ctrl`+key.
///
/// Command presses are *buffered* rather than emitted immediately, so we can decide
/// after the fact whether a Command press became a Ctrl combo (copy/paste), a lone
/// Super tap (opens GNOME Activities), or a real Super combo (e.g. Super+D).
pub struct Mapper {
    // Rules, resolved to raw codes once.
    meta_codes: Vec<u16>,
    remapped: Vec<u16>,
    output_mod: u16,
    panic: u16,

    /// Command codes currently held down, in press order.
    held_metas: Vec<u16>,
    /// The Command code we've actually forwarded a *down* for (needs a matching up).
    emitted_meta: Option<u16>,
    /// The most recently pressed Command code — what we forward if we must flush.
    active_meta: Option<u16>,
    /// Whether the current Command hold has done something yet; if not, releasing it
    /// emits a lone Super tap.
    meta_used: bool,
    /// The key currently being translated to `output_mod + <key>`, so the release is
    /// clean even if Command is let go before the key.
    translating: Option<u16>,

    /// Set when the emergency ungrab combo is pressed.
    pub quit: bool,
}

impl Mapper {
    pub fn new() -> Self {
        Mapper {
            meta_codes: config::META_KEYS.iter().map(|k| k.code()).collect(),
            remapped: config::REMAPPED.iter().map(|k| k.code()).collect(),
            output_mod: config::OUTPUT_MOD.code(),
            panic: config::PANIC_KEY.code(),
            held_metas: Vec::new(),
            emitted_meta: None,
            active_meta: None,
            meta_used: false,
            translating: None,
            quit: false,
        }
    }

    fn is_meta(&self, code: u16) -> bool {
        self.meta_codes.contains(&code)
    }

    fn is_remapped(&self, code: u16) -> bool {
        self.remapped.contains(&code)
    }

    fn meta_held(&self) -> bool {
        !self.held_metas.is_empty()
    }

    fn all_metas_held(&self) -> bool {
        self.meta_codes.iter().all(|m| self.held_metas.contains(m))
    }

    /// Feed one input event; returns the events to emit (may be empty).
    pub fn process(&mut self, ev: &InputEvent) -> Vec<InputEvent> {
        // Only key events are handled; SYN/MSC etc. are dropped (the caller adds its
        // own SYN after each non-empty batch).
        if ev.event_type() != EventType::KEY {
            return Vec::new();
        }
        let code = ev.code();
        let val = ev.value(); // 0 = up, 1 = down, 2 = autorepeat
        let mut out = Vec::new();

        // --- Command keys: buffered ---
        if self.is_meta(code) {
            if val == 1 {
                let was_held = self.meta_held();
                if !self.held_metas.contains(&code) {
                    self.held_metas.push(code);
                }
                self.active_meta = Some(code);
                if !was_held {
                    self.meta_used = false;
                    self.emitted_meta = None;
                }
                // Buffer the press: emit nothing yet.
            } else if val == 0 {
                self.held_metas.retain(|&m| m != code);
                if self.emitted_meta == Some(code) {
                    // We had forwarded this Command down (real Super combo) — release it.
                    out.push(key_ev(code, 0));
                    self.emitted_meta = None;
                } else if !self.meta_used && !self.meta_held() {
                    // Pressed and released alone: emit a Super tap (opens Activities).
                    out.push(key_ev(code, 1));
                    out.push(key_ev(code, 0));
                }
                if !self.meta_held() {
                    self.active_meta = None;
                }
            }
            // val == 2 (repeat) on a Command key: ignore.
            return out;
        }

        // --- Remapped keys (C / V): the actual translation ---
        if self.is_remapped(code) {
            if self.meta_held() {
                match val {
                    1 => {
                        self.meta_used = true;
                        // Suppress Command so the app sees only Ctrl+<key>.
                        if let Some(m) = self.emitted_meta.take() {
                            out.push(key_ev(m, 0));
                        }
                        out.push(key_ev(self.output_mod, 1));
                        out.push(key_ev(code, 1));
                        self.translating = Some(code);
                    }
                    2 => out.push(key_ev(code, 2)), // repeat, modifier stays down
                    _ => {
                        out.push(key_ev(code, 0));
                        out.push(key_ev(self.output_mod, 0));
                        self.translating = None;
                    }
                }
            } else if self.translating == Some(code) && val == 0 {
                // Command released before the key — finish the Ctrl combo cleanly.
                out.push(key_ev(code, 0));
                out.push(key_ev(self.output_mod, 0));
                self.translating = None;
            } else {
                out.push(key_ev(code, val)); // plain passthrough
            }
            return out;
        }

        // --- Emergency ungrab: all Command keys held + panic key ---
        if code == self.panic && val == 1 && self.all_metas_held() {
            self.quit = true;
            return out;
        }

        // --- Any other key ---
        // If Command is held but not yet forwarded, this is a real Super combo
        // (e.g. Super+D) — flush the buffered Command down first.
        if self.meta_held() && self.emitted_meta.is_none() {
            if let Some(m) = self.active_meta {
                out.push(key_ev(m, 1));
                self.emitted_meta = Some(m);
                self.meta_used = true;
            }
        }
        out.push(key_ev(code, val));
        out
    }
}

impl Default for Mapper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes() -> (u16, u16, u16, u16, u16, u16, u16) {
        (
            Key::KEY_LEFTMETA.code(),
            Key::KEY_RIGHTMETA.code(),
            Key::KEY_C.code(),
            Key::KEY_V.code(),
            Key::KEY_D.code(),
            Key::KEY_ESC.code(),
            Key::KEY_LEFTCTRL.code(),
        )
    }

    /// Reduce emitted events to (code, value) pairs for easy assertions.
    fn pairs(evs: &[InputEvent]) -> Vec<(u16, i32)> {
        evs.iter()
            .filter(|e| e.event_type() == EventType::KEY)
            .map(|e| (e.code(), e.value()))
            .collect()
    }

    #[test]
    fn cmd_c_becomes_ctrl_c() {
        let (meta, _, c, _, _, _, ctrl) = codes();
        let mut m = Mapper::new();
        assert!(pairs(&m.process(&key_ev(meta, 1))).is_empty()); // Command buffered
        assert_eq!(pairs(&m.process(&key_ev(c, 1))), vec![(ctrl, 1), (c, 1)]);
        assert_eq!(pairs(&m.process(&key_ev(c, 0))), vec![(c, 0), (ctrl, 0)]);
        // Releasing Command after a copy must NOT tap Super (would open Activities).
        assert!(pairs(&m.process(&key_ev(meta, 0))).is_empty());
    }

    #[test]
    fn cmd_v_becomes_ctrl_v() {
        let (meta, _, _, v, _, _, ctrl) = codes();
        let mut m = Mapper::new();
        m.process(&key_ev(meta, 1));
        assert_eq!(pairs(&m.process(&key_ev(v, 1))), vec![(ctrl, 1), (v, 1)]);
    }

    #[test]
    fn lone_cmd_taps_super() {
        let (meta, ..) = codes();
        let mut m = Mapper::new();
        assert!(pairs(&m.process(&key_ev(meta, 1))).is_empty());
        assert_eq!(pairs(&m.process(&key_ev(meta, 0))), vec![(meta, 1), (meta, 0)]);
    }

    #[test]
    fn super_plus_other_passes_through() {
        let (meta, _, _, _, d, _, _) = codes();
        let mut m = Mapper::new();
        m.process(&key_ev(meta, 1));
        // Super+D: buffered Command is flushed, then D.
        assert_eq!(pairs(&m.process(&key_ev(d, 1))), vec![(meta, 1), (d, 1)]);
        assert_eq!(pairs(&m.process(&key_ev(d, 0))), vec![(d, 0)]);
        assert_eq!(pairs(&m.process(&key_ev(meta, 0))), vec![(meta, 0)]);
    }

    #[test]
    fn plain_c_is_untouched() {
        let (_, _, c, ..) = codes();
        let mut m = Mapper::new();
        assert_eq!(pairs(&m.process(&key_ev(c, 1))), vec![(c, 1)]);
        assert_eq!(pairs(&m.process(&key_ev(c, 0))), vec![(c, 0)]);
    }

    #[test]
    fn meta_released_before_letter_still_closes_ctrl() {
        let (meta, _, c, _, _, _, ctrl) = codes();
        let mut m = Mapper::new();
        m.process(&key_ev(meta, 1));
        assert_eq!(pairs(&m.process(&key_ev(c, 1))), vec![(ctrl, 1), (c, 1)]);
        m.process(&key_ev(meta, 0)); // release Command while C still held
        assert_eq!(pairs(&m.process(&key_ev(c, 0))), vec![(c, 0), (ctrl, 0)]);
    }

    #[test]
    fn both_cmd_plus_esc_quits() {
        let (lmeta, rmeta, _, _, _, esc, _) = codes();
        let mut m = Mapper::new();
        m.process(&key_ev(lmeta, 1));
        m.process(&key_ev(rmeta, 1));
        m.process(&key_ev(esc, 1));
        assert!(m.quit);
    }
}
