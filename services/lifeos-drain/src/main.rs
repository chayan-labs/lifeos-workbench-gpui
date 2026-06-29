use std::time::{SystemTime, UNIX_EPOCH};

fn get_current_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

#[tokio::main]
async fn main() {
    println!("lifeos-drain starting... Atomic job queue consumer running.");
    
    // In a real run, this would connect to the Turso client:
    // let db = libsql::Builder::new_local("lifeos.db").build().await.unwrap();
    // let conn = db.connect().unwrap();
    
    let worker_id = format!("mac-drain-{}", get_current_epoch());
    println!("Worker identified as: {}", worker_id);
}
