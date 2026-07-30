use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use tracing_subscriber::fmt::MakeWriter;

#[derive(Clone, Default)]
pub(crate) struct TestTracingCapture {
    output: Arc<Mutex<Vec<u8>>>,
}

impl TestTracingCapture {
    pub(crate) fn subscriber(&self) -> impl tracing::Subscriber + Send + Sync {
        tracing_subscriber::fmt()
            .without_time()
            .with_target(false)
            .with_ansi(false)
            .with_max_level(tracing::Level::INFO)
            .with_writer(self.clone())
            .finish()
    }

    pub(crate) fn contents(&self) -> String {
        self.output
            .lock()
            .map(|output| String::from_utf8_lossy(&output).into_owned())
            .unwrap_or_default()
    }

    pub(crate) fn clear(&self) {
        if let Ok(mut output) = self.output.lock() {
            output.clear();
        }
    }
}

impl<'writer> MakeWriter<'writer> for TestTracingCapture {
    type Writer = Self;

    fn make_writer(&'writer self) -> Self::Writer {
        self.clone()
    }
}

impl Write for TestTracingCapture {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut output = self
            .output
            .lock()
            .map_err(|_| io::Error::other("测试日志缓冲区锁已损坏"))?;
        output.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
