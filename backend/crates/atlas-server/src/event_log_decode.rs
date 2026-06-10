use std::collections::{BTreeSet, HashMap};
use std::str::FromStr;

use alloy::dyn_abi::EventExt;
use alloy::json_abi::{Event, JsonAbi};
use alloy::primitives::B256;
use atlas_common::{AtlasError, EventLog, ProxyContract};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};

use crate::contract_abi::{load_combined_abi, ResolvedContractAbi};

pub const EVENT_LOG_DECODE_PENDING: &str = "pending";
pub const EVENT_LOG_DECODE_DECODED: &str = "decoded";
pub const EVENT_LOG_DECODE_NO_ABI: &str = "no_abi";
pub const EVENT_LOG_DECODE_NO_MATCHING_EVENT: &str = "no_matching_event";
pub const EVENT_LOG_DECODE_FAILED: &str = "decode_failed";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecodedEventParam {
    pub name: String,
    pub r#type: String,
    pub value: String,
    pub indexed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredDecodedEventLog {
    pub event_name: String,
    pub event_signature: String,
    pub decoded_params: Vec<DecodedEventParam>,
}

#[derive(Debug, Clone)]
pub struct EventLogApiResponse {
    pub id: i64,
    pub tx_hash: String,
    pub log_index: i32,
    pub address: String,
    pub topic0: String,
    pub topic1: Option<String>,
    pub topic2: Option<String>,
    pub topic3: Option<String>,
    pub data: String,
    pub block_number: i64,
    pub decode_status: String,
    pub decoded_at: Option<DateTime<Utc>>,
    pub decode_attempted_at: Option<DateTime<Utc>>,
    pub decode_source: Option<String>,
    pub event_name: Option<String>,
    pub event_signature: Option<String>,
    pub decoded_params: Option<Vec<DecodedEventParam>>,
}

impl serde::Serialize for EventLogApiResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("EventLogApiResponse", 17)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("tx_hash", &self.tx_hash)?;
        state.serialize_field("log_index", &self.log_index)?;
        state.serialize_field("address", &self.address)?;
        state.serialize_field("topic0", &self.topic0)?;
        state.serialize_field("topic1", &self.topic1)?;
        state.serialize_field("topic2", &self.topic2)?;
        state.serialize_field("topic3", &self.topic3)?;
        state.serialize_field("data", &self.data)?;
        state.serialize_field("block_number", &self.block_number)?;
        state.serialize_field("decode_status", &self.decode_status)?;
        state.serialize_field("decoded_at", &self.decoded_at)?;
        state.serialize_field("decode_attempted_at", &self.decode_attempted_at)?;
        state.serialize_field("decode_source", &self.decode_source)?;
        state.serialize_field("event_name", &self.event_name)?;
        state.serialize_field("event_signature", &self.event_signature)?;
        state.serialize_field("decoded_params", &self.decoded_params)?;
        state.end()
    }
}

impl From<&EventLog> for EventLogApiResponse {
    fn from(log: &EventLog) -> Self {
        let stored = log
            .decoded
            .as_ref()
            .and_then(|value| serde_json::from_value::<StoredDecodedEventLog>(value.clone()).ok());

        Self {
            id: log.id,
            tx_hash: log.tx_hash.clone(),
            log_index: log.log_index,
            address: log.address.clone(),
            topic0: log.topic0.clone(),
            topic1: log.topic1.clone(),
            topic2: log.topic2.clone(),
            topic3: log.topic3.clone(),
            data: format!("0x{}", hex::encode(&log.data)),
            block_number: log.block_number,
            decode_status: log.decode_status.clone(),
            decoded_at: log.decoded_at,
            decode_attempted_at: log.decode_attempted_at,
            decode_source: log.decode_source.clone(),
            event_name: stored.as_ref().map(|decoded| decoded.event_name.clone()),
            event_signature: stored
                .as_ref()
                .map(|decoded| decoded.event_signature.clone()),
            decoded_params: stored.map(|decoded| decoded.decoded_params),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecodeOutcome {
    pub decoded: Option<serde_json::Value>,
    pub decode_status: String,
    pub decode_source: Option<String>,
    pub decoded_at: Option<DateTime<Utc>>,
    pub decode_attempted_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct EventCandidate {
    event: Event,
    full_signature: String,
}

#[derive(Debug, Clone)]
pub struct EventLogDecoder {
    source: &'static str,
    selectors: HashMap<String, Vec<EventCandidate>>,
}

impl EventLogDecoder {
    pub fn from_resolved_abi(resolved: &ResolvedContractAbi) -> Result<Self, AtlasError> {
        let mut abi: JsonAbi = serde_json::from_value(resolved.abi.clone()).map_err(|e| {
            AtlasError::Internal(format!(
                "failed to parse contract ABI for event decoding: {e}"
            ))
        })?;
        abi.dedup();

        let mut selectors: HashMap<String, Vec<EventCandidate>> = HashMap::new();
        for event in abi.events() {
            if event.anonymous {
                continue;
            }

            let selector = format!("0x{}", hex::encode(event.selector().as_slice()));
            selectors.entry(selector).or_default().push(EventCandidate {
                event: event.clone(),
                full_signature: event.full_signature(),
            });
        }

        Ok(Self {
            source: resolved.source,
            selectors,
        })
    }

    pub fn decode_log(&self, log: &EventLog) -> DecodeAttempt {
        let Some(candidates) = self.selectors.get(&log.topic0) else {
            return DecodeAttempt {
                decoded: None,
                decode_status: EVENT_LOG_DECODE_NO_MATCHING_EVENT,
                decode_source: Some(self.source.to_string()),
            };
        };

        let topics = match log_topics(log) {
            Ok(topics) => topics,
            Err(_) => {
                return DecodeAttempt {
                    decoded: None,
                    decode_status: EVENT_LOG_DECODE_FAILED,
                    decode_source: Some(self.source.to_string()),
                }
            }
        };

        for candidate in candidates {
            match candidate
                .event
                .decode_log_parts(topics.iter().copied(), &log.data)
            {
                Ok(decoded_event) => {
                    let mut decoded_params = Vec::with_capacity(candidate.event.inputs.len());
                    let mut indexed_iter = decoded_event.indexed.into_iter();
                    let mut body_iter = decoded_event.body.into_iter();

                    for (index, input) in candidate.event.inputs.iter().enumerate() {
                        let value = if input.indexed {
                            indexed_iter.next()
                        } else {
                            body_iter.next()
                        };

                        let Some(value) = value else {
                            return DecodeAttempt {
                                decoded: None,
                                decode_status: EVENT_LOG_DECODE_FAILED,
                                decode_source: Some(self.source.to_string()),
                            };
                        };

                        decoded_params.push(DecodedEventParam {
                            name: if input.name.is_empty() {
                                format!("param{index}")
                            } else {
                                input.name.clone()
                            },
                            r#type: input.ty.clone(),
                            value: format_dyn_value(&value),
                            indexed: input.indexed,
                        });
                    }

                    let decoded = StoredDecodedEventLog {
                        event_name: candidate.event.name.clone(),
                        event_signature: candidate.full_signature.clone(),
                        decoded_params,
                    };

                    return DecodeAttempt {
                        decoded: Some(
                            serde_json::to_value(decoded)
                                .expect("stored decoded event log should serialize"),
                        ),
                        decode_status: EVENT_LOG_DECODE_DECODED,
                        decode_source: Some(self.source.to_string()),
                    };
                }
                Err(_) => continue,
            }
        }

        DecodeAttempt {
            decoded: None,
            decode_status: EVENT_LOG_DECODE_FAILED,
            decode_source: Some(self.source.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecodeAttempt {
    pub decoded: Option<serde_json::Value>,
    pub decode_status: &'static str,
    pub decode_source: Option<String>,
}

pub fn apply_decode_attempt(
    log: &EventLog,
    attempt: DecodeAttempt,
    attempted_at: DateTime<Utc>,
) -> DecodeOutcome {
    let has_successful_decode =
        log.decode_status == EVENT_LOG_DECODE_DECODED && log.decoded.as_ref().is_some();

    if attempt.decode_status == EVENT_LOG_DECODE_DECODED {
        return DecodeOutcome {
            decoded: attempt.decoded,
            decode_status: EVENT_LOG_DECODE_DECODED.to_string(),
            decode_source: attempt.decode_source,
            decoded_at: Some(attempted_at),
            decode_attempted_at: attempted_at,
        };
    }

    if has_successful_decode {
        return DecodeOutcome {
            decoded: log.decoded.clone(),
            decode_status: log.decode_status.clone(),
            decode_source: log.decode_source.clone(),
            decoded_at: log.decoded_at,
            decode_attempted_at: attempted_at,
        };
    }

    DecodeOutcome {
        decoded: None,
        decode_status: attempt.decode_status.to_string(),
        decode_source: attempt.decode_source,
        decoded_at: None,
        decode_attempted_at: attempted_at,
    }
}

pub async fn build_decoder_for_address(
    pool: &PgPool,
    rpc_url: &str,
    address: &str,
) -> Result<Option<EventLogDecoder>, AtlasError> {
    match load_combined_abi(pool, rpc_url, address).await? {
        Some(resolved) => EventLogDecoder::from_resolved_abi(&resolved).map(Some),
        None => Ok(None),
    }
}

pub async fn enqueue_decode_jobs(
    pool: &PgPool,
    addresses: &[String],
    full_rescan: bool,
) -> Result<(), AtlasError> {
    if addresses.is_empty() {
        return Ok(());
    }

    let deduped = dedupe_addresses(addresses);
    sqlx::query(
        "INSERT INTO event_log_decode_jobs
            (address, full_rescan, requested_at, updated_at, error_message)
         SELECT address, $2, NOW(), NOW(), NULL
         FROM unnest($1::text[]) AS t(address)
         ON CONFLICT (address) DO UPDATE SET
            full_rescan = event_log_decode_jobs.full_rescan OR EXCLUDED.full_rescan,
            requested_at = EXCLUDED.requested_at,
            updated_at = EXCLUDED.updated_at,
            error_message = NULL",
    )
    .bind(&deduped)
    .bind(full_rescan)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn enqueue_jobs_for_verified_contract_tx(
    tx: &mut Transaction<'_, Postgres>,
    address: &str,
) -> Result<(), AtlasError> {
    let mut addresses = vec![address.to_string()];
    let proxies: Vec<ProxyContract> = sqlx::query_as(
        "SELECT proxy_address, implementation_address, proxy_type, admin_address,
                detected_at_block, last_checked_block, updated_at
         FROM proxy_contracts
         WHERE implementation_address = $1",
    )
    .bind(address)
    .fetch_all(&mut **tx)
    .await?;

    for proxy in proxies {
        addresses.push(proxy.proxy_address);
    }

    let deduped = dedupe_addresses(&addresses);
    sqlx::query(
        "INSERT INTO event_log_decode_jobs
            (address, full_rescan, requested_at, updated_at, error_message)
         SELECT address, TRUE, NOW(), NOW(), NULL
         FROM unnest($1::text[]) AS t(address)
         ON CONFLICT (address) DO UPDATE SET
            full_rescan = TRUE,
            requested_at = EXCLUDED.requested_at,
            updated_at = EXCLUDED.updated_at,
            error_message = NULL",
    )
    .bind(&deduped)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn dedupe_addresses(addresses: &[String]) -> Vec<String> {
    let mut set = BTreeSet::new();
    for address in addresses {
        set.insert(address.to_lowercase());
    }
    set.into_iter().collect()
}

fn log_topics(log: &EventLog) -> Result<Vec<B256>, AtlasError> {
    let mut topics = Vec::with_capacity(4);
    topics.push(parse_topic(&log.topic0)?);
    if let Some(topic) = &log.topic1 {
        topics.push(parse_topic(topic)?);
    }
    if let Some(topic) = &log.topic2 {
        topics.push(parse_topic(topic)?);
    }
    if let Some(topic) = &log.topic3 {
        topics.push(parse_topic(topic)?);
    }
    Ok(topics)
}

fn parse_topic(topic: &str) -> Result<B256, AtlasError> {
    B256::from_str(topic)
        .map_err(|e| AtlasError::Internal(format!("failed to parse event topic {topic}: {e}")))
}

fn format_dyn_value(value: &alloy::dyn_abi::DynSolValue) -> String {
    use alloy::dyn_abi::DynSolValue;

    match value {
        DynSolValue::Bool(value) => value.to_string(),
        DynSolValue::Int(value, _) => value.to_string(),
        DynSolValue::Uint(value, _) => value.to_string(),
        DynSolValue::FixedBytes(word, size) => {
            format!("0x{}", hex::encode(&word.as_slice()[..*size]))
        }
        DynSolValue::Address(value) => format!("{value:?}").to_lowercase(),
        DynSolValue::Function(value) => format!("{value:?}").to_lowercase(),
        DynSolValue::Bytes(value) => format!("0x{}", hex::encode(value)),
        DynSolValue::String(value) => value.clone(),
        DynSolValue::Array(values)
        | DynSolValue::FixedArray(values)
        | DynSolValue::Tuple(values) => {
            let formatted = values
                .iter()
                .map(format_dyn_value)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{formatted}]")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_log(topic0: &str, decoded: Option<serde_json::Value>) -> EventLog {
        let has_decoded = decoded.is_some();
        EventLog {
            id: 1,
            tx_hash: "0x1".to_string(),
            log_index: 0,
            address: "0x1111111111111111111111111111111111111111".to_string(),
            topic0: topic0.to_string(),
            topic1: Some(
                "0x0000000000000000000000002222222222222222222222222222222222222222".to_string(),
            ),
            topic2: None,
            topic3: None,
            data: vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 7,
            ],
            block_number: 1,
            decoded,
            decode_status: if has_decoded {
                EVENT_LOG_DECODE_DECODED.to_string()
            } else {
                EVENT_LOG_DECODE_PENDING.to_string()
            },
            decoded_at: None,
            decode_attempted_at: None,
            decode_source: if has_decoded {
                Some(crate::contract_abi::DIRECT_ABI_SOURCE.to_string())
            } else {
                None
            },
        }
    }

    #[test]
    fn apply_decode_attempt_preserves_existing_success_when_new_attempt_fails() {
        let existing = StoredDecodedEventLog {
            event_name: "Transfer".to_string(),
            event_signature: "Transfer(address,uint256)".to_string(),
            decoded_params: vec![],
        };
        let log = base_log(
            "0xdeadbeef",
            Some(serde_json::to_value(existing.clone()).unwrap()),
        );
        let attempted_at = Utc::now();

        let outcome = apply_decode_attempt(
            &log,
            DecodeAttempt {
                decoded: None,
                decode_status: EVENT_LOG_DECODE_NO_ABI,
                decode_source: None,
            },
            attempted_at,
        );

        assert_eq!(outcome.decode_status, EVENT_LOG_DECODE_DECODED);
        assert_eq!(
            outcome.decoded,
            Some(serde_json::to_value(existing).unwrap())
        );
        assert_eq!(
            outcome.decode_source,
            Some(crate::contract_abi::DIRECT_ABI_SOURCE.to_string())
        );
        assert_eq!(outcome.decode_attempted_at, attempted_at);
    }

    #[test]
    fn decoder_decodes_transfer_event() {
        let abi = serde_json::json!([
            {
                "type": "event",
                "name": "Transfer",
                "anonymous": false,
                "inputs": [
                    {"name": "from", "type": "address", "indexed": true},
                    {"name": "value", "type": "uint256", "indexed": false}
                ]
            }
        ]);
        let decoder = EventLogDecoder::from_resolved_abi(&ResolvedContractAbi {
            abi,
            source: crate::contract_abi::DIRECT_ABI_SOURCE,
        })
        .unwrap();
        let topic0 = decoder.selectors.keys().next().expect("selector").clone();
        let log = base_log(&topic0, None);

        let attempt = decoder.decode_log(&log);
        assert_eq!(attempt.decode_status, EVENT_LOG_DECODE_DECODED);
        let stored: StoredDecodedEventLog =
            serde_json::from_value(attempt.decoded.expect("decoded payload")).unwrap();
        assert_eq!(stored.event_name, "Transfer");
        assert_eq!(stored.decoded_params.len(), 2);
        assert_eq!(stored.decoded_params[0].name, "from");
        assert_eq!(stored.decoded_params[0].indexed, true);
    }
}
