// A core that never answers, used to prove that the launcher kills it and reports the timeout
// instead of waiting forever.
fn main() {
    std::thread::sleep(std::time::Duration::from_secs(120));
}
