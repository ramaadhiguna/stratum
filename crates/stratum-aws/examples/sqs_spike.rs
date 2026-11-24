// Local stack settings shared by all examples (examples/common/mod.rs).
mod common;

// Starts the main function with the tokio runtime, which is required for asynchronous operations in Rust.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The SQS client comes from stratum-aws, pointed at ElasticMQ by the local settings.
    let settings = common::local_settings();
    let sqs_client = stratum_aws::sqs_client(&settings).await;

    // Check queue URL for its name
    let url_output = sqs_client
        .get_queue_url()
        .queue_name("raw-files-queue")
        .send()
        .await?;
    let queue_url = url_output.queue_url().ok_or("Queue URL not found")?;
    println!("Queue URL: {}", queue_url);

    // Send a message to queue
    let body = "Hello, SQS!";
    let send_message = sqs_client
        .send_message()
        .queue_url(queue_url)
        .message_body(body)
        .send()
        .await?;
    println!(
        "Sent message with ID: {}",
        send_message.message_id().unwrap_or_default()
    );

    // Now receive the message from the queue
    let receive_message = sqs_client
        .receive_message()
        .queue_url(queue_url)
        .max_number_of_messages(1)
        .wait_time_seconds(5) // Long polling for 5 seconds to make sure we get the message
        .send()
        .await?;
    let message = receive_message
        .messages()
        .first()
        .ok_or("No messages received")?;
    println!("Received message: {}", message.body().unwrap_or_default());

    // Delete the message from the queue after processing using receipt handle to avoid it being received again
    let receipt_handle = message.receipt_handle().ok_or("No receipt handle found")?;
    sqs_client
        .delete_message()
        .queue_url(queue_url)
        .receipt_handle(receipt_handle)
        .send()
        .await?;

    // Validate that the message was deleted by trying to receive it again
    let receive_message_after_delete = sqs_client
        .receive_message()
        .queue_url(queue_url)
        .max_number_of_messages(1)
        .wait_time_seconds(5)
        .send()
        .await?;
    assert!(
        receive_message_after_delete.messages().is_empty(),
        "Message was not deleted from the queue"
    );

    Ok(())
}
