use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::Utc;
use std::sync::Arc;

use crate::api::error::ApiResult;
use crate::api::handlers::stats::{Window, WindowQuery};
use crate::api::AppState;
use atlas_common::{
    AtlasError, Erc20Balance, Erc20Contract, Erc20Holder, Erc20Transfer, PaginatedResponse,
    Pagination,
};

/// GET /api/tokens - List all ERC-20 tokens
pub async fn list_tokens(
    State(state): State<Arc<AppState>>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<Erc20Contract>>> {
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM erc20_contracts")
        .fetch_one(&state.pool)
        .await?;

    let tokens: Vec<Erc20Contract> = sqlx::query_as(
        "SELECT address, name, symbol, decimals, total_supply, first_seen_block
         FROM erc20_contracts
         ORDER BY first_seen_block DESC
         LIMIT $1 OFFSET $2",
    )
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(PaginatedResponse::new(
        tokens,
        pagination.page,
        pagination.limit,
        total.0,
    )))
}

/// Token detail response with holder count
#[derive(Debug, Clone, serde::Serialize)]
pub struct TokenDetailResponse {
    #[serde(flatten)]
    pub contract: Erc20Contract,
    pub holder_count: i64,
    pub transfer_count: i64,
}

/// GET /api/tokens/:address - Get token details
pub async fn get_token(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> ApiResult<Json<TokenDetailResponse>> {
    let address = normalize_address(&address);

    let mut contract: Erc20Contract = sqlx::query_as(
        "SELECT address, name, symbol, decimals, total_supply, first_seen_block
         FROM erc20_contracts
         WHERE address = $1",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AtlasError::NotFound(format!("Token {} not found", address)))?;

    let holder_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM erc20_balances WHERE contract_address = $1 AND balance > 0",
    )
    .bind(&address)
    .fetch_one(&state.pool)
    .await?;

    let transfer_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM erc20_transfers WHERE contract_address = $1")
            .bind(&address)
            .fetch_one(&state.pool)
            .await?;

    // Compute total_supply from balances if not set
    if contract.total_supply.is_none() {
        let computed_supply: Option<(bigdecimal::BigDecimal,)> = sqlx::query_as(
            "SELECT COALESCE(SUM(balance), 0) FROM erc20_balances WHERE contract_address = $1 AND balance > 0",
        )
        .bind(&address)
        .fetch_optional(&state.pool)
        .await?;

        contract.total_supply = computed_supply.map(|(s,)| s);
    }

    Ok(Json(TokenDetailResponse {
        contract,
        holder_count: holder_count.0,
        transfer_count: transfer_count.0,
    }))
}

/// GET /api/tokens/:address/holders - Get token holders
pub async fn get_token_holders(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<Erc20Holder>>> {
    let address = normalize_address(&address);

    // Verify token exists
    let exists: Option<(String,)> =
        sqlx::query_as("SELECT address FROM erc20_contracts WHERE address = $1 LIMIT 1")
            .bind(&address)
            .fetch_optional(&state.pool)
            .await?;
    if exists.is_none() {
        return Err(AtlasError::NotFound(format!("Token {} not found", address)).into());
    }

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM erc20_balances WHERE contract_address = $1 AND balance > 0",
    )
    .bind(&address)
    .fetch_one(&state.pool)
    .await?;

    // Get total supply for percentage calculation
    // First try to get it from the contract, if NULL compute from sum of balances
    let total_supply: Option<bigdecimal::BigDecimal> = {
        let stored: Option<(Option<bigdecimal::BigDecimal>,)> =
            sqlx::query_as("SELECT total_supply FROM erc20_contracts WHERE address = $1")
                .bind(&address)
                .fetch_optional(&state.pool)
                .await?;

        match stored {
            Some((Some(ts),)) => Some(ts),
            _ => {
                // Compute from sum of balances
                let computed: Option<(bigdecimal::BigDecimal,)> = sqlx::query_as(
                    "SELECT COALESCE(SUM(balance), 0) FROM erc20_balances
                     WHERE contract_address = $1 AND balance > 0",
                )
                .bind(&address)
                .fetch_optional(&state.pool)
                .await?;
                computed.map(|(s,)| s)
            }
        }
    };

    let balances: Vec<Erc20Balance> = sqlx::query_as(
        "SELECT address, contract_address, balance, last_updated_block
         FROM erc20_balances
         WHERE contract_address = $1 AND balance > 0
         ORDER BY balance DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(&address)
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    // Convert to Erc20Holder with percentage
    let holders: Vec<Erc20Holder> = balances
        .into_iter()
        .map(|b| {
            let percentage = total_supply.as_ref().and_then(|ts| {
                use bigdecimal::ToPrimitive;
                let balance_f = b.balance.to_f64()?;
                let supply_f = ts.to_f64()?;
                if supply_f > 0.0 {
                    Some((balance_f / supply_f) * 100.0)
                } else {
                    None
                }
            });
            Erc20Holder {
                address: b.address,
                balance: b.balance,
                percentage,
            }
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        holders,
        pagination.page,
        pagination.limit,
        total.0,
    )))
}

/// GET /api/tokens/:address/transfers - Get token transfers
pub async fn get_token_transfers(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<Erc20Transfer>>> {
    let address = normalize_address(&address);

    let total: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM erc20_transfers WHERE contract_address = $1")
            .bind(&address)
            .fetch_one(&state.pool)
            .await?;

    let transfers: Vec<Erc20Transfer> = sqlx::query_as(
        "SELECT id, tx_hash, log_index, contract_address, from_address, to_address, value, block_number, timestamp
         FROM erc20_transfers
         WHERE contract_address = $1
         ORDER BY block_number DESC, log_index DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(&address)
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(PaginatedResponse::new(
        transfers,
        pagination.page,
        pagination.limit,
        total.0,
    )))
}

/// GET /api/addresses/:address/tokens - Get ERC-20 balances for address
pub async fn get_address_tokens(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<AddressTokenBalance>>> {
    let address = normalize_address(&address);

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM erc20_balances
         WHERE address = $1 AND balance > 0",
    )
    .bind(&address)
    .fetch_one(&state.pool)
    .await?;

    let balances: Vec<AddressTokenBalance> = sqlx::query_as(
        "SELECT b.address, b.contract_address, b.balance, b.last_updated_block,
                c.name, c.symbol, c.decimals
         FROM erc20_balances b
         JOIN erc20_contracts c ON b.contract_address = c.address
         WHERE b.address = $1 AND b.balance > 0
         ORDER BY b.balance DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(&address)
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(PaginatedResponse::new(
        balances,
        pagination.page,
        pagination.limit,
        total.0,
    )))
}

/// Token balance with contract info for address endpoint
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct AddressTokenBalance {
    pub address: String,
    pub contract_address: String,
    pub balance: bigdecimal::BigDecimal,
    pub last_updated_block: i64,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: i16,
}

/// Chart point returned by GET /api/tokens/:address/chart
#[derive(serde::Serialize)]
pub struct TokenChartPoint {
    pub bucket: String,
    pub transfer_count: i64,
    pub volume: f64,
}

/// GET /api/tokens/:address/chart?window=1h|6h|24h|7d
///
/// Returns transfer count and volume (in human-readable token units) per time
/// bucket for the given token contract. Anchored to the latest transfer
/// timestamp so charts show data even when the indexer is catching up.
pub async fn get_token_chart(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(params): Query<WindowQuery>,
) -> ApiResult<Json<Vec<TokenChartPoint>>> {
    let address = normalize_address(&address);
    let window = params.window;
    let bucket_secs = window.bucket_secs();

    // Fetch token decimals (default 18 if not found)
    let decimals: i16 = sqlx::query_as::<_, (i16,)>(
        "SELECT decimals FROM erc20_contracts WHERE address = $1",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?
    .map(|(d,)| d)
    .unwrap_or(18);

    let rows: Vec<(chrono::DateTime<Utc>, i64, bigdecimal::BigDecimal)> = sqlx::query_as(
        r#"
        WITH latest AS (SELECT MAX(timestamp) AS max_ts FROM blocks),
        agg AS (
            SELECT
                (timestamp - (timestamp % $2))::bigint AS bucket_ts,
                COUNT(*)::bigint                        AS transfer_count,
                COALESCE(SUM(value), 0)                AS volume
            FROM erc20_transfers, latest
            WHERE contract_address = $1
              AND timestamp >= max_ts - $3
              AND timestamp <= max_ts
            GROUP BY 1
        )
        SELECT
            to_timestamp(gs::float8)                        AS bucket,
            COALESCE(a.transfer_count, 0)::bigint           AS transfer_count,
            COALESCE(a.volume, 0::numeric)                  AS volume
        FROM generate_series(
            (SELECT (max_ts - $3) - ((max_ts - $3) % $2) FROM latest),
            (SELECT max_ts - (max_ts % $2) FROM latest),
            $2::bigint
        ) AS gs
        LEFT JOIN agg a ON a.bucket_ts = gs
        ORDER BY gs ASC
        "#,
    )
    .bind(&address)
    .bind(bucket_secs)
    .bind(window.duration_secs())
    .fetch_all(&state.pool)
    .await?;

    let divisor = bigdecimal::BigDecimal::from(10_i64.pow(decimals.clamp(0, 18) as u32));
    let points = rows
        .into_iter()
        .map(|(bucket, transfer_count, sum_value)| {
            use bigdecimal::ToPrimitive;
            let volume = (&sum_value / &divisor).to_f64().unwrap_or(0.0);
            TokenChartPoint {
                bucket: bucket.to_rfc3339(),
                transfer_count,
                volume,
            }
        })
        .collect();

    Ok(Json(points))
}

fn normalize_address(address: &str) -> String {
    if address.starts_with("0x") {
        address.to_lowercase()
    } else {
        format!("0x{}", address.to_lowercase())
    }
}
