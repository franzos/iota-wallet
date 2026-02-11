use std::sync::Arc;

use anyhow::Result;
use iota_sdk::types::{Address, Digest, ObjectId};

use crate::network::{
    NetworkClient, NetworkStatus, StakedIotaSummary, TokenBalance, TransactionDetailsSummary,
    TransferResult,
};
use crate::signer::Signer;

pub struct WalletService {
    network: NetworkClient,
    signer: Arc<dyn Signer>,
    network_name: String,
}

impl WalletService {
    pub fn new(
        network: NetworkClient,
        signer: Arc<dyn Signer>,
        network_name: String,
    ) -> Self {
        Self {
            network,
            signer,
            network_name,
        }
    }

    pub fn address(&self) -> &Address {
        self.signer.address()
    }

    pub fn network_name(&self) -> &str {
        &self.network_name
    }

    pub fn signer(&self) -> &Arc<dyn Signer> {
        &self.signer
    }

    pub async fn balance(&self) -> Result<u64> {
        self.network.balance(self.signer.address()).await
    }

    pub async fn send(&self, recipient: Address, amount: u64) -> Result<TransferResult> {
        self.network
            .send_iota(self.signer.as_ref(), self.signer.address(), recipient, amount)
            .await
    }

    pub async fn sweep_all(&self, recipient: Address) -> Result<(TransferResult, u64)> {
        self.network
            .sweep_all(self.signer.as_ref(), self.signer.address(), recipient)
            .await
    }

    pub async fn stake(&self, validator: Address, amount: u64) -> Result<TransferResult> {
        self.network
            .stake_iota(self.signer.as_ref(), self.signer.address(), validator, amount)
            .await
    }

    pub async fn unstake(&self, staked_object_id: ObjectId) -> Result<TransferResult> {
        self.network
            .unstake_iota(self.signer.as_ref(), self.signer.address(), staked_object_id)
            .await
    }

    pub async fn faucet(&self) -> Result<()> {
        self.network.faucet(self.signer.address()).await
    }

    pub async fn get_stakes(&self) -> Result<Vec<StakedIotaSummary>> {
        self.network.get_stakes(self.signer.address()).await
    }

    pub async fn get_token_balances(&self) -> Result<Vec<TokenBalance>> {
        self.network.get_token_balances(self.signer.address()).await
    }

    pub async fn sync_transactions(&self) -> Result<()> {
        self.network.sync_transactions(self.signer.address()).await
    }

    pub async fn transaction_details(&self, digest: &Digest) -> Result<TransactionDetailsSummary> {
        self.network.transaction_details(digest).await
    }

    pub async fn status(&self) -> Result<NetworkStatus> {
        self.network.status().await
    }
}
