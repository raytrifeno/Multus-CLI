use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use crate::types::Result;

pub(crate) fn run_with_spinner<T, F>(message: &str, func: F) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let message_text = message.to_string();

    let spinner = thread::spawn(move || {
        let frames = ['|', '/', '-', '\\'];
        let mut idx = 0usize;
        while !stop_flag.load(Ordering::Relaxed) {
            print!("\r{} {}", message_text, frames[idx % frames.len()]);
            let _ = io::stdout().flush();
            idx += 1;
            thread::sleep(Duration::from_millis(80));
        }
        print!("\r{}\r", " ".repeat(message_text.len() + 2));
        let _ = io::stdout().flush();
    });

    let result = func();
    stop.store(true, Ordering::Relaxed);
    let _ = spinner.join();
    result
}
