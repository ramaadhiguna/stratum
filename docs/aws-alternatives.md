# Free alternatives to the AWS services used in Stratum

Spike date: 2026-11-16. Status: **decided**. The choices below are what the stories in [`Epic/`](../Epic/) assume.

## Goal

Run the whole pipeline on a laptop (WSL2 + Docker) for free, with no AWS or vendor account, while keeping the code portable to real AWS.

Two rules guided every choice:

1. **Infrastructure we only *consume*** (object store, queue, key-value store, database): use a free stand-in that speaks the **real AWS wire protocol**. Our code then uses the official **AWS SDK for Rust**, and moving to real AWS later means removing the `endpoint_url` override and nothing else.
2. **Services whose *behaviour* is the interesting part** (Firehose buffering, the EventBridge scheduler): **build them ourselves in Rust**. Free alternatives exist for both, but building them teaches the most Rust (async, channels, timers, traits, compression, graceful shutdown).

---

## Decision summary

| AWS service | What Stratum needs from it | Chosen alternative | License | Build own? |
|---|---|---|---|---|
| **API Gateway + Lambda** (ingest API) | HTTPS endpoint, API-key auth, body-size limit, throttling | **`stratum-ingest-api`**: Axum + tower-http | MIT (crates) | ✅ build (it's our app) |
| **Kinesis Data Firehose** | PutRecordBatch, buffer by size/time, GZIP, write to S3 with time-partitioned keys, retry, error prefix | **`stratum-firehose`**: our own Rust service | — | ✅ build (learning) |
| **S3** | CreateBucket, PutObject, GetObject (streaming), ListObjectsV2 (prefix, StartAfter, pagination), HeadObject, DeleteObjects | **RustFS** (`rustfs/rustfs`) | Apache-2.0 | ❌ use |
| **EventBridge Scheduler** (cron only) | cron expressions, run a job on schedule, no double-runs, retry, run history | **`stratum-scheduler`**: our own Rust service (`croner` crate + Tokio + Postgres advisory locks) | — | ✅ build (learning) |
| **SQS** | Send/SendBatch, long-poll Receive, visibility timeout and ChangeMessageVisibility, Delete, DLQ via redrive policy, GetQueueAttributes | **ElasticMQ** (`softwaremill/elasticmq-native`) | Apache-2.0 | ❌ use |
| **DynamoDB** | CreateTable, PutItem/BatchWriteItem, GetItem, Query on a composite key | **DynamoDB Local** (`amazon/dynamodb-local`) | Free (AWS licence, not OSS) | ❌ use |
| **RDS for PostgreSQL** | Postgres with partitioning, advisory locks, COPY | **PostgreSQL** official image (`postgres`, pin a major version) | PostgreSQL License | ❌ use |
| **CloudWatch Logs/Metrics** | Structured logs, counters/histograms, alerting on DLQ depth | **`tracing`** JSON logs to stdout + **Prometheus** `/metrics` (optional **Grafana**) | MIT / Apache-2.0 / AGPL-3.0 | ❌ use |
| **IAM / Secrets Manager** | Credentials for the above | Static dev credentials in a git-ignored `.env` file | — | n/a |

No service lacked a free alternative, so no extra "build a clone" stories were needed. Firehose and the scheduler are built on purpose for learning value. Off-the-shelf fallbacks for both are listed below in case you'd rather skip them.

---

## Per-service details

### S3: RustFS (primary), Garage (fallback)

**Why RustFS:** It's an S3-compatible object store **written in Rust** and licensed **Apache-2.0**. It reached **1.0 GA in September 2026**. It runs as one container, serves the S3 API on `:9000` and a web console on `:9001`, which is handy for seeing the files Firehose writes. Being Rust, its source is also good reading later.

- Single-node, single-disk mode has no redundancy. That's fine for local development.
- Set your own `RUSTFS_ACCESS_KEY` / `RUSTFS_SECRET_KEY` (don't keep the well-known default).
- The container runs as UID `10001`. A host bind mount needs that ownership, so a **named Docker volume** is simpler.

**Considered:**

| Option | Verdict |
|---|---|
| **Garage** (Deuxfleurs, Rust, AGPL-3.0) | Good **fallback**. Mature and light. v2.3+ supports `garage server --single-node --default-bucket`, which removes the old manual layout steps. No bucket notifications, but we poll anyway. |
| **SeaweedFS** (Go, Apache-2.0) | Solid, but more moving parts than we need. |
| **MinIO** | ❌ **Avoid.** The community edition went into maintenance mode in Dec 2025. Official Docker images and binaries stopped in Oct 2025, and the GitHub repository was later archived (sources disagree on whether that was Feb or Apr 2026). Only unofficial or community-fork images remain, which carries supply-chain risk. |
| **Ceph RGW** | Far too heavy for a laptop. |

**AWS SDK gotchas against any non-AWS S3** (covered in story STR-104):
- Enable **path-style addressing** (`force_path_style(true)`). Otherwise the SDK sends requests to `http://<bucket>.localhost:9000`.
- Newer AWS SDKs compute **extra checksums by default** (`WhenSupported`). Some S3-compatible servers reject these headers. If uploads fail with signature or checksum errors, set request checksum calculation to `WhenRequired`, either in config or with `AWS_REQUEST_CHECKSUM_CALCULATION=when_required`.
- Use any region string (for example `us-east-1`) and the static keys you configured.

### SQS: ElasticMQ

**Why:** It's the long-standing **SQS-compatible** message queue (Scala, Apache-2.0), published as a **native-image container**, so it starts fast without a JVM. It implements the SQS behaviours our worker depends on:
- **visibility timeout** and `ChangeMessageVisibility` (so a worker can extend its lease on a big file)
- **long polling** (`WaitTimeSeconds`)
- **dead-letter queues** via `deadLettersQueue { name, maxReceiveCount }` in its config file, which also **creates queues at startup**. That means no bootstrap script is needed.
- The SQS API is on `9324`. The native image (1.7.1) ships **no web UI**, so read queue depth with `aws sqs get-queue-attributes` (verified in STR-103).

**Risk to verify in the spike (STR-104):** The AWS SDKs moved SQS from the XML "Query" protocol to the **AWS JSON protocol** (2023). ElasticMQ supports the JSON protocol, and its 1.6/1.7 release notes include JSON-protocol fixes. Still, confirm `aws-sdk-sqs` for Rust can send, receive and delete against it before building on it.

**Considered:**

| Option | Verdict |
|---|---|
| **Moto server** (`motoserver/moto`, Python, Apache-2.0) | Fallback. One container that mocks S3, SQS and DynamoDB together, but it aims at test mocking rather than realistic semantics. |
| **Floci** (Java, MIT, `:4566`) | A young (2026) all-in-one LocalStack replacement with broad service coverage. Worth watching, and a fallback if you prefer one container. |
| **pgmq** (Postgres extension) | Good queue, but not the SQS API. We'd lose portability to real SQS. |
| **LocalStack** | ❌ **Not "no-account" any more.** Since **23 Mar 2026** the image requires a `LOCALSTACK_AUTH_TOKEN` (the free Hobby tier needs sign-up), and the old community repo was archived. |

### DynamoDB: DynamoDB Local

**Why:** It's **made by AWS** (Java, Docker image `amazon/dynamodb-local`, v3.x; latest seen 3.3.0, Jan 2026), so its API behaviour is the closest to the real service. It's free to use but **not open source**, which is fine for our "free" goal.
- Run it with `-sharedDb`. Without that flag, it keeps a separate database per access key and region pair, a classic "my table disappeared" trap.
- Use `-inMemory` for throwaway data, or mount a volume and drop `-inMemory` to persist.
- Want a GUI? **`dynamodb-admin`** is an npm package (`npx dynamodb-admin`), which uses your Node background.

**Considered:** ScyllaDB Alternator (heavier, different licence terms), Moto, Floci.

### Kinesis Data Firehose: build `stratum-firehose`

The Firehose functions we mimic (not the whole service):

| Firehose feature | Our version |
|---|---|
| `PutRecordBatch` API | `POST /v1/streams/{stream}/records` (JSON array, max 500 records / 4 MiB, like Firehose) |
| Buffering hints (size *or* interval, whichever comes first) | Configurable `buffer_max_bytes` (default 5 MiB) and `buffer_interval` (default 60 s) per stream |
| GZIP compression | `flate2` gzip, newline-delimited JSON (NDJSON) |
| S3 key format `YYYY/MM/DD/HH/<stream>-<ver>-<timestamp>-<random>` | `raw/<stream>/year=YYYY/month=MM/day=DD/hour=HH/<stream>-<yyyyMMddHHmmss>-<uuid>.ndjson.gz` |
| Retry and error output prefix | Exponential backoff, then write to the `errors/` prefix |
| Throttling | Bounded channel; returns `429` when the buffer is full (backpressure) |

**Off-the-shelf fallback:** **Vector** (Rust, MPL-2.0) with an `http_server` source and an `aws_s3` sink does batching and compression too. **Fluent Bit** (Apache-2.0) with an `http` input and an `s3` output also works.

### EventBridge Scheduler (cron): build `stratum-scheduler`

What we mimic: cron expressions, invoking a target (an in-process job), "flexible time window off", a retry policy, and run history.

- Parse cron with **`croner`** (or `cron`) and compute the next fire time. `tokio::time::sleep_until` waits for it.
- **No double-runs:** each run takes a **Postgres advisory lock** keyed by job name. A second scheduler instance skips the job.
- **History:** a `stratum.job_runs` table (start, end, status, error).

**Off-the-shelf fallback:** `tokio-cron-scheduler` crate, `pg_cron` (cron inside Postgres), or plain system cron calling `stratum-cli jobs run <name>`.

### API Gateway + Lambda: build `stratum-ingest-api`

This is our app, not infrastructure. Axum provides routing and extractors. `tower-http` adds body-size limits, timeouts, request IDs and tracing. API keys are hashed in config and mapped to a tenant. We run long-lived services, not functions, so there's no Lambda equivalent to replace.

### RDS for PostgreSQL: official Postgres image

Pin a major version tag (don't use `latest`). We rely on **declarative partitioning**, **advisory locks**, `ON CONFLICT DO NOTHING`, and `COPY`/`UNNEST` bulk loads, all core Postgres features.

### CloudWatch: tracing + Prometheus

`tracing` + `tracing-subscriber` (JSON output) for logs, and `metrics` + `metrics-exporter-prometheus` for a `/metrics` endpoint per service. Prometheus and Grafana containers are **optional** (story STR-606).

---

## Local port map

| Component | Port(s) |
|---|---|
| `stratum-ingest-api` | 8080 |
| `stratum-firehose` | 8081 |
| `stratum-scheduler` (health/metrics) | 8082 |
| `stratum-worker` (health/metrics) | 8083 |
| RustFS (S3 API / console) | 9000 / 9001 |
| ElasticMQ (SQS API) | 9324 |
| DynamoDB Local | 8000 |
| PostgreSQL | 5432 |

---

## Sources

- MinIO status: [itsfoss: MinIO moves away from open source](https://itsfoss.com/news/minio-moves-away-from-open-source/), [linuxiac: MinIO ends active development](https://linuxiac.com/minio-ends-active-development/), [MinIO CE in 2026: retired upstream](https://glukhov.org/data-infrastructure/object-storage/minio-dead/)
- RustFS: [RustFS 1.0 reaches GA (linuxiac)](https://linuxiac.com/rustfs-1-0-s3-compatible-object-storage-reaches-ga/), [RustFS Docker install docs](https://docs.rustfs.com/installation/docker/), [Self-hosted S3 alternatives in 2026](https://glukhov.org/data-infrastructure/object-storage/self-hosted-s3-alternatives/)
- Garage: [Garage quick start](https://garagehq.deuxfleurs.fr/documentation/quick-start/)
- LocalStack change: [LocalStack now requires an account (DEV)](https://dev.to/peytongreen_dev/localstack-now-requires-an-account-heres-how-to-test-aws-in-python-without-one-in-2026-2153), [Free LocalStack alternatives in 2026](https://dev.to/vieiralucas/free-localstack-alternatives-in-2026-fakecloud-ministack-floci-moto-4h78)
- Floci: [floci-io/floci on GitHub](https://github.com/floci-io/floci)
- ElasticMQ: [softwaremill/elasticmq](https://github.com/softwaremill/elasticmq), [ElasticMQ releases](https://github.com/softwaremill/elasticmq/releases)
- SQS JSON protocol: [Amazon SQS AWS JSON protocol FAQs](https://docs.aws.amazon.com/AWSSimpleQueueService/latest/SQSDeveloperGuide/sqs-json-faqs.html)
- DynamoDB Local: [Release history](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/DynamoDBLocalHistory.html), [DynamoDB Local 3.0.0 announcement](https://aws.amazon.com/about-aws/whats-new/2025/07/amazon-dynamodb-major-version-release-version-3-0-0)
- S3 checksum defaults: [AWS SDKs: data integrity protections for S3](https://docs.aws.amazon.com/sdkref/latest/guide/feature-dataintegrity.html), [aws-sdk-s3 config source (docs.rs)](https://docs.rs/aws-sdk-s3/latest/src/aws_sdk_s3/config.rs.html), [GitLab Runner fix for third-party S3](https://gitlab.com/gitlab-org/gitlab-runner/-/merge_requests/6691)
- Vector (Firehose fallback): [HTTP Server source](https://vector.dev/docs/reference/configuration/sources/http_server)
- Crate versions checked on crates.io on 2026-11-16 (tokio 1.54, axum 0.8, sqlx 0.9, aws-sdk-s3 1.159, aws-sdk-sqs 1.121, aws-sdk-dynamodb 1.139, croner 4.0, moka 0.12).
