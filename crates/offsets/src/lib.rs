//! Game version detection, so one build supports every patch smashline has offsets for.
//!
//! Offsets are written as `ver!(v13_0_5, v13_0_4)`. The version is read from the title's display
//! version the first time any offset is resolved, then cached.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum GameVersion {
    V13_0_4 = 1,
    V13_0_5 = 2,
}

static VERSION: AtomicU8 = AtomicU8::new(0);

fn detect() -> GameVersion {
    let display = skyline::nn::oe::get_display_version();
    match display.as_str() {
        "13.0.4" => GameVersion::V13_0_4,
        "13.0.5" => GameVersion::V13_0_5,
        other => {
            println!(
                "[smashline] Unsupported game version {}, using 13.0.5 offsets. Expect crashes!",
                other
            );
            GameVersion::V13_0_5
        }
    }
}

pub fn version() -> GameVersion {
    match VERSION.load(Ordering::Acquire) {
        1 => GameVersion::V13_0_4,
        2 => GameVersion::V13_0_5,
        _ => {
            let version = detect();
            VERSION.store(version as u8, Ordering::Release);
            version
        }
    }
}

/// An offset into the main executable, per supported game version.
#[derive(Copy, Clone, Debug)]
pub struct Offset {
    pub v13_0_5: usize,
    pub v13_0_4: usize,
}

impl Offset {
    pub const fn new(v13_0_5: usize, v13_0_4: usize) -> Self {
        Self { v13_0_5, v13_0_4 }
    }

    pub fn get(self) -> usize {
        match version() {
            GameVersion::V13_0_5 => self.v13_0_5,
            GameVersion::V13_0_4 => self.v13_0_4,
        }
    }
}

/// `ver!(v13_0_5, v13_0_4)` resolves to the offset for the running game version.
#[macro_export]
macro_rules! ver {
    ($v13_0_5:expr, $v13_0_4:expr) => {
        $crate::Offset::new($v13_0_5, $v13_0_4).get()
    };
}
