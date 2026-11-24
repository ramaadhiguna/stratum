// Imports
use aws_sdk_dynamodb::types::{
    AttributeDefinition, AttributeValue, BillingMode, KeySchemaElement, KeyType,
    ScalarAttributeType,
};

// Local stack settings shared by all examples (examples/common/mod.rs).
mod common;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The DynamoDB client comes from stratum-aws, pointed at DynamoDB Local by the local settings.
    let settings = common::local_settings();
    let client = stratum_aws::dynamodb_client(&settings).await;

    // A temporary table, deleted at the end of the run.
    let table = "stratum-spike-sources";

    // Create a table with a single string key, source_id. The table is deleted at the end of the run.
    client
        .create_table()
        .table_name(table)
        .attribute_definitions(
            AttributeDefinition::builder()
                .attribute_name("source_id")
                .attribute_type(ScalarAttributeType::S)
                .build()?,
        )
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name("source_id")
                .key_type(KeyType::Hash)
                .build()?,
        )
        .billing_mode(BillingMode::PayPerRequest)
        .send()
        .await?;
    println!("created table {table}");

    // Put an item into the table, with a string and a number attribute.
    // DynamoDB sends numbers as text, so we send it as a string.
    // This is also an examples on how AttributeValue can be used to represent different types of values, such as strings and numbers.
    // String attributes are represented as AttributeValue::S, and number attributes are represented as AttributeValue::N.
    client
        .put_item()
        .table_name(table)
        .item("source_id", AttributeValue::S("sensor-42".to_string()))
        .item("product_type", AttributeValue::S("thermostat".to_string()))
        .item("retention_days", AttributeValue::N("30".to_string()))
        .send()
        .await?;
    println!("put item sensor-42");

    // Get the item back out of the table, using a consistent read to make sure we get the latest value.
    let got = client
        .get_item()
        .table_name(table)
        .key("source_id", AttributeValue::S("sensor-42".to_string()))
        .consistent_read(true)
        .send()
        .await?;
    let item = got.item().ok_or("item not found")?;
    println!("got item: {item:?}");

    // A string attribute: compare the whole enum value.
    assert_eq!(
        item.get("product_type"),
        Some(&AttributeValue::S("thermostat".to_string()))
    );

    // A number attribute: DynamoDB sends numbers as text, so match it, then parse.
    let retention_days: u32 = match item.get("retention_days") {
        Some(AttributeValue::N(number)) => number.parse()?,
        other => return Err(format!("expected a number, got {other:?}").into()),
    };
    assert_eq!(retention_days, 30);
    println!("retention_days = {retention_days}");

    // Clean up by deleting the table. This is a temporary table, so we don't need to keep it around.
    client.delete_table().table_name(table).send().await?;
    println!("deleted table {table}");

    Ok(())
}
