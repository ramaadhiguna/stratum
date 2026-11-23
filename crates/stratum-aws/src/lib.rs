//! AWS client construction for Stratum.
//!
//! Builds S3, SQS and DynamoDB clients that point at the local stand-ins
//! (RustFS, ElasticMQ, DynamoDB Local) or at real AWS, depending on config.
//! See `docs/aws-alternatives.md`. Implemented in STR-104.
