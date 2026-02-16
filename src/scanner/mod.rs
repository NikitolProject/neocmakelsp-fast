mod cache;
mod parallel;
pub mod watcher;

#[allow(unused_imports)]
pub use cache::{CachedEntry, DIRECTORY_CACHE, DirectoryCache};
#[allow(unused_imports)]
pub use parallel::{ScanOptions, scan_directory, scan_directory_recursive};
pub use watcher::{get_file_watcher, init_file_watcher, watch_workspace};
