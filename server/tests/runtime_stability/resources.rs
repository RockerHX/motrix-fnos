use serde_json::{json, Value};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Debug, serde::Serialize)]
struct Resources {
    rss_bytes: u64,
    cpu_ns: u64,
    descriptors: usize,
    threads: usize,
}

#[cfg(target_os = "macos")]
#[allow(deprecated)] // Reuse libc's Darwin timebase binding for this test.
fn sample(pid: u32) -> Resources {
    let mut task: libc::proc_taskinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of_val(&task) as i32;
    assert_eq!(
        unsafe {
            libc::proc_pidinfo(
                pid as i32,
                libc::PROC_PIDTASKINFO,
                0,
                (&mut task as *mut libc::proc_taskinfo).cast(),
                size,
            )
        },
        size
    );
    let bytes = unsafe {
        libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDLISTFDS,
            0,
            std::ptr::null_mut(),
            0,
        )
    };
    assert!(bytes > 0);
    let mut fds = vec![0u8; bytes as usize + 1024];
    let used = unsafe {
        libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDLISTFDS,
            0,
            fds.as_mut_ptr().cast(),
            fds.len() as i32,
        )
    };
    assert!(used > 0 && used < fds.len() as i32);
    let mut timebase: libc::mach_timebase_info = unsafe { std::mem::zeroed() };
    assert_eq!(unsafe { libc::mach_timebase_info(&mut timebase) }, 0);
    Resources {
        rss_bytes: task.pti_resident_size,
        // proc_taskinfo uses Mach absolute ticks, including on Apple Silicon.
        cpu_ns: ((task.pti_total_user + task.pti_total_system) as u128 * timebase.numer as u128
            / timebase.denom as u128) as u64,
        descriptors: used as usize / std::mem::size_of::<libc::proc_fdinfo>(),
        threads: task.pti_threadnum as usize,
    }
}

#[cfg(target_os = "linux")]
fn sample(pid: u32) -> Resources {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap();
    let number = |name: &str| -> u64 {
        status
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    let fields: Vec<_> = stat
        .rsplit_once(')')
        .unwrap()
        .1
        .split_whitespace()
        .collect();
    let ticks = fields[11].parse::<u64>().unwrap() + fields[12].parse::<u64>().unwrap();
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    assert!(hz > 0);
    Resources {
        rss_bytes: number("VmRSS:") * 1024,
        cpu_ns: ticks * 1_000_000_000 / hz as u64,
        descriptors: std::fs::read_dir(format!("/proc/{pid}/fd"))
            .unwrap()
            .count(),
        threads: number("Threads:") as usize,
    }
}

async fn websocket(addr: SocketAddr) -> TcpStream {
    let mut socket = TcpStream::connect(addr).await.unwrap();
    socket.write_all(b"GET /jsonrpc HTTP/1.1\r\nHost: localhost\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n").await.unwrap();
    let mut response = Vec::new();
    while !response.ends_with(b"\r\n\r\n") {
        response.push(socket.read_u8().await.unwrap());
        assert!(response.len() < 4096);
    }
    assert!(response.starts_with(b"HTTP/1.1 101"));
    socket
}

pub async fn check_connection_churn(pid: u32, base: &str, rpc: SocketAddr, token: &str) {
    // These are local debug-build regression limits, not NAS performance targets.
    const RSS_GROWTH: u64 = 16 * 1024 * 1024;
    let mut baseline = None;
    let mut samples = Vec::new();
    let mut latencies = Vec::new();
    for wave in 0..15 {
        let mut workers = tokio::task::JoinSet::new();
        for _ in 0..16 {
            let (base, token) = (base.to_string(), token.to_string());
            workers.spawn(async move {
                let client = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(2))
                    .pool_max_idle_per_host(0).build().unwrap();
                let mut times = Vec::new();
                for path in ["/api/tasks", "/api/aria2/process", "/api/aria2/rpc"] {
                    let started = Instant::now();
                    let response = client.get(format!("{base}{path}")).bearer_auth(&token).send().await.unwrap();
                    assert!(response.status().is_success());
                    response.json::<Value>().await.unwrap();
                    times.push(started.elapsed().as_micros() as u64);
                }
                let started = Instant::now();
                let response: Value = client.post(format!("http://{rpc}/jsonrpc"))
                    .json(&json!({"jsonrpc":"2.0","id":1,"method":"aria2.getVersion","params":["token:stability-rpc"]}))
                    .send().await.unwrap().json().await.unwrap();
                assert!(response.get("result").is_some());
                times.push(started.elapsed().as_micros() as u64);
                let mut sse = client.get(format!("{base}/api/events")).bearer_auth(&token).send().await.unwrap();
                assert!(sse.status().is_success());
                let mut initial = String::new();
                while !initial.contains("event: tasks.snapshot") {
                    initial.push_str(&String::from_utf8_lossy(&sse.chunk().await.unwrap().unwrap()));
                }
                let mut socket = tokio::time::timeout(Duration::from_secs(2), websocket(rpc)).await.unwrap();
                // A masked empty Close frame exercises the server's normal close path.
                socket.write_all(&[0x88, 0x80, 0, 0, 0, 0]).await.unwrap();
                let _ = tokio::time::timeout(Duration::from_secs(2), socket.read_u8()).await.unwrap();
                drop((socket, sse, client));
                times
            });
        }
        while let Some(result) = workers.join_next().await {
            let times = result.unwrap();
            if wave >= 3 {
                latencies.extend(times);
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        let current = sample(pid);
        if wave == 2 {
            baseline = Some(sample(pid));
        }
        if let Some(baseline) = &baseline {
            assert!(
                current.rss_bytes <= baseline.rss_bytes + RSS_GROWTH,
                "RSS growth: {baseline:?} -> {current:?}"
            );
            assert!(
                current.descriptors <= baseline.descriptors + 8,
                "FD growth: {baseline:?} -> {current:?}"
            );
            assert!(
                current.threads <= baseline.threads + 4,
                "thread growth: {baseline:?} -> {current:?}"
            );
            samples.push(current);
        }
    }
    latencies.sort_unstable();
    let p50 = latencies[latencies.len() / 2];
    let p95 = latencies[(latencies.len() - 1) * 95 / 100];
    assert!(
        p95 < 1_000_000,
        "local request p95 exceeded one second: {p95}us"
    );
    let before_idle = sample(pid);
    let idle_started = Instant::now();
    tokio::time::sleep(Duration::from_secs(1)).await;
    let after_idle = sample(pid);
    let idle_cpu_percent = (after_idle.cpu_ns - before_idle.cpu_ns) as f64
        / idle_started.elapsed().as_nanos() as f64
        * 100.0;
    assert!(
        idle_cpu_percent < 20.0,
        "idle CPU exceeded 20% of one core: {idle_cpu_percent}"
    );
    eprintln!(
        "connection resource sample: {}",
        json!({
            "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
            "waves": 15, "concurrency": 16, "http_rpc_reads": 960, "sse_cycles": 240, "websocket_cycles": 240,
            "warmup_waves": 3, "samples": samples, "p50_us": p50, "p95_us": p95,
            "idle_cpu_percent": idle_cpu_percent,
        })
    );
}
