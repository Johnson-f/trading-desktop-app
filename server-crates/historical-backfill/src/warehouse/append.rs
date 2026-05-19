//! Append-only write path. Converts `&[Bar]` to an Arrow `RecordBatch`,
//! partitions it via `RecordBatchPartitionSplitter`, fans out per-partition
//! Parquet files through `FanoutWriter` over `RollingFileWriterBuilder`,
//! and commits the resulting data files with a `FastAppendAction`.
//!
//! Limitation: between `fanout.close()` and `tx.commit()`, a transient
//! catalog-commit failure leaves orphan Parquet files in storage that no
//! snapshot references. Periodic orphan-file cleanup is required; track
//! this with Iceberg's `ExpireSnapshots` + orphan-file scan jobs.

use std::sync::Arc;

use anyhow::{Context, Result};
use iceberg::arrow::RecordBatchPartitionSplitter;
use iceberg::transaction::{ApplyTransactionAction, Transaction};
use iceberg::writer::base_writer::data_file_writer::DataFileWriterBuilder;
use iceberg::writer::file_writer::ParquetWriterBuilder;
use iceberg::writer::file_writer::location_generator::{
    DefaultFileNameGenerator, DefaultLocationGenerator,
};
use iceberg::writer::file_writer::rolling_writer::RollingFileWriterBuilder;
use iceberg::writer::partitioning::PartitioningWriter;
use iceberg::writer::partitioning::fanout_writer::FanoutWriter;
use iceberg::{Catalog, NamespaceIdent, TableIdent};
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;

use crate::bar::{Bar, bars_to_record_batch};
use crate::warehouse::Table;

pub async fn append_bars(
    catalog: &dyn Catalog,
    namespace: &str,
    table: Table,
    bars: &[Bar],
) -> Result<usize> {
    if bars.is_empty() {
        return Ok(0);
    }

    let ident = TableIdent::new(
        NamespaceIdent::new(namespace.to_string()),
        table.name().to_string(),
    );
    let tbl = catalog
        .load_table(&ident)
        .await
        .context("load_table for append")?;

    let batch = bars_to_record_batch(bars).context("bars -> record batch")?;

    let location_gen =
        DefaultLocationGenerator::new(tbl.metadata().clone()).context("location generator")?;
    let suffix = format!(
        "{}-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        std::process::id()
    );
    let file_name_gen = DefaultFileNameGenerator::new(
        "data".to_string(),
        Some(suffix),
        iceberg::spec::DataFileFormat::Parquet,
    );

    let props = WriterProperties::builder()
        .set_compression(Compression::ZSTD(
            ZstdLevel::try_new(3).context("zstd level 3")?,
        ))
        .build();

    // bar_arrow_schema() embeds PARQUET:field_id metadata on every field so
    // the default FieldMatchMode::Id lookup resolves correctly.
    let parquet_builder = ParquetWriterBuilder::new(props, tbl.metadata().current_schema().clone());

    let rolling_writer_builder = RollingFileWriterBuilder::new_with_default_file_size(
        parquet_builder,
        tbl.file_io().clone(),
        location_gen,
        file_name_gen,
    );

    let data_file_builder = DataFileWriterBuilder::new(rolling_writer_builder);

    // The table is partitioned by year(ts). Use RecordBatchPartitionSplitter to
    // split the batch by partition, then write each partition's sub-batch via
    // FanoutWriter so the resulting DataFiles carry the correct partition value.
    let partition_spec = Arc::new(tbl.metadata().default_partition_spec().as_ref().clone());
    let iceberg_schema = tbl.metadata().current_schema().clone();
    let splitter =
        RecordBatchPartitionSplitter::try_new_with_computed_values(iceberg_schema, partition_spec)
            .context("build partition splitter")?;

    let partitioned_batches = splitter.split(&batch).context("split batch by partition")?;

    let mut fanout = FanoutWriter::new(data_file_builder);
    for (partition_key, sub_batch) in partitioned_batches {
        fanout
            .write(partition_key, sub_batch)
            .await
            .context("write partition batch")?;
    }
    let data_files = fanout.close().await.context("close fanout writer")?;

    let tx = Transaction::new(&tbl);
    let action = tx.fast_append().add_data_files(data_files);
    let tx = action.apply(tx).context("apply fast_append action")?;
    tx.commit(catalog)
        .await
        .context("commit append transaction")?;

    Ok(bars.len())
}
