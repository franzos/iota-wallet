/// Command definitions and parsing for the wallet REPL and one-shot mode.
use anyhow::{Result, bail};
use iota_sdk::types::Address;

use crate::display;
use crate::network::{NetworkClient, TransactionFilter};
use crate::wallet::Wallet;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Show wallet balance
    Balance,
    /// Show wallet address
    Address,
    /// Transfer IOTA to another address: transfer <address> <amount>
    Transfer { recipient: Address, amount: u64 },
    /// Show transaction history: show_transfers [in|out|all]
    ShowTransfers { filter: TransactionFilter },
    /// Request faucet tokens (testnet/devnet only)
    Faucet,
    /// Show seed phrase (mnemonic)
    Seed,
    /// Print help
    Help { command: Option<String> },
    /// Exit the wallet
    Exit,
}

impl Command {
    /// Parse a command from a raw input string.
    pub fn parse(input: &str) -> Result<Self> {
        let input = input.trim();
        if input.is_empty() {
            bail!("No command entered. Type 'help' for a list of commands.");
        }

        let mut parts = input.splitn(3, char::is_whitespace);
        let cmd = parts.next().unwrap().to_lowercase();
        let arg1 = parts.next().map(|s| s.trim());
        let arg2 = parts.next().map(|s| s.trim());

        match cmd.as_str() {
            "balance" | "bal" => Ok(Command::Balance),

            "address" | "addr" => Ok(Command::Address),

            "transfer" | "send" => {
                let addr_str = arg1.ok_or_else(|| {
                    anyhow::anyhow!(
                        "Missing recipient address. Usage: transfer <address> <amount>"
                    )
                })?;
                let amount_str = arg2.ok_or_else(|| {
                    anyhow::anyhow!(
                        "Missing amount. Usage: transfer <address> <amount>"
                    )
                })?;

                let recipient = Address::from_hex(addr_str).map_err(|e| {
                    anyhow::anyhow!("Invalid recipient address '{addr_str}': {e}")
                })?;

                let amount = display::parse_iota_amount(amount_str).map_err(|e| {
                    anyhow::anyhow!("Invalid amount '{amount_str}': {e}")
                })?;

                if amount == 0 {
                    bail!("Cannot send 0 IOTA.");
                }

                Ok(Command::Transfer { recipient, amount })
            }

            "show_transfers" | "transfers" | "txs" => {
                let filter = TransactionFilter::from_str_opt(arg1);
                Ok(Command::ShowTransfers { filter })
            }

            "faucet" => Ok(Command::Faucet),

            "seed" => Ok(Command::Seed),

            "help" | "?" => Ok(Command::Help {
                command: arg1.map(|s| s.to_string()),
            }),

            "exit" | "quit" | "q" => Ok(Command::Exit),

            other => bail!(
                "Unknown command: '{other}'. Type 'help' for a list of commands."
            ),
        }
    }

    /// Whether this command should prompt for confirmation before executing.
    pub fn requires_confirmation(&self) -> bool {
        matches!(self, Command::Seed)
    }

    /// Execute a command and return the output string.
    pub async fn execute(
        &self,
        wallet: &Wallet,
        network: &NetworkClient,
        json_output: bool,
    ) -> Result<String> {
        match self {
            Command::Balance => {
                let nanos = network.balance(wallet.address()).await?;
                if json_output {
                    Ok(display::format_balance_json(nanos))
                } else {
                    Ok(display::format_balance(nanos))
                }
            }

            Command::Address => {
                let addr = wallet.address().to_string();
                if json_output {
                    Ok(display::format_address_json(&addr))
                } else {
                    Ok(addr)
                }
            }

            Command::Transfer { recipient, amount } => {
                let result = network
                    .send_iota(
                        wallet.private_key(),
                        wallet.address(),
                        *recipient,
                        *amount,
                    )
                    .await?;

                if json_output {
                    Ok(serde_json::json!({
                        "digest": result.digest,
                        "status": result.status,
                        "amount_nanos": amount,
                        "amount_iota": display::nanos_to_iota(*amount),
                        "recipient": recipient.to_string(),
                    })
                    .to_string())
                } else {
                    Ok(format!(
                        "Transaction sent!\n  Digest: {}\n  Status: {}\n  Amount: {} -> {}",
                        result.digest,
                        result.status,
                        display::format_balance(*amount),
                        recipient,
                    ))
                }
            }

            Command::ShowTransfers { filter } => {
                let txs = network.transactions(wallet.address(), filter.clone()).await?;
                if json_output {
                    let json_txs: Vec<serde_json::Value> = txs
                        .iter()
                        .map(|tx| {
                            serde_json::json!({
                                "digest": tx.digest,
                                "kind": tx.kind,
                                "timestamp": tx.timestamp,
                                "sender": tx.sender,
                                "amount": tx.amount,
                            })
                        })
                        .collect();
                    Ok(serde_json::to_string_pretty(&json_txs)?)
                } else {
                    Ok(display::format_transactions(&txs))
                }
            }

            Command::Faucet => {
                if wallet.is_mainnet() {
                    bail!("Faucet is not available on mainnet.");
                }
                network.faucet(wallet.address()).await?;
                let addr = wallet.address().to_string();
                if json_output {
                    Ok(serde_json::json!({
                        "status": "ok",
                        "address": addr,
                    })
                    .to_string())
                } else {
                    Ok(format!("Faucet tokens requested for {addr}. It may take a moment to arrive."))
                }
            }

            Command::Seed => {
                if json_output {
                    Ok(serde_json::json!({
                        "mnemonic": wallet.mnemonic(),
                    })
                    .to_string())
                } else {
                    Ok(format!(
                        "Seed phrase (keep this secret!):\n  {}",
                        wallet.mnemonic()
                    ))
                }
            }

            Command::Help { command } => Ok(help_text(command.as_deref())),

            Command::Exit => Ok(String::new()),
        }
    }
}

#[must_use]
pub fn help_text(command: Option<&str>) -> String {
    match command {
        Some("balance") | Some("bal") => {
            "balance\n  Show the IOTA balance for this wallet.\n  Alias: bal".to_string()
        }
        Some("address") | Some("addr") => {
            "address\n  Show the wallet's primary address.\n  Alias: addr".to_string()
        }
        Some("transfer") | Some("send") => {
            "transfer <address> <amount>\n  Send IOTA to another address.\n  Amount is in IOTA (e.g. '1.5' for 1.5 IOTA).\n  Alias: send".to_string()
        }
        Some("show_transfers") | Some("transfers") | Some("txs") => {
            "show_transfers [in|out|all]\n  Show transaction history.\n  Filter: 'in' (received), 'out' (sent), 'all' (default).\n  Aliases: transfers, txs".to_string()
        }
        Some("faucet") => {
            "faucet\n  Request test tokens from the faucet.\n  Only available on testnet and devnet.".to_string()
        }
        Some("seed") => {
            "seed\n  Display the wallet's seed phrase (mnemonic).\n  Keep this secret!".to_string()
        }
        Some("exit") | Some("quit") | Some("q") => {
            "exit\n  Exit the wallet.\n  Aliases: quit, q".to_string()
        }
        Some(other) => format!("Unknown command: '{other}'. Type 'help' for a list."),
        None => {
            "Available commands:\n\
             \n\
             \x20 balance          Show wallet balance\n\
             \x20 address          Show wallet address\n\
             \x20 transfer         Send IOTA to an address\n\
             \x20 show_transfers   Show transaction history\n\
             \x20 faucet           Request testnet/devnet tokens\n\
             \x20 seed             Show seed phrase\n\
             \x20 help [cmd]       Show help for a command\n\
             \x20 exit             Exit the wallet\n\
             \n\
             Type 'help <command>' for detailed help on a specific command."
                .to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_balance() {
        assert_eq!(Command::parse("balance").unwrap(), Command::Balance);
        assert_eq!(Command::parse("bal").unwrap(), Command::Balance);
        assert_eq!(Command::parse("  balance  ").unwrap(), Command::Balance);
    }

    #[test]
    fn parse_address() {
        assert_eq!(Command::parse("address").unwrap(), Command::Address);
        assert_eq!(Command::parse("addr").unwrap(), Command::Address);
    }

    #[test]
    fn parse_transfer() {
        let cmd = Command::parse(
            "transfer 0x0000a4984bd495d4346fa208ddff4f5d5e5ad48c21dec631ddebc99809f16900 1.5",
        )
        .unwrap();
        match cmd {
            Command::Transfer { recipient, amount } => {
                assert_eq!(
                    format!("{recipient}"),
                    "0x0000a4984bd495d4346fa208ddff4f5d5e5ad48c21dec631ddebc99809f16900"
                );
                assert_eq!(amount, 1_500_000_000);
            }
            other => panic!("expected Transfer, got {other:?}"),
        }
    }

    #[test]
    fn parse_transfer_alias() {
        let cmd = Command::parse(
            "send 0x0000a4984bd495d4346fa208ddff4f5d5e5ad48c21dec631ddebc99809f16900 2",
        )
        .unwrap();
        assert!(matches!(cmd, Command::Transfer { .. }));
    }

    #[test]
    fn parse_transfer_missing_amount() {
        let result = Command::parse(
            "transfer 0x0000a4984bd495d4346fa208ddff4f5d5e5ad48c21dec631ddebc99809f16900",
        );
        assert!(result.is_err());
    }

    #[test]
    fn parse_transfer_zero_amount() {
        let result = Command::parse(
            "transfer 0x0000a4984bd495d4346fa208ddff4f5d5e5ad48c21dec631ddebc99809f16900 0",
        );
        assert!(result.is_err());
    }

    #[test]
    fn parse_show_transfers() {
        assert_eq!(
            Command::parse("show_transfers").unwrap(),
            Command::ShowTransfers {
                filter: TransactionFilter::All
            }
        );
        assert_eq!(
            Command::parse("show_transfers in").unwrap(),
            Command::ShowTransfers {
                filter: TransactionFilter::In
            }
        );
        assert_eq!(
            Command::parse("txs out").unwrap(),
            Command::ShowTransfers {
                filter: TransactionFilter::Out
            }
        );
    }

    #[test]
    fn parse_faucet() {
        assert_eq!(Command::parse("faucet").unwrap(), Command::Faucet);
    }

    #[test]
    fn parse_seed() {
        assert_eq!(Command::parse("seed").unwrap(), Command::Seed);
    }

    #[test]
    fn parse_help() {
        assert_eq!(
            Command::parse("help").unwrap(),
            Command::Help { command: None }
        );
        assert_eq!(
            Command::parse("help balance").unwrap(),
            Command::Help {
                command: Some("balance".to_string())
            }
        );
    }

    #[test]
    fn parse_exit() {
        assert_eq!(Command::parse("exit").unwrap(), Command::Exit);
        assert_eq!(Command::parse("quit").unwrap(), Command::Exit);
        assert_eq!(Command::parse("q").unwrap(), Command::Exit);
    }

    #[test]
    fn parse_unknown_command() {
        let result = Command::parse("foobar");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("foobar"));
    }

    #[test]
    fn parse_empty_input() {
        assert!(Command::parse("").is_err());
        assert!(Command::parse("   ").is_err());
    }

    #[test]
    fn help_text_general() {
        let text = help_text(None);
        assert!(text.contains("balance"));
        assert!(text.contains("transfer"));
        assert!(text.contains("faucet"));
    }

    #[test]
    fn help_text_specific() {
        let text = help_text(Some("transfer"));
        assert!(text.contains("<address>"));
        assert!(text.contains("<amount>"));
    }

    #[test]
    fn help_text_unknown() {
        let text = help_text(Some("nonexistent"));
        assert!(text.contains("Unknown command"));
    }

    #[test]
    fn parse_case_insensitive() {
        assert_eq!(Command::parse("BALANCE").unwrap(), Command::Balance);
        assert_eq!(Command::parse("Balance").unwrap(), Command::Balance);
        assert_eq!(Command::parse("EXIT").unwrap(), Command::Exit);
    }

    #[test]
    fn seed_requires_confirmation() {
        assert!(Command::Seed.requires_confirmation());
        assert!(!Command::Balance.requires_confirmation());
        assert!(!Command::Address.requires_confirmation());
    }
}
