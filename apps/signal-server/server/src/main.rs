use tokio::net::TcpListener;
use tokio::signal;

// The accept loop + connection handling now lives in the library
// (`starlab_signal_server::run`) so it can be embedded in-process (CLI
// `simulate` + end-to-end tests) as well as run as this standalone binary.

#[tokio::main]
async fn main() {
    // Bind address is configurable via MPC_SIGNAL_BIND (default 0.0.0.0:9000)
    // so tests / multiple instances can use an ephemeral port.
    let bind = std::env::var("MPC_SIGNAL_BIND").unwrap_or_else(|_| "0.0.0.0:9000".to_string());
    let listener = TcpListener::bind(&bind).await.unwrap();
    eprintln!("Signal server listening on {bind}");

    let shutdown_signal = async {
        signal::ctrl_c()
            .await
            .expect("Failed to listen for shutdown signal");
        eprintln!("Shutdown signal received. Terminating...");
    };

    tokio::select! {
        _ = starlab_signal_server::run(listener) => {},
        _ = shutdown_signal => {},
    }

    eprintln!("Server has shut down.");
}
