//! WebAssembly entry points for the web page.
//!
//! A plain exported-memory interface, no bindings generator: JavaScript asks for a
//! buffer, writes the input bytes into it, calls `scan` or `diff`, then reads the JSON
//! result from `output_ptr` / `output_len`. See `web/engine.js`.

use std::cell::RefCell;

use jarsnitch_core::Limits;

/// Input slots: old name, old bytes, new name, new bytes. `scan` uses the first two.
const SLOTS: usize = 4;

thread_local! {
    static INPUT: RefCell<[Vec<u8>; SLOTS]> = RefCell::new(Default::default());
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Makes input slot `slot` `len` bytes long and returns where to write them. Calls into
/// the module can grow its memory, so callers write right after this returns.
#[unsafe(no_mangle)]
pub extern "C" fn input(slot: u32, len: u32) -> *mut u8 {
    INPUT.with_borrow_mut(|inputs| {
        let Some(buf) = inputs.get_mut(slot as usize) else { return std::ptr::null_mut() };
        *buf = vec![0; len as usize];
        buf.as_mut_ptr()
    })
}

/// Scans slots 0 (name) and 1 (bytes). Returns the length of the JSON report.
#[unsafe(no_mangle)]
pub extern "C" fn scan() -> u32 {
    let json = INPUT.with_borrow(|[name, bytes, ..]| {
        let report = jarsnitch_core::scan(&String::from_utf8_lossy(name), bytes, &Limits::default());
        serde_json::to_vec(&report)
    });
    finish(json)
}

/// Compares slots 0–1 (old) with slots 2–3 (new). Returns the length of the JSON diff.
#[unsafe(no_mangle)]
pub extern "C" fn diff() -> u32 {
    let json = INPUT.with_borrow(|[old_name, old, new_name, new]| {
        let d = jarsnitch_core::diff(&String::from_utf8_lossy(old_name), old, &String::from_utf8_lossy(new_name), new, &Limits::default());
        serde_json::to_vec(&d)
    });
    finish(json)
}

#[unsafe(no_mangle)]
pub extern "C" fn output_ptr() -> *const u8 {
    OUTPUT.with_borrow(|out| out.as_ptr())
}

/// Frees the inputs and the last result.
#[unsafe(no_mangle)]
pub extern "C" fn clear() {
    INPUT.with_borrow_mut(|inputs| *inputs = Default::default());
    OUTPUT.with_borrow_mut(|out| *out = Vec::new());
}

fn finish(json: serde_json::Result<Vec<u8>>) -> u32 {
    // Serializing these types can't fail; an empty result tells JavaScript it did anyway.
    let json = json.unwrap_or_default();
    INPUT.with_borrow_mut(|inputs| *inputs = Default::default());
    OUTPUT.with_borrow_mut(|out| *out = json);
    OUTPUT.with_borrow(|out| out.len() as u32)
}
