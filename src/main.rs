//! key-remap-rs — a macOS-style Cmd->Ctrl key remapper for Linux (Phase 1).
//!
//! Turns the physical Command key (Linux sees it as Super/Meta) into Ctrl for
//! copy/paste:
//!   * Cmd+C -> Ctrl+C
//!   * Cmd+V -> Ctrl+V
//! Everything else passes through untouched: plain Super still opens the GNOME
//! Activities overview, Super+<other> still works for window management, and a
//! plain Ctrl+C still sends SIGINT.
//!
//! It works below the display server at the Linux evdev/uinput layer, so it is
//! independent of Wayland vs X11. Each physical keyboard is grabbed exclusively
//! (EVIOCGRAB) and its events are rewritten and re-emitted through a single
//! virtual keyboard.
//!
//! Emergency escape hatch: press BOTH Command keys + Esc to ungrab and exit.
//!
//! Layout:
//!   * `config`   — remap rules, as data
//!   * `remapper` — the transformation logic (pure, unit-tested)
//!   * `device`   — evdev input grab + uinput output (Linux-only I/O)
//!
//! Run (Phase 1): `cargo build --release && sudo ./target/release/key-remap-rs`

mod config;
mod device;
mod remapper;

fn main() -> std::io::Result<()> {
    device::run()
}
