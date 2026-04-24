use iec104sim_core::master::{MasterConfig, MasterConnection, MasterState};
use std::io::Read;
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use tokio::time::{sleep, Duration};

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

/// Spawn a throwaway TCP server on 127.0.0.1:port that accepts one connection,
/// drains STARTDT ACT, then closes the socket when `close_rx` fires.
fn start_throwaway_slave(port: u16, close_rx: mpsc::Receiver<()>) {
    thread::spawn(move || {
        let listener = TcpListener::bind(("127.0.0.1", port)).unwrap();
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .ok();
        let mut buf = [0u8; 64];
        // Drain whatever the master sends (STARTDT ACT) until told to close.
        loop {
            if close_rx.try_recv().is_ok() {
                break;
            }
            let _ = stream.read(&mut buf);
        }
        // Dropping `stream` here closes the TCP connection → master sees EOF.
    });
}

/// When the peer closes the socket, the master's background receiver must observe EOF,
/// update state to Disconnected, and broadcast a state-change event.
#[tokio::test]
async fn master_detects_peer_close_and_broadcasts_state() {
    let port = free_port();
    let (close_tx, close_rx) = mpsc::channel();
    start_throwaway_slave(port, close_rx);
    // Give the listener time to bind.
    sleep(Duration::from_millis(100)).await;

    let mut master = MasterConnection::new(MasterConfig {
        target_address: "127.0.0.1".to_string(),
        port,
        common_address: 1,
        ..Default::default()
    });

    let mut state_rx = master.subscribe_state();
    state_rx.mark_unchanged();
    master.connect().await.unwrap();
    assert_eq!(master.state(), MasterState::Connected);

    // Drain Connecting → Connected transitions so we observe only what happens after peer close.
    sleep(Duration::from_millis(100)).await;
    state_rx.mark_unchanged();

    close_tx.send(()).unwrap();

    let mut saw_disconnected = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(200), state_rx.changed()).await {
            Ok(Ok(())) => {
                if *state_rx.borrow_and_update() == MasterState::Disconnected {
                    saw_disconnected = true;
                    break;
                }
            }
            Ok(Err(_)) => break,
            Err(_) => continue,
        }
    }

    assert!(
        saw_disconnected,
        "expected a Disconnected notification after peer close"
    );
    assert_eq!(
        master.state(),
        MasterState::Disconnected,
        "core state must reflect Disconnected"
    );

    // Reconnect must not be blocked by a stale AlreadyConnected guard.
    let (close_tx2, close_rx2) = mpsc::channel();
    start_throwaway_slave(port, close_rx2);
    sleep(Duration::from_millis(150)).await;
    master
        .connect()
        .await
        .expect("reconnect after peer close should succeed");
    assert_eq!(master.state(), MasterState::Connected);

    close_tx2.send(()).ok();
}
