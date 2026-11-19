//! Controlled page/image helper cases; no production files or print submission.
use super::*;

impl SmokeCheck {
    pub(super) fn page_helper_advance(
        &mut self,
        app: &mut Application,
        ui: &Workspace,
        mode: &str,
    ) {
        let result = (|| -> anyhow::Result<()> {
            if !matches!(
                mode,
                "image-preflight"
                    | "image-unit-write"
                    | "image-save"
                    | "pages-save"
                    | "pages-retry"
            ) {
                anyhow::bail!("Unknown page helper diagnostic mode");
            }
            let marker = PathBuf::from(
                std::env::var_os("DPDF_SMOKE_HELPER_READY")
                    .ok_or_else(|| anyhow::anyhow!("Missing page helper readiness path"))?,
            );
            let destination = PathBuf::from(
                std::env::var_os("DPDF_SMOKE_HELPER_DESTINATION")
                    .ok_or_else(|| anyhow::anyhow!("Missing page export destination"))?,
            );
            let command = |id| -> anyhow::Result<Command> {
                let inputs: Vec<PathBuf> =
                    serde_json::from_str(&std::env::var("DPDF_SMOKE_HELPER_PAGE_INPUTS")?)?;
                let kind = match mode {
                    "pages-save" => crate::assembly::OperationKind::Extract,
                    "pages-retry" => crate::assembly::OperationKind::Merge,
                    _ => crate::assembly::OperationKind::Replace,
                };
                let selection = if mode == "pages-retry" { "" } else { "1" };
                Ok(Command::ExportPages {
                    id,
                    operation: crate::assembly::PageOperation {
                        kind,
                        pages: selection.into(),
                        source_pages: selection.into(),
                    },
                    additional: inputs,
                    destination: destination.clone(),
                })
            };
            match self.stage {
                0 if ui.get_review_ready() && app.pages.iter().any(|page| page.ready) => {
                    let tab = &app.tabs[app.active];
                    self.tab_identity = Some(tab.info.id);
                    self.tab_reopen = Some(tab.info.path.clone());
                    let request = command(tab.info.id)?;
                    let arm = PathBuf::from(
                        std::env::var_os("DPDF_SMOKE_HELPER_ARM")
                            .ok_or_else(|| anyhow::anyhow!("Missing page helper arm path"))?,
                    );
                    std::fs::write(arm, b"armed")?;
                    if !app.command(ui, request) {
                        anyhow::bail!("Page export was not admitted");
                    }
                    app.export_pending = true;
                    ui.set_export_busy(true);
                    self.stage = 1;
                }
                1 if marker.exists() => {
                    app.go_to(ui, 24);
                    self.stage = 2;
                }
                2 if app.pages.iter().any(|page| page.page == 24 && page.ready) => {
                    if !app.export_pending || !ui.get_export_busy() {
                        anyhow::bail!("Page export completed before its held helper");
                    }
                    println!("helper: rendered page 25 while page export helper remained stalled");
                    if mode == "image-save" {
                        println!("helper: owned page export stayed pending until normal shutdown");
                        self.stage = 99;
                        let _ = slint::quit_event_loop();
                    } else {
                        app.close(ui, app.active);
                        self.stage = 3;
                    }
                }
                3 if app.tabs.is_empty() && !app.export_pending && !ui.get_export_busy() => {
                    if mode == "pages-retry" {
                        app.open(ui, self.tab_reopen.clone().unwrap());
                        self.stage = 4;
                    } else {
                        println!(
                            "helper: tab close cancelled page export without publishing a late result"
                        );
                        self.stage = 99;
                        let _ = slint::quit_event_loop();
                    }
                }
                4 if ui.get_review_ready() && app.pages.iter().any(|page| page.ready) => {
                    let id = app.tabs[app.active].info.id;
                    if Some(id) == self.tab_identity {
                        anyhow::bail!("Page export retry reused closed identity");
                    }
                    if !app.command(ui, command(id)?) {
                        anyhow::bail!("Page export retry was not admitted");
                    }
                    app.export_pending = true;
                    ui.set_export_busy(true);
                    self.stage = 5;
                }
                5 if !app.export_pending
                    && !ui.get_export_busy()
                    && ui.get_review_ready()
                    && app.tabs[app.active].info.path == destination
                    && app.tabs[app.active].info.pages.len() == 55 =>
                {
                    println!(
                        "helper: page export retry published 55 pages after cancelling its prior generation"
                    );
                    self.stage = 99;
                    let _ = slint::quit_event_loop();
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.failure = Some(format!("{error:#}"));
            let _ = slint::quit_event_loop();
        }
    }
}
