use alloy::network::Ethereum;
use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, WalletProvider};
use alloy::rpc::types::TransactionRequest;
use atlas_common::AtlasError;
use futures::future::{BoxFuture, FutureExt};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const MAX_COOLDOWN_KEYS: usize = 4096;

#[derive(Debug, Clone, serde::Serialize)]
pub struct FaucetInfo {
    pub amount_wei: String,
    pub balance_wei: String,
    pub cooldown_minutes: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FaucetTxResponse {
    pub tx_hash: String,
}

pub type SharedFaucetBackend = Arc<dyn FaucetBackend>;

pub trait FaucetBackend: Send + Sync {
    fn info(&self) -> BoxFuture<'static, Result<FaucetInfo, AtlasError>>;
    fn request_faucet(
        &self,
        recipient: Address,
        client_ip: String,
    ) -> BoxFuture<'static, Result<FaucetTxResponse, AtlasError>>;
}

pub struct FaucetService<P> {
    provider: Arc<P>,
    amount_wei: U256,
    cooldown_minutes: u64,
    cooldown_duration: Duration,
    cooldowns: Arc<Mutex<CooldownState>>,
}

impl<P> FaucetService<P>
where
    P: Provider<Ethereum> + WalletProvider<Ethereum> + Send + Sync + 'static,
{
    pub fn new(provider: P, amount_wei: U256, cooldown_minutes: u64) -> Self {
        let cooldown_duration = Duration::from_secs(cooldown_minutes * 60);
        Self {
            provider: Arc::new(provider),
            amount_wei,
            cooldown_minutes,
            cooldown_duration,
            cooldowns: Arc::new(Mutex::new(CooldownState::new(MAX_COOLDOWN_KEYS))),
        }
    }
}

impl<P> FaucetBackend for FaucetService<P>
where
    P: Provider<Ethereum> + WalletProvider<Ethereum> + Send + Sync + 'static,
{
    fn info(&self) -> BoxFuture<'static, Result<FaucetInfo, AtlasError>> {
        let provider = Arc::clone(&self.provider);
        let amount_wei = self.amount_wei;
        let cooldown_minutes = self.cooldown_minutes;

        async move {
            let balance = provider
                .get_balance(provider.default_signer_address())
                .await
                .map_err(|err| AtlasError::Rpc(err.to_string()))?;

            Ok(FaucetInfo {
                amount_wei: amount_wei.to_string(),
                balance_wei: balance.to_string(),
                cooldown_minutes,
            })
        }
        .boxed()
    }

    fn request_faucet(
        &self,
        recipient: Address,
        client_ip: String,
    ) -> BoxFuture<'static, Result<FaucetTxResponse, AtlasError>> {
        let provider = Arc::clone(&self.provider);
        let amount_wei = self.amount_wei;
        let cooldown_duration = self.cooldown_duration;
        let cooldowns = Arc::clone(&self.cooldowns);
        let address_key = recipient.to_checksum(None);
        let ip_key = client_ip;

        async move {
            let reservation = {
                let mut cooldowns = cooldowns.lock().await;
                cooldowns.acquire(address_key.clone(), ip_key.clone(), cooldown_duration)?
            };

            let tx = TransactionRequest::default()
                .to(recipient)
                .value(amount_wei);
            match provider.send_transaction(tx).await {
                Ok(pending) => Ok(FaucetTxResponse {
                    tx_hash: pending.tx_hash().to_string(),
                }),
                Err(err) => {
                    let mut cooldowns = cooldowns.lock().await;
                    cooldowns.rollback(&reservation);
                    Err(AtlasError::Rpc(err.to_string()))
                }
            }
        }
        .boxed()
    }
}

#[derive(Debug, Clone)]
struct Reservation {
    address_key: String,
    ip_key: String,
    expiry: Instant,
}

#[derive(Debug)]
struct CooldownState {
    address_store: CooldownStore,
    ip_store: CooldownStore,
}

impl CooldownState {
    fn new(max_entries: usize) -> Self {
        Self {
            address_store: CooldownStore::new(max_entries),
            ip_store: CooldownStore::new(max_entries),
        }
    }

    fn acquire(
        &mut self,
        address_key: String,
        ip_key: String,
        ttl: Duration,
    ) -> Result<Reservation, AtlasError> {
        let now = Instant::now();
        self.address_store.cleanup(now);
        self.ip_store.cleanup(now);

        let retry_after = self
            .address_store
            .retry_after(&address_key, now)
            .into_iter()
            .chain(self.ip_store.retry_after(&ip_key, now))
            .max();

        if let Some(retry_after) = retry_after {
            return Err(cooldown_error(retry_after));
        }

        let expiry = now + ttl;
        self.address_store.reserve(address_key.clone(), expiry);
        self.ip_store.reserve(ip_key.clone(), expiry);

        Ok(Reservation {
            address_key,
            ip_key,
            expiry,
        })
    }

    fn rollback(&mut self, reservation: &Reservation) {
        self.address_store
            .release_if_matches(&reservation.address_key, reservation.expiry);
        self.ip_store
            .release_if_matches(&reservation.ip_key, reservation.expiry);
    }
}

#[derive(Debug)]
struct CooldownStore {
    max_entries: usize,
    entries: HashMap<String, Instant>,
    expiries: BTreeMap<Instant, HashSet<String>>,
}

impl CooldownStore {
    fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            entries: HashMap::new(),
            expiries: BTreeMap::new(),
        }
    }

    fn cleanup(&mut self, now: Instant) {
        while let Some(expiry) = self.expiries.keys().next().copied() {
            if expiry > now {
                break;
            }

            let Some(keys) = self.expiries.remove(&expiry) else {
                break;
            };

            for key in keys {
                if self.entries.get(&key).copied() == Some(expiry) {
                    self.entries.remove(&key);
                }
            }
        }
    }

    fn retry_after(&self, key: &str, now: Instant) -> Option<Duration> {
        self.entries
            .get(key)
            .and_then(|expiry| expiry.checked_duration_since(now))
    }

    fn reserve(&mut self, key: String, expiry: Instant) {
        if let Some(old_expiry) = self.entries.get(&key).copied() {
            self.remove_from_index(old_expiry, &key);
        } else if self.entries.len() >= self.max_entries {
            self.evict_oldest();
        }

        self.entries.insert(key.clone(), expiry);
        self.expiries.entry(expiry).or_default().insert(key);
    }

    fn release_if_matches(&mut self, key: &str, expiry: Instant) {
        if self.entries.get(key).copied() != Some(expiry) {
            return;
        }

        self.entries.remove(key);
        self.remove_from_index(expiry, key);
    }

    fn evict_oldest(&mut self) {
        let Some(expiry) = self.expiries.keys().next().copied() else {
            return;
        };
        let Some(key) = self
            .expiries
            .get(&expiry)
            .and_then(|keys| keys.iter().next().cloned())
        else {
            return;
        };

        self.entries.remove(&key);
        self.remove_from_index(expiry, &key);
    }

    fn remove_from_index(&mut self, expiry: Instant, key: &str) {
        let mut remove_bucket = false;
        if let Some(keys) = self.expiries.get_mut(&expiry) {
            keys.remove(key);
            remove_bucket = keys.is_empty();
        }

        if remove_bucket {
            self.expiries.remove(&expiry);
        }
    }
}

fn cooldown_error(retry_after: Duration) -> AtlasError {
    AtlasError::TooManyRequests {
        message: "Faucet cooldown active".to_string(),
        retry_after_seconds: duration_to_retry_after_seconds(retry_after),
    }
}

fn duration_to_retry_after_seconds(duration: Duration) -> u64 {
    duration
        .as_secs()
        .saturating_add(u64::from(duration.subsec_nanos() > 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cooldown_store_expires_entries() {
        let ttl = Duration::from_secs(60);
        let mut store = CooldownStore::new(4);
        let now = Instant::now();
        let key = "0xabc".to_string();

        store.reserve(key.clone(), now + ttl);
        assert_eq!(store.retry_after(&key, now), Some(ttl));

        store.cleanup(now + ttl + Duration::from_secs(1));
        assert_eq!(
            store.retry_after(&key, now + ttl + Duration::from_secs(1)),
            None
        );
    }

    #[test]
    fn cooldown_store_evicts_when_full() {
        let ttl = Duration::from_secs(60);
        let mut store = CooldownStore::new(1);
        let now = Instant::now();

        store.reserve("first".to_string(), now + ttl);
        store.reserve("second".to_string(), now + ttl);

        assert!(!store.entries.contains_key("first"));
        assert!(store.entries.contains_key("second"));
    }

    #[test]
    fn state_rejects_active_address_or_ip() {
        let ttl = Duration::from_secs(30);
        let mut state = CooldownState::new(8);
        let address = "0x0000000000000000000000000000000000000001".to_string();
        let ip = "127.0.0.1".to_string();

        let reservation = state.acquire(address.clone(), ip.clone(), ttl).unwrap();
        let err = state.acquire(address.clone(), ip.clone(), ttl).unwrap_err();

        match err {
            AtlasError::TooManyRequests {
                retry_after_seconds,
                ..
            } => assert!(retry_after_seconds > 0),
            other => panic!("expected too many requests, got {other:?}"),
        }

        state.rollback(&reservation);
        assert!(state.acquire(address, ip, ttl).is_ok());
    }

    #[test]
    fn duration_rounds_up_partial_seconds() {
        assert_eq!(duration_to_retry_after_seconds(Duration::from_millis(1)), 1);
        assert_eq!(duration_to_retry_after_seconds(Duration::from_secs(5)), 5);
    }
}
