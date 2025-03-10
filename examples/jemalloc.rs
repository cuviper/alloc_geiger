use std::thread;
use std::time::Duration;

use alloc_geiger::Geiger;
use jemallocator::Jemalloc;

#[global_allocator]
static ALLOC: Geiger<Jemalloc> = Geiger::with_alloc(Jemalloc);

fn main() {
    let delay = Duration::from_millis(1000);
    for i in 1..10 {
        thread::sleep(delay / i);
        let _ = Box::new(i);
    }
}
