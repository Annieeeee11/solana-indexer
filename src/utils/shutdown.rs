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

async fn wait_for_any_task(tasks: &mut [(&mut JoinHandle<()>, &str)]) {
    loop {
        for (handle, label) in tasks.iter_mut() {
            if handle.is_finished() {
                let dummy = tokio::spawn(async {});
                let finished = std::mem::replace(*handle, dummy);
                log_join_error(label, finished.await);
                return;
            }
        }
        tokio::task::yield_now().await;
    }
}

/// Wait for Ctrl+C (broadcasting shutdown) or any labeled task to finish.
pub async fn wait_ctrl_c_or_any(
    shutdown_tx: broadcast::Sender<()>,
    message: &str,
    tasks: &mut [(&mut JoinHandle<()>, &str)],
) {
    if tasks.is_empty() {
        wait_ctrl_c(shutdown_tx, message).await;
        return;
    }

    tokio::select! {
        () = wait_ctrl_c(shutdown_tx, message) => {}
        () = wait_for_any_task(tasks) => {}
    }
}

/// Abort tasks and await their completion.
pub async fn abort_join_handles(handles: impl IntoIterator<Item = JoinHandle<()>>) {
    for handle in handles {
        handle.abort();
        let _ = handle.await;
    }
}
