//! A Rust allocator which makes sound when active, like a Geiger counter.
//!
//! The [`rodio`] crate is used to emit [sinc] pulses each time the allocator is
//! used, excluding its own allocator activity.
//!
//! Inspired by [Malloc Geiger].
//!
//!
//! ## Usage
//!
//! To use `alloc_geiger` add it as a dependency:
//!
//! ```toml
//! # Cargo.toml
//! [dependencies]
//! alloc_geiger = "0.3"
//! ```
//!
//! To set `alloc_geiger::Geiger` as the global allocator, it must be initialized
//! with an underlying allocator. The `type System` alias and the `new()` method
//! make it easy to use the default system allocator:
//!
//! ```rust
//! #[global_allocator]
//! static ALLOC: alloc_geiger::System = alloc_geiger::System::new();
//!
//! fn main() {
//!     // ...
//! }
//! ```
//!
//! Alternatives like [`jemallocator`] may also be used:
//!
//! ```rust
//! use alloc_geiger::Geiger;
//! use jemallocator::Jemalloc;
//!
//! #[global_allocator]
//! static ALLOC: Geiger<Jemalloc> = Geiger::with_alloc(Jemalloc);
//!
//! fn main() {
//!     // ...
//! }
//! ```
//!
//! [`rodio`]: https://crates.io/crates/rodio
//! [sinc]: https://en.wikipedia.org/wiki/Sinc_function
//! [Malloc Geiger]: https://github.com/laserallan/malloc_geiger
//! [`jemallocator`]: https://crates.io/crates/jemallocator

use rodio::{OutputStream, OutputStreamHandle, Source};
use std::alloc::{self, GlobalAlloc, Layout};
use std::cell::Cell;
use std::f32::consts::PI;
use std::fmt;
use std::ops::Range;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Barrier, OnceLock};
use std::time::Duration;

/// Geiger counter allocator.
#[derive(Default)]
pub struct Geiger<Alloc> {
    inner: Alloc,
    stream_handle: OnceLock<Option<OutputStreamHandle>>,
    /// non-blocking protection against recursive init
    init: AtomicBool,
}

impl<Alloc: fmt::Debug> fmt::Debug for Geiger<Alloc> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Geiger")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

/// `Geiger` allocator based on `std::alloc::System`.
pub type System = Geiger<alloc::System>;

thread_local! {
    /// Guard against recursion
    static BUSY: Cell<bool> = const { Cell::new(false) };
}

impl System {
    pub const fn new() -> Self {
        Geiger::with_alloc(alloc::System)
    }
}

impl<Alloc> Geiger<Alloc> {
    pub const fn with_alloc(inner: Alloc) -> Self {
        Geiger {
            inner,
            stream_handle: OnceLock::new(),
            init: AtomicBool::new(false),
        }
    }

    fn bell(&self) {
        BUSY.with(|busy| {
            if !busy.replace(true) {
                if let Some(handle) = self.get_handle() {
                    let _ = handle.play_raw(Pulse::new());
                }
                busy.set(false);
            }
        });
    }

    fn get_handle(&self) -> &Option<OutputStreamHandle> {
        if let Some(handle) = self.stream_handle.get() {
            handle
        } else if !self.init.swap(true, Ordering::AcqRel) {
            self.stream_handle.get_or_init(rodio_init)
        } else {
            &None
        }
    }
}

unsafe impl<Alloc: GlobalAlloc> GlobalAlloc for Geiger<Alloc> {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.bell();
        self.inner.alloc(layout)
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        self.bell();
        self.inner.alloc_zeroed(layout)
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.bell();
        self.inner.dealloc(ptr, layout)
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        self.bell();
        self.inner.realloc(ptr, layout, new_size)
    }
}

fn rodio_init() -> Option<OutputStreamHandle> {
    if let Ok((stream, handle)) = OutputStream::try_default() {
        let (source, barrier) = BusySource::new();
        if let Ok(()) = handle.play_raw(source) {
            barrier.wait();
            std::mem::forget(stream);
            return Some(handle);
        }
    }
    None
}

struct BusySource {
    busy_address: usize,
    barrier: Option<Arc<Barrier>>,
}

impl BusySource {
    fn new() -> (Self, Arc<Barrier>) {
        let barrier = Arc::new(Barrier::new(2));
        let source = BusySource {
            busy_address: BUSY.with(|busy| busy as *const _ as usize),
            barrier: Some(Arc::clone(&barrier)),
        };
        (source, barrier)
    }
}

impl Iterator for BusySource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        BUSY.with(|busy| {
            if self.busy_address == busy as *const _ as usize {
                Some(0.0)
            } else {
                busy.set(true);
                self.barrier.take()?.wait();
                None
            }
        })
    }
}

impl Source for BusySource {
    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        1
    }

    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// Simple pulse based on the sinc function, sin(x)/x
struct Pulse {
    range: Range<i16>,
}

impl Pulse {
    const PEAK: f32 = 0.5;

    const SAMPLE_RATE: u32 = 48_000;
    const PERIOD_MILLIS: u32 = 4;
    const PERIOD_SAMPLES: u32 = Self::SAMPLE_RATE / (Self::PERIOD_MILLIS * 1000);
    const SAMPLE_SCALE: f32 = 2.0 * PI / Self::PERIOD_SAMPLES as f32;

    const fn new() -> Self {
        let i = Self::PERIOD_SAMPLES as i16 * 4;
        Pulse { range: -i..i }
    }
}

impl Iterator for Pulse {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        match self.range.next() {
            None => None,
            Some(0) => Some(Self::PEAK),
            Some(i) => {
                let x = f32::from(i) * Self::SAMPLE_SCALE;
                Some(x.sin() / x * Self::PEAK)
            }
        }
    }
}

impl Source for Pulse {
    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        Self::SAMPLE_RATE
    }

    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
