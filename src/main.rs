mod monitor;

fn main() {
    if let Err(e) = monitor::start_monitor() {
        eprintln!("Error: {e}");
    }
}
