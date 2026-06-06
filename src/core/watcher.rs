use anyhow::Result;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

/// File change event from the watcher.
/// Note: notify-debouncer-mini only provides debounced "any change" events,
/// not specific Create/Modify/Remove. Consumers inspect the filesystem to determine
/// what actually happened.
#[derive(Debug, Clone)]
pub struct FileChangeEvent {
    pub path: PathBuf,
}

/// File watcher that monitors directories for changes using notify + debouncer.
pub struct FileWatcher {
    _debouncer: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
}

impl FileWatcher {
    /// Start watching a list of directories recursively.
    /// Returns the watcher (must be held alive) and a receiver for file change events.
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
                        let _ = tx.send(FileChangeEvent { path: event.path.clone() });
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
