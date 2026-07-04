//! Remap rules, as data.
//!
//! Phase 1 hardcodes these; Phase 3 is expected to load them from a file. Keeping
//! them in one place (rather than scattered through the logic) is what makes that
//! future change small.

use evdev::Key;

/// Keys that act as the macOS Command modifier. Linux reports the Apple Command
/// key as Super/Meta.
pub const META_KEYS: [Key; 2] = [Key::KEY_LEFTMETA, Key::KEY_RIGHTMETA];

/// The modifier a held Command key is translated into.
pub const OUTPUT_MOD: Key = Key::KEY_LEFTCTRL;

/// Keys that become `OUTPUT_MOD + <same key>` when pressed with a Command key held.
/// (With Shift also held, they become `Ctrl+Shift+<key>` — e.g. terminal copy.)
pub const REMAPPED: [Key; 14] = [
    Key::KEY_C, // copy
    Key::KEY_V, // paste
    Key::KEY_A, // select all
    Key::KEY_X, // cut
    Key::KEY_Z, // undo   (Cmd+Shift+Z -> Ctrl+Shift+Z = redo in many apps)
    Key::KEY_T, // new tab
    Key::KEY_F, // find
    Key::KEY_S, // save
    Key::KEY_W, // close tab/window
    Key::KEY_N, // new window
    Key::KEY_R, // reload
    Key::KEY_P, // print
    Key::KEY_O, // open
    Key::KEY_Q, // quit
];

/// Emergency ungrab: all `META_KEYS` held together plus this key exits the daemon.
pub const PANIC_KEY: Key = Key::KEY_ESC;

/// A device is treated as a real keyboard only if it reports all of these — this
/// filters out power buttons, lid switches, etc., which also advertise KEY events.
pub const KEYBOARD_MARKERS: [Key; 3] = [Key::KEY_A, Key::KEY_LEFTCTRL, Key::KEY_C];
