use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use kovaak_pulse::model::WatcherEvent;
use kovaak_pulse::storage::db::Database;
use kovaak_pulse::watcher::{spawn_watcher, WatcherConfig};

#[test]
fn test_watcher_end_to_end_latency_under_one_second() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build Tokio runtime");

    rt.block_on(async {
        let root = std::env::temp_dir().join(format!(
            "kovaak_pulse_watcher_test_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let stats_dir = root.join("stats");
        let perf_dir = root.join("performances");
        let db_path = root.join("pulse.db");

        fs::create_dir_all(&stats_dir).expect("create stats dir");
        fs::create_dir_all(&perf_dir).expect("create perf dir");

        let cfg = WatcherConfig::new(stats_dir.clone(), perf_dir.clone(), db_path.clone());
        let (mut rx, handle) = spawn_watcher(cfg).expect("spawn_watcher failed");

        // Wait for WatcherReady
        let ready = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timed out waiting for WatcherReady")
            .expect("watcher channel closed");
        assert!(
            matches!(ready, WatcherEvent::WatcherReady { .. }),
            "expected WatcherReady, got {:?}",
            ready
        );

        // Give OS watcher a tiny moment to settle before writing
        tokio::time::sleep(Duration::from_millis(50)).await;

        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures");
        let src_csv = fixtures.join("Micro Flick - Challenge - 2026.11.20-21.58.16 Stats.csv");
        let src_perf =
            fixtures.join("Micro Flick - Challenge - 2026.11.20-21.58.16 Performance.perf");

        let dst_csv = stats_dir.join("Micro Flick - Challenge - 2026.11.20-21.58.16 Stats.csv");
        let dst_perf =
            perf_dir.join("Micro Flick - Challenge - 2026.11.20-21.58.16 Performance.perf");

        // Simulate KovaaK's writing both `.csv` and `.perf`
        let start = Instant::now();
        fs::copy(&src_csv, &dst_csv).expect("copy csv");
        fs::copy(&src_perf, &dst_perf).expect("copy perf");

        let event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
            .await
            .expect("timed out waiting for RunIngested")
            .expect("watcher channel closed before RunIngested");
        let wall_elapsed = start.elapsed();

        match event {
            WatcherEvent::RunIngested {
                run_id,
                scenario,
                score,
                accuracy,
                is_new_pb,
                elapsed_ms,
                ..
            } => {
                println!(
                    "Watcher E2E latency: wall={}ms (internal={}ms), scenario={}, score={:.2}, acc={:.2}%",
                    wall_elapsed.as_millis(),
                    elapsed_ms,
                    scenario,
                    score,
                    accuracy
                );
                assert!(
                    wall_elapsed < Duration::from_secs(1),
                    "End-to-end watcher latency {}ms exceeded 1000ms target",
                    wall_elapsed.as_millis()
                );
                assert_eq!(scenario, "Micro Flick");
                assert!((score - 7054.838867).abs() < 1e-3);
                assert!(is_new_pb);

                // Confirm full run + perf ticks + kills persisted in SQLite
                let db = Database::open(&db_path).expect("open db");
                let detail = db
                    .get_run_detail(run_id)
                    .expect("get_run_detail")
                    .expect("run detail missing");
                assert!(detail.run.has_perf);
                assert_eq!(detail.ticks.len(), 24);
                assert_eq!(detail.kills.len(), 81);
            }
            other => panic!("Expected WatcherEvent::RunIngested, got {:?}", other),
        }

        drop(handle);
        let _ = fs::remove_dir_all(&root);
    });
}
