//! Address labels system
//!
//! Provides curated labels for known addresses (bridges, governance, etc.)

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::error::ApiResult;
use crate::handlers::auth::require_admin;
use crate::AppState;
use atlas_common::{AddressLabel, AddressLabelInput, AtlasError, PaginatedResponse, Pagination};

/// Query parameters for label filtering
#[derive(Debug, Deserialize)]
pub struct LabelQuery {
    /// Filter by tag
    pub tag: Option<String>,
    /// Search by name
    pub search: Option<String>,
    #[serde(flatten)]
    pub pagination: Pagination,
}

/// GET /api/labels - List all address labels
pub async fn list_labels(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LabelQuery>,
) -> ApiResult<Json<PaginatedResponse<AddressLabel>>> {
    let (total, labels) = if let Some(tag) = &query.tag {
        let total: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM address_labels WHERE $1 = ANY(tags)")
                .bind(tag)
                .fetch_one(&state.pool)
                .await?;

        let labels: Vec<AddressLabel> = sqlx::query_as(
            "SELECT address, name, tags, created_at, updated_at
             FROM address_labels
             WHERE $1 = ANY(tags)
             ORDER BY name ASC
             LIMIT $2 OFFSET $3",
        )
        .bind(tag)
        .bind(query.pagination.limit())
        .bind(query.pagination.offset())
        .fetch_all(&state.pool)
        .await?;

        (total.0, labels)
    } else if let Some(search) = &query.search {
        let search_pattern = format!("%{}%", search.to_lowercase());

        let total: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM address_labels WHERE LOWER(name) LIKE $1")
                .bind(&search_pattern)
                .fetch_one(&state.pool)
                .await?;

        let labels: Vec<AddressLabel> = sqlx::query_as(
            "SELECT address, name, tags, created_at, updated_at
             FROM address_labels
             WHERE LOWER(name) LIKE $1
             ORDER BY name ASC
             LIMIT $2 OFFSET $3",
        )
        .bind(&search_pattern)
        .bind(query.pagination.limit())
        .bind(query.pagination.offset())
        .fetch_all(&state.pool)
        .await?;

        (total.0, labels)
    } else {
        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM address_labels")
            .fetch_one(&state.pool)
            .await?;

        let labels: Vec<AddressLabel> = sqlx::query_as(
            "SELECT address, name, tags, created_at, updated_at
             FROM address_labels
             ORDER BY name ASC
             LIMIT $1 OFFSET $2",
        )
        .bind(query.pagination.limit())
        .bind(query.pagination.offset())
        .fetch_all(&state.pool)
        .await?;

        (total.0, labels)
    };

    Ok(Json(PaginatedResponse::new(
        labels,
        query.pagination.page,
        query.pagination.limit,
        total,
    )))
}

/// GET /api/labels/:address - Get label for specific address
pub async fn get_label(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> ApiResult<Json<AddressLabel>> {
    let address = normalize_address(&address);

    let label: AddressLabel = sqlx::query_as(
        "SELECT address, name, tags, created_at, updated_at
         FROM address_labels
         WHERE LOWER(address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AtlasError::NotFound(format!("Label for {} not found", address)))?;

    Ok(Json(label))
}

/// GET /api/labels/tags - Get all available tags
pub async fn list_tags(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<TagCount>>> {
    let tags: Vec<TagCount> = sqlx::query_as(
        "SELECT unnest(tags) as tag, COUNT(*) as count
         FROM address_labels
         GROUP BY tag
         ORDER BY count DESC, tag ASC",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(tags))
}

/// Tag with count
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct TagCount {
    pub tag: String,
    pub count: i64,
}

/// POST /api/labels - Create or update a label
pub async fn upsert_label(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<AddressLabelInput>,
) -> ApiResult<Json<AddressLabel>> {
    require_admin(&headers, &state)?;

    let address = normalize_address(&input.address);

    let label: AddressLabel = sqlx::query_as(
        "INSERT INTO address_labels (address, name, tags, created_at, updated_at)
         VALUES ($1, $2, $3, NOW(), NOW())
         ON CONFLICT (address) DO UPDATE SET
            name = $2,
            tags = $3,
            updated_at = NOW()
         RETURNING address, name, tags, created_at, updated_at",
    )
    .bind(&address)
    .bind(&input.name)
    .bind(&input.tags)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(label))
}

/// DELETE /api/labels/:address - Delete a label
pub async fn delete_label(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(address): Path<String>,
) -> ApiResult<Json<()>> {
    require_admin(&headers, &state)?;

    let address = normalize_address(&address);

    let result = sqlx::query("DELETE FROM address_labels WHERE LOWER(address) = LOWER($1)")
        .bind(&address)
        .execute(&state.pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AtlasError::NotFound(format!("Label for {} not found", address)).into());
    }

    Ok(Json(()))
}

/// Bulk import labels from JSON
#[derive(Debug, Deserialize)]
pub struct BulkLabelsInput {
    pub labels: Vec<AddressLabelInput>,
}

/// POST /api/labels/bulk - Bulk import labels
pub async fn bulk_import_labels(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<BulkLabelsInput>,
) -> ApiResult<Json<BulkImportResult>> {
    require_admin(&headers, &state)?;

    let mut imported = 0;
    let mut errors = Vec::new();

    for label in input.labels {
        let address = normalize_address(&label.address);

        match sqlx::query(
            "INSERT INTO address_labels (address, name, tags, created_at, updated_at)
             VALUES ($1, $2, $3, NOW(), NOW())
             ON CONFLICT (address) DO UPDATE SET
                name = $2,
                tags = $3,
                updated_at = NOW()",
        )
        .bind(&address)
        .bind(&label.name)
        .bind(&label.tags)
        .execute(&state.pool)
        .await
        {
            Ok(_) => imported += 1,
            Err(e) => errors.push(format!("{}: {}", address, e)),
        }
    }

    Ok(Json(BulkImportResult { imported, errors }))
}

#[derive(Debug, serde::Serialize)]
pub struct BulkImportResult {
    pub imported: usize,
    pub errors: Vec<String>,
}

/// GET /api/addresses/:address with label enrichment
/// This is used to enrich address responses with labels
pub async fn get_address_with_label(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> ApiResult<Json<AddressWithLabel>> {
    let address = normalize_address(&address);

    let addr: atlas_common::Address = sqlx::query_as(
        "SELECT address, is_contract, first_seen_block, tx_count
         FROM addresses
         WHERE LOWER(address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AtlasError::NotFound(format!("Address {} not found", address)))?;

    let label: Option<AddressLabel> = sqlx::query_as(
        "SELECT address, name, tags, created_at, updated_at
         FROM address_labels
         WHERE LOWER(address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?;

    Ok(Json(AddressWithLabel {
        address: addr,
        label,
    }))
}

#[derive(Debug, serde::Serialize)]
pub struct AddressWithLabel {
    #[serde(flatten)]
    pub address: atlas_common::Address,
    pub label: Option<AddressLabel>,
}

fn normalize_address(address: &str) -> String {
    if address.starts_with("0x") {
        address.to_lowercase()
    } else {
        format!("0x{}", address.to_lowercase())
    }
}
