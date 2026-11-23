# Stratum
A Rust-powered data infrastructure layer for managing data warehouses, data models, and unified data access through a single API.

## Prerequisites

These steps are for Linux (Debian/Ubuntu), including WSL2 on Windows. For other platforms, see [rustup.rs](https://rustup.rs).

### 1. System build tools

Rust uses the system linker to produce the final binary. Some common crates also link against OpenSSL.

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev
```

### 2. Rust toolchain (via rustup)

`rustup` installs and manages Rust toolchains. The default installation includes `rustc`, `cargo`, `clippy` and `rustfmt`.

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # choose "1) Proceed with standard installation"
source "$HOME/.cargo/env"
```

This project uses the 2024 edition, so it needs **Rust 1.85 or newer**. Run `rustup update` to get the latest stable release.

### 3. Docker Engine + Compose (inside WSL2)

Stratum's local stand-ins for AWS run as containers. These are Docker's official steps for Ubuntu, run inside the WSL distro. Docker Engine is free and needs no Docker Desktop.

```bash
# Remove conflicting packages (fine if none are installed)
for pkg in docker.io docker-doc docker-compose docker-compose-v2 podman-docker containerd runc; do sudo apt-get remove -y $pkg; done

# Add Docker's apt repository and signing key
sudo apt-get update
sudo apt-get install -y ca-certificates curl
sudo install -m 0755 -d /etc/apt/keyrings
sudo curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc
sudo chmod a+r /etc/apt/keyrings/docker.asc
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/ubuntu $(. /etc/os-release && echo "${UBUNTU_CODENAME:-$VERSION_CODENAME}") stable" \
  | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
sudo apt-get update

# Install the engine, CLI and Compose plugin
sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin

# Run docker without sudo, and start it automatically with WSL
sudo usermod -aG docker "$USER"
sudo systemctl enable --now docker.service containerd.service
```

Then **open a new terminal** (or restart VS Code's WSL window) so the `docker` group applies.

> **WSL note:** the daemon is started by systemd. Make sure `/etc/wsl.conf` contains `[boot]` with `systemd=true`. If you change that file, run `wsl --shutdown` from Windows and reopen the terminal.

### 4. Verify the installation

```bash
rustup --version
rustc --version
cargo --version
cargo clippy --version
cargo fmt --version
docker version            # both Client and Server sections, without sudo
docker compose version
docker run --rm hello-world
```

### 5. Editor setup (VS Code)

If you use WSL, install these extensions **in WSL** rather than locally on Windows:

- **rust-analyzer** (`rust-lang.rust-analyzer`): language server, required
- **Even Better TOML** (`tamasfe.even-better-toml`): `Cargo.toml` support
- **CodeLLDB** (`vadimcn.vscode-lldb`): debugger, optional

Optionally, set `rust-analyzer.check.command` to `clippy` to show lint warnings inline.

## Project layout

Stratum is a Cargo workspace (like npm workspaces): one root `Cargo.toml` and one crate per component under `crates/`.

| Crate | Kind | Role |
|---|---|---|
| `stratum-core` | library | Shared domain model, config and errors |
| `stratum-aws` | library | S3/SQS/DynamoDB clients for the local stand-ins or real AWS |
| `stratum-db` | library | PostgreSQL pool, migrations and repositories |
| `stratum-ingest-api` | binary | HTTP ingest API (≈ API Gateway) |
| `stratum-firehose` | binary | Buffers records into gzip files on S3 (≈ Kinesis Firehose) |
| `stratum-scheduler` | binary | Cron jobs (≈ EventBridge Scheduler) |
| `stratum-worker` | binary | Classifies raw files and loads Postgres (SQS consumer) |
| `stratum-cli` | binary | Developer and operations CLI |

Other folders: `deploy/` (Docker Compose stack), `docs/` ([architecture](docs/architecture.md), [AWS alternatives](docs/aws-alternatives.md)) and `Epic/` (the [project board](Epic/README.md)).

## Build & test

```bash
cargo build --workspace                                  # build everything
cargo run -p stratum-ingest-api                          # run one binary (-p = package)
cargo test --workspace                                   # all tests
cargo fmt --all --check                                  # formatting
cargo clippy --workspace --all-targets -- -D warnings    # lints (warnings fail)
```

## Local AWS stand-ins

Free containers replace the AWS services (see [docs/aws-alternatives.md](docs/aws-alternatives.md) for why each was chosen):

| AWS | Stand-in | Endpoint (from WSL or Windows) |
|---|---|---|
| S3 | RustFS | API `http://localhost:9000`, console http://localhost:9001/rustfs/console/ |
| SQS | ElasticMQ | `http://localhost:9324` |
| DynamoDB | DynamoDB Local | `http://localhost:8000` |
| RDS PostgreSQL | postgres:18 | `localhost:5432`, database `stratum` |

### Start / stop

```bash
cp deploy/.env.example deploy/.env                       # once; dev-only credentials (git-ignored)
docker compose -f deploy/docker-compose.yml up -d --wait # start and wait until healthy
docker compose -f deploy/docker-compose.yml ps           # status
docker compose -f deploy/docker-compose.yml down         # stop (data is kept)
docker compose -f deploy/docker-compose.yml down -v      # stop and wipe all data
```

At startup the stack:

- creates the buckets `stratum-raw` and `stratum-staged` (the one-shot `s3-bootstrap` service)
- creates the queues `raw-files` and `transform`, each with a dead-letter queue (`*-dlq`, after 5 receives) from `deploy/elasticmq/elasticmq.conf`

DynamoDB tables are created later by `stratum-cli catalog init` (STR-401).

### Credentials

Use the values from `deploy/.env` (defaults shown):

- **RustFS (S3):** access key `stratum`, secret `stratum-dev-secret`. Also use these to log in to the console.
- **ElasticMQ and DynamoDB Local:** accept any access key and secret.
- **PostgreSQL:** user `stratum`, password `stratum-dev`.

### Poke at the services without installing the AWS CLI

```bash
alias awsl='docker run --rm --network host -e AWS_ACCESS_KEY_ID=stratum -e AWS_SECRET_ACCESS_KEY=stratum-dev-secret -e AWS_DEFAULT_REGION=us-east-1 amazon/aws-cli:2.41.2'

awsl s3 ls --endpoint-url http://localhost:9000                     # buckets
awsl sqs list-queues --endpoint-url http://localhost:9324           # queues
awsl sqs get-queue-attributes --endpoint-url http://localhost:9324 \
  --queue-url http://localhost:9324/000000000000/raw-files --attribute-names All
awsl dynamodb list-tables --endpoint-url http://localhost:8000      # tables
docker compose -f deploy/docker-compose.yml exec postgres psql -U stratum -d stratum   # SQL shell
```
