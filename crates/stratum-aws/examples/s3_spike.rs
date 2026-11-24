// Imports
use aws_sdk_s3::primitives::ByteStream;

// Local stack settings shared by all examples (examples/common/mod.rs).
mod common;

// Starts the main function with the tokio runtime, which is required for asynchronous operations in Rust.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The S3 client comes from stratum-aws: endpoint, region, local credentials and
    // path-style addressing (bucket in the path, not the hostname) are set there.
    let settings = common::local_settings();
    let s3_client = stratum_aws::s3_client(&settings).await;

    // Values used to setup the S3 bucket and object. These are hardcoded for this example.
    let bucket_name = "stratum-raw";
    let object_key = "spike/hello.txt";
    let object_payload = "Hello, S3!";

    // Put object into the S3 bucket. This is an asynchronous operation, so we await its completion.
    s3_client
        .put_object()
        .bucket(bucket_name)
        .key(object_key)
        .body(ByteStream::from(object_payload.as_bytes().to_vec()))
        .send()
        .await?;

    // Confirm that the object was put into the bucket by printing a message to the console.
    let listed = s3_client
        .list_objects_v2()
        .bucket(bucket_name)
        .prefix("spike/")
        .send()
        .await?;
    for obj in listed.contents() {
        println!(
            "Found object: {:?} ({:?} bytes)",
            obj.key().unwrap_or_default(),
            obj.size().unwrap_or_default()
        );
    }
    let found = listed
        .contents()
        .iter()
        .any(|obj| obj.key() == Some(object_key));
    assert!(found, "Object not found in bucket after put operation");

    // Get the object and compare the size
    let get_object = s3_client
        .get_object()
        .bucket(bucket_name)
        .key(object_key)
        .send()
        .await?;
    let bytes = get_object.body.collect().await?.into_bytes();
    assert_eq!(
        &bytes[..],
        object_payload.as_bytes(),
        "Object content does not match"
    );

    Ok(())
}
