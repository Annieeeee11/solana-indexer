use tokio::sync::broadcast;
use tokio::task::JoinHandle;

/// Creates a shutdown broadcast sender (receivers subscribe as needed).
pub fn channel() -> broadcast::Sender<()> {
    let (tx, _) = broadcast::channel(1);
    tx
}

/// Spawns a task that broadcasts shutdown when Ctrl+C is received.
pub fn spawn_on_ctrl_c(shutdown_tx: broadcast::Sender<()>, message: impl Into<String>) {
    let message = message.into();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("{message}");
        let _ = shutdown_tx.send(());
    });
}

pub fn log_join_error(label: &str, result: std::result::Result<(), tokio::task::JoinError>) {
    if let Err(e) = result {
        tracing::error!("{label} task failed: {e}");
    }
}

async fn wait_ctrl_c(shutdown_tx: broadcast::Sender<()>, message: &str) {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("{message}");
    let _ = shutdown_tx.send(());
}

/// Wait for Ctrl+C (broadcasting shutdown) or any of two tasks to finish.
pub async fn wait_ctrl_c_or_2(
    shutdown_tx: broadcast::Sender<()>,
    message: &str,
    h0: &mut JoinHandle<()>,
    l0: &str,
    h1: &mut JoinHandle<()>,
    l1: &str,
) {
    tokio::select! {
        () = wait_ctrl_c(shutdown_tx, message) => {}
        result = h0 => log_join_error(l0, result),
        result = h1 => log_join_error(l1, result),
    }
}

/// Wait for Ctrl+C (broadcasting shutdown) or any of three tasks to finish.
pub async fn wait_ctrl_c_or_3(
    shutdown_tx: broadcast::Sender<()>,
    message: &str,
    h0: &mut JoinHandle<()>,
    l0: &str,
    h1: &mut JoinHandle<()>,
    l1: &str,
    h2: &mut JoinHandle<()>,
    l2: &str,
) {
    tokio::select! {
        () = wait_ctrl_c(shutdown_tx, message) => {}
        result = h0 => log_join_error(l0, result),
        result = h1 => log_join_error(l1, result),
        result = h2 => log_join_error(l2, result),
    }
}

/// Wait for Ctrl+C (broadcasting shutdown) or any of four tasks to finish.
pub async fn wait_ctrl_c_or_4(
    shutdown_tx: broadcast::Sender<()>,
    message: &str,
    h0: &mut JoinHandle<()>,
    l0: &str,
    h1: &mut JoinHandle<()>,
    l1: &str,
    h2: &mut JoinHandle<()>,
    l2: &str,
    h3: &mut JoinHandle<()>,
    l3: &str,
) {
    tokio::select! {
        () = wait_ctrl_c(shutdown_tx, message) => {}
        result = h0 => log_join_error(l0, result),
        result = h1 => log_join_error(l1, result),
        result = h2 => log_join_error(l2, result),
        result = h3 => log_join_error(l3, result),
    }
}

/// Abort tasks and await their completion.
pub async fn abort_join_handles(handles: impl IntoIterator<Item = JoinHandle<()>>) {
    for handle in handles {
        handle.abort();
        let _ = handle.await;
    }
}
