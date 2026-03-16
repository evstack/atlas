use atlas_common::Block;
use sqlx::PgPool;
use std::collections::VecDeque;
use tokio::sync::RwLock;
use tracing::warn;

const BLOCK_COLUMNS: &str =
    "number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at";

pub(crate) struct HeadTracker {
    replay_capacity: usize,
    state: RwLock<HeadState>,
}

#[derive(Default)]
struct HeadState {
    latest: Option<Block>,
    replay: VecDeque<Block>,
}

pub(crate) struct ReplaySnapshot {
    pub buffer_start: Option<i64>,
    pub buffer_end: Option<i64>,
    pub blocks_after_cursor: Vec<Block>,
}

impl HeadTracker {
    pub(crate) async fn bootstrap(
        pool: &PgPool,
        replay_capacity: usize,
    ) -> Result<Self, sqlx::Error> {
        let mut blocks = sqlx::query_as::<_, Block>(&format!(
            "SELECT {} FROM blocks ORDER BY number DESC LIMIT $1",
            BLOCK_COLUMNS
        ))
        .bind(replay_capacity as i64)
        .fetch_all(pool)
        .await?;
        blocks.reverse();

        let latest = blocks.last().cloned();
        let replay = VecDeque::from(blocks);

        Ok(Self {
            replay_capacity,
            state: RwLock::new(HeadState { latest, replay }),
        })
    }

    pub(crate) fn empty(replay_capacity: usize) -> Self {
        Self {
            replay_capacity,
            state: RwLock::new(HeadState::default()),
        }
    }

    pub(crate) async fn clear(&self) {
        let mut state = self.state.write().await;
        *state = HeadState::default();
    }

    pub(crate) async fn publish_committed_batch(&self, blocks: &[Block]) {
        if blocks.is_empty() {
            return;
        }

        let mut state = self.state.write().await;
        let mut latest_number = state.latest.as_ref().map(|block| block.number);

        for block in blocks {
            if latest_number.is_some_and(|latest| block.number <= latest) {
                warn!(
                    current_latest = ?latest_number,
                    incoming_block = block.number,
                    "ignoring non-advancing committed block publication"
                );
                continue;
            }

            state.latest = Some(block.clone());
            state.replay.push_back(block.clone());
            latest_number = Some(block.number);
        }

        while state.replay.len() > self.replay_capacity {
            state.replay.pop_front();
        }
    }

    pub(crate) async fn latest(&self) -> Option<Block> {
        self.state.read().await.latest.clone()
    }

    pub(crate) async fn replay_after(&self, after_block: Option<i64>) -> ReplaySnapshot {
        let state = self.state.read().await;

        let blocks_after_cursor = state
            .replay
            .iter()
            .filter(|block| after_block.is_none_or(|cursor| block.number > cursor))
            .cloned()
            .collect();

        ReplaySnapshot {
            buffer_start: state.replay.front().map(|block| block.number),
            buffer_end: state.replay.back().map(|block| block.number),
            blocks_after_cursor,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn sample_block(number: i64) -> Block {
        Block {
            number,
            hash: format!("0x{number:064x}"),
            parent_hash: format!("0x{:064x}", number.saturating_sub(1)),
            timestamp: 1_700_000_000 + number,
            gas_used: 21_000,
            gas_limit: 30_000_000,
            transaction_count: 1,
            indexed_at: Utc.timestamp_opt(1_700_000_000 + number, 0).unwrap(),
        }
    }

    #[tokio::test]
    async fn replay_after_returns_full_buffer_for_empty_cursor() {
        let tracker = HeadTracker::empty(3);
        tracker
            .publish_committed_batch(&[sample_block(10), sample_block(11)])
            .await;

        let snapshot = tracker.replay_after(None).await;
        let numbers: Vec<i64> = snapshot
            .blocks_after_cursor
            .into_iter()
            .map(|block| block.number)
            .collect();

        assert_eq!(numbers, vec![10, 11]);
        assert_eq!(snapshot.buffer_start, Some(10));
        assert_eq!(snapshot.buffer_end, Some(11));
    }

    #[tokio::test]
    async fn publish_committed_batch_trims_oldest_blocks() {
        let tracker = HeadTracker::empty(2);
        tracker
            .publish_committed_batch(&[sample_block(10), sample_block(11), sample_block(12)])
            .await;

        let snapshot = tracker.replay_after(None).await;
        let numbers: Vec<i64> = snapshot
            .blocks_after_cursor
            .into_iter()
            .map(|block| block.number)
            .collect();

        assert_eq!(numbers, vec![11, 12]);
        assert_eq!(tracker.latest().await.unwrap().number, 12);
    }

    #[tokio::test]
    async fn publish_committed_batch_ignores_non_advancing_blocks() {
        let tracker = HeadTracker::empty(3);
        tracker.publish_committed_batch(&[sample_block(10)]).await;
        tracker
            .publish_committed_batch(&[sample_block(9), sample_block(10)])
            .await;

        let snapshot = tracker.replay_after(None).await;
        let numbers: Vec<i64> = snapshot
            .blocks_after_cursor
            .into_iter()
            .map(|block| block.number)
            .collect();

        assert_eq!(numbers, vec![10]);
        assert_eq!(tracker.latest().await.unwrap().number, 10);
    }
}
