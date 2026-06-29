use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    println!("lifeos-pipelines starting... Agent DAG and Actions engine active.");
    println!("Listening for pipeline triggers...");

    loop {
        sleep(Duration::from_secs(10)).await;
    }
}
