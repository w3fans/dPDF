// Exercise the exact vendored Wayland URI parser in the normal application test gate.
#[cfg(all(test, target_os = "linux"))]
#[path = "../third_party/winit/src/platform_impl/linux/wayland/uri_list.rs"]
mod wayland_uri_list_tests;

mod about;
mod app;
mod assembly;
mod background_jobs;
mod block_probe;
mod cache;
mod document;
mod encrypted;
#[cfg(target_os = "linux")]
mod helper_process;
#[cfg(target_os = "windows")]
#[path = "helper_process_windows.rs"]
mod helper_process;
mod image_pages;
mod imaging;
mod mark_studio;
mod marks;
#[cfg(target_os = "linux")]
mod native_print;
mod overlay;
mod page_bookmarks;
mod pages;
mod pdf;
mod pdf_units;
mod placement_ui;
mod platform;
mod preferences;
mod print_plan;
mod recovery;
mod review;
mod snapshot;
mod storage;
#[cfg(any(target_os = "windows", feature = "check-windows-print"))]
mod windows_print;

slint::include_modules!();

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().skip(1);
    if let Some(first) = args.next() {
        if first == "--version" {
            println!(
                "dPDF {} (reader development preview)",
                env!("CARGO_PKG_VERSION")
            );
            return Ok(());
        }
        if first == "--probe-block" {
            return block_probe::run_cli(args);
        }
        if first == "--review" {
            return review::run_cli(args);
        }
        if first == "--smoke-about" {
            let output = std::path::PathBuf::from(
                args.next()
                    .ok_or_else(|| anyhow::anyhow!("--smoke-about OUTPUT.ppm [dark]"))?,
            );
            let dark = match args.next() {
                None => false,
                Some(flag) if flag == "dark" => true,
                _ => anyhow::bail!("--smoke-about OUTPUT.ppm [dark]"),
            };
            if args.next().is_some() {
                anyhow::bail!("Too many About diagnostic arguments");
            }
            return about::smoke(&output, dark);
        }
        if first == "--benchmark-reader" {
            let source =
                std::path::PathBuf::from(args.next().ok_or_else(|| {
                    anyhow::anyhow!("--benchmark-reader INPUT [--compare-eager]")
                })?);
            let compare = match args.next() {
                None => false,
                Some(flag) if flag == "--compare-eager" => true,
                _ => anyhow::bail!("--benchmark-reader INPUT [--compare-eager]"),
            };
            if args.next().is_some() {
                anyhow::bail!("Too many reader benchmark arguments");
            }
            return pdf::benchmark_reader(&source, compare);
        }
        if first == "--marks" {
            return marks::run_cli(args);
        }
        #[cfg(target_os = "linux")]
        if first == "--native-print" || first == "--test-native-print" {
            return native_print::run_cli(args, first == "--test-native-print");
        }
        #[cfg(target_os = "windows")]
        if first == "--native-print" || first == "--test-native-print" {
            return windows_print::run_cli(args, first == "--test-native-print");
        }
        if first == "--overlay" {
            return overlay::run_cli(args);
        }
        if first == "--render-page" {
            let input = std::path::PathBuf::from(
                args.next()
                    .ok_or_else(|| anyhow::anyhow!("--render-page INPUT PAGE OUTPUT.ppm"))?,
            );
            let page: usize = args
                .next()
                .ok_or_else(|| anyhow::anyhow!("Missing page"))?
                .to_string_lossy()
                .parse()?;
            let output = std::path::PathBuf::from(
                args.next()
                    .ok_or_else(|| anyhow::anyhow!("Missing output PPM"))?,
            );
            let width = args
                .next()
                .map(|value| value.to_string_lossy().parse::<i32>())
                .transpose()?
                .unwrap_or(1000);
            if page == 0 || !(32..=3072).contains(&width) || args.next().is_some() {
                anyhow::bail!(
                    "--render-page INPUT PAGE OUTPUT.ppm [WIDTH:32..3072] (pages start at 1)"
                );
            }
            return pdf::render_probe(&input, page - 1, &output, width);
        }
        if first == "--smoke-marks" {
            return mark_studio::smoke(std::path::PathBuf::from(args.next().ok_or_else(|| {
                anyhow::anyhow!(
                    "--smoke-marks IMAGE requires a generated fixture and explicit DPDF_DATA_DIR"
                )
            })?));
        }
        if first == "--help" || first == "-h" {
            println!(
                "dPDF — compact native PDF reader\n\nUsage: dpdf [PDF ...]\n       dpdf --probe PDF\n       dpdf --probe-page PDF PAGE\n       dpdf --probe-search PDF QUERY\n       dpdf --probe-select PDF PAGE X Y WIDTH HEIGHT\n       dpdf --smoke-ui SAMPLE.pdf\n       dpdf --assemble OP INPUT OUTPUT PAGES SOURCE_PAGES [SOURCE_PDF_OR_IMAGE ...]\n       dpdf --marks list|import|size|clean ...\n       dpdf --overlay INPUT OUTPUT PLACEMENTS.json\n       dpdf --render-page INPUT PAGE OUTPUT.ppm [WIDTH]\n       dpdf --probe-block NEW_DIRECTORY GENERATED_CASE\n       dpdf --version\n\nPDFium: set DPDF_PDFIUM_LIBRARY or place the library beside the executable."
            );
            return Ok(());
        }
        if first == "--smoke-ui" {
            app::smoke(std::path::PathBuf::from(args.next().ok_or_else(|| {
                anyhow::anyhow!("--smoke-ui needs the generated sample PDF path")
            })?))?;
            return Ok(());
        }
        if first == "--probe-search" {
            let path = args
                .next()
                .ok_or_else(|| anyhow::anyhow!("--probe-search needs a PDF path and query"))?;
            let query = args
                .next()
                .ok_or_else(|| anyhow::anyhow!("--probe-search needs a query"))?;
            return pdf::probe_search(&std::path::PathBuf::from(path), &query.to_string_lossy());
        }
        if first == "--probe-select" {
            let path = args
                .next()
                .ok_or_else(|| anyhow::anyhow!("--probe-select PDF PAGE X Y WIDTH HEIGHT"))?;
            let page: usize = args
                .next()
                .ok_or_else(|| anyhow::anyhow!("Missing page number"))?
                .to_string_lossy()
                .parse()?;
            if page == 0 {
                anyhow::bail!("Page numbers start at 1");
            }
            let mut values = Vec::new();
            for _ in 0..4 {
                values.push(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("Missing normalized rectangle coordinate"))?
                        .to_string_lossy()
                        .parse::<f32>()?,
                );
            }
            return pdf::probe_selection(
                &std::path::PathBuf::from(path),
                page - 1,
                document::NormalizedRect {
                    x: values[0],
                    y: values[1],
                    width: values[2],
                    height: values[3],
                },
            );
        }
        if first == "--assemble" {
            let kind = args.next().ok_or_else(|| {
                anyhow::anyhow!(
                    "--assemble OPERATION INPUT OUTPUT PAGES SOURCE_PAGES [SOURCE_PDF_OR_IMAGE ...]"
                )
            })?;
            let input = std::path::PathBuf::from(
                args.next()
                    .ok_or_else(|| anyhow::anyhow!("Missing input PDF"))?,
            );
            let output = std::path::PathBuf::from(
                args.next()
                    .ok_or_else(|| anyhow::anyhow!("Missing output PDF"))?,
            );
            let pages = args
                .next()
                .ok_or_else(|| anyhow::anyhow!("Missing page selection"))?
                .to_string_lossy()
                .into_owned();
            let source_pages = args
                .next()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Missing source page selection (use an empty string when unused)"
                    )
                })?
                .to_string_lossy()
                .into_owned();
            let operation = assembly::PageOperation {
                kind: assembly::OperationKind::from_name(&kind.to_string_lossy())?,
                pages,
                source_pages,
            };
            let bytes = std::sync::Arc::new(std::fs::read(&input)?);
            let mut sources = vec![assembly::SourceSnapshot { path: input, bytes }];
            let additional: Vec<_> = args.map(std::path::PathBuf::from).collect();
            if !additional.is_empty() {
                let pdfium = pdf::bind_pdfium()?;
                let primary = pdfium.load_pdf_from_byte_slice(&sources[0].bytes, None)?;
                let prepared =
                    image_pages::prepare(&pdfium, &primary, &sources[0], &operation, &additional)?;
                drop(primary);
                sources.extend(prepared);
            }
            eprintln!(
                "Page assembly invalidates any existing cryptographic signatures in the result; source files are preserved."
            );
            let result = assembly::export(&sources, &operation, &output)?;
            println!(
                "saved_pages={} output={}",
                result.pages,
                result.path.display()
            );
            return Ok(());
        }
        if first == "--probe-page" {
            let path = args.next().ok_or_else(|| {
                anyhow::anyhow!("--probe-page needs a PDF path and one-based page number")
            })?;
            let page: usize = args
                .next()
                .ok_or_else(|| anyhow::anyhow!("Missing page number"))?
                .to_string_lossy()
                .parse()?;
            if page == 0 {
                anyhow::bail!("Page numbers start at 1");
            }
            return pdf::probe_page(&std::path::PathBuf::from(path), page - 1);
        }
        if first == "--probe" {
            return pdf::probe(&std::path::PathBuf::from(
                args.next()
                    .ok_or_else(|| anyhow::anyhow!("--probe needs a PDF path"))?,
            ));
        }
        let paths = std::iter::once(first)
            .chain(args)
            .map(std::path::PathBuf::from)
            .collect();
        app::run(paths)?;
    } else {
        app::run(vec![])?;
    }
    Ok(())
}
