//! Review preparation only; held helper prevents any native submission.
use super::*;
impl SmokeCheck {
    pub(super) fn print_helper_advance(
        &mut self,
        app: &mut Application,
        ui: &Workspace,
        mode: &str,
    ) {
        let result = (|| -> anyhow::Result<()> {
            anyhow::ensure!(
                matches!(
                    mode,
                    "print-review-close" | "print-review-shutdown" | "print-discovery"
                ),
                "Unknown print preparation diagnostic"
            );
            anyhow::ensure!(
                std::env::var("GTK_PRINT_BACKENDS").as_deref() == Ok("file"),
                "Print diagnostic requires GTK's file-only backend"
            );
            let marker = PathBuf::from(
                std::env::var_os("DPDF_SMOKE_HELPER_READY")
                    .ok_or_else(|| anyhow::anyhow!("Missing helper marker"))?,
            );
            match self.stage {
                0 if mode == "print-discovery"
                    && ui.get_review_ready()
                    && app.pages.iter().any(|page| page.ready) =>
                {
                    app.discover_printers(ui);
                    anyhow::ensure!(app.printer_loading, "Printer discovery was not admitted");
                    self.stage = 4;
                }
                4 if marker.exists() => {
                    app.go_to(ui, 24);
                    self.stage = 5;
                }
                5 if app.pages.iter().any(|page| page.page == 24 && page.ready) => {
                    anyhow::ensure!(
                        app.printer_loading && ui.get_printer_loading(),
                        "Printer discovery finished before held helper returned"
                    );
                    println!("helper: rendered page 25 while printer discovery remained stalled");
                    println!(
                        "helper: owned printer discovery stayed pending until normal shutdown"
                    );
                    self.stage = 99;
                    let _ = slint::quit_event_loop();
                }
                0 if ui.get_review_ready() && app.pages.iter().any(|page| page.ready) => {
                    let tab = &app.tabs[app.active];
                    let mut review = tab
                        .review
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("Missing review model"))?
                        .model
                        .clone();
                    review.remarks.push(crate::review::Remark {
                        id: 1,
                        page: 0,
                        revision: review.current,
                        kind: crate::review::Kind::Note,
                        bounds: crate::document::NormalizedRect {
                            x: 0.1,
                            y: 0.1,
                            width: 0.1,
                            height: 0.1,
                        },
                        text: "Generated print preparation cancellation note".into(),
                    });
                    let command = Command::PrintReviewed {
                        id: tab.info.id,
                        review,
                        current: 0,
                        range: String::new(),
                        printer: String::new(),
                        fit: true,
                        native: true,
                    };
                    let arm = PathBuf::from(
                        std::env::var_os("DPDF_SMOKE_HELPER_ARM")
                            .ok_or_else(|| anyhow::anyhow!("Missing helper arm"))?,
                    );
                    std::fs::write(arm, b"armed")?;
                    anyhow::ensure!(app.command(ui, command), "Reviewed print was not admitted");
                    app.print_pending = true;
                    ui.set_print_busy(true);
                    self.stage = 1;
                }
                1 if marker.exists() => {
                    app.go_to(ui, 24);
                    self.stage = 2;
                }
                2 if app.pages.iter().any(|page| page.page == 24 && page.ready) => {
                    anyhow::ensure!(
                        app.print_pending && ui.get_print_busy(),
                        "Reviewed print finished before held metadata returned"
                    );
                    println!(
                        "helper: rendered page 25 while reviewed print helper remained stalled"
                    );
                    if mode == "print-review-close" {
                        app.close(ui, app.active);
                        self.stage = 3;
                    } else {
                        println!(
                            "helper: owned reviewed print preparation stayed pending until normal shutdown"
                        );
                        self.stage = 99;
                        let _ = slint::quit_event_loop();
                    }
                }
                3 if app.tabs.is_empty() && !app.print_pending && !ui.get_print_busy() => {
                    println!("helper: tab close cancelled reviewed print before native handoff");
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
