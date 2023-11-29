use ethers::providers::{JsonRpcClient, Middleware, Provider};
use ethers::types::{Block, Transaction, TxHash, H256};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Refund {
    pub signal_tx: H256,
    pub refund_tx: H256,
    pub value: u64,
}

impl Refund {
    pub async fn scan_refund<T: JsonRpcClient>(
        tx: &Transaction,
        block: &Block<TxHash>,
        eth_client: &Provider<T>,
    ) -> Option<Refund> {
        let txn_index = tx.transaction_index.unwrap().as_u64();
        let txns = &block.transactions.as_slice()[(txn_index + 1) as usize..];
        let from = tx.from; // User
        let builder = block.author.unwrap();

        // Check for refund txn from builder to user
        for txn_hash in txns {
            // Get the txn from the hash
            let txn = eth_client
                .get_transaction(*txn_hash)
                .await
                .unwrap()
                .unwrap();
            if txn.from == builder && txn.to.unwrap() == from {
                return Some(Refund {
                    signal_tx: tx.hash,
                    refund_tx: txn.hash,
                    value: txn.value.as_u64(),
                });
            }
        }

        None
    }
}
