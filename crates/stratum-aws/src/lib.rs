//! AWS client construction for Stratum.
//!
//! Builds S3, SQS and DynamoDB clients that point at the local stand-ins
//! (RustFS, ElasticMQ, DynamoDB Local) or at real AWS, depending on [`AwsSettings`].
//!
//! The rule is per service: a service **with** an endpoint configured talks to that
//! endpoint, signed with [`AwsSettings::local_credentials`] when given. A service
//! **without** one uses the SDK's default config chain (env vars, profile, web
//! identity, ECS, EC2 metadata) with no local overrides, as on real AWS.
//!
//! Findings behind these choices: `docs/spikes/STR-104-aws-sdk-local.md`.

use aws_config::{BehaviorVersion, Region, SdkConfig};

pub use aws_credential_types::Credentials;

/// Provider name attached to credentials built by [`local_credentials`].
/// It shows up in the SDK's debug output, so you can tell where credentials came from.
pub const LOCAL_CREDENTIALS_PROVIDER: &str = "stratum-local";

/// Where and how to reach AWS or its local stand-ins.
///
/// `AwsSettings::default()` (every field `None`) means real AWS with the default
/// config chain. `Debug` output is safe to log: [`Credentials`] redacts the secret.
#[derive(Debug, Clone, Default)]
pub struct AwsSettings {
    pub region: Option<String>,
    /// S3 endpoint override, e.g. `http://localhost:9000` for RustFS.
    /// When set, the client also uses path-style addressing.
    pub s3_endpoint: Option<String>,
    /// SQS endpoint override, e.g. `http://localhost:9324` for ElasticMQ.
    pub sqs_endpoint: Option<String>,
    /// DynamoDB endpoint override, e.g. `http://localhost:8000` for DynamoDB Local.
    pub dynamodb_endpoint: Option<String>,
    /// Static credentials for the services that have an endpoint override.
    /// Ignored for services without one (they use the default chain).
    pub local_credentials: Option<Credentials>,
}

/// Static credentials for the local stand-ins, tagged with [`LOCAL_CREDENTIALS_PROVIDER`].
pub fn local_credentials(
    access_key_id: impl Into<String>,
    secret_access_key: impl Into<String>,
) -> Credentials {
    Credentials::new(
        access_key_id,
        secret_access_key,
        None,
        None,
        LOCAL_CREDENTIALS_PROVIDER,
    )
}

/// Builds an S3 client. With `s3_endpoint` set, it targets that endpoint with
/// path-style addressing (`http://host/bucket/key`), which S3-compatible servers need.
pub async fn s3_client(settings: &AwsSettings) -> aws_sdk_s3::Client {
    let endpoint = settings.s3_endpoint.as_deref();
    let shared = load_sdk_config(settings, endpoint).await;
    let config = aws_sdk_s3::config::Builder::from(&shared)
        .force_path_style(endpoint.is_some())
        .build();
    aws_sdk_s3::Client::from_conf(config)
}

/// Builds an SQS client. With `sqs_endpoint` set, it targets that endpoint.
pub async fn sqs_client(settings: &AwsSettings) -> aws_sdk_sqs::Client {
    let shared = load_sdk_config(settings, settings.sqs_endpoint.as_deref()).await;
    aws_sdk_sqs::Client::new(&shared)
}

/// Builds a DynamoDB client. With `dynamodb_endpoint` set, it targets that endpoint.
pub async fn dynamodb_client(settings: &AwsSettings) -> aws_sdk_dynamodb::Client {
    let shared = load_sdk_config(settings, settings.dynamodb_endpoint.as_deref()).await;
    aws_sdk_dynamodb::Client::new(&shared)
}

/// Loads the shared SDK config for one service.
async fn load_sdk_config(settings: &AwsSettings, endpoint: Option<&str>) -> SdkConfig {
    let mut loader = aws_config::defaults(BehaviorVersion::latest());

    if let Some(region) = &settings.region {
        loader = loader.region(Region::new(region.clone()));
    }

    if let Some(url) = endpoint {
        loader = loader.endpoint_url(url);
        if let Some(credentials) = &settings.local_credentials {
            loader = loader.credentials_provider(credentials.clone());
        }
    }

    loader.load().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_credential_types::provider::ProvideCredentials;

    const LOCAL_S3: &str = "http://localhost:9000";

    /// Region is always set so `load()` never waits on EC2 metadata for a region.
    fn settings_with_local_credentials() -> AwsSettings {
        AwsSettings {
            region: Some("us-east-1".to_string()),
            local_credentials: Some(local_credentials("test-key", "test-secret")),
            ..AwsSettings::default()
        }
    }

    #[tokio::test]
    async fn endpoint_override_applies_endpoint_and_local_credentials() {
        let sdk = load_sdk_config(&settings_with_local_credentials(), Some(LOCAL_S3)).await;

        assert_eq!(sdk.endpoint_url(), Some(LOCAL_S3));
        let credentials = sdk
            .credentials_provider()
            .expect("a credentials provider is always set")
            .provide_credentials()
            .await
            .expect("static credentials resolve without I/O");
        assert_eq!(credentials.access_key_id(), "test-key");
    }

    // STR-104 AC5: no endpoint means the default chain, even if local credentials exist.
    #[tokio::test]
    async fn no_endpoint_falls_back_to_default_chain() {
        let sdk = load_sdk_config(&settings_with_local_credentials(), None).await;

        assert_eq!(sdk.endpoint_url(), None);
        let provider = sdk
            .credentials_provider()
            .expect("the default chain is always set");
        assert!(
            !format!("{provider:?}").contains(LOCAL_CREDENTIALS_PROVIDER),
            "local credentials must not leak into the default chain: {provider:?}"
        );
    }

    #[tokio::test]
    async fn region_override_is_applied() {
        let sdk = load_sdk_config(&settings_with_local_credentials(), None).await;
        assert_eq!(sdk.region().map(|r| r.as_ref()), Some("us-east-1"));
    }

    #[test]
    fn debug_output_redacts_the_secret() {
        let printed = format!("{:?}", settings_with_local_credentials());
        assert!(printed.contains("test-key"));
        assert!(!printed.contains("test-secret"), "secret leaked: {printed}");
    }
}
