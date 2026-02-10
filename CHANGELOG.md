# Changelog

## [0.1.2] - 2026-02-10

### Added
- `password` command to change wallet encryption password (CLI + GUI)
- Custom node URLs validated for HTTPS; `--insecure` flag to allow plain HTTP
- GUI: Transaction history pagination with cursor-based page navigation

### Changed
- Moved `validate_wallet_name` and `list_wallets` to core (shared by CLI and GUI)
- Extracted `sign_and_execute` helper — deduplicated transaction signing across 4 methods

### Fixed
- Transaction execution now errors on failure instead of showing "sent!" with a failure status
- Token balance display no longer truncates `u128` values to `u64`
- Sweep gas cost handled correctly for negative (rebate) values
- Library `expect()` panics replaced with `Result` propagation
- Atomic wallet file writes (write→fsync→rename) to prevent corruption on crash
- File permissions set atomically via `OpenOptions` on Unix
- GUI: secret fields (passwords, mnemonic) now zeroized from memory instead of just cleared
- GUI: wallet name validated against path traversal
- GUI: mnemonic recovery input masked on screen

## [0.1.1] - 2026-02-10

### Added
- Staking support: `stake`, `unstake`, and `stakes` commands
- `sweep_all` command to send entire balance minus gas
- `show_transfer` command to look up a transaction by digest
- `tokens` command to show all coin/token balances
- `status` command: shows current epoch, gas price, network, and node URL; accepts optional custom node
- GUI frontend (`iota-wallet-gui`) using iced

### Changed
- Transfer and stake now require confirmation before signing
- Transaction history sorted by epoch and lamport version (newest first)

### Fixed
- Wallet address no longer printed twice on creation/recovery
