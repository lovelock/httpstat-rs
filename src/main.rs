mod output;
mod timing;

use std::process;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn run() -> i32 {
    eprintln!("httpstat {} — Task 3 will implement curl subprocess", VERSION);
    0
}

fn main() {
    let code = run();
    process::exit(code);
}
