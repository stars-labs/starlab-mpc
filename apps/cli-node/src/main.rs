//! `mpc-wallet-cli` — headless, scriptable front-end for the MPC wallet.
//!
//! Drives the same Elm core as the TUI/native via
//! `tui_node::elm::HeadlessRunner`, exposing a newline-delimited JSON
//! protocol on stdin/stdout (see [`protocol`]). Built for LLM/agent
//! control and automated end-to-end testing.
//!
//! IMPORTANT: stdout carries ONLY protocol JSON. All logs go to stderr.

use clap::{Parser, Subcommand};
use mpc_wallet_cli::oneshot::{self, OneShotOpts};
use mpc_wallet_cli::policy::{self, AutoApprovePolicy};
use mpc_wallet_cli::protocol;
use mpc_wallet_cli::serve::{self, ServeOpts};
use mpc_wallet_cli::simulate::{self, SimulateOpts};
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "mpc-wallet-cli", version, about = "Headless MPC wallet CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the JSONL daemon: read commands on stdin, emit events on stdout.
    Serve(ServeArgs),
    /// Run a full N-node DKG in one process (embedded signal server) and
    /// print a JSON summary. Self-contained — ideal for CI / smoke tests.
    Simulate(SimulateArgs),
    /// Wallet one-shot commands (list / create).
    Wallet {
        #[command(subcommand)]
        sub: WalletCmd,
    },
    /// Session one-shot commands (join).
    Session {
        #[command(subcommand)]
        sub: SessionCmd,
    },
    /// Initiate a threshold signing and block until it completes.
    Sign {
        #[arg(long)]
        wallet_id: String,
        #[arg(long)]
        message: String,
        #[arg(long, default_value = "utf8")]
        encoding: String,
        #[command(flatten)]
        pw: PasswordArgs,
        #[command(flatten)]
        common: OneShot,
    },
    /// Initiate a networked share refresh/resharing of an existing wallet and
    /// block until it completes. Announces a reshare session the retained
    /// signers join (via `session join` / `serve`); the group address is
    /// preserved and the refreshed share replaces the old one on disk (#56).
    Reshare {
        #[arg(long)]
        wallet_id: String,
        #[command(flatten)]
        pw: PasswordArgs,
        #[command(flatten)]
        common: OneShot,
    },
    /// Simulate a share refresh/resharing in one process and print a JSON
    /// summary (group key preserved, refreshed quorum signs, old share
    /// rejected). Self-contained — exercises the resharing engine (#45).
    ReshareSimulate(ReshareSimArgs),
    /// Print the command/event protocol catalog as JSON (self-discovery).
    Schema,
}

#[derive(clap::Args)]
struct ReshareSimArgs {
    /// Number of participants in the initial wallet.
    #[arg(long, default_value_t = 3)]
    nodes: usize,
    /// Signing threshold (K of N). Preserved by the refresh (can't be lowered).
    #[arg(long, default_value_t = 2)]
    threshold: u16,
    /// Ciphersuite: secp256k1 (default) or ed25519.
    #[arg(long, default_value = "secp256k1")]
    curve: String,
    /// Comma-separated participant ids to KEEP after the refresh (1-based;
    /// default: all). Omit an id to remove that device.
    #[arg(long, value_delimiter = ',')]
    keep: Vec<u16>,
}

/// Shared flags for one-shot commands.
#[derive(clap::Args)]
struct OneShot {
    #[arg(long, default_value = "cli-node")]
    device_id: String,
    #[arg(long, default_value = "~/.frost_keystore")]
    keystore: String,
    #[arg(long, default_value = "wss://panda.qzz.io")]
    signal_server: String,
    /// Tenant room (REQUIRED by the server): a strong, shared id all
    /// participants of a ceremony use. Merged into the signal URL as
    /// `?room=<id>`. Generate one with `uuidgen`. Without it the server
    /// rejects the connection.
    #[arg(long)]
    room: Option<String>,
    #[arg(long, default_value_t = 90)]
    timeout: u64,
    /// Ciphersuite: secp256k1 (default; Ethereum/Bitcoin) or ed25519 (Solana).
    /// ed25519 yields a standard RFC-8032 signature that ANY off-the-shelf
    /// verifier (and Solana) can check — ideal for an independently-checkable
    /// demo. All participants of one ceremony must use the same curve.
    #[arg(long, default_value = "secp256k1")]
    curve: String,
    #[arg(long, default_value = "")]
    log_level: String,
}

/// Password input (file/env preferred over the argv-visible flag).
#[derive(clap::Args)]
struct PasswordArgs {
    #[arg(long)]
    password: Option<String>,
    #[arg(long)]
    password_env: Option<String>,
    #[arg(long)]
    password_file: Option<String>,
}

impl PasswordArgs {
    fn resolve(&self) -> anyhow::Result<String> {
        policy::resolve_password(
            self.password.as_deref(),
            self.password_env.as_deref(),
            self.password_file.as_deref(),
        )
    }
}

#[derive(Subcommand)]
enum WalletCmd {
    /// List wallets in the keystore (no network).
    List {
        #[command(flatten)]
        common: OneShot,
    },
    /// Create a shared wallet via DKG; blocks until complete.
    Create {
        #[arg(long, default_value = "Wallet")]
        name: String,
        #[arg(long, default_value_t = 2)]
        threshold: u16,
        #[arg(long, default_value_t = 3)]
        total: u16,
        #[command(flatten)]
        pw: PasswordArgs,
        #[command(flatten)]
        common: OneShot,
    },
}

#[derive(Subcommand)]
enum SessionCmd {
    /// Join a discovered DKG/signing session; blocks until complete.
    Join {
        #[arg(long)]
        session_id: String,
        #[command(flatten)]
        pw: PasswordArgs,
        #[command(flatten)]
        common: OneShot,
    },
}

impl OneShot {
    /// Fail fast on a too-weak `--room` for the hosted server, instead of
    /// dialing and waiting 15s for the server to reject it (the common footgun:
    /// `--room test-1`). The hosted multi-tenant server requires a strong room
    /// (≥16 chars of `[A-Za-z0-9_-]`); a local `ws://` server needs none, so we
    /// only enforce this for `wss://`.
    fn validate_room(&self) -> anyhow::Result<()> {
        if let Some(r) = self.room.as_deref() {
            let hosted = self.signal_server.starts_with("wss://");
            if hosted && !is_strong_room(r) {
                anyhow::bail!(
                    "--room \"{r}\" is too weak for the hosted server ({}). It needs ≥16 chars of \
                     [A-Za-z0-9_-] (got {} char(s)). Generate a strong one and share the SAME value \
                     with every device:\n      --room \"$(uuidgen | tr -d -)\"\n    (a local \
                     --signal-server ws://<host-ip>:9000 needs no room.)",
                    self.signal_server,
                    r.chars().count()
                );
            }
        }
        Ok(())
    }

    fn init_and_opts(&self) -> OneShotOpts {
        if !self.log_level.is_empty() {
            let _ = tracing_subscriber::fmt()
                .with_writer(std::io::stderr)
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_new(&self.log_level)
                        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
                )
                .with_ansi(false)
                .try_init();
        }
        OneShotOpts {
            device_id: self.device_id.clone(),
            keystore_path: expand_tilde(&self.keystore),
            signal_url: with_room(&self.signal_server, self.room.as_deref()),
            timeout_secs: self.timeout,
            curve: self.curve.clone(),
        }
    }
}

/// Exit 0 if the one-shot reported success, else 1.
fn finish(ok: anyhow::Result<bool>) -> anyhow::Result<()> {
    match ok {
        Ok(true) => Ok(()),
        Ok(false) => std::process::exit(1),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

#[derive(clap::Args)]
struct SimulateArgs {
    /// Number of participants (devices).
    #[arg(long, default_value_t = 2)]
    nodes: usize,
    /// Signers required (K of N). Defaults to N (all).
    #[arg(long)]
    threshold: Option<u16>,
    /// Ciphersuite (currently secp256k1).
    #[arg(long, default_value = "secp256k1")]
    curve: String,
    /// Overall timeout in seconds.
    #[arg(long, default_value_t = 90)]
    timeout: u64,
    /// If set, after DKG sign this message with a quorum and verify it.
    #[arg(long)]
    sign: Option<String>,
    /// External signal server URL (e.g. wss://panda.qzz.io). When omitted, an
    /// isolated server is embedded in-process. Use this to smoke-test a
    /// deployed/remote server.
    #[arg(long)]
    signal_server: Option<String>,
    /// Tenant room merged into `--signal-server` as `?room=<id>` (the deployed
    /// server requires a strong room). Ignored when no `--signal-server`.
    #[arg(long)]
    room: Option<String>,
    /// tracing filter (stderr); empty to silence.
    #[arg(long, default_value = "")]
    log_level: String,
}

#[derive(clap::Args)]
struct ServeArgs {
    /// Stable identity for this node (used in the DKG participant set).
    #[arg(long, default_value = "cli-node")]
    device_id: String,
    /// Keystore directory. Use an isolated dir per node when testing.
    #[arg(long, default_value = "~/.frost_keystore")]
    keystore: String,
    /// Signal server URL.
    #[arg(long, default_value = "wss://panda.qzz.io")]
    signal_server: String,
    /// Tenant room (REQUIRED by the server) merged as `?room=<id>`. All
    /// participants of a ceremony must use the same strong id (e.g. a UUID).
    #[arg(long)]
    room: Option<String>,
    /// Ciphersuite: secp256k1 (default) or ed25519 (Solana; RFC-8032
    /// signatures any standard verifier accepts). Same curve on all nodes.
    #[arg(long, default_value = "secp256k1")]
    curve: String,
    /// tracing filter (stderr).
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Auto-approve incoming signing requests (OFF by default).
    #[arg(long)]
    auto_approve: bool,
    /// Restrict auto-approval to these wallet ids (repeatable; empty = any).
    #[arg(long = "approve-wallet")]
    approve_wallet: Vec<String>,
    /// Cap the number of auto-approvals for this process.
    #[arg(long)]
    approve_max: Option<usize>,
    /// Password to unlock the wallet when auto-approving (discouraged on argv).
    #[arg(long)]
    approve_password: Option<String>,
    /// Env var holding the auto-approve password.
    #[arg(long)]
    approve_password_env: Option<String>,
    /// File holding the auto-approve password.
    #[arg(long)]
    approve_password_file: Option<String>,
}

#[tokio::main]
async fn main() {
    // Single clean exit point: print the error chain as one line and exit 1 —
    // no Rust "Error: …" + stack backtrace in a user's face (and consistent with
    // the `finish()` one-shot paths). `{:#}` renders anyhow's full cause chain
    // on one line without a backtrace, even when RUST_BACKTRACE is set.
    if let Err(e) = run().await {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

/// Parse argv, but on an *unrecognized subcommand* add a one-line tip pointing
/// at the right command for the intuitive misses (e.g. `wallet join` — joining
/// lives under `session join`; `session create` — creating is `wallet create`).
/// Otherwise behaves exactly like `Cli::parse()` (clap prints + exits).
fn parse_cli() -> Cli {
    use clap::error::{ContextKind, ContextValue, ErrorKind};
    match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            if e.kind() == ErrorKind::InvalidSubcommand {
                if let Some(ContextValue::String(bad)) = e.get(ContextKind::InvalidSubcommand) {
                    let tip = match bad.as_str() {
                        "join" => Some(
                            "to join a wallet another device created, run:\n  \
                             mpc-wallet-cli session join --session-id <id> --room <room> \
                             --device-id <unique> --password <pw>\n\
                             (the <id> is the dkg_… that `wallet create` prints.)",
                        ),
                        "create" => Some(
                            "to create a shared wallet, run:\n  \
                             mpc-wallet-cli wallet create --room <room> --device-id <unique> \
                             --password <pw>",
                        ),
                        _ => None,
                    };
                    if let Some(tip) = tip {
                        let _ = e.print();
                        eprintln!("\ntip: {tip}");
                        std::process::exit(e.exit_code());
                    }
                }
            }
            e.exit();
        }
    }
}

async fn run() -> anyhow::Result<()> {
    let cli = parse_cli();
    match cli.command {
        Command::Schema => {
            println!("{}", protocol::schema_json());
            Ok(())
        }
        Command::ReshareSimulate(args) => {
            let r = mpc_wallet_cli::reshare::run_reshare_simulation(
                args.nodes,
                args.threshold,
                &args.curve,
                args.keep,
            )?;
            println!("{}", r.to_json());
            if r.ok { Ok(()) } else { std::process::exit(1) }
        }
        Command::Wallet { sub } => match sub {
            WalletCmd::List { common } => {
                finish(oneshot::wallet_list(common.init_and_opts()).await)
            }
            WalletCmd::Create {
                name,
                threshold,
                total,
                pw,
                common,
            } => {
                common.validate_room()?;
                let password = pw.resolve()?;
                finish(
                    oneshot::wallet_create(
                        common.init_and_opts(),
                        name,
                        threshold,
                        total,
                        password,
                    )
                    .await,
                )
            }
        },
        Command::Session { sub } => match sub {
            SessionCmd::Join {
                session_id,
                pw,
                common,
            } => {
                common.validate_room()?;
                let password = pw.resolve()?;
                finish(oneshot::session_join(common.init_and_opts(), session_id, password).await)
            }
        },
        Command::Sign {
            wallet_id,
            message,
            encoding,
            pw,
            common,
        } => {
            common.validate_room()?;
            let password = pw.resolve()?;
            finish(oneshot::sign(common.init_and_opts(), wallet_id, message, encoding, password).await)
        }
        Command::Reshare {
            wallet_id,
            pw,
            common,
        } => {
            common.validate_room()?;
            let password = pw.resolve()?;
            finish(oneshot::reshare(common.init_and_opts(), wallet_id, password).await)
        }
        Command::Simulate(args) => {
            if !args.log_level.is_empty() {
                let _ = tracing_subscriber::fmt()
                    .with_writer(std::io::stderr)
                    .with_env_filter(
                        tracing_subscriber::EnvFilter::try_new(&args.log_level)
                            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
                    )
                    .with_ansi(false)
                    .try_init();
            }
            let threshold = args.threshold.unwrap_or(args.nodes as u16);
            let opts = SimulateOpts {
                nodes: args.nodes,
                threshold,
                curve: args.curve,
                signal_url: args
                    .signal_server
                    .as_deref()
                    .map(|u| with_room(u, args.room.as_deref())),
                timeout_secs: args.timeout,
            };
            let ok = if let Some(msg) = args.sign {
                let r = simulate::run_signing_simulation(opts, &msg).await?;
                println!("{}", r.to_json());
                r.verified
            } else {
                let r = simulate::run_simulation(opts).await?;
                println!("{}", r.to_json());
                r.agreed
            };
            if ok {
                Ok(())
            } else {
                std::process::exit(1);
            }
        }
        Command::Serve(args) => {
            // Logs MUST go to stderr so stdout stays pure JSONL.
            tracing_subscriber::fmt()
                .with_writer(std::io::stderr)
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_new(&args.log_level)
                        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
                )
                .with_ansi(false)
                .init();

            let keystore_path = expand_tilde(&args.keystore);
            let approve_password = if args.auto_approve {
                policy::resolve_password(
                    args.approve_password.as_deref(),
                    args.approve_password_env.as_deref(),
                    args.approve_password_file.as_deref(),
                )?
            } else {
                String::new()
            };
            let auto_approve = Arc::new(AutoApprovePolicy::new(
                args.auto_approve,
                args.approve_wallet,
                args.approve_max,
            ));
            serve::serve(ServeOpts {
                device_id: args.device_id,
                keystore_path,
                signal_url: with_room(&args.signal_server, args.room.as_deref()),
                curve: args.curve,
                auto_approve,
                approve_password,
            })
            .await
        }
    }
}

/// A "strong" room the hosted multi-tenant server will accept: ≥16 chars of
/// `[A-Za-z0-9_-]` (mirrors the server's `MIN_ROOM_LEN` / `isValidRoom`).
fn is_strong_room(r: &str) -> bool {
    r.chars().count() >= 16 && r.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Merge a tenant `room` into a signal-server URL as a `room` query param.
/// No-op when `room` is None/empty (the server then rejects the connection,
/// which surfaces a clear "a strong ?room is required" error). Appends with
/// `?` or `&` depending on whether the URL already has a query.
fn with_room(url: &str, room: Option<&str>) -> String {
    match room {
        Some(r) if !r.is_empty() && !url.contains("room=") => {
            if url.contains('?') {
                format!("{url}&room={r}")
            } else if url.splitn(2, "://").nth(1).unwrap_or(url).contains('/') {
                // already has a path → just add the query
                format!("{url}?room={r}")
            } else {
                // no path (e.g. wss://host) → a WS handshake needs one
                format!("{url}/?room={r}")
            }
        }
        _ => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{is_strong_room, with_room};

    #[test]
    fn strong_room_requires_16_safe_chars() {
        assert!(!is_strong_room("test-1")); // the reported footgun (6 chars)
        assert!(!is_strong_room("short"));
        assert!(!is_strong_room("0123456789abcde")); // 15 chars
        assert!(is_strong_room("0123456789abcdef")); // 16 chars
        assert!(is_strong_room("7f3a9c2e4b1d4e8a9c2f001122334455")); // uuid, dashes stripped
        assert!(!is_strong_room("has space chars!!")); // invalid chars
    }

    #[test]
    fn with_room_inserts_path_and_query() {
        assert_eq!(with_room("wss://h", Some("r")), "wss://h/?room=r");
        assert_eq!(with_room("wss://h/", Some("r")), "wss://h/?room=r");
        assert_eq!(with_room("wss://h/p", Some("r")), "wss://h/p?room=r");
        assert_eq!(with_room("wss://h/?x=1", Some("r")), "wss://h/?x=1&room=r");
        assert_eq!(with_room("wss://h/?room=keep", Some("r")), "wss://h/?room=keep");
        assert_eq!(with_room("wss://h", None), "wss://h");
        assert_eq!(with_room("wss://h", Some("")), "wss://h");
    }
}

/// Expand a leading `~/` to the user's home dir.
fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    path.to_string()
}
