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

use rodio::{ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Sample, SampleRate, Source};
use std::alloc::{self, GlobalAlloc, Layout};
use std::cell::Cell;
use std::fmt;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Barrier, OnceLock};
use std::time::Duration;

/// Geiger counter allocator.
#[derive(Default)]
pub struct Geiger<Alloc> {
    inner: Alloc,
    sink_handle: OnceLock<Option<MixerDeviceSink>>,
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
            sink_handle: OnceLock::new(),
            init: AtomicBool::new(false),
        }
    }

    fn bell(&self) {
        BUSY.with(|busy| {
            if !busy.replace(true) {
                if let Some(handle) = self.get_handle() {
                    handle.mixer().add(Pulse::new());
                }
                busy.set(false);
            }
        });
    }

    fn get_handle(&self) -> &Option<MixerDeviceSink> {
        if let Some(handle) = self.sink_handle.get() {
            handle
        } else if !self.init.swap(true, Ordering::AcqRel) {
            self.sink_handle.get_or_init(rodio_init)
        } else {
            &None
        }
    }
}

unsafe impl<Alloc: GlobalAlloc> GlobalAlloc for Geiger<Alloc> {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.bell();
        unsafe { self.inner.alloc(layout) }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        self.bell();
        unsafe { self.inner.alloc_zeroed(layout) }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.bell();
        unsafe { self.inner.dealloc(ptr, layout) }
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        self.bell();
        unsafe { self.inner.realloc(ptr, layout, new_size) }
    }
}

fn rodio_init() -> Option<MixerDeviceSink> {
    let mut sink_handle = DeviceSinkBuilder::open_default_sink().ok()?;
    sink_handle.log_on_drop(false);
    let (source, barrier) = BusySource::new();
    sink_handle.mixer().add(source);
    barrier.wait();
    Some(sink_handle)
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
    type Item = Sample;

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
    fn channels(&self) -> ChannelCount {
        const { ChannelCount::new(1).unwrap() }
    }

    fn sample_rate(&self) -> SampleRate {
        const { SampleRate::new(100).unwrap() }
    }

    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// Simple pulse based on the sinc function, sin(x)/x
struct Pulse {
    iter: std::slice::Iter<'static, Sample>,
}

impl Pulse {
    fn new() -> Self {
        Pulse { iter: PULSE.iter() }
    }
}

impl Iterator for Pulse {
    type Item = Sample;

    fn next(&mut self) -> Option<Self::Item> {
        // NB: `Sample` could be `f32` or `f64`
        Some(Sample::from(*self.iter.next()?))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl Source for Pulse {
    fn channels(&self) -> ChannelCount {
        const { ChannelCount::new(1).unwrap() }
    }

    fn sample_rate(&self) -> SampleRate {
        const { SampleRate::new(48_000).unwrap() }
    }

    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// Sinc function samples generated by `examples/sinc.rs`.
///
/// If Rust gets `const fn sin`, we could consider generating the static samples directly,
/// but it's not much trouble to just copy (and format) it manually.
///
/// We also don't care about accuracy here, as long as it sounds okay.
#[rustfmt::skip]
static PULSE: [f32; 97] = [
    1.3913767e-8, -0.010158828, -0.017978106, -0.02122066, -0.018795315, -0.011103839,
    -3.017612e-8, 0.011645474, 0.020674838, 0.024485376, 0.02176298, 0.0129044745,
    1.2652691e-9, -0.013641826, -0.02432333, -0.02893726, -0.025843548, -0.015402081,
    -2.1502865e-8, 0.016464282, 0.02953546, 0.035367765, 0.031807438, 0.019098602, 1.3913767e-8,
    -0.020759324, -0.037590593, -0.045472838, -0.041349664, -0.025129724, -1.2652691e-9,
    0.02808616, 0.051687073, 0.06366198, 0.059070963, 0.03672807, 1.3913767e-8, -0.04340587,
    -0.08269934, -0.10610329, -0.10337417, -0.06820928, -1.3913767e-8, 0.09549298, 0.20674832,
    0.31830987, 0.41349667, 0.47746482, 0.5, 0.47746482, 0.41349667, 0.31830987, 0.20674832,
    0.09549298, -1.3913767e-8, -0.06820928, -0.10337417, -0.10610329, -0.08269934, -0.04340587,
    1.3913767e-8, 0.03672807, 0.059070963, 0.06366198, 0.051687073, 0.02808616, -1.2652691e-9,
    -0.025129724, -0.041349664, -0.045472838, -0.037590593, -0.020759324, 1.3913767e-8,
    0.019098602, 0.031807438, 0.035367765, 0.02953546, 0.016464282, -2.1502865e-8, -0.015402081,
    -0.025843548, -0.02893726, -0.02432333, -0.013641826, 1.2652691e-9, 0.0129044745, 0.02176298,
    0.024485376, 0.020674838, 0.011645474, -3.017612e-8, -0.011103839, -0.018795315, -0.02122066,
    -0.017978106, -0.010158828, 1.3913767e-8,
];
