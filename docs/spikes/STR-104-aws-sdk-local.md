# Spike STR-104: AWS SDK for Rust against the local stand-ins

**Date:** 2026-11-24 · **Story:** [STR-104](../../Epic/E1-foundations-and-local-platform/Stories/STR-104-spike-aws-sdk-rust-local/main.json) · **Code:** `crates/stratum-aws/` (library + `examples/`)

## Verdict

All three SDK crates work against the stand-ins. The only overrides needed are the endpoint, region and credentials, plus path-style addressing for S3. No protocol workarounds were needed, and ElasticMQ handles the SQS JSON protocol. The client-building code now lives in `stratum-aws` (see [What was promoted](#what-was-promoted-into-stratum-aws)).

## Versions tested

| Component | Version |
|---|---|
| Rust toolchain / workspace `rust-version` | 1.100.0 / 1.94.1 |
| aws-config · aws-credential-types | 1.13.3 · 1.3.1 |
| aws-sdk-s3 · aws-sdk-sqs · aws-sdk-dynamodb | 1.161.0 · 1.122.0 · 1.140.0 |
| tokio | 1.54.1 |
| RustFS · ElasticMQ native · DynamoDB Local | 1.0.6 · 1.7.1 · 3.3.2 |

## Settings that differ from the SDK defaults (AC4)

| Setting | S3 → RustFS | SQS → ElasticMQ | DynamoDB → DynamoDB Local | Real AWS |
|---|---|---|---|---|
| Endpoint URL | `http://localhost:9000` | `http://localhost:9324` | `http://localhost:8000` | none (SDK resolves it) |
| Path-style addressing | **required**: `force_path_style(true)` | n/a | n/a | off (virtual-hosted) |
| Checksum mode | SDK default works; `RequestChecksumCalculation::WhenRequired` **not** needed | n/a | n/a | default |
| Region | must be set; we use `us-east-1` | must be set; queue URLs carry the region/account from `elasticmq.conf` | must be set; any value works (`-sharedDb`) | from the default chain |
| Credentials | must match the RustFS keys (`stratum` / `stratum-dev-secret` by default) | any value, but some must exist | any value, but some must exist | default chain |

"Some must exist" means the SDK signs every request and refuses to send without credentials, even to servers that ignore the signature.

## ElasticMQ and the SQS JSON protocol

**It works.** aws-sdk-sqs sends AWS JSON 1.0 requests. ElasticMQ 1.7.1 (served by `pekko-http`) answered with `application/json`, including errors, which arrive with an `x-amzn-query-error` header (e.g. `QueueDoesNotExist`). These calls were exercised: GetQueueUrl, SendMessage, ReceiveMessage (5 s long poll) and DeleteMessage.

## Findings and gotchas

1. **`rust-version` caps dependency versions.** `cargo add` is MSRV-aware.
   - With `rust-version = "1.85"` it picked aws-sdk-s3 1.96 and aws-config 1.8.11.
   - `"1.94"` counts as 1.94.0, which still picked older versions: aws-sdk-s3 1.137 and aws-config 1.8.18.
   - The current SDK needs `"1.94.1"`.
2. **S3 path-style is required.** With `force_path_style(false)`, the SDK sends requests to `stratum-raw.localhost:9000`, which has no DNS entry. The result is `DispatchFailure … dns error … Name or service not known`.
3. **Checksums:** RustFS 1.0.6 accepts the SDK's default request checksums on PutObject.
4. **Credential chain fallback.** With no credentials configured, the chain tries Environment, Profile, WebIdentityToken, EcsContainer, then Ec2InstanceMetadata. The last step waits about 1 s for a connection timeout, then fails with `DispatchFailure(CredentialsNotLoaded …)`. `stratum-aws` now sets static local credentials whenever a local endpoint is configured.
5. **Wrong port, confusing error.** Pointing SQS at 9001 (the RustFS console) produced `501 NotImplemented` XML: "unknown service 'sqs' in credential scope". The SDK then reported a JSON `DeserializeError`, because it expected `{` and got `<`. **Read the response body** for the real cause.
6. **Wrong name.** Using the bucket name as the queue name gave `400 QueueDoesNotExist`. The queues are now named `raw-files-queue` and `transform-queue` (each with a `-dlq`).
7. **ElasticMQ reads its config only at startup.** After editing `deploy/elasticmq/elasticmq.conf`, run `docker compose -f deploy/docker-compose.yml restart elasticmq`. Queues live in memory, so a restart also drops their messages.
8. **Visibility timeout hides, it doesn't delete.** Within the 60 s visibility timeout, an empty second receive doesn't prove the delete worked. Check `ApproximateNumberOfMessages` and `ApproximateNumberOfMessagesNotVisible` instead (both should be 0).
9. **Leftovers from interrupted runs.**
   - An undeleted SQS message reappears after 60 s. Purge the queue with `awsl sqs purge-queue`.
   - A spike table that wasn't dropped makes the next CreateTable fail with `ResourceInUseException`. Remove it with `awsl dynamodb delete-table`.
10. **DynamoDB Local creates tables instantly** (ACTIVE straight away). Real DynamoDB goes through `CREATING` first, so real code needs `client.wait_until_table_exists()`.
11. **DynamoDB numbers arrive as text** (`AttributeValue::N("30")`), so they must be parsed.
12. **How to read SDK errors:**
    - `ServiceError`: the server answered. Read `status` and `body`.
    - `DispatchFailure`: the request was never sent, because of DNS, the connection or credentials.

## What was promoted into `stratum-aws`

```rust
pub struct AwsSettings {
    pub region: Option<String>,
    pub s3_endpoint: Option<String>,
    pub sqs_endpoint: Option<String>,
    pub dynamodb_endpoint: Option<String>,
    pub local_credentials: Option<Credentials>,
}
pub fn local_credentials(access_key_id, secret_access_key) -> Credentials;
pub async fn s3_client(&AwsSettings) -> aws_sdk_s3::Client;
pub async fn sqs_client(&AwsSettings) -> aws_sdk_sqs::Client;
pub async fn dynamodb_client(&AwsSettings) -> aws_sdk_dynamodb::Client;
```

The rules are per service:
- **Endpoint set:** use that endpoint, sign with `local_credentials`, and (for S3) turn on path-style.
- **No endpoint:** the SDK's default chain, with no local overrides (AC5).
- **`region`:** applied whenever it is set.
- **Logging:** `Debug` output is safe to log, because `Credentials` redacts the secret.

Unit tests in `src/lib.rs` cover: endpoint plus local credentials applied, no endpoint means the default chain (AC5), the region override, and the redacted `Debug`.

The examples share `examples/common/mod.rs` (the local stack settings), so they no longer need `AWS_*` environment variables.

## Recommendations for later stories

- **STR-106 (config).** Its description plans a single `endpoint_url` in `AwsSettings`. Locally, each service has its own port, so keep **one endpoint per service** as above. Move `AwsSettings` into the config layer there, and keep the rule "endpoint set means local overrides".
- **STR-401 (`catalog init`).** Wait for `ACTIVE` after CreateTable (finding 10).
- **STR-403 (SQS consumer).** Design around finding 8: messages come back after the visibility timeout, so processing must be idempotent and long jobs need heartbeats.
- **Definition of Done.** The examples hard-code local endpoints and use `assert!`, which DOD6 and DOD8 forbid in non-test code. Decide whether examples and spikes are exempt.
