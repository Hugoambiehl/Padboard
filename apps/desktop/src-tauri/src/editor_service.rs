//! Runs the [`Editor`] for the app: applies the frontend's edits, tells the
//! UI at once, saves `profiles.json` once edits pause, and pushes the active
//! layout to the tablets at a bounded rate.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::sync::{Notify, mpsc::UnboundedSender, watch};

use crate::config::edit::{Edit, EditOutcome, Editor, EditorState};
use crate::config::{ActiveLayout, Config, EditError};
use crate::server::ServerEvent;
use crate::storage::ConfigDir;

/// Quiet time before saving.
const SAVE_AFTER: Duration = Duration::from_millis(500);
/// At most one layout push to the tablets per period (while typing).
const PUSH_EVERY: Duration = Duration::from_millis(120);

pub type StateListener = Box<dyn Fn(EditorState) + Send + Sync>;

pub struct EditorService {
    editor: Mutex<Editor>,
    dir: ConfigDir,
    layout: watch::Sender<Arc<ActiveLayout>>,
    events: UnboundedSender<ServerEvent>,
    listener: StateListener,
    save: Notify,
    push: Notify,
    dirty: Mutex<bool>,
}

impl std::fmt::Debug for EditorService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorService").finish_non_exhaustive()
    }
}

impl EditorService {
    /// Starts the save and push tasks on the current Tokio runtime.
    pub fn start(
        config: Config,
        dir: ConfigDir,
        layout: watch::Sender<Arc<ActiveLayout>>,
        events: UnboundedSender<ServerEvent>,
        listener: StateListener,
    ) -> Arc<Self> {
        let service = Arc::new(Self {
            editor: Mutex::new(Editor::new(config)),
            dir,
            layout,
            events,
            listener,
            save: Notify::new(),
            push: Notify::new(),
            dirty: Mutex::new(false),
        });
        let saver = Arc::clone(&service);
        tauri::async_runtime::spawn(async move { saver.save_loop().await });
        let pusher = Arc::clone(&service);
        tauri::async_runtime::spawn(async move { pusher.push_loop().await });
        service
    }

    fn lock(&self) -> MutexGuard<'_, Editor> {
        self.editor.lock().unwrap_or_else(PoisonError::into_inner)
    }

    #[must_use]
    pub fn state(&self) -> EditorState {
        self.lock().state()
    }

    #[must_use]
    pub fn config(&self) -> Config {
        self.lock().config().clone()
    }

    /// # Errors
    /// Why the edit was refused (nothing changed).
    pub fn edit(&self, edit: Edit) -> Result<EditOutcome, EditError> {
        let (outcome, state) = {
            let mut editor = self.lock();
            let before = editor.config().clone();
            let outcome = editor.apply(edit)?;
            let changed = *editor.config() != before;
            (outcome, changed.then(|| editor.state()))
        };
        if let Some(state) = state {
            self.changed(state);
        }
        Ok(outcome)
    }

    pub fn undo(&self) -> bool {
        self.history(Editor::undo)
    }

    pub fn redo(&self) -> bool {
        self.history(Editor::redo)
    }

    pub fn checkpoint(&self) {
        self.lock().checkpoint();
    }

    fn history(&self, step: fn(&mut Editor) -> bool) -> bool {
        let state = {
            let mut editor = self.lock();
            step(&mut editor).then(|| editor.state())
        };
        let done = state.is_some();
        if let Some(state) = state {
            self.changed(state);
        }
        done
    }

    fn changed(&self, state: EditorState) {
        *self.dirty.lock().unwrap_or_else(PoisonError::into_inner) = true;
        (self.listener)(state);
        self.save.notify_one();
        self.push.notify_one();
    }

    /// Saves now if an edit is pending (app exit).
    pub fn flush(&self) {
        let pending =
            std::mem::take(&mut *self.dirty.lock().unwrap_or_else(PoisonError::into_inner));
        if pending && let Err(e) = self.dir.save(&self.config()) {
            tracing::error!("cannot save the profiles: {e}");
        }
    }

    async fn save_loop(&self) {
        loop {
            self.save.notified().await;
            // Wait for a pause in the edits.
            loop {
                tokio::select! {
                    () = self.save.notified() => {}
                    () = tokio::time::sleep(SAVE_AFTER) => break,
                }
            }
            let config = self.config();
            let dir = self.dir.clone();
            *self.dirty.lock().unwrap_or_else(PoisonError::into_inner) = false;
            let saved = tauri::async_runtime::spawn_blocking(move || dir.save(&config)).await;
            match saved {
                Ok(Ok(())) => tracing::debug!("profiles saved"),
                Ok(Err(e)) => tracing::error!("cannot save the profiles: {e}"),
                Err(e) => tracing::error!("save task failed: {e}"),
            }
        }
    }

    async fn push_loop(&self) {
        loop {
            self.push.notified().await;
            let active = Arc::new(self.config().active_layout());
            // Unchanged for the tablets (e.g. an edit of another profile): no push.
            if self.layout.borrow().layout != active.layout
                || self.layout.borrow().actions != active.actions
            {
                self.layout.send_replace(Arc::clone(&active));
                let _ = self.events.send(ServerEvent::LayoutChanged(active));
            }
            // Edits arriving meanwhile leave a permit: the next push carries them.
            tokio::time::sleep(PUSH_EVERY).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::config::seed;

    #[tokio::test]
    async fn typing_is_throttled_for_tablets_and_saved_after_a_pause() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = ConfigDir::open(tmp.path()).unwrap();
        let config = seed::config();
        let (layout, mut layouts) = watch::channel(Arc::new(config.active_layout()));
        let (events, _events_rx) = tokio::sync::mpsc::unbounded_channel();
        let states = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&states);
        let service = EditorService::start(
            config,
            dir.clone(),
            layout,
            events,
            Box::new(move |_| {
                counter.fetch_add(1, Ordering::Relaxed);
            }),
        );

        let pushes = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&pushes);
        tokio::spawn(async move {
            while layouts.changed().await.is_ok() {
                seen.fetch_add(1, Ordering::Relaxed);
            }
        });

        // 20 keystrokes, 10 ms apart: 200 ms of typing.
        let mut label = String::new();
        for c in "Copier le texte vite".chars() {
            label.push(c);
            service
                .edit(Edit::SetLabel {
                    button_id: "copy".into(),
                    label: label.clone(),
                })
                .unwrap();
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        tokio::time::sleep(PUSH_EVERY * 3).await;
        let pushed = pushes.load(Ordering::Relaxed);
        // Every keystroke reaches the UI, a few reach the tablets.
        assert_eq!(states.load(Ordering::Relaxed), 20);
        assert!((1..=4).contains(&pushed), "{pushed} pushes");
        assert!(!dir.path_of::<Config>().exists(), "saved before the pause");

        tokio::time::sleep(SAVE_AFTER * 2).await;
        let saved: Config = dir.load().unwrap().unwrap();
        let button = saved.profiles[0].pages[0]
            .buttons
            .iter()
            .find(|b| b.id == "copy")
            .unwrap();
        assert_eq!(button.label, "Copier le texte vite");
        // One undo entry for the whole typing.
        assert!(service.undo());
        assert!(!service.undo());
    }
}
