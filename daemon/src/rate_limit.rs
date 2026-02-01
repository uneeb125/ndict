use governor::{clock, state::NotKeyed, state::InMemoryState, Quota, RateLimiter};
use std::num::NonZeroU32;

/// Command rate limiter to prevent command flooding.
/// Uses a token bucket algorithm via governor crate.
pub struct CommandRateLimiter {
    /// The underlying rate limiter from governor
    limiter: RateLimiter<NotKeyed, InMemoryState, clock::DefaultClock>,
    /// Whether rate limiting is enabled
    enabled: bool,
}

impl CommandRateLimiter {
    /// Create a new rate limiter with the specified configuration.
    ///
    /// # Arguments
    /// * `commands_per_second` - Maximum sustained rate of commands (e.g., 10)
    /// * `burst_capacity` - Maximum burst of commands (e.g., 20)
    /// * `enabled` - Whether rate limiting is enabled
    ///
    /// # Returns
    /// A new CommandRateLimiter instance
    ///
    /// # Panics
    /// Panics if `commands_per_second` or `burst_capacity` is 0
    pub fn new(commands_per_second: u32, burst_capacity: u32, enabled: bool) -> Self {
        let quota = Quota::per_second(Self::non_zero(commands_per_second))
            .allow_burst(Self::non_zero(burst_capacity));

        Self {
            limiter: RateLimiter::direct(quota),
            enabled,
        }
    }

    /// Check if a command is allowed to proceed.
    ///
    /// This is an immediate check that does not wait for tokens to become available.
    /// Returns true if the command is allowed, false if rate limited.
    ///
    /// # Returns
    /// * `true` - Command is allowed to proceed
    /// * `false` - Command is rate limited and should be rejected
    pub fn check(&self) -> bool {
        if !self.enabled {
            return true;
        }

        self.limiter.check().is_ok()
    }

    /// Acquire permission to proceed, waiting if necessary.
    ///
    /// This method will block until a token becomes available.
    /// For CLI commands, you typically want to use `check()` instead
    /// to avoid blocking the connection.
    ///
    /// # Returns
    /// `true` when permission is acquired
    pub async fn acquire(&self) -> bool {
        if !self.enabled {
            return true;
        }

        self.limiter.until_ready().await;
        true
    }

    /// Convert u32 to NonZeroU32, panicking if value is 0.
    fn non_zero(value: u32) -> NonZeroU32 {
        NonZeroU32::new(value).expect("commands_per_second and burst_capacity must be non-zero")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_rate_limiter_new() {
        let limiter = CommandRateLimiter::new(10, 20, true);
        assert!(limiter.enabled);
    }

    #[test]
    fn test_command_rate_limiter_disabled() {
        let limiter = CommandRateLimiter::new(10, 20, false);
        assert!(!limiter.enabled);
        assert!(limiter.check());
    }

    #[test]
    fn test_command_rate_limiter_check_allowed() {
        let limiter = CommandRateLimiter::new(10, 20, true);
        // First request should be allowed
        assert!(limiter.check());
    }

    #[test]
    fn test_command_rate_limiter_burst() {
        let limiter = CommandRateLimiter::new(10, 20, true);

        // Test burst capacity - allow up to 20 requests instantly
        for _ in 0..20 {
            assert!(limiter.check(), "Burst capacity should allow 20 requests");
        }

        // Next request should be rate limited
        assert!(!limiter.check(), "Should be rate limited after burst exhausted");
    }

    #[test]
    #[should_panic(expected = "non-zero")]
    fn test_command_rate_limiter_zero_commands_per_second() {
        CommandRateLimiter::new(0, 20, true);
    }

    #[test]
    #[should_panic(expected = "non-zero")]
    fn test_command_rate_limiter_zero_burst_capacity() {
        CommandRateLimiter::new(10, 0, true);
    }

    #[tokio::test]
    async fn test_command_rate_limiter_acquire() {
        let limiter = CommandRateLimiter::new(10, 20, true);
        assert!(limiter.acquire().await);
    }

    #[tokio::test]
    async fn test_command_rate_limiter_acquire_disabled() {
        let limiter = CommandRateLimiter::new(10, 20, false);
        assert!(limiter.acquire().await);
    }
}
