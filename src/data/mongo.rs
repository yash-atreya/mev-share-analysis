use super::event::Event;
use ethers::types::H256;
use mongodb::{
    bson::{doc, Document},
    options::{ClientOptions, FindOptions, Hint as IndexHint, UpdateOptions},
    Client, Collection, Cursor,
};
#[derive(Clone)]
pub struct MongoClient {
    pub client: Client,
    pub collection: Collection<Event>,
}

const APP_NAME: &str = "MEV-Share-Analytics";

impl MongoClient {
    pub async fn new(conn_str: &str, db_name: &str, collection_name: &str) -> Self {
        let mut client_options = ClientOptions::parse(conn_str).await.unwrap();
        client_options.app_name = Some(APP_NAME.to_string());
        let client = Client::with_options(client_options).unwrap();

        let collection: Collection<Event> = client.database(db_name).collection(collection_name);

        MongoClient { client, collection }
    }

    pub async fn write_event(&self, event: Event) {
        let res = self
            .collection
            .insert_one(event, None)
            .await
            .unwrap_or_else(|error| panic!("Failed to insert event: {}", error));
        println!("Inserted event with _id: {:?}", res.inserted_id);
    }

    pub async fn read_events(
        &self,
        filter: Option<Document>,
        find_options: Option<FindOptions>,
    ) -> Cursor<Event> {
        self.collection
            .find(filter, find_options)
            .await
            .unwrap_or_else(|error| panic!("Failed to read event: {}", error))
    }

    pub async fn read_event(&self, hash: H256) -> Option<Event> {
        let filter = doc! {"hint.hash": serde_json::to_string(&hash).unwrap().replace('\"', "")}; // TODO: Find better way, super unsexy rust.
        self.collection
            .find_one(filter, None)
            .await
            .unwrap_or_else(|error| panic!("Failed to read event: {}", error))
    }

    pub async fn count(&self) -> u64 {
        self.collection
            .count_documents(None, None)
            .await
            .unwrap_or_else(|error| panic!("Failed to count events: {}", error))
    }

    pub async fn write_events(&self, events: Vec<Event>) -> u64 {
        let res = self
            .collection
            .insert_many(events, None)
            .await
            .unwrap_or_else(|error| panic!("Failed to insert events: {}", error));
        res.inserted_ids.len() as u64
    }

    pub async fn update_event(&self, hash: H256, update: Document) -> u64 {
        let filter = doc! {"hint.hash": serde_json::to_string(&hash).unwrap().replace('\"', "")};
        let index_hint = IndexHint::Keys(doc! {"hint.hash": 1});
        let options = UpdateOptions::builder()
            .hint(index_hint)
            .bypass_document_validation(true)
            .build();
        let res = self
            .collection
            .update_many(filter, update, options)
            .await
            .unwrap_or_else(|error| panic!("Failed to update event: {}", error));
        res.modified_count
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::super::super::refunds::{landing::Landing, refund::Refund};
    use super::*;
    use ethers::types::{H160, H256, U256};
    use mev_share::sse::{EventHistory, Hint};
    use mongodb::{
        bson::{doc, to_document, Document},
        options::{AggregateOptions, ClientOptions},
        Client, Collection,
    };

    const DB_NAME: &str = "mev-share-test";
    const EVENTS_COLLECTION: &str = "events";
    const MONGO_CONN_STR: &str = "mongodb://localhost:27017";
    #[tokio::test]
    async fn test_write_event() {
        let TEST_EVENT: Event = Event::new(EventHistory {
            hint: Hint {
                hash: H256::random(),
                txs: vec![],
                logs: vec![],
                mev_gas_price: None,
                gas_used: None,
            },
            block: 00000000,
            timestamp: 00000000,
        });
        // Create mongo client
        let mongo_client = MongoClient::new(MONGO_CONN_STR, DB_NAME, EVENTS_COLLECTION).await;

        // Write event
        mongo_client.write_event(TEST_EVENT.clone()).await;

        // Read event
        let read_event = mongo_client.read_event(TEST_EVENT.hint.hash).await;

        match read_event {
            Some(event) => assert_eq!(event, TEST_EVENT),
            None => println!("No event found for provided hash"),
        }
    }

    #[tokio::test]
    async fn test_update_event() {
        let TEST_EVENT: Event = Event::new(EventHistory {
            hint: Hint {
                hash: H256::random(),
                txs: vec![],
                logs: vec![],
                mev_gas_price: None,
                gas_used: None,
            },
            block: 00000000,
            timestamp: 00000000,
        });

        // Create mongo client
        let mongo_client = MongoClient::new(MONGO_CONN_STR, DB_NAME, EVENTS_COLLECTION).await;

        // Write event
        mongo_client.write_event(TEST_EVENT.clone()).await;

        // Refund
        let refund = Refund {
            signal_tx: H256::zero(),
            refund_tx: H256::zero(),
            value: U256::zero().as_u64(),
        };
        let refund_doc = to_document(&refund).unwrap();

        // Landing
        let landing = Landing {
            block: 00000001,
            timestamp: 00000002,
            builder: H160::random(),
        };
        let landing_doc = to_document(&landing).unwrap();
        // Update event
        let update = doc! {"$set": {"refund": refund_doc, "landing": landing_doc, "landed": true}};
        let res = mongo_client
            .update_event(TEST_EVENT.hint.hash, update)
            .await;

        // Read event
        let read_event = mongo_client.read_event(TEST_EVENT.hint.hash).await;

        match read_event {
            Some(event) => assert_eq!(event.block, 00000000),
            None => println!("No event found for provided hash"),
        }
    }
}
