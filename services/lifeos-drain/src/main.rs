use std::time::{Duration, SystemTime, UNIX_EPOCH};
use libsql::Builder;
use tokio::time::sleep;

fn get_current_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

#[tokio::main]
async fn main() {
    println!("lifeos-drain starting... Atomic job queue consumer running.");
    
    let db_path = "lifeos.db";
    let db = match Builder::new_local(db_path).build().await {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open DB: {}", e);
            return;
        }
    };
    let conn = db.connect().unwrap();
    
    let worker_id = format!("mac-drain-{}", get_current_epoch());
    println!("Worker identified as: {}", worker_id);

    loop {
        // Atomic claim
        let claim_sql = r#"
            UPDATE jobs SET status='running', claimed_by=?1, claimed_at=?2
            WHERE id = (SELECT id FROM jobs
                        WHERE status='queued' AND (run_after IS NULL OR run_after<=?3)
                        ORDER BY priority DESC, created_at ASC LIMIT 1)
            RETURNING id, kind, payload;
        "#;
        
        let now = get_current_epoch();
        let mut stmt = match conn.prepare(claim_sql).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Prepare failed: {}", e);
                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        match stmt.query(libsql::params![worker_id.clone(), now as i64, now as i64]).await {
            Ok(mut rows) => {
                if let Ok(Some(row)) = rows.next().await {
                    let job_id: String = row.get(0).unwrap();
                    let kind: String = row.get(1).unwrap();
                    let payload: String = row.get(2).unwrap();
                    
                    println!("Claimed job {}: kind={}, payload={}", job_id, kind, payload);
                    
                    // Simulate job execution
                    sleep(Duration::from_secs(2)).await;
                    
                    // Mark as done
                    let _ = conn.execute("UPDATE jobs SET status='done' WHERE id=?1", libsql::params![job_id]).await;
                    println!("Finished job {}", job_id);
                } else {
                    // No jobs available
                }
            },
            Err(e) => {
                eprintln!("Query failed: {}", e);
            }
        }

        // Reaper for stuck jobs (5 minutes timeout)
        let reaper_sql = "UPDATE jobs SET status='queued', claimed_by=NULL, claimed_at=NULL WHERE status='running' AND claimed_at < ?1";
        let timeout_thresh = now - 300;
        let _ = conn.execute(reaper_sql, libsql::params![timeout_thresh as i64]).await;

        sleep(Duration::from_secs(3)).await;
    }
}
