//! Device I/O: discover and grab physical keyboards, and emit through a single
//! uinput virtual keyboard. All the Linux-only, side-effecting code lives here so
//! that `remapper` stays pure and testable.

use std::sync::{Arc, Mutex};
use std::thread;

use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AttributeSet, Device, EventType, InputEvent, Key,
};

use crate::config;
use crate::remapper::Mapper;

/// Discover keyboards, build the virtual device, and run a grab+remap loop per
/// keyboard. Blocks until every device thread stops (or the process exits).
pub fn run() -> std::io::Result<()> {
    let mut keyboards: Vec<Device> = Vec::new();
    for (path, dev) in evdev::enumerate() {
        if is_keyboard(&dev) {
            let name = dev.name().unwrap_or("<unnamed>").to_string();
            println!("grabbing keyboard: {} ({})", name, path.display());
            keyboards.push(dev);
        }
    }
    if keyboards.is_empty() {
        eprintln!("no keyboard devices found (need read access to /dev/input/* — run with sudo)");
        std::process::exit(1);
    }

    let virt = Arc::new(Mutex::new(build_virtual_device(&keyboards)?));

    let mut handles = Vec::new();
    for mut dev in keyboards {
        if let Err(e) = dev.grab() {
            eprintln!("failed to grab {}: {e}", dev.name().unwrap_or("<unnamed>"));
            continue;
        }
        let virt = Arc::clone(&virt);
        handles.push(thread::spawn(move || run_device(dev, virt)));
    }

    for h in handles {
        let _ = h.join();
    }
    Ok(())
}

/// Build one virtual keyboard whose key set is the union of every grabbed device's
/// keys plus the keys we synthesize — the virtual device must declare support for
/// any key we might emit.
fn build_virtual_device(keyboards: &[Device]) -> std::io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<Key>::new();
    for dev in keyboards {
        if let Some(supported) = dev.supported_keys() {
            for k in supported.iter() {
                keys.insert(k);
            }
        }
    }
    for k in config::META_KEYS {
        keys.insert(k);
    }
    for k in config::REMAPPED {
        keys.insert(k);
    }
    keys.insert(config::OUTPUT_MOD);

    VirtualDeviceBuilder::new()?
        .name("key-remap-rs virtual keyboard")
        .with_keys(&keys)?
        .build()
}

/// A device looks like a real keyboard if it reports all of `KEYBOARD_MARKERS`.
fn is_keyboard(dev: &Device) -> bool {
    dev.supported_keys().map_or(false, |k| {
        config::KEYBOARD_MARKERS.iter().all(|&marker| k.contains(marker))
    })
}

fn syn() -> InputEvent {
    InputEvent::new(EventType::SYNCHRONIZATION, 0, 0)
}

/// Blocking event loop for a single grabbed keyboard.
fn run_device(mut dev: Device, virt: Arc<Mutex<VirtualDevice>>) {
    let mut mapper = Mapper::new();
    loop {
        // Collect into a Vec so the mutable borrow from `fetch_events` ends before
        // we may need to call `dev.ungrab()` in the loop body.
        let events: Vec<InputEvent> = match dev.fetch_events() {
            Ok(evts) => evts.collect(),
            Err(e) => {
                eprintln!("device read error, stopping: {e}");
                return;
            }
        };
        for ev in events {
            let mut out = mapper.process(&ev);
            if mapper.quit {
                let _ = dev.ungrab();
                println!("panic combo pressed — ungrabbing and exiting");
                std::process::exit(0);
            }
            if !out.is_empty() {
                out.push(syn());
                if let Ok(mut v) = virt.lock() {
                    let _ = v.emit(&out);
                }
            }
        }
    }
}
