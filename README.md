# key-remap-rs

A small background daemon that gives a **MacBook Pro running Fedora** macOS-style
copy/paste. It remaps the physical **Command** key (which Linux sees as Super/Meta)
onto **Ctrl** for the shortcuts you use most:

- `Cmd+C` → `Ctrl+C` (copy)
- `Cmd+V` → `Ctrl+V` (paste)

Everything else is left alone: pressing **Super** on its own still opens the GNOME
Activities overview, `Super`+other keys still work for window management, and a plain
`Ctrl+C` still sends SIGINT in a terminal.

## Project status

**Phase 1 — working.** Global `Cmd+C`/`Cmd+V` remapping, with an emergency escape hatch
(**both Command keys + Esc** ungrabs the keyboard and exits).

> ⚠️ This has been developed on macOS, where the Linux input layer can't be compiled, so
> the device I/O is **not yet verified on real hardware**. The remap logic is covered by
> unit tests (`cargo test`) that run anywhere. Expect to iterate on the first build.

Planned next:

- **Phase 2 — terminal-aware:** in terminals emit `Ctrl+Shift+C`/`Ctrl+Shift+V` so plain
  `Ctrl+C` can stay as SIGINT. Needs focused-window detection (a GNOME Shell extension on
  Wayland).
- **Phase 3 — robustness:** keyboard hotplug, a config file for custom mappings.

## How it works

Wayland provides no global key-remapping API, so `key-remap-rs` works *below* the display
server at the Linux **evdev/uinput** layer — the same approach as `keyd`, `xremap`, and
`kmonad`. It exclusively grabs each physical keyboard and re-emits rewritten events through
a single virtual keyboard. This makes it independent of Wayland vs X11.

Source layout:

| File | Responsibility |
| --- | --- |
| `src/main.rs` | Entry point; wires the modules together. |
| `src/config.rs` | The remap rules, as data. |
| `src/remapper.rs` | The remap logic (pure, unit-tested). |
| `src/device.rs` | Linux-only I/O: grab keyboards, emit via uinput. |

## Requirements

- A Linux machine (built and tested for **Fedora Workstation, GNOME on Wayland**).
- Rust **≥ 1.85** (the crate uses edition 2024). Install via [rustup](https://rustup.rs):
  ```sh
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  source "$HOME/.cargo/env"
  ```

## Build

```sh
cargo test              # run the remap-logic unit tests
cargo build --release   # produces ./target/release/key-remap-rs
```

## Try it (with sudo, no install)

Before installing anything, confirm it behaves:

```sh
sudo ./target/release/key-remap-rs
```

It prints the keyboards it grabbed. Test **Cmd+C / Cmd+V** in a GUI app.
**To stop:** press **both Command keys + Esc**, or `Ctrl+C` in the launching terminal.

> Keep an SSH session or a second machine handy the first few times — this daemon grabs
> your keyboard, so you want a way to kill it if it misbehaves.

## Install as a service (runs at boot)

Running as root needs no extra permissions setup — it can access `/dev/input` and
`/dev/uinput` directly.

```sh
sudo cp target/release/key-remap-rs /usr/local/bin/
sudo cp packaging/key-remap-rs.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now key-remap-rs
```

Check status and follow logs:

```sh
systemctl status key-remap-rs
journalctl -u key-remap-rs -f
```

Manage it:

```sh
sudo systemctl stop key-remap-rs      # stop now
sudo systemctl disable key-remap-rs   # don't start at boot
```

The panic combo (both Command keys + Esc) exits cleanly, so systemd will **not**
auto-restart it — it's your in-person kill switch. `systemctl start` brings it back.

### Running as your normal user instead of root

If you'd rather not run a root keyboard daemon, install the udev rule and add yourself to
the `input` group — see the header of `packaging/99-uinput.rules` for the steps.

## Recovery if it locks you out at boot

Because the service starts before login, a bad build could make the GUI keyboard unusable.
Escape hatches, in order:

1. **SSH in** from another machine: `sudo systemctl stop key-remap-rs`.
2. Boot to a **rescue shell**: at the GRUB menu press `e`, add
   `systemd.unit=rescue.target` to the kernel line, boot, then
   `systemctl disable key-remap-rs`.
3. The **panic combo** (both Command keys + Esc), if a keyboard still responds.

This is why the sudo test above matters before installing to boot.

## Prior art

`keyd`, `xremap`, `kmonad`, and `Kinto.sh` already solve macOS-style remapping and are
worth a look. `key-remap-rs` is a focused, from-scratch Rust take on the same idea.
