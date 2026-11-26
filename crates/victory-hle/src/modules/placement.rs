#![allow(non_snake_case)]
//! Where and how eagerly the title's threads run: the processors it ties
//! them to, their priorities, and the process's priority boost. The title
//! sets these for the console's eight cores; passed to the host as they are
//! (the default), or left to the host's scheduler: `VICTORY_AFFINITY=free`,
//! `VICTORY_PRIORITY=free`, `VICTORY_PRIORITY_BOOST=keep`. The first calls
//! of each are logged.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::core::BOOL;

use crate::{HleModule, hle_export};

/// `GROUP_AFFINITY`.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct GroupAffinity {
    mask: usize,
    group: u16,
    reserved: [u16; 3],
}

mod host {
    use super::GroupAffinity;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::core::BOOL;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn SetThreadGroupAffinity(thread: HANDLE, affinity: *const GroupAffinity, previous: *mut GroupAffinity) -> BOOL;
        pub fn GetThreadGroupAffinity(thread: HANDLE, affinity: *mut GroupAffinity) -> BOOL;
    }
}

fn switch(name: &str, value: &str, cell: &'static OnceLock<bool>) -> bool {
    *cell.get_or_init(|| std::env::var(name).is_ok_and(|v| v.trim() == value))
}

/// Logs the first 64 calls of a kind.
fn said(count: &AtomicU32) -> bool {
    count.fetch_add(1, Ordering::Relaxed) < 64
}

fn thread_id(thread: HANDLE) -> u32 {
    // SAFETY: a handle the title passes; 0 if it is not a thread's.
    unsafe { windows_sys::Win32::System::Threading::GetThreadId(thread) }
}

pub extern "system" fn SetThreadGroupAffinity(
    thread: HANDLE,
    affinity: *const GroupAffinity,
    previous: *mut GroupAffinity,
) -> BOOL {
    static FREE: OnceLock<bool> = OnceLock::new();
    static SAID: AtomicU32 = AtomicU32::new(0);
    let free = switch("VICTORY_AFFINITY", "free", &FREE);
    if said(&SAID) {
        // SAFETY: the title passes its affinity (null tolerated).
        let asked = unsafe { affinity.as_ref() }.copied();
        tracing::info!(thread = thread_id(thread), ?asked, free, "the title ties a thread to processors");
    }
    if free {
        if !previous.is_null() {
            // SAFETY: the title passes room for the previous affinity.
            unsafe { host::GetThreadGroupAffinity(thread, previous) };
        }
        return 1;
    }
    // SAFETY: the title's arguments, as it passed them.
    unsafe { host::SetThreadGroupAffinity(thread, affinity, previous) }
}

pub extern "system" fn GetThreadGroupAffinity(thread: HANDLE, affinity: *mut GroupAffinity) -> BOOL {
    // SAFETY: the title's arguments.
    unsafe { host::GetThreadGroupAffinity(thread, affinity) }
}

pub extern "system" fn SetThreadPriority(thread: HANDLE, priority: i32) -> BOOL {
    static FREE: OnceLock<bool> = OnceLock::new();
    static SAID: AtomicU32 = AtomicU32::new(0);
    let free = switch("VICTORY_PRIORITY", "free", &FREE);
    if said(&SAID) {
        tracing::info!(thread = thread_id(thread), priority, free, "the title sets a thread's priority");
    }
    if free {
        return 1;
    }
    // SAFETY: the title's arguments.
    unsafe { windows_sys::Win32::System::Threading::SetThreadPriority(thread, priority) }
}

pub extern "system" fn SetProcessPriorityBoost(process: HANDLE, disable: BOOL) -> BOOL {
    static KEEP: OnceLock<bool> = OnceLock::new();
    let keep = switch("VICTORY_PRIORITY_BOOST", "keep", &KEEP);
    tracing::info!(disable, keep, "the title sets the process's priority boost");
    if keep {
        return 1;
    }
    // SAFETY: the title's arguments.
    unsafe { windows_sys::Win32::System::Threading::SetProcessPriorityBoost(process, disable) }
}

pub(crate) fn module() -> HleModule {
    HleModule {
        name: "placement",
        dlls: &["api-ms-win-core-processtopology-*", "api-ms-win-core-processthreads-*", "kernel32.dll"],
        exports: vec![
            hle_export!(SetThreadGroupAffinity),
            hle_export!(GetThreadGroupAffinity),
            hle_export!(SetThreadPriority),
            hle_export!(SetProcessPriorityBoost),
        ],
    }
}
