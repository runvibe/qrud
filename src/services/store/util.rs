use std::time::{SystemTime, UNIX_EPOCH};

use uuid::Uuid;

pub(super) fn new_uuid() -> String {
    Uuid::now_v7().to_string()
}

pub(super) fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_millis() as i64
}
