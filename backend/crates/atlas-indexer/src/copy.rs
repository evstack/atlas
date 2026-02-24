use anyhow::Result;
use tokio::pin;
use tokio_postgres::{
    binary_copy::BinaryCopyInWriter,
    types::{Type, ToSql},
    Transaction,
};

use crate::batch::BlockBatch;

pub async fn copy_blocks(tx: &mut Transaction<'_>, batch: &BlockBatch) -> Result<()> {
    if batch.b_numbers.is_empty() {
        return Ok(());
    }

    tx.batch_execute(
        "CREATE TEMP TABLE IF NOT EXISTS tmp_blocks (
            number BIGINT,
            hash TEXT,
            parent_hash TEXT,
            timestamp BIGINT,
            gas_used BIGINT,
            gas_limit BIGINT,
            transaction_count INT
        ) ON COMMIT DELETE ROWS;
        TRUNCATE tmp_blocks;",
    )
    .await?;

    let sink = tx
        .copy_in(
            "COPY tmp_blocks (number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count) FROM STDIN BINARY",
        )
        .await?;
    let writer = BinaryCopyInWriter::new(
        sink,
        &[
            Type::INT8,
            Type::TEXT,
            Type::TEXT,
            Type::INT8,
            Type::INT8,
            Type::INT8,
            Type::INT4,
        ],
    );
    pin!(writer);

    for i in 0..batch.b_numbers.len() {
        let row: [&(dyn ToSql + Sync); 7] = [
            &batch.b_numbers[i],
            &batch.b_hashes[i],
            &batch.b_parent_hashes[i],
            &batch.b_timestamps[i],
            &batch.b_gas_used[i],
            &batch.b_gas_limits[i],
            &batch.b_tx_counts[i],
        ];
        writer.as_mut().write(&row).await?;
    }

    writer.finish().await?;

    tx.execute(
        "INSERT INTO blocks (number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count)
         SELECT number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count
         FROM tmp_blocks
         ON CONFLICT (number) DO UPDATE SET
            hash = EXCLUDED.hash,
            parent_hash = EXCLUDED.parent_hash,
            timestamp = EXCLUDED.timestamp,
            gas_used = EXCLUDED.gas_used,
            gas_limit = EXCLUDED.gas_limit,
            transaction_count = EXCLUDED.transaction_count,
            indexed_at = NOW()",
        &[],
    )
    .await?;

    Ok(())
}

pub async fn copy_transactions(tx: &mut Transaction<'_>, batch: &BlockBatch) -> Result<()> {
    if batch.t_hashes.is_empty() {
        return Ok(());
    }

    tx.batch_execute(
        "CREATE TEMP TABLE IF NOT EXISTS tmp_transactions (
            hash TEXT,
            block_number BIGINT,
            block_index INT,
            from_address TEXT,
            to_address TEXT,
            value TEXT,
            gas_price TEXT,
            gas_used BIGINT,
            input_data BYTEA,
            status BOOLEAN,
            contract_created TEXT,
            timestamp BIGINT
        ) ON COMMIT DELETE ROWS;
        TRUNCATE tmp_transactions;",
    )
    .await?;

    let sink = tx
        .copy_in(
            "COPY tmp_transactions (hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, contract_created, timestamp)
             FROM STDIN BINARY",
        )
        .await?;
    let writer = BinaryCopyInWriter::new(
        sink,
        &[
            Type::TEXT,
            Type::INT8,
            Type::INT4,
            Type::TEXT,
            Type::TEXT,
            Type::TEXT,
            Type::TEXT,
            Type::INT8,
            Type::BYTEA,
            Type::BOOL,
            Type::TEXT,
            Type::INT8,
        ],
    );
    pin!(writer);

    for i in 0..batch.t_hashes.len() {
        let to_addr = &batch.t_tos[i];
        let contract_created = &batch.t_contracts_created[i];

        let row: [&(dyn ToSql + Sync); 12] = [
            &batch.t_hashes[i],
            &batch.t_block_numbers[i],
            &batch.t_block_indices[i],
            &batch.t_froms[i],
            to_addr,
            &batch.t_values[i],
            &batch.t_gas_prices[i],
            &batch.t_gas_used[i],
            &batch.t_input_data[i],
            &batch.t_statuses[i],
            contract_created,
            &batch.t_timestamps[i],
        ];
        writer.as_mut().write(&row).await?;
    }

    writer.finish().await?;

    tx.execute(
        "INSERT INTO transactions
            (hash, block_number, block_index, from_address, to_address,
             value, gas_price, gas_used, input_data, status, contract_created, timestamp)
         SELECT hash, block_number, block_index, from_address, to_address,
                value::numeric, gas_price::numeric, gas_used, input_data, status, contract_created, timestamp
         FROM tmp_transactions
         ON CONFLICT (hash, block_number) DO NOTHING",
        &[],
    )
    .await?;

    Ok(())
}

pub async fn copy_event_logs(tx: &mut Transaction<'_>, batch: &BlockBatch) -> Result<()> {
    if batch.el_tx_hashes.is_empty() {
        return Ok(());
    }

    tx.batch_execute(
        "CREATE TEMP TABLE IF NOT EXISTS tmp_event_logs (
            tx_hash TEXT,
            log_index INT,
            address TEXT,
            topic0 TEXT,
            topic1 TEXT,
            topic2 TEXT,
            topic3 TEXT,
            data BYTEA,
            block_number BIGINT
        ) ON COMMIT DELETE ROWS;
        TRUNCATE tmp_event_logs;",
    )
    .await?;

    let sink = tx
        .copy_in(
            "COPY tmp_event_logs (tx_hash, log_index, address, topic0, topic1, topic2, topic3, data, block_number)
             FROM STDIN BINARY",
        )
        .await?;
    let writer = BinaryCopyInWriter::new(
        sink,
        &[
            Type::TEXT,
            Type::INT4,
            Type::TEXT,
            Type::TEXT,
            Type::TEXT,
            Type::TEXT,
            Type::TEXT,
            Type::BYTEA,
            Type::INT8,
        ],
    );
    pin!(writer);

    for i in 0..batch.el_tx_hashes.len() {
        let row: [&(dyn ToSql + Sync); 9] = [
            &batch.el_tx_hashes[i],
            &batch.el_log_indices[i],
            &batch.el_addresses[i],
            &batch.el_topic0s[i],
            &batch.el_topic1s[i],
            &batch.el_topic2s[i],
            &batch.el_topic3s[i],
            &batch.el_datas[i],
            &batch.el_block_numbers[i],
        ];
        writer.as_mut().write(&row).await?;
    }

    writer.finish().await?;

    tx.execute(
        "INSERT INTO event_logs
            (tx_hash, log_index, address, topic0, topic1, topic2, topic3, data, block_number)
         SELECT tx_hash, log_index, address, topic0, topic1, topic2, topic3, data, block_number
         FROM tmp_event_logs
         ON CONFLICT (tx_hash, log_index, block_number) DO NOTHING",
        &[],
    )
    .await?;

    Ok(())
}

pub async fn copy_nft_transfers(tx: &mut Transaction<'_>, batch: &BlockBatch) -> Result<()> {
    if batch.nt_tx_hashes.is_empty() {
        return Ok(());
    }

    tx.batch_execute(
        "CREATE TEMP TABLE IF NOT EXISTS tmp_nft_transfers (
            tx_hash TEXT,
            log_index INT,
            contract_address TEXT,
            token_id TEXT,
            from_address TEXT,
            to_address TEXT,
            block_number BIGINT,
            timestamp BIGINT
        ) ON COMMIT DELETE ROWS;
        TRUNCATE tmp_nft_transfers;",
    )
    .await?;

    let sink = tx
        .copy_in(
            "COPY tmp_nft_transfers (tx_hash, log_index, contract_address, token_id, from_address, to_address, block_number, timestamp)
             FROM STDIN BINARY",
        )
        .await?;
    let writer = BinaryCopyInWriter::new(
        sink,
        &[
            Type::TEXT,
            Type::INT4,
            Type::TEXT,
            Type::TEXT,
            Type::TEXT,
            Type::TEXT,
            Type::INT8,
            Type::INT8,
        ],
    );
    pin!(writer);

    for i in 0..batch.nt_tx_hashes.len() {
        let row: [&(dyn ToSql + Sync); 8] = [
            &batch.nt_tx_hashes[i],
            &batch.nt_log_indices[i],
            &batch.nt_contracts[i],
            &batch.nt_token_ids[i],
            &batch.nt_froms[i],
            &batch.nt_tos[i],
            &batch.nt_block_numbers[i],
            &batch.nt_timestamps[i],
        ];
        writer.as_mut().write(&row).await?;
    }

    writer.finish().await?;

    tx.execute(
        "INSERT INTO nft_transfers
            (tx_hash, log_index, contract_address, token_id, from_address, to_address, block_number, timestamp)
         SELECT tx_hash, log_index, contract_address, token_id::numeric, from_address, to_address, block_number, timestamp
         FROM tmp_nft_transfers
         ON CONFLICT (tx_hash, log_index, block_number) DO NOTHING",
        &[],
    )
    .await?;

    Ok(())
}

pub async fn copy_erc20_transfers(tx: &mut Transaction<'_>, batch: &BlockBatch) -> Result<()> {
    if batch.et_tx_hashes.is_empty() {
        return Ok(());
    }

    tx.batch_execute(
        "CREATE TEMP TABLE IF NOT EXISTS tmp_erc20_transfers (
            tx_hash TEXT,
            log_index INT,
            contract_address TEXT,
            from_address TEXT,
            to_address TEXT,
            value TEXT,
            block_number BIGINT,
            timestamp BIGINT
        ) ON COMMIT DELETE ROWS;
        TRUNCATE tmp_erc20_transfers;",
    )
    .await?;

    let sink = tx
        .copy_in(
            "COPY tmp_erc20_transfers (tx_hash, log_index, contract_address, from_address, to_address, value, block_number, timestamp)
             FROM STDIN BINARY",
        )
        .await?;
    let writer = BinaryCopyInWriter::new(
        sink,
        &[
            Type::TEXT,
            Type::INT4,
            Type::TEXT,
            Type::TEXT,
            Type::TEXT,
            Type::TEXT,
            Type::INT8,
            Type::INT8,
        ],
    );
    pin!(writer);

    for i in 0..batch.et_tx_hashes.len() {
        let row: [&(dyn ToSql + Sync); 8] = [
            &batch.et_tx_hashes[i],
            &batch.et_log_indices[i],
            &batch.et_contracts[i],
            &batch.et_froms[i],
            &batch.et_tos[i],
            &batch.et_values[i],
            &batch.et_block_numbers[i],
            &batch.et_timestamps[i],
        ];
        writer.as_mut().write(&row).await?;
    }

    writer.finish().await?;

    tx.execute(
        "INSERT INTO erc20_transfers
            (tx_hash, log_index, contract_address, from_address, to_address, value, block_number, timestamp)
         SELECT tx_hash, log_index, contract_address, from_address, to_address, value::numeric, block_number, timestamp
         FROM tmp_erc20_transfers
         ON CONFLICT (tx_hash, log_index, block_number) DO NOTHING",
        &[],
    )
    .await?;

    Ok(())
}
