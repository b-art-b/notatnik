//! Runs the in-memory WebDAV server of the tests until it is killed; the browser tests use it. Prints
//! `fakedav <url> <user> <password>` when it is ready.
fn main() {
    let dav = notatnik_sync::fakedav::FakeDav::start();
    println!("fakedav {} {} {}", dav.url, dav.user, dav.password);
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}
