# Contributing to Open Editor

Thank you for helping build Open Editor. The project is at an early stage, so please open an issue before starting a large feature or architecture change.

## Development workflow

1. Fork the repository and create a focused branch.
2. Install the prerequisites in the README and run the existing checks.
3. Keep changes small and include tests for behavior you add or change.
4. Never commit source media, model weights, credentials, signing certificates, or generated project caches.
5. Open a pull request describing the user-visible result, implementation choices, testing, and privacy impact.

## Design rules

- Never mutate original media.
- Store canonical time as an integer value plus timescale.
- Route every timeline mutation through the shared command dispatcher.
- Validate expected project revisions and make mutations invertible.
- Keep hosted-model context minimal and visible to the user.
- Require confirmation for destructive or out-of-scope filesystem operations.

By contributing, you agree that your contributions are licensed under Apache-2.0.
