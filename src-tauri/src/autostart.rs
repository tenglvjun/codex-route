use tauri::{AppHandle, Runtime};

pub trait AutostartBackend {
    fn set_enabled(&self, enabled: bool) -> Result<(), String>;
}

pub fn sync_launch_at_login<B: AutostartBackend>(
    backend: &B,
    enabled: bool,
) -> Result<(), String> {
    backend.set_enabled(enabled)
}

pub struct TauriAutostartBackend<'a, R: Runtime> {
    app: &'a AppHandle<R>,
}

impl<'a, R: Runtime> TauriAutostartBackend<'a, R> {
    pub fn new(app: &'a AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> AutostartBackend for TauriAutostartBackend<'_, R> {
    fn set_enabled(&self, enabled: bool) -> Result<(), String> {
        use tauri_plugin_autostart::ManagerExt;

        if enabled {
            self.app
                .autolaunch()
                .enable()
                .map_err(|error| error.to_string())
        } else {
            self.app
                .autolaunch()
                .disable()
                .map_err(|error| error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{sync_launch_at_login, AutostartBackend};
    use std::cell::RefCell;

    struct FakeBackend {
        calls: RefCell<Vec<bool>>,
        error: Option<String>,
    }

    impl AutostartBackend for FakeBackend {
        fn set_enabled(&self, enabled: bool) -> Result<(), String> {
            self.calls.borrow_mut().push(enabled);
            self.error.clone().map_or(Ok(()), Err)
        }
    }

    #[test]
    fn forwards_enable_and_disable_once() {
        let backend = FakeBackend { calls: RefCell::new(Vec::new()), error: None };
        sync_launch_at_login(&backend, true).unwrap();
        sync_launch_at_login(&backend, false).unwrap();
        assert_eq!(*backend.calls.borrow(), vec![true, false]);
    }

    #[test]
    fn returns_registration_error() {
        let backend = FakeBackend {
            calls: RefCell::new(Vec::new()),
            error: Some("login item denied".to_string()),
        };
        assert_eq!(sync_launch_at_login(&backend, true), Err("login item denied".to_string()));
        assert_eq!(*backend.calls.borrow(), vec![true]);
    }
}
