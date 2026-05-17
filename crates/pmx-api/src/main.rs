#[tokio::main]
async fn main() {
    // v0.9 intentionally does not bind a production listener by default.
    // Add a listener only after auth middleware, store wiring, and gateway wiring exist.
    let _app = pmx_api::app();
    eprintln!("pmx-api scaffold initialized; no listener bound in v0.9");
}
