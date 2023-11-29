use crate::refunds::{landing::Landing, refund::Refund};
use mev_share::sse::{EventHistory, Hint};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub block: u64,
    pub timestamp: u64,
    pub hint: Hint,
    pub refund: Option<Refund>,
    pub landing: Option<Landing>,
    pub landed: Option<bool>,
}

impl Event {
    pub fn new(event: EventHistory) -> Event {
        Event {
            block: event.block,
            timestamp: event.timestamp,
            hint: event.hint,
            refund: None,
            landing: None,
            landed: None,
        }
    }
}
