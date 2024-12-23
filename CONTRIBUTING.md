# Contributing Guidelines

Thank you for your interest in contributing to RISE Chain! This document provides essential guidelines for contributing.

## How to Contribute

1. **Fork and Clone**
   - Fork the repository
   - Clone it locally

2. **Make Changes**
   - Create a feature branch
   - Write clean, performant Rust code
   - Add tests and documentation
   - Run `cargo fmt` and `cargo clippy`

3. **Test**
   ```sh
   cargo test --workspace --release -- --test-threads=1
   ```

4. **Submit**
   - Push to your fork
   - Create a Pull Request
   - Link relevant issues

## Development Setup

1. Requirements:
   - Rust toolchain
   - cmake (for snmalloc)

2. Build:
   ```sh
   cargo build
   ```

## Guidelines

- Include tests for new features
- Follow Rust style conventions

## Questions?

- Open an issue
- Check documentation

Thank you for contribution!
