use super::event::AuthenticationEvent;
use std::{
    io::{self, Write},
    sync::Arc,
    time::Duration,
};
use tokio::sync::Semaphore;

/// Returning Ok acknowledges a local stdout write, never Firehose or S3 acceptance.
pub(crate) trait AuditSink: Send + Sync {
    fn write(&self, event: &AuthenticationEvent) -> io::Result<()>;
}

pub(crate) struct StdoutSink;

impl AuditSink for StdoutSink {
    fn write(&self, event: &AuthenticationEvent) -> io::Result<()> {
        // The process-wide stdout lock also serializes against operational JSON logs.
        write_event(io::stdout().lock(), event)
    }
}

pub(crate) fn write_event(mut writer: impl Write, event: &AuthenticationEvent) -> io::Result<()> {
    let mut line = serde_json::to_vec(&serde_json::json!({"audit": event}))?;
    line.push(b'\n');
    writer.write_all(&line)?;
    writer.flush()
}

#[derive(Clone)]
pub(crate) struct AuditEmitter {
    sink: Arc<dyn AuditSink>,
    permits: Arc<Semaphore>,
    timeout: Duration,
}

impl AuditEmitter {
    pub(crate) fn new(sink: Arc<dyn AuditSink>) -> Self {
        Self {
            sink,
            permits: Arc::new(Semaphore::new(16)),
            timeout: Duration::from_secs(2),
        }
    }

    pub(crate) async fn emit(&self, event: AuthenticationEvent) -> io::Result<()> {
        let permit = self
            .permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| io::Error::new(io::ErrorKind::WouldBlock, "audit writer is busy"))?;
        let sink = self.sink.clone();
        let worker = tokio::task::spawn_blocking(move || {
            // Keep the permit on timeout; a blocked write must not create unbounded workers.
            let _permit = permit;
            sink.write(&event)
        });
        tokio::time::timeout(self.timeout, worker)
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "audit write timed out"))?
            .map_err(|_| io::Error::other("audit writer failed"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Condvar, Mutex};

    #[test]
    fn write_and_flush_failures_are_reported() {
        struct BrokenWriter {
            fail_write: bool,
        }
        impl Write for BrokenWriter {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if self.fail_write {
                    Err(io::Error::other("write failed"))
                } else {
                    Ok(bytes.len())
                }
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::Error::other("flush failed"))
            }
        }
        for fail_write in [true, false] {
            assert!(write_event(
                BrokenWriter { fail_write },
                &crate::audit::event::tests::event()
            )
            .is_err());
        }
    }

    #[tokio::test(start_paused = true)]
    async fn timeout_keeps_admission_bounded_until_the_blocking_writer_finishes() {
        struct GatedSink {
            started: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
            released: Mutex<bool>,
            release: Condvar,
        }
        impl AuditSink for GatedSink {
            fn write(&self, _: &AuthenticationEvent) -> io::Result<()> {
                if let Some(started) = self.started.lock().unwrap().take() {
                    let _ = started.send(());
                }
                let mut released = self.released.lock().unwrap();
                while !*released {
                    released = self.release.wait(released).unwrap();
                }
                Ok(())
            }
        }
        let (started, did_start) = tokio::sync::oneshot::channel();
        let sink = Arc::new(GatedSink {
            started: Mutex::new(Some(started)),
            released: Mutex::new(false),
            release: Condvar::new(),
        });
        let emitter = AuditEmitter::new(sink.clone());
        let running = emitter.clone();
        let worker =
            tokio::spawn(async move { running.emit(crate::audit::event::tests::event()).await });
        did_start.await.unwrap();
        tokio::time::advance(Duration::from_secs(3)).await;
        let timed_out = worker.await.unwrap();
        let remaining = emitter.permits.available_permits();
        let permits = emitter.permits.clone().try_acquire_many_owned(15).unwrap();
        let rejected = emitter.emit(crate::audit::event::tests::event()).await;
        // Always release the external writer before assertions, including on a test failure.
        *sink.released.lock().unwrap() = true;
        sink.release.notify_all();
        drop(permits);
        assert_eq!(timed_out.unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert_eq!(remaining, 15);
        assert_eq!(rejected.unwrap_err().kind(), io::ErrorKind::WouldBlock);
    }
}
