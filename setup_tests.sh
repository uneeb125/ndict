#!/bin/bash
# setup_tests.sh
# Setup script for ndict test infrastructure

set -e

echo "=========================================="
echo "  ndict Test Infrastructure Setup"
echo "=========================================="
echo

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Step 1: Create test directories
echo -e "${YELLOW}[1/4]${NC} Creating test directories..."
mkdir -p daemon/tests/common
echo -e "${GREEN}✓${NC} Test directories created"

# Step 2: Check if dependencies are installed
echo
echo -e "${YELLOW}[2/4]${NC} Checking Rust toolchain..."

if ! command -v cargo &> /dev/null; then
    echo -e "${RED}✗${NC} cargo not found. Please install Rust first."
    echo "  Visit: https://rustup.rs/"
    exit 1
fi

RUST_VERSION=$(rustc --version)
echo -e "${GREEN}✓${NC} Rust toolchain: $RUST_VERSION"

# Step 3: Fetch dependencies
echo
echo -e "${YELLOW}[3/4]${NC} Fetching test dependencies..."
cargo fetch
echo -e "${GREEN}✓${NC} Dependencies fetched"

# Step 4: Create test helper modules
echo
echo -e "${YELLOW}[4/4]${NC} Creating test helper modules..."

# Create daemon/tests/common/mod.rs
cat > daemon/tests/common/mod.rs << 'EOF'
// Common test helpers for ndict daemon tests
//
// This module provides utilities for:
// - User interaction and confirmation
// - Test lifecycle management
// - Test output formatting

use std::io::{self, Write};

/// Ask user to confirm an action
pub fn confirm_action(prompt: &str) -> bool {
    print!("\n[CONFIRM] {}\nPress 'y' to confirm, any other key to skip: ", prompt);
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
EOF

echo -e "${GREEN}✓${NC} Test helpers created"

echo
echo "=========================================="
echo -e "${GREEN}  Setup Complete!${NC}"
echo "=========================================="
echo
echo "Next steps:"
echo "  1. Run: cargo test --workspace"
echo "  2. Run interactive tests: cargo test --workspace --ignored"
echo "  3. Generate coverage: cargo tarpaulin --workspace --out Html"
echo
echo "For more information, see TESTING_PLAN.md"
