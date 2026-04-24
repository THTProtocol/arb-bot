use tokio::signal;
use tracing::info;

pub async fn wait() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("ctrl+c handler");
        info!("Received SIGINT");
    };

    let terminate = async {
        #[cfg(unix)]
        {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("SIGTERM handler")
                .recv()
                .await;
            info!("Received SIGTERM");
        }
        #[cfg(not(unix))]
        {
            std::future::pending::<()>().await;
        }
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
