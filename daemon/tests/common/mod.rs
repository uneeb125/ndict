// Common test helpers for ndict daemon tests
//
// This module provides utilities for:
// - User interaction and confirmation
// - Test lifecycle management
// - Test output formatting

use std::io::{self, Write};

/// Ask user to confirm an action
pub fn confirm_action(prompt: &str) -> bool {
    print!(
        "\n[CONFIRM] {}\nPress 'y' to confirm, any other key to skip: ",
        prompt
    );
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    input.trim().to_lowercase() == "y"
}

/// Pause and wait for user to press Enter
pub fn wait_for_user(prompt: &str) {
    println!("\n[PAUSE] {}", prompt);
    print!("Press Enter to continue...");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
}

/// Print a section header
pub fn print_header(title: &str) {
    println!("\n{}", "=".repeat(60));
    println!("  {}", title);
    println!("{}", "=".repeat(60));
}

/// Print a success message
pub fn print_success(message: &str) {
    println!("\n✓ {}", message);
}

/// Print an error message
pub fn print_error(message: &str) {
    println!("\n✗ {}", message);
}

/// Print an info message
pub fn print_info(message: &str) {
    println!("\nℹ {}", message);
}
