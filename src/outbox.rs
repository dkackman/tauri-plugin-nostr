/// Returns the number of seconds to wait before the next retry attempt.
/// Implements capped exponential backoff: min(2^attempts, 300).
#[allow(dead_code)] // used in Phase 3 outbox retry loop
pub fn backoff_secs(attempts: u32) -> u64 {
    let base: u64 = 2u64.saturating_pow(attempts);
    base.min(300)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_is_exponential_up_to_cap() {
        assert_eq!(backoff_secs(0), 1);
        assert_eq!(backoff_secs(1), 2);
        assert_eq!(backoff_secs(2), 4);
        assert_eq!(backoff_secs(3), 8);
        assert_eq!(backoff_secs(4), 16);
        assert_eq!(backoff_secs(5), 32);
        assert_eq!(backoff_secs(6), 64);
        assert_eq!(backoff_secs(7), 128);
        assert_eq!(backoff_secs(8), 256);
        assert_eq!(backoff_secs(9), 300);  // capped
        assert_eq!(backoff_secs(10), 300); // still capped
        assert_eq!(backoff_secs(20), 300); // still capped
    }
}
