use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use notify::{
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{CreateKind, ModifyKind, RemoveKind, RenameMode},
};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::cache::DIRECTORY_CACHE;

static FILE_WATCHER: OnceLock<FileWatcherHandle> = OnceLock::new();

pub struct FileWatcherHandle {
    watch_tx: mpsc::UnboundedSender<WatchCommand>,
}

enum WatchCommand {
    Watch(PathBuf),
    Unwatch(PathBuf),
    Shutdown,
}

impl FileWatcherHandle {
    pub fn watch(&self, path: PathBuf) {
        if let Err(e) = self.watch_tx.send(WatchCommand::Watch(path)) {
            warn!("Failed to send watch command: {}", e);
        }
    }

    pub fn unwatch(&self, path: PathBuf) {
        if let Err(e) = self.watch_tx.send(WatchCommand::Unwatch(path)) {
            warn!("Failed to send unwatch command: {}", e);
        }
    }

    pub fn shutdown(&self) {
        let _ = self.watch_tx.send(WatchCommand::Shutdown);
    }
}

pub fn init_file_watcher() -> Option<&'static FileWatcherHandle> {
    FILE_WATCHER.get_or_init(|| {
        let (watch_tx, watch_rx) = mpsc::unbounded_channel();
        tokio::spawn(run_watcher(watch_rx));
        info!("File watcher initialized");
        FileWatcherHandle { watch_tx }
    });
    FILE_WATCHER.get()
}

pub fn get_file_watcher() -> Option<&'static FileWatcherHandle> {
    FILE_WATCHER.get()
}

async fn run_watcher(mut cmd_rx: mpsc::UnboundedReceiver<WatchCommand>) {
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let watcher_result = RecommendedWatcher::new(
        move |result: Result<Event, notify::Error>| {
            if let Ok(event) = result {
                let _ = event_tx.send(event);
            }
        },
        Config::default()
            .with_poll_interval(Duration::from_secs(2))
    );

    let mut watcher = match watcher_result {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to create file watcher: {}", e);
            return;
        }
    };

    info!("File watcher started");

    loop {
        tokio::select! {
            Some(cmd) = cmd_rx.recv() => {
                match cmd {
                    WatchCommand::Watch(path) => {
                        debug!("Watching: {}", path.display());
                        if let Err(e) = watcher.watch(&path, RecursiveMode::NonRecursive) {
                            warn!("Failed to watch {}: {}", path.display(), e);
                        }
                    }
                    WatchCommand::Unwatch(path) => {
                        debug!("Unwatching: {}", path.display());
                        if let Err(e) = watcher.unwatch(&path) {
                            warn!("Failed to unwatch {}: {}", path.display(), e);
                        }
                    }
                    WatchCommand::Shutdown => {
                        info!("File watcher shutting down");
                        break;
                    }
                }
            }
            Some(event) = event_rx.recv() => {
                handle_fs_event(event);
            }
            else => break,
        }
    }
}

fn handle_fs_event(event: Event) {
    let should_invalidate = matches!(
        event.kind,
        EventKind::Create(CreateKind::File | CreateKind::Folder)
            | EventKind::Remove(RemoveKind::File | RemoveKind::Folder)
            | EventKind::Modify(ModifyKind::Name(RenameMode::Both | RenameMode::From | RenameMode::To))
    );

    if !should_invalidate {
        return;
    }

    for path in event.paths {
        debug!("FS event {:?} for: {}", event.kind, path.display());
        if let Some(parent) = path.parent() {
            let parent_buf = parent.to_path_buf();
            DIRECTORY_CACHE.invalidate(&parent_buf);
            debug!("Invalidated cache for: {}", parent_buf.display());
        }
        if matches!(event.kind, EventKind::Remove(RemoveKind::Folder)) {
            DIRECTORY_CACHE.invalidate_subtree(&path);
            debug!("Invalidated subtree for: {}", path.display());
        }
    }
}

pub fn watch_workspace(root: &PathBuf) {
    let Some(watcher) = get_file_watcher() else {
        return;
    };
    watcher.watch(root.clone());
    for subdir in ["src", "include", "lib", "cmake", "tests", "test", "modules"] {
        let path = root.join(subdir);
        if path.exists() && path.is_dir() {
            watcher.watch(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::tempdir;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_watcher_initialization() {
        // Just test that initialization doesn't panic
        let handle = init_file_watcher();
        assert!(handle.is_some());
    }

    #[tokio::test]
    async fn test_handle_fs_event_create() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();

        // Pre-populate cache
        DIRECTORY_CACHE.insert(dir_path.clone(), vec![]);
        assert!(DIRECTORY_CACHE.get(&dir_path).is_some());

        // Simulate create event
        let event = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![dir_path.join("new_file.txt")],
            attrs: Default::default(),
        };

        handle_fs_event(event);

        // Cache should be invalidated
        assert!(DIRECTORY_CACHE.get(&dir_path).is_none());
    }

    #[tokio::test]
    async fn test_handle_fs_event_remove_dir() {
        let dir = tempdir().unwrap();
        let parent_path = dir.path().to_path_buf();
        let child_path = parent_path.join("subdir");
        let grandchild_path = child_path.join("nested");

        // Pre-populate cache
        DIRECTORY_CACHE.insert(parent_path.clone(), vec![]);
        DIRECTORY_CACHE.insert(child_path.clone(), vec![]);
        DIRECTORY_CACHE.insert(grandchild_path.clone(), vec![]);

        // Simulate remove folder event
        let event = Event {
            kind: EventKind::Remove(RemoveKind::Folder),
            paths: vec![child_path.clone()],
            attrs: Default::default(),
        };

        handle_fs_event(event);

        // Parent cache should be invalidated (parent of removed dir)
        assert!(DIRECTORY_CACHE.get(&parent_path).is_none());
        // Child and grandchild should be invalidated (subtree)
        assert!(DIRECTORY_CACHE.get(&child_path).is_none());
        assert!(DIRECTORY_CACHE.get(&grandchild_path).is_none());
    }
}
