# MEV-Share Analysis

This repository can be used to retrieve historical events sent to [mev-share](https://docs.flashbots.net/flashbots-protect/mev-share) and scan for any refunds that were sent to the users. This is still a WIP 🚧 and but it gets the job done and was useful in writing this [thread for users on how to configure the hint params](https://x.com/YashAtreya/status/1727486273558835420?s=20) to have a better chance at getting a refund if mev is generated.

This repository can be used by data analysts and searchers.

## Installation

**Prerequisites**

- [Rust](https://www.rust-lang.org/tools/install)
- [MongoDB](https://docs.mongodb.com/manual/installation/)

```bash
git clone https://github.com/yash-atreya/mev-share-analysis.git
cd mev-share-analysis
cargo build
```

## Usage

1. Retrieving all historical events

   ```bash
   # This retrieves all events since inception of mev-share (block: 17422191) to the latest block and continues to listen for new events until exited manually
   cargo run -- events &
   ```

2. Retrieving events between a particular block range

   ```bash
   # This retrieves all events between the block range 17422191 and 17422199
   cargo run -- events --block-start 17422191 --block-end 17422199
   ```

3. Scan for refunds

   ```bash
   # This scans for refunds in the database
   cargo run -- scan-refunds &
   ```

   **Note:** This takes all events in the database and looks whether they landed onchain and if they triggered a refund. You **cannot** specify a block range to scan for refunds only in that subset of events.

   **Imp**: You will need to create an index on the `hint.hash` field before running this.

   ```bash
   db.events_collection.createIndex({"hint.hash": 1})
   ```

## TODO

- [ ] Parallelize fetching historical events
- [ ] Scanning for refunds in a block range
