use std::collections::HashSet;

use anyhow::{Context, Result};
use iota_sdk::graphql_client::pagination::{Direction, PaginationFilter};
use iota_sdk::graphql_client::query_types::TransactionsFilter;
use iota_sdk::types::{Digest, Transaction};

use super::transfer::{extract_transfer_amount, extract_transfer_recipient};
use super::types::{
    transaction_summary_from_graphql, TransactionDetailsSummary, TransactionDirection,
    TransactionFilter, TransactionSummary,
};
use super::NetworkClient;
use crate::cache::TransactionCache;

impl NetworkClient {
    /// Query recent transactions involving the given address.
    ///
    /// Always queries both sent and received to determine true direction,
    /// since outgoing txs can also appear in recv queries (change).
    pub async fn transactions(
        &self,
        address: &iota_sdk::types::Address,
        filter: TransactionFilter,
    ) -> Result<Vec<TransactionSummary>> {
        let (sent, recv) = futures::try_join!(
            self.query_transactions(
                TransactionsFilter {
                    sign_address: Some(*address),
                    ..Default::default()
                },
                TransactionDirection::Out,
            ),
            self.query_transactions(
                TransactionsFilter {
                    recv_address: Some(*address),
                    ..Default::default()
                },
                TransactionDirection::In,
            ),
        )?;

        // Merge: sent takes priority (a tx you signed is "out" even if you also received change)
        let seen: HashSet<String> = sent.iter().map(|t| t.digest.clone()).collect();
        let mut all = sent;
        for tx in recv {
            if !seen.contains(&tx.digest) {
                all.push(tx);
            }
        }
        all.sort_by(|a, b| {
            b.epoch
                .cmp(&a.epoch)
                .then(b.lamport_version.cmp(&a.lamport_version))
        });

        // Apply filter
        match filter {
            TransactionFilter::All => Ok(all),
            TransactionFilter::In => Ok(all
                .into_iter()
                .filter(|t| t.direction == Some(TransactionDirection::In))
                .collect()),
            TransactionFilter::Out => Ok(all
                .into_iter()
                .filter(|t| t.direction == Some(TransactionDirection::Out))
                .collect()),
        }
    }

    async fn query_transactions(
        &self,
        filter: TransactionsFilter,
        direction: TransactionDirection,
    ) -> Result<Vec<TransactionSummary>> {
        let page = self
            .client
            .transactions_data_effects(Some(filter), PaginationFilter::default())
            .await
            .context("Failed to query transactions")?;

        let summaries = page
            .data()
            .iter()
            .map(|item| transaction_summary_from_graphql(item, direction))
            .collect();

        Ok(summaries)
    }

    /// Sync transactions for the given address into the local cache.
    ///
    /// Opens the cache internally so no `&TransactionCache` is held across
    /// await points (Connection is Send but not Sync).
    ///
    /// Fetches up to `lookback_epochs` of history. Already-cached transactions
    /// are skipped but don't stop pagination (the window may have widened).
    pub async fn sync_transactions(
        &self,
        address: &iota_sdk::types::Address,
        lookback_epochs: u64,
    ) -> Result<()> {
        let network_str = self.network.to_string();
        let address_str = address.to_string();

        // Phase 1: read known digests + last sync point from cache (sync, then drop)
        let (known, last_synced_epoch) = {
            let cache = TransactionCache::open()?;
            let k = cache.known_digests(&network_str, &address_str)?;
            let e = cache.get_sync_epoch(&network_str, &address_str)?;
            (k, e)
        };

        // Phase 2: fetch from network (async — no cache held)
        let current_epoch = self
            .client
            .epoch(None)
            .await
            .context("Failed to query current epoch")?
            .context("No epoch data available")?
            .epoch_id;

        let min_epoch = sync_min_epoch(current_epoch, lookback_epochs, last_synced_epoch);

        let (sent, recv) = futures::try_join!(
            self.fetch_paginated(
                TransactionsFilter {
                    sign_address: Some(*address),
                    ..Default::default()
                },
                TransactionDirection::Out,
                &known,
                min_epoch,
            ),
            self.fetch_paginated(
                TransactionsFilter {
                    recv_address: Some(*address),
                    ..Default::default()
                },
                TransactionDirection::In,
                &known,
                min_epoch,
            ),
        )?;

        // Phase 3: write results atomically (all-or-nothing)
        let cache = TransactionCache::open()?;
        cache.commit_sync(&network_str, &address_str, &sent, &recv, current_epoch)?;

        Ok(())
    }

    /// Paginate backward (newest first) through transactions, collecting results
    /// until we hit known digests or pass the minimum epoch.
    async fn fetch_paginated(
        &self,
        filter: TransactionsFilter,
        direction: TransactionDirection,
        known: &HashSet<String>,
        min_epoch: u64,
    ) -> Result<Vec<TransactionSummary>> {
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let pagination = PaginationFilter {
                direction: Direction::Backward,
                cursor: cursor.clone(),
                limit: Some(50),
            };

            let page = self
                .client
                .transactions_data_effects(Some(filter.clone()), pagination)
                .await
                .context("Failed to query transactions")?;

            let data = page.data();
            if data.is_empty() {
                break;
            }

            let mut hit_boundary = false;
            for item in data {
                let digest = item.tx.transaction.digest().to_string();
                let epoch = item.effects.epoch();

                // Skip items outside the lookback window
                if epoch < min_epoch {
                    hit_boundary = true;
                    continue;
                }

                // Skip already-known transactions (dedup only — don't stop
                // pagination, since the lookback window may have widened).
                if known.contains(&digest) {
                    continue;
                }

                all.push(transaction_summary_from_graphql(item, direction));
            }

            let info = page.page_info();
            if hit_boundary || !info.has_previous_page {
                break;
            }
            cursor = info.start_cursor.clone();
        }

        Ok(all)
    }

    /// Look up a transaction by its digest, returning data and effects.
    pub async fn transaction_details(&self, digest: &Digest) -> Result<TransactionDetailsSummary> {
        let data_effects = self
            .client
            .transaction_data_effects(*digest)
            .await
            .context("Failed to query transaction")?
            .ok_or_else(|| anyhow::anyhow!("Transaction not found: {digest}"))?;

        let tx = &data_effects.tx.transaction;
        let effects = &data_effects.effects;

        let (sender, amount) = match tx {
            Transaction::V1(v1) => {
                let sender = v1.sender.to_string();
                let amount = extract_transfer_amount(&v1.kind);
                (sender, amount)
            }
        };

        let status = format!("{:?}", effects.status());
        let gas = effects.gas_summary();
        let net = gas.net_gas_usage();
        let fee = u64::try_from(net).ok().filter(|&f| f > 0);

        // Try to extract the recipient from the TransferObjects command
        let recipient = match tx {
            Transaction::V1(v1) => extract_transfer_recipient(&v1.kind),
        };

        Ok(TransactionDetailsSummary {
            digest: digest.to_string(),
            status,
            sender,
            recipient,
            amount,
            fee,
        })
    }
}

/// Compute the oldest epoch to fetch, bridging any gap between the last
/// sync point and the current lookback window.
///
/// `last_synced_epoch` is the `current_epoch` recorded during the previous
/// sync (0 means "never synced").
fn sync_min_epoch(current_epoch: u64, lookback_epochs: u64, last_synced_epoch: u64) -> u64 {
    let window_min = current_epoch.saturating_sub(lookback_epochs);
    if last_synced_epoch > 0 && last_synced_epoch < window_min {
        last_synced_epoch
    } else {
        window_min
    }
}

#[cfg(test)]
mod tests {
    use super::sync_min_epoch;

    #[test]
    fn first_sync() {
        // Never synced before — use the lookback window as-is
        assert_eq!(sync_min_epoch(100, 7, 0), 93);
    }

    #[test]
    fn first_sync_short_history() {
        // Current epoch smaller than lookback — saturates to 0
        assert_eq!(sync_min_epoch(3, 7, 0), 0);
    }

    #[test]
    fn no_gap() {
        // Last sync is recent, falls within the lookback window
        assert_eq!(sync_min_epoch(15, 7, 14), 8);
    }

    #[test]
    fn bridge_gap() {
        // Offline gap: last sync at epoch 3, now at 14 with 7-day lookback.
        // window_min would be 7, but we need to go back to 3 to fill 4–6.
        assert_eq!(sync_min_epoch(14, 7, 3), 3);
    }

    #[test]
    fn bridge_long_absence() {
        // Synced at epoch 3, now at 100 with 7-day lookback.
        // Bridges all the way back to the last sync point.
        assert_eq!(sync_min_epoch(100, 7, 3), 3);
    }

    #[test]
    fn widened_window_no_bridge_needed() {
        // Synced at epoch 100 with lookback 7, now at 101 with lookback 30.
        // last_synced (100) > window_min (71) — no gap to bridge.
        // (fetch_paginated handles the wider window by paging past known txs.)
        assert_eq!(sync_min_epoch(101, 30, 100), 71);
    }

    #[test]
    fn last_synced_exactly_at_window_min() {
        // Edge case: last_synced == window_min — no gap
        assert_eq!(sync_min_epoch(100, 7, 93), 93);
    }
}
