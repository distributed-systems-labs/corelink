use tokio::time::{sleep, Duration};
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    info!("CoreLink Network Simulator");
    info!("Spawning 5 virtual nodes...");

    let mut handles = vec![];

    for i in 0..5 {
        let handle = tokio::spawn(async move {
            simulate_node(i).await;
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }
}

async fn simulate_node(id: usize) {
    let node_id = format!("node-{}", id);
    info!("[{}] Node starting...", node_id);

    loop {
        sleep(Duration::from_secs(5)).await;
        info!("[{}] Heartbeat", node_id);
    }
}
