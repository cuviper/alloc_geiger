use std::thread;
use std::time::Duration;

#[global_allocator]
static ALLOC: alloc_geiger::System = alloc_geiger::SYSTEM;

fn main() {
    let delay = Duration::from_millis(1000);
    for i in 1..10 {
        thread::sleep(delay / i);
        let _ = Box::new(i);
    }
}
