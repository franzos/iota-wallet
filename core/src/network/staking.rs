use anyhow::{Context, Result};
use iota_sdk::transaction_builder::TransactionBuilder;
use iota_sdk::types::{Address, ObjectId};

use super::types::{StakeStatus, StakedIotaSummary, TransferResult};
use super::NetworkClient;
use crate::signer::Signer;

/// Extract a string field from a JSON value and parse it via `FromStr`.
fn json_str_field<T: std::str::FromStr>(node: &serde_json::Value, key: &str) -> Option<T> {
    node.get(key)
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
}

/// Extract a hex-encoded ObjectId from a JSON value.
fn json_object_id(node: &serde_json::Value, key: &str) -> Option<ObjectId> {
    node.get(key)
        .and_then(|v| v.as_str())
        .and_then(|s| ObjectId::from_hex(s).ok())
}

impl NetworkClient {
    /// Stake IOTA to a validator.
    /// Amount is in nanos (1 IOTA = 1_000_000_000 nanos).
    pub async fn stake_iota(
        &self,
        signer: &dyn Signer,
        sender: &Address,
        validator: Address,
        amount: u64,
    ) -> Result<TransferResult> {
        let mut builder = TransactionBuilder::new(*sender).with_client(&self.client);
        builder.stake(amount, validator);
        let tx = builder
            .finish()
            .await
            .context("Failed to build stake transaction")?;
        self.sign_and_execute(&tx, signer).await
    }

    /// Unstake a previously staked IOTA object.
    pub async fn unstake_iota(
        &self,
        signer: &dyn Signer,
        sender: &Address,
        staked_object_id: ObjectId,
    ) -> Result<TransferResult> {
        let mut builder = TransactionBuilder::new(*sender).with_client(&self.client);
        builder.unstake(staked_object_id);
        let tx = builder
            .finish()
            .await
            .context("Failed to build unstake transaction")?;
        self.sign_and_execute(&tx, signer).await
    }

    /// Query all StakedIota objects owned by the given address, including
    /// estimated rewards computed by the network.
    pub async fn get_stakes(&self, address: &Address) -> Result<Vec<StakedIotaSummary>> {
        let query = serde_json::json!({
            "query": r#"query ($owner: IotaAddress!) {
                address(address: $owner) {
                    stakedIotas {
                        nodes {
                            address
                            stakeStatus
                            activatedEpoch { epochId }
                            poolId
                            principal
                            estimatedReward
                        }
                    }
                }
            }"#,
            "variables": {
                "owner": address.to_string()
            }
        });

        let data = self
            .execute_query(query, "Failed to query staked objects")
            .await?;
        let nodes = data
            .get("address")
            .and_then(|a| a.get("stakedIotas"))
            .and_then(|s| s.get("nodes"))
            .and_then(|n| n.as_array())
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        let mut stakes = Vec::new();
        for node in nodes {
            let object_id = json_object_id(node, "address");
            let pool_id = json_object_id(node, "poolId");
            let principal = json_str_field::<u64>(node, "principal").unwrap_or(0);
            let stake_activation_epoch = node
                .get("activatedEpoch")
                .and_then(|v| v.get("epochId"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let estimated_reward = json_str_field::<u64>(node, "estimatedReward");
            let status = match node.get("stakeStatus").and_then(|v| v.as_str()) {
                Some("ACTIVE") => StakeStatus::Active,
                Some("PENDING") => StakeStatus::Pending,
                _ => StakeStatus::Unstaked,
            };

            if let (Some(object_id), Some(pool_id)) = (object_id, pool_id) {
                stakes.push(StakedIotaSummary {
                    object_id,
                    pool_id,
                    principal,
                    stake_activation_epoch,
                    estimated_reward,
                    status,
                });
            }
        }

        Ok(stakes)
    }
}
