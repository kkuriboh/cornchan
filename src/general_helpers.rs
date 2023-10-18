use std::time::SystemTime;

pub fn current_timestamp() -> u64 {
    unsafe {
        SystemTime::UNIX_EPOCH
            .elapsed()
            .unwrap_unchecked()
            .as_secs()
    }
}

pub fn slugify(s: &str) -> String {
    let mut ret = String::new();

    for c in s.trim().chars() {
        if [' ', '\r', '\n', '\t'].contains(&c) {
            ret.push('_')
        } else {
            ret.push(c);
        }
    }

    ret
}
