// Imports
use aws_config::BehaviorVersion;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::primitives::ByteStream;

// Starts the main function with the tokio runtime, which is required for asynchronous operations in Rust.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // This is the SDK Config, in a sense you setup endpoint, region, and other configurations for the AWS SDK.
    let sdk_config = aws_config::defaults(BehaviorVersion::latest())
        .endpoint_url("http://localhost:9000")
        .region(Region::new("us-east-1"))
        .load()
        .await;

    // This is the S3 Config, which is built from the SDK Config.
    // It sets the force_path_style to true, tells S3 to put bucket names in the path instead of the hostname
    // Which is often required for local S3-compatible services like MinIO.
    let s3_config = aws_sdk_s3::config::Builder::from(&sdk_config)
        .force_path_style(true)
        .build();
    let client = aws_sdk_s3::Client::from_conf(s3_config);

    // Values used to setup the S3 bucket and object. These are hardcoded for this example.
    let bucket_name = "stratum-raw";
    let object_key = "spike/hello.txt";
    let object_payload = "Hello, S3!";

    // Put object into the S3 bucket. This is an asynchronous operation, so we await its completion.
    client
        .put_object()
        .bucket(bucket_name)
        .key(object_key)
        .body(ByteStream::from(object_payload.as_bytes().to_vec()))
        .send()
        .await?;

    // Confirm that the object was put into the bucket by printing a message to the console.
    let listed = client
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
    let get_object = client
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
