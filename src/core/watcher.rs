use anyhow::Result;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

/// File change event
#[derive(Debug, Clone)]
pub enum FileChangeEvent {
    Changed(PathBuf),
}

/// File watcher that monitors directories for changes
pub struct FileWatcher {
    _debouncer: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
}

impl FileWatcher {
    /// Start watching a list of directories
    pub fn watch(
        directories: &[PathBuf],
        debounce_ms: u64,
    ) -> Result<(Self, mpsc::Receiver<FileChangeEvent>)> {
        let (tx, rx) = mpsc::channel();

        let mut debouncer = new_debouncer(
            Duration::from_millis(debounce_ms),
            move |result: DebounceEventResult| {
                if let Ok(events) = result {
                    for event in events {
                        let _ = tx.send(FileChangeEvent::Changed(event.path.clone()));
                    }
                }
            },
        )
        .map_err(|e| anyhow::anyhow!("Failed to create file watcher: {}", e))?;

        for dir in directories {
            if dir.exists() {
                if let Err(e) = debouncer.watcher().watch(dir, notify::RecursiveMode::Recursive) {
                    eprintln!("Warning: failed to watch {}: {}", dir.display(), e);
                }
            }
        }

        Ok((Self { _debouncer: debouncer }, rx))
    }
}
