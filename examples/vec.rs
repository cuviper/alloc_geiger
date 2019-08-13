#[global_allocator]
static ALLOC: alloc_geiger::System = alloc_geiger::SYSTEM;

fn main() {
    let mut vec = Vec::new();
    for i in 0..100_000_000 {
        // We'll get sound each time this reallocates.
        vec.push(i);
    }
}
