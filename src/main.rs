use dotenv::dotenv;
use ethers::providers::{Http, JsonRpcClient, Provider};
use futures::TryStreamExt;
use mev_share::sse::{EventClient, EventHistory, EventHistoryInfo, EventHistoryParams};
use mev_share_analysis::{
    cli::{Cli, Commands},
    data::{event::Event, mongo::MongoClient},
    refunds::{landing::Landing, refund::Refund},
};
use mongodb::{
    bson::{doc, to_document},
    options::{FindOptions, Hint as IndexHint},
};
use std::{env, thread::available_parallelism};
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    // SETUP

    // ENDPOINTS
    const HISTORY: &str = "https://mev-share.flashbots.net/api/v1/history";
    const HISTORY_INFO: &str = "https://mev-share.flashbots.net/api/v1/history/info";
    let _rpc_url = env::var("RPC_URL").unwrap_or_else(|_| "http://localhost:8545".into());
    const MONGO_URL: &str = "mongodb://localhost:27017";
    // let ipc_path = "/tmp/reth.ipc"; // Use when you're running a local node

    // PROVIDER
    // let provider = Provider::connect_ipc(ipc_path).await.unwrap();
    let provider = Provider::<Http>::try_from(_rpc_url).unwrap(); // Unused. Ideally use your own node if using the RPC.

    // MONGO
    const DB_NAME: &str = "mev-share-test";
    const EVENTS_COLLECTION: &str = "events"; // Name of the collection to save the events in.
    let mongo = MongoClient::new(MONGO_URL, DB_NAME, EVENTS_COLLECTION).await;

    let mev_share_client = EventClient::default();
    let cli = Cli::parse_args();

    match cli.command {
        Some(Commands::Events {
            block_start,
            block_end,
        }) => {
            let info = get_historical_info(&mev_share_client, HISTORY_INFO).await;
            let offset = 0; // Offset the request by `offset` events.
            println!(
                "Fetching MEV-Share events from block {} to block {}",
                block_start.unwrap_or(info.min_block),
                block_end.unwrap_or(info.max_block)
            );
            let start = std::time::Instant::now();
            fetch_history(
                &mev_share_client,
                &mongo,
                HISTORY,
                info,
                offset,
                block_start,
                block_end,
            )
            .await;
            let end = std::time::Instant::now();
            println!("Took {:?} to fetch events", end - start);
        }
        Some(Commands::ScanRefunds) => {
            println!("Retrieving refunds for events in db...");
            // TODO: Add filter for querying by block in range `block_start : block_end` and using the index `block: 1`.
            let available_cores = available_parallelism()
                .unwrap_or(std::num::NonZeroUsize::try_from(4).unwrap())
                .get() as u64; // Uses all cores if available.
            let count = mongo.count().await; // Num. of documents in DB.

            // TODO: Divide blocks_per_core instead of docs_per_core when querying by block.
            let docs_per_core = (count - (count % available_cores)) / available_cores;

            let mut skip_docs: u64 = 0;
            let mut handlers = vec![];
            for _i in 0..available_cores {
                let index_hint = IndexHint::Keys(doc! {"hint.hash": 1});
                let options = FindOptions::builder()
                    .batch_size(10_000_000) // 10 million
                    .allow_disk_use(true)
                    .hint(index_hint)
                    .skip(skip_docs)
                    .limit(docs_per_core as i64)
                    .build();
                skip_docs += docs_per_core; // TODO: Reconfigure this when querying by block.
                let mongo = mongo.clone();
                let provider = provider.clone();
                handlers.push(tokio::task::spawn(async move {
                    check_landing_and_refund(&provider, options, &mongo).await
                }));
            }

            let start = std::time::Instant::now();
            let results = futures::future::join_all(handlers).await;
            let end = std::time::Instant::now();

            let mut total_landings = 0;
            let mut total_refunded = 0;
            let mut total_refunds = 0;
            let mut total_iterations = 0;
            for result in results {
                let (landings, refunded, refunds, iterations) = result.unwrap();
                total_landings += landings;
                total_refunded += refunded;
                total_refunds += refunds;
                total_iterations += iterations;
            }

            println!("Took {:?} to scan {} events", end - start, total_iterations);
            println!(
                "Total landings: {} | Total refunds: {} | Total refunded: {} wei",
                total_landings, total_refunds, total_refunded
            );
        }
        None => {
            println!("No command provided");
        }
    }

    Ok(())
}

async fn get_historical_info(client: &EventClient, endpoint: &str) -> EventHistoryInfo {
    client
        .event_history_info(endpoint)
        .await
        .expect("Failed to get historical info")
}

async fn get_historical_events(
    client: &EventClient,
    endpoint: &str,
    params: &EventHistoryParams,
) -> Result<Vec<EventHistory>, reqwest::Error> {
    client.event_history(endpoint, params.clone()).await
}

async fn fetch_history(
    client: &EventClient,
    mongo: &MongoClient,
    endpoint: &str,
    info: EventHistoryInfo,
    offset: u64,
    block_start: Option<u64>,
    block_end: Option<u64>,
) {
    // Set Initial Params
    let mut params = EventHistoryParams {
        block_start: if block_start.is_some() {
            block_start
        } else {
            Some(info.min_block)
        },
        block_end: if block_end.is_some() {
            block_end
        } else {
            Some(info.max_block)
        },
        timestamp_start: None,
        timestamp_end: None,
        limit: Some(info.max_limit),
        offset: Some(offset),
    };
    let mut sync_complete = false;
    loop {
        // Get Historical Events
        match get_historical_events(client, endpoint, &params).await {
            Ok(events) => {
                let mut next_offset = params.offset.unwrap();
                if events.is_empty() {
                    // Fetch new info
                    let new_info = get_historical_info(
                        client,
                        "https://mev-share.flashbots.net/api/v1/history/info",
                    )
                    .await;

                    if block_end.is_some() && block_end.unwrap() < new_info.max_block {
                        println!("Fetched events till block {}", block_end.unwrap());
                        println!("Exiting...");
                        break;
                    }
                    // Update Params
                    params.block_end = Some(new_info.max_block);
                    params.limit = Some(new_info.max_limit);
                    params.offset = Some(next_offset);
                    println!("Sleeping for 12 seconds...waiting for events to be indexed");
                    std::thread::sleep(std::time::Duration::from_secs(12));
                } else {
                    // Map the incoming events<EventHistory> to events<Event>
                    let events: Vec<Event> = events.into_iter().map(Event::new).collect();
                    // Write Events to DB
                    let num_events_written = mongo.write_events(events).await;

                    // Update Params
                    next_offset = params.offset.unwrap() + num_events_written;
                    params.offset = Some(next_offset);

                    // Check if Sync Complete
                    if next_offset >= info.count && !sync_complete {
                        println!("Sync Complete!");
                        sync_complete = true;
                    }
                }
            }
            Err(_error) => {
                // @Dev Occasionally, the MEV-Share API will return an error. We do not update the offset and retry in the next iteration.
            }
        }
    }
}

async fn check_landing_and_refund<T: JsonRpcClient>(
    provider: &Provider<T>,
    find_options: FindOptions,
    mongo: &MongoClient,
) -> (u64, u128, u64, u64) {
    // Read Events from DB using cursor
    let mut cursor = mongo.read_events(None, Some(find_options)).await;

    let mut iterations = 0;
    let mut total_refunded: u128 = 0;
    let mut total_landings: u64 = 0;
    let mut total_refunds: u64 = 0;
    while let Some(event_doc) = cursor.try_next().await.unwrap() {
        // Check if event landed onchain using hint.hash
        let hash = event_doc.hint.hash;
        let (landing, target_tx, landing_block) = Landing::get_landing_for_tx(hash, provider).await;

        match landing {
            Some(landing) => {
                // Check if refund txn exists
                let target_txn = target_tx.unwrap();
                let block = landing_block.unwrap();
                total_landings += 1;
                let refund = Refund::scan_refund(&target_txn, &block, provider).await;

                match refund {
                    Some(refund) => {
                        // Update event in DB with landed: true and refund params
                        let update = doc! {"$set": doc! {"landed": true, "landing": to_document(&landing).unwrap(), "refund": to_document(&refund).unwrap()}};
                        mongo.update_event(hash, update).await;
                        total_refunds += 1;
                        total_refunded += refund.value as u128;
                    }
                    None => {
                        // Update event in DB with landed: true and landing parameters.
                        let update = doc! {"$set": doc! {"landed": true, "landing": to_document(&landing).unwrap()}};
                        mongo.update_event(hash, update).await;
                    }
                }
            }
            None => {
                // Do nothing
            }
        }
        iterations += 1;
    }
    (total_landings, total_refunded, total_refunds, iterations)
}
