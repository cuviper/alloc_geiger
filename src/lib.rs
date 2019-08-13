//! A Rust allocator which makes sound when active, like a Geiger counter.
//!
//! Currently this just writes an ASCII [`BEL`] to `/dev/tty`.
//!
//! Inspired by [Malloc Geiger].
//!
//! [`BEL`]: https://en.wikipedia.org/wiki/Bell_character
//! [Malloc Geiger]: https://github.com/laserallan/malloc_geiger
//!
//! ## Usage
//!
//! To use `alloc_geiger` add it as a dependency:
//!
//! ```toml
//! # Cargo.toml
//! [dependencies]
//! alloc_geiger = "0.1.0"
//! ```
//!
//! To set `alloc_geiger::Geiger` as the global allocator, it must be initialized
//! with an underlying allocator. The `type System` alias and its `const SYSTEM`
//! make it easy to use the default system allocator:
//!
//! ```rust
//! #[global_allocator]
//! static ALLOC: alloc_geiger::System = alloc_geiger::SYSTEM;
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
//! static ALLOC: Geiger<Jemalloc> = Geiger::new(Jemalloc);
//!
//! fn main() {
//!     // ...
//! }
//! ```
//!
//! [`jemallocator`]: https://crates.io/crates/jemallocator

use once_cell::sync::OnceCell;
use std::alloc::{self, GlobalAlloc, Layout};
use std::cell::Cell;
use std::fs::{File, OpenOptions};
use std::io::Write;

/// Geiger counter allocator.
#[derive(Debug, Default)]
pub struct Geiger<Alloc> {
    inner: Alloc,
    tty: OnceCell<Option<File>>,
}

/// `Geiger` allocator based on `std::alloc::System`.
pub type System = Geiger<alloc::System>;

/// `Geiger` allocator based on `std::alloc::System`.
pub const SYSTEM: System = Geiger::new(alloc::System);

fn open_tty() -> Option<File> {
    OpenOptions::new().append(true).open("/dev/tty").ok()
}

impl<Alloc> Geiger<Alloc> {
    pub const fn new(inner: Alloc) -> Self {
        Geiger {
            inner,
            tty: OnceCell::new(),
        }
    }

    fn bell(&self) {
        const BEL: u8 = 0x07;

        thread_local! {
            // Guard against recursion
            static BUSY: Cell<bool> = Cell::new(false);
        }

        BUSY.with(|busy| {
            if !busy.replace(true) {
                let tty = self.tty.get_or_init(open_tty);
                if let Some(ref mut file) = tty.as_ref() {
                    file.write_all(&[BEL]).ok();
                }
                busy.set(false);
            }
        });
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
