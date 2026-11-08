//! Product information follows the owner's RUWAC/GSE About-page principles.
//! All assets are local; external links open only after an explicit user action.
use crate::AboutWindow;
use anyhow::{Context, Result};
use slint::ComponentHandle;

pub const SUPPORT_URL: &str = "https://idz.si/bs/dpdf-donate.html";
const LINKS: [&str; 4] = [
    "https://github.com/w3fans/dPDF",
    "https://github.com/w3fans/dPDF/issues",
    "https://github.com/w3fans/dPDF/blob/main/LICENSE",
    SUPPORT_URL,
];

pub struct Session {
    ui: AboutWindow,
}
impl Session {
    pub fn new() -> Result<Self> {
        let ui = AboutWindow::new()?;
        ui.set_app_version(env!("CARGO_PKG_VERSION").into());
        ui.set_support_url(SUPPORT_URL.into());
        let weak = ui.as_weak();
        ui.on_close_about(move || {
            if let Some(window) = weak.upgrade() {
                let _ = window.hide();
            }
        });
        let weak = ui.as_weak();
        ui.on_open_link(move |index| {
            let Some(url) = usize::try_from(index)
                .ok()
                .and_then(|i| LINKS.get(i))
                .copied()
            else {
                return;
            };
            let weak = weak.clone();
            std::thread::spawn(move || {
                let result = crate::platform::open_url(url).map_err(|e| format!("{e:#}"));
                let _ = weak.upgrade_in_event_loop(move |window| {
                    window.set_status(match result {
                        Ok(()) => "Opened in your browser".into(),
                        Err(error) => format!("Cannot open link: {error}").into(),
                    });
                });
            });
        });
        Ok(Self { ui })
    }
    pub fn show(&self, dark: bool) -> Result<()> {
        self.ui.set_dark(dark);
        self.ui.set_status("".into());
        self.ui.show().context("Show About dPDF")
    }
    pub fn hide(&self) -> Result<()> {
        self.ui.hide().context("Close About dPDF")
    }
}

pub fn smoke(output: &std::path::Path, dark: bool) -> Result<()> {
    let session = Session::new()?;
    session.ui.set_qr_visible(true);
    session.show(dark)?;
    let weak = session.ui.as_weak();
    let output = output.to_path_buf();
    let failure = std::rc::Rc::new(std::cell::RefCell::new(None));
    let result = failure.clone();
    let timer = slint::Timer::default();
    let phase = std::cell::Cell::new(0);
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(500),
        move || {
            if let Some(window) = weak.upgrade() {
                let path = if phase.get() == 0 {
                    output.clone()
                } else {
                    output.with_file_name(format!(
                        "{}-details.ppm",
                        output.file_stem().unwrap_or_default().to_string_lossy()
                    ))
                };
                if let Err(error) = crate::snapshot::write(window.window(), &path) {
                    *result.borrow_mut() = Some(format!("{error:#}"));
                }
                if phase.get() == 0 {
                    window.set_scroll_y(window.get_scroll_min());
                    phase.set(1);
                    return;
                }
                let _ = window.hide();
            }
            let _ = slint::quit_event_loop();
        },
    );
    slint::run_event_loop()?;
    if let Some(message) = failure.borrow_mut().take() {
        anyhow::bail!("{message}");
    }
    println!(
        "about: local version, author, Apache license, support/QR and Slint attribution rendered; no link opened"
    );
    Ok(())
}
