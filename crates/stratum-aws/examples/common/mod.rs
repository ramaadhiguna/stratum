//! Settings shared by the examples: the local stand-ins from `deploy/docker-compose.yml`.
//!
//! Each example pulls this in with `mod common;`. Cargo doesn't build it as an
//! example of its own because the folder has no `main.rs`.

use stratum_aws::AwsSettings;

/// Settings for the local Docker Compose stack.
///
/// Keys default to the dev values in `deploy/.env.example`. `S3_ACCESS_KEY` and
/// `S3_SECRET_KEY` override them, the same variables Compose reads for RustFS.
/// ElasticMQ and DynamoDB Local accept any keys, so the same pair is used everywhere.
pub fn local_settings() -> AwsSettings {
    let access_key = std::env::var("S3_ACCESS_KEY").unwrap_or_else(|_| "stratum".to_string());
    let secret_key =
        std::env::var("S3_SECRET_KEY").unwrap_or_else(|_| "stratum-dev-secret".to_string());

    AwsSettings {
        region: Some("us-east-1".to_string()),
        s3_endpoint: Some("http://localhost:9000".to_string()),
        sqs_endpoint: Some("http://localhost:9324".to_string()),
        dynamodb_endpoint: Some("http://localhost:8000".to_string()),
        local_credentials: Some(stratum_aws::local_credentials(access_key, secret_key)),
    }
}
