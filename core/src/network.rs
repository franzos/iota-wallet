/// Thin wrapper around the SDK's GraphQL client for network operations.
use anyhow::{Context, Result, bail};
use iota_sdk::crypto::ed25519::Ed25519PrivateKey;
use iota_sdk::crypto::IotaSigner;
use iota_sdk::graphql_client::faucet::FaucetClient;
use iota_sdk::graphql_client::pagination::PaginationFilter;
use iota_sdk::graphql_client::query_types::TransactionsFilter;
use iota_sdk::graphql_client::Client;
use iota_sdk::transaction_builder::TransactionBuilder;
use iota_sdk::types::Address;

use crate::wallet::{Network, NetworkConfig};

pub struct NetworkClient {
    client: Client,
    network: Network,
}

impl NetworkClient {
    pub fn new(config: &NetworkConfig) -> Result<Self> {
        let client = match &config.network {
            Network::Testnet => Client::new_testnet(),
            Network::Mainnet => Client::new_mainnet(),
            Network::Devnet => Client::new_devnet(),
            Network::Custom => {
                let url = config
                    .custom_url
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Custom network requires a node URL"))?;
                Client::new(url)
                    .context("Failed to create client with custom URL")?
            }
        };

        Ok(Self {
            client,
            network: config.network,
        })
    }

    /// Query the IOTA balance for an address (in nanos).
    pub async fn balance(&self, address: &Address) -> Result<u64> {
        let balance = self
            .client
            .balance(*address, None)
            .await
            .context("Failed to query balance")?;
        Ok(balance.unwrap_or(0))
    }

    /// Send IOTA from the signer's address to a recipient.
    /// Amount is in nanos (1 IOTA = 1_000_000_000 nanos).
    pub async fn send_iota(
        &self,
        private_key: &Ed25519PrivateKey,
        sender: &Address,
        recipient: Address,
        amount: u64,
    ) -> Result<TransferResult> {
        let mut builder = TransactionBuilder::new(*sender).with_client(&self.client);
        builder.send_iota(recipient, amount);

        let tx = builder
            .finish()
            .await
            .context("Failed to build transaction")?;

        // Dry run first to catch errors before spending gas
        let dry_run = self
            .client
            .dry_run_tx(&tx, false)
            .await
            .context("Dry run failed")?;
        if let Some(err) = dry_run.error {
            bail!("Transaction would fail: {err}");
        }

        let signature = private_key
            .sign_transaction(&tx)
            .map_err(|e| anyhow::anyhow!("Failed to sign transaction: {e}"))?;

        let effects = self
            .client
            .execute_tx(&[signature], &tx, None)
            .await
            .context("Failed to execute transaction")?;

        let digest = effects.digest().to_string();
        let status = format!("{:?}", effects.status());

        Ok(TransferResult { digest, status })
    }

    /// Request tokens from the faucet (testnet/devnet only).
    pub async fn faucet(&self, address: &Address) -> Result<()> {
        match &self.network {
            Network::Mainnet => bail!("Faucet is not available on mainnet"),
            Network::Testnet => {
                FaucetClient::new_testnet()
                    .request_and_wait(*address)
                    .await
                    .map_err(|e| anyhow::anyhow!("Faucet request failed: {e}"))?;
            }
            Network::Devnet => {
                FaucetClient::new_devnet()
                    .request_and_wait(*address)
                    .await
                    .map_err(|e| anyhow::anyhow!("Faucet request failed: {e}"))?;
            }
            Network::Custom => {
                bail!("Faucet is not available for custom networks. Use --testnet or --devnet.");
            }
        }
        Ok(())
    }

    /// Query recent transactions involving the given address.
    pub async fn transactions(
        &self,
        address: &Address,
        filter: TransactionFilter,
    ) -> Result<Vec<TransactionSummary>> {
        let gql_filter = match filter {
            TransactionFilter::In => TransactionsFilter {
                recv_address: Some(*address),
                ..Default::default()
            },
            TransactionFilter::Out => TransactionsFilter {
                sign_address: Some(*address),
                ..Default::default()
            },
            TransactionFilter::All => {
                // The SDK filter doesn't support OR, so query both and merge
                let sent = self.query_transactions(
                    TransactionsFilter {
                        sign_address: Some(*address),
                        ..Default::default()
                    },
                ).await?;
                let recv = self.query_transactions(
                    TransactionsFilter {
                        recv_address: Some(*address),
                        ..Default::default()
                    },
                ).await?;

                let mut all = sent;
                for tx in recv {
                    if !all.iter().any(|t| t.digest == tx.digest) {
                        all.push(tx);
                    }
                }
                // TODO: sort by timestamp descending once timestamps are available
                all.sort_by(|a, b| b.digest.cmp(&a.digest));
                return Ok(all);
            }
        };

        self.query_transactions(gql_filter).await
    }

    async fn query_transactions(
        &self,
        filter: TransactionsFilter,
    ) -> Result<Vec<TransactionSummary>> {
        let page = self
            .client
            .transactions(Some(filter), PaginationFilter::default())
            .await
            .context("Failed to query transactions")?;

        let summaries = page
            .data()
            .iter()
            .map(|tx| {
                let digest = tx.transaction.digest().to_string();
                TransactionSummary {
                    digest,
                    kind: "transaction".to_string(),
                    // TODO: populate from SDK when richer transaction data is available
                    timestamp: None,
                    sender: None,
                    amount: None,
                }
            })
            .collect();

        Ok(summaries)
    }

    pub fn network(&self) -> &Network {
        &self.network
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}

pub struct TransferResult {
    pub digest: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransactionFilter {
    All,
    In,
    Out,
}

impl std::str::FromStr for TransactionFilter {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "in" => Ok(Self::In),
            "out" => Ok(Self::Out),
            "all" => Ok(Self::All),
            other => Err(format!(
                "Unknown transaction filter: '{other}'. Use 'in', 'out', or 'all'."
            )),
        }
    }
}

impl TransactionFilter {
    pub fn from_str_opt(s: Option<&str>) -> Self {
        s.and_then(|s| s.parse().ok()).unwrap_or(Self::All)
    }
}

#[derive(Debug, Clone)]
pub struct TransactionSummary {
    pub digest: String,
    pub kind: String,
    /// Transaction timestamp, if available from the SDK.
    pub timestamp: Option<String>,
    /// Sender address, if available from the SDK.
    pub sender: Option<String>,
    /// Amount in nanos, if available from the SDK.
    pub amount: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_network_without_url_fails() {
        let config = NetworkConfig {
            network: Network::Custom,
            custom_url: None,
        };

        let result = NetworkClient::new(&config);
        assert!(result.is_err(), "Custom network without URL should fail");
        let err = result.err().expect("already checked is_err").to_string();
        assert!(
            err.contains("Custom network requires a node URL"),
            "error should mention missing URL, got: {err}"
        );
    }
}
