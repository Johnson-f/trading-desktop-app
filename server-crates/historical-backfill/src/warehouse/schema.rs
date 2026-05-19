//! Iceberg schema, partition spec, sort order for bars tables.
//! Both `bars_1d` and `bars_1m` use the same logical schema. Partition
//! granularity is table-specific: `bars_1d` uses `Transform::Year` on `ts`
//! (column name `ts_year`); `bars_1m` uses `Transform::Month` on `ts`
//! (column name `ts_month`) to keep per-partition file sizes manageable.

use anyhow::{Context, Result};
use iceberg::spec::{
    NestedField, NullOrder, PartitionSpec, PrimitiveType, Schema, SortDirection, SortField,
    SortOrder, Transform, Type,
};

use crate::warehouse::Table;

/// Build the Iceberg schema for a bars table. Both `Table::Daily` and
/// `Table::Minute` currently return identical schemas — the parameter is
/// retained so we can diverge later (e.g., a `seq_no` column on the minute
/// table) without breaking call sites. Field IDs 1–8 are assigned in
/// declared order.
pub fn iceberg_schema(_table: Table) -> Result<Schema> {
    Schema::builder()
        .with_fields(vec![
            NestedField::required(1, "symbol", Type::Primitive(PrimitiveType::String)).into(),
            NestedField::required(2, "ts", Type::Primitive(PrimitiveType::Timestamptz)).into(),
            NestedField::required(3, "open", Type::Primitive(PrimitiveType::Int)).into(),
            NestedField::required(4, "high", Type::Primitive(PrimitiveType::Int)).into(),
            NestedField::required(5, "low", Type::Primitive(PrimitiveType::Int)).into(),
            NestedField::required(6, "close", Type::Primitive(PrimitiveType::Int)).into(),
            NestedField::required(7, "volume", Type::Primitive(PrimitiveType::Long)).into(),
            NestedField::required(8, "version", Type::Primitive(PrimitiveType::Long)).into(),
        ])
        .build()
        .context("build Iceberg schema")
}

pub fn partition_spec(schema: &Schema, table: Table) -> Result<PartitionSpec> {
    // PartitionSpec::builder requires an owned SchemaRef (Arc<Schema>); clone is unavoidable.
    let builder = PartitionSpec::builder(schema.clone());
    match table {
        Table::Daily => builder
            .add_partition_field("ts", "ts_year", Transform::Year)
            .context("add ts_year partition field")?
            .build()
            .context("build partition spec"),
        Table::Minute => builder
            .add_partition_field("ts", "ts_month", Transform::Month)
            .context("add ts_month partition field")?
            .build()
            .context("build partition spec"),
    }
}

pub fn sort_order(schema: &Schema) -> Result<SortOrder> {
    let symbol_id = schema
        .field_id_by_name("symbol")
        .context("symbol field not found in schema")?;
    let ts_id = schema
        .field_id_by_name("ts")
        .context("ts field not found in schema")?;

    SortOrder::builder()
        .with_sort_field(
            SortField::builder()
                .source_id(symbol_id)
                .transform(Transform::Identity)
                .direction(SortDirection::Ascending)
                .null_order(NullOrder::First)
                .build(),
        )
        .with_sort_field(
            SortField::builder()
                .source_id(ts_id)
                .transform(Transform::Identity)
                .direction(SortDirection::Ascending)
                .null_order(NullOrder::First)
                .build(),
        )
        .build_unbound()
        .context("build sort order")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iceberg_schema_daily_has_eight_fields() {
        let schema = iceberg_schema(Table::Daily).unwrap();
        let fields = schema.as_struct().fields();
        assert_eq!(fields.len(), 8);
        let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "symbol", "ts", "open", "high", "low", "close", "volume", "version"
            ]
        );
    }

    #[test]
    fn partition_spec_daily_partitions_by_year_of_ts() {
        let schema = iceberg_schema(Table::Daily).unwrap();
        let spec = partition_spec(&schema, Table::Daily).unwrap();
        assert_eq!(spec.fields().len(), 1);
        assert_eq!(spec.fields()[0].name, "ts_year");
    }

    #[test]
    fn partition_spec_minute_partitions_by_month_of_ts() {
        let schema = iceberg_schema(Table::Minute).unwrap();
        let spec = partition_spec(&schema, Table::Minute).unwrap();
        assert_eq!(spec.fields().len(), 1);
        assert_eq!(spec.fields()[0].name, "ts_month");
    }

    #[test]
    fn sort_order_has_two_fields() {
        let schema = iceberg_schema(Table::Daily).unwrap();
        let order = sort_order(&schema).unwrap();
        assert_eq!(order.fields.len(), 2);
    }

    #[test]
    fn iceberg_schema_daily_and_minute_are_identical() {
        let daily = iceberg_schema(Table::Daily).unwrap();
        let minute = iceberg_schema(Table::Minute).unwrap();
        let daily_fields = daily.as_struct().fields();
        let minute_fields = minute.as_struct().fields();
        assert_eq!(daily_fields.len(), minute_fields.len());
        for (d, m) in daily_fields.iter().zip(minute_fields.iter()) {
            assert_eq!(d.name, m.name, "field name mismatch");
            assert_eq!(d.id, m.id, "field id mismatch");
            assert_eq!(d.field_type, m.field_type, "field type mismatch");
            assert_eq!(d.required, m.required, "field required mismatch");
        }
    }
}
