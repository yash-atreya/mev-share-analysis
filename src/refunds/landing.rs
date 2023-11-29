use ethers::providers::{JsonRpcClient, Middleware, Provider};
use ethers::types::{Address, Block, Transaction, TxHash, H256};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Landing {
    pub block: u64,
    pub timestamp: u64,
    pub builder: Address,
}

impl Landing {
    pub async fn get_landing_for_tx<T: JsonRpcClient>(
        target_hash: H256,
        eth_client: &Provider<T>,
    ) -> (Option<Landing>, Option<Transaction>, Option<Block<TxHash>>) {
        let tx = eth_client.get_transaction(target_hash).await.unwrap();
        match tx {
            Some(tx) => {
                let block_number = tx.block_number.unwrap().as_u64();
                let block = eth_client.get_block(block_number).await.unwrap().unwrap();
                let timestamp = block.timestamp.as_u64();
                let builder: Address = block.author.unwrap();
                (
                    Some(Landing {
                        block: block_number,
                        timestamp,
                        builder,
                    }),
                    Some(tx),
                    Some(block),
                )
            }
            None => (None, None, None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotenv::dotenv;
    use ethers::providers::{Http, Middleware, Provider};
    use ethers::types::{Address, H256};
    use std::str::FromStr;

    #[tokio::test]
    async fn test_get_landing_for_tx() {
        dotenv().ok();
        let rpc = std::env::var("RPC_URL").expect("RPC_URL not set");
        let provider = Provider::<Http>::try_from(rpc).unwrap();
        // let provider = Provider::<Ipc>::connect_ipc("tmp/reth.ipc").await.unwrap();
        let landing = Landing::get_landing_for_tx(
            H256::from_str("0x604a87e9837c45ea4289089bfa22f97a0c91ee7e3d88da2bef59ebf35322092f")
                .unwrap(),
            &provider,
        )
        .await;

        match landing.0 {
            Some(landing) => {
                assert_eq!(
                    landing,
                    Landing {
                        block: 17650375,
                        timestamp: 1688835419,
                        builder: Address::from_str("0xdafea492d9c6733ae3d56b7ed1adb60692c98bc5")
                            .unwrap(),
                    }
                );
            }
            None => println!("Tx did not land onchain"),
        }
    }
}
