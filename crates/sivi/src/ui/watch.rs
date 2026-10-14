//! The watcher that reports a change of a file the deck builds from, under the deck's directory.

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};
use std::path::Path;

/// The extensions of the files a build reads, which a change of reports; the PDF the build writes is left out.
const WATCHED: [&str; 8] = ["md", "toml", "png", "jpg", "jpeg", "svg", "ttf", "otf"];

/// A running watcher, which stops when dropped.
pub(super) struct Watch {
    _watcher: RecommendedWatcher,
}

fn is_watched(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) && event.paths.iter().any(|path| {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| WATCHED.contains(&extension))
    })
}

/// Watches `dir` and everything under it, calling `changed` for every change of a file a build reads.
///
/// # Errors
///
/// Returns the error when the host cannot watch the directory.
pub(super) fn start<F>(dir: &Path, changed: F) -> Result<Watch, notify::Error>
where
    F: Fn() + Send + 'static,
{
    let mut watcher = notify::recommended_watcher(move |result: Result<Event, notify::Error>| {
        if let Ok(event) = result
            && is_watched(&event)
        {
            changed();
        }
    })?;
    watcher.watch(dir, RecursiveMode::Recursive)?;
    Ok(Watch { _watcher: watcher })
}
