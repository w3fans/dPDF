//! Generated-only PDFium region-replacement experiment. No arbitrary PDF input.
//! This does not enable production Secure Signature Block export.
use anyhow::{Context, Result, bail, ensure};
use pdfium_render::prelude::*;
use std::{
    io::Write,
    marker::PhantomData,
    os::raw::{c_int, c_ulong, c_void},
    path::PathBuf,
    ptr,
    rc::Rc,
};

const DEVICE_GRID: i32 = 1_000_000;
const BLOCK_DPI: u32 = 400;

const MAX_SAVED_BYTES: usize = 32 * 1024 * 1024;

struct Engine {
    api: Box<dyn PdfiumLibraryBindings>,
    _thread: PhantomData<Rc<()>>,
}
impl Engine {
    fn new() -> Result<Self> {
        let api = crate::pdf::bind_pdfium_library()?;
        // This explicit CLI path owns the sole engine in its process. It never
        // starts the reader worker or constructs a high-level Pdfium instance.
        unsafe {
            api.FPDF_InitLibrary();
        }
        Ok(Self {
            api,
            _thread: PhantomData,
        })
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        unsafe {
            self.api.FPDF_DestroyLibrary();
        }
    }
}

struct Document<'api, 'data> {
    api: &'api dyn PdfiumLibraryBindings,
    handle: FPDF_DOCUMENT,
    page: FPDF_PAGE,
    _data: PhantomData<&'data [u8]>,
}
impl<'api, 'data> Document<'api, 'data> {
    fn open(api: &'api dyn PdfiumLibraryBindings, bytes: &'data [u8]) -> Result<Self> {
        // The borrowed immutable input remains alive until this document closes.
        unsafe {
            let handle = api.FPDF_LoadMemDocument64(bytes, None);
            ensure!(!handle.is_null(), "Cannot load generated PDF");
            let page = api.FPDF_LoadPage(handle, 0);
            if page.is_null() {
                api.FPDF_CloseDocument(handle);
                bail!("Cannot load generated page");
            }
            Ok(Self {
                api,
                handle,
                page,
                _data: PhantomData,
            })
        }
    }

    fn geometry(&self) -> Result<Geometry> {
        let api = self.api;
        // Handles belong to this live document; all output pointers are local,
        // correctly sized values and remain valid for the duration of each call.
        unsafe {
            let points = [
                f64::from(api.FPDF_GetPageWidthF(self.page)),
                f64::from(api.FPDF_GetPageHeightF(self.page)),
            ];
            ensure!(
                points
                    .iter()
                    .all(|p| p.is_finite() && *p > 0.0 && *p <= 2000.0),
                "Unsupported page dimensions"
            );
            let mut devices = Vec::new();
            for (x, y) in [(90.0, 60.0), (280.0, 60.0), (90.0, 140.0), (280.0, 140.0)] {
                let (mut dx, mut dy) = (0, 0);
                ensure!(
                    api.FPDF_PageToDevice(
                        self.page,
                        0,
                        0,
                        DEVICE_GRID,
                        DEVICE_GRID,
                        0,
                        x,
                        y,
                        &mut dx,
                        &mut dy
                    ) != 0,
                    "Page mapping failed"
                );
                devices.push((dx, dy));
            }
            let x0 = devices.iter().map(|p| p.0).min().unwrap();
            let x1 = devices.iter().map(|p| p.0).max().unwrap();
            let y0 = devices.iter().map(|p| p.1).min().unwrap();
            let y1 = devices.iter().map(|p| p.1).max().unwrap();
            ensure!(
                0 <= x0 && x0 < x1 && x1 <= DEVICE_GRID && 0 <= y0 && y0 < y1 && y1 <= DEVICE_GRID,
                "Region does not fit displayed page"
            );
            let mut corners = Vec::new();
            for (x, y) in [(x0, y1), (x1, y1), (x0, y0), (x1, y0)] {
                let (mut px, mut py) = (0.0, 0.0);
                ensure!(
                    api.FPDF_DeviceToPage(
                        self.page,
                        0,
                        0,
                        DEVICE_GRID,
                        DEVICE_GRID,
                        0,
                        x,
                        y,
                        &mut px,
                        &mut py
                    ) != 0,
                    "Inverse page mapping failed"
                );
                ensure!(
                    px.is_finite() && py.is_finite(),
                    "Nonfinite inverse geometry"
                );
                corners.push((px, py));
            }
            let (bl, br, tl) = (corners[0], corners[1], corners[2]);
            Ok(Geometry {
                normalized: [
                    x0 as f64 / f64::from(DEVICE_GRID),
                    y0 as f64 / f64::from(DEVICE_GRID),
                    (x1 - x0) as f64 / f64::from(DEVICE_GRID),
                    (y1 - y0) as f64 / f64::from(DEVICE_GRID),
                ],
                bounds: [
                    corners.iter().map(|p| p.0).fold(f64::INFINITY, f64::min),
                    corners.iter().map(|p| p.1).fold(f64::INFINITY, f64::min),
                    corners
                        .iter()
                        .map(|p| p.0)
                        .fold(f64::NEG_INFINITY, f64::max),
                    corners
                        .iter()
                        .map(|p| p.1)
                        .fold(f64::NEG_INFINITY, f64::max),
                ],
                image_matrix: [
                    br.0 - bl.0,
                    br.1 - bl.1,
                    tl.0 - bl.0,
                    tl.1 - bl.1,
                    bl.0,
                    bl.1,
                ],
                points,
                block_points: [
                    (x1 - x0) as f64 / f64::from(DEVICE_GRID) * points[0],
                    (y1 - y0) as f64 / f64::from(DEVICE_GRID) * points[1],
                ],
            })
        }
    }

    fn select(&self, geometry: &Geometry) -> Result<Vec<FPDF_PAGEOBJECT>> {
        unsafe {
            let api = self.api;
            ensure!(
                api.FPDFPage_GetAnnotCount(self.page) == 0,
                "REFUSED: annotations require independent review handling"
            );
            let count = api.FPDFPage_CountObjects(self.page);
            ensure!((1..=4000).contains(&count), "REFUSED: invalid object count");
            let [left, bottom, right, top] = geometry.bounds;
            let mut selected = Vec::new();
            for index in 0..count {
                let object = api.FPDFPage_GetObject(self.page, index);
                ensure!(!object.is_null(), "REFUSED: missing page object");
                let kind = api.FPDFPageObj_GetType(object);
                ensure!(
                    matches!(kind, 1..=3),
                    "REFUSED: form/shading or unsupported object"
                );
                ensure!(
                    api.FPDFPageObj_CountMarks(object) == 0,
                    "REFUSED: marked content / alternate extraction text"
                );
                if kind == 1 {
                    ensure!(
                        (0..=3).contains(&api.FPDFTextObj_GetTextRenderMode(object)),
                        "REFUSED: text clipping or unknown render mode"
                    );
                }
                let clip = api.FPDFPageObj_GetClipPath(object);
                ensure!(
                    clip.is_null() || api.FPDFClipPath_CountPaths(clip) < 0,
                    "REFUSED: clipping requires unsupported graphics-state handling"
                );
                let (mut x0, mut y0, mut x1, mut y1) = (0.0, 0.0, 0.0, 0.0);
                ensure!(
                    api.FPDFPageObj_GetBounds(object, &mut x0, &mut y0, &mut x1, &mut y1) != 0,
                    "REFUSED: missing object bounds"
                );
                ensure!(
                    [x0, y0, x1, y1].iter().all(|v| v.is_finite()),
                    "REFUSED: nonfinite bounds"
                );
                let (x0, y0, x1, y1) = (f64::from(x0), f64::from(y0), f64::from(x1), f64::from(y1));
                let intersects = x1 > left && x0 < right && y1 > bottom && y0 < top;
                ensure!(
                    !intersects || (x0 >= left && y0 >= bottom && x1 <= right && y1 <= top),
                    "REFUSED: object crosses region boundary"
                );
                if intersects {
                    selected.push(object);
                }
            }
            ensure!(!selected.is_empty(), "REFUSED: empty block");
            Ok(selected)
        }
    }

    fn bake(&self) -> Result<(Vec<u8>, Geometry, [i32; 2], usize)> {
        let geometry = self.geometry()?;
        let selected = self.select(&geometry)?;
        let width = (geometry.block_points[0] * f64::from(BLOCK_DPI) / 72.0).ceil() as i32;
        let height = (geometry.block_points[1] * f64::from(BLOCK_DPI) / 72.0).ceil() as i32;
        ensure!(
            (1..=4096).contains(&width)
                && (1..=4096).contains(&height)
                && i64::from(width) * i64::from(height) <= 8 * 1024 * 1024,
            "REFUSED: raster budget exceeded"
        );
        // Only generated inputs reach this method. RAII retains every handle and
        // the bitmap until the nonincremental save is complete. No file is mutated.
        unsafe {
            let api = self.api;
            let bitmap = Bitmap {
                api,
                handle: api.FPDFBitmap_CreateEx(width, height, 2, ptr::null_mut(), 0),
            };
            ensure!(!bitmap.handle.is_null(), "Cannot allocate opaque BGR block");
            ensure!(
                api.FPDFBitmap_FillRect(bitmap.handle, 0, 0, width, height, 0xffffffff) != 0,
                "Cannot fill block background"
            );
            let sx = f64::from(width) / geometry.block_points[0];
            let sy = f64::from(height) / geometry.block_points[1];
            let matrix = FS_MATRIX {
                a: sx as f32,
                b: 0.0,
                c: 0.0,
                d: sy as f32,
                e: (-geometry.normalized[0] * geometry.points[0] * sx) as f32,
                f: (-geometry.normalized[1] * geometry.points[1] * sy) as f32,
            };
            let clip = FS_RECTF {
                left: 0.0,
                top: 0.0,
                right: width as f32,
                bottom: height as f32,
            };
            api.FPDF_RenderPageBitmapWithMatrix(bitmap.handle, self.page, &matrix, &clip, 0);
            for object in &selected {
                ensure!(
                    api.FPDFPage_RemoveObject(self.page, *object) != 0,
                    "Object removal failed"
                );
                api.FPDFPageObj_Destroy(*object);
            }
            let mut image = PendingObject {
                api,
                handle: api.FPDFPageObj_NewImageObj(self.handle),
            };
            ensure!(!image.handle.is_null(), "Cannot create combined image");
            ensure!(
                api.FPDFImageObj_SetBitmap(ptr::null_mut(), 0, image.handle, bitmap.handle) != 0,
                "Cannot assign combined raster"
            );
            let [a, b, c, d, e, f] = geometry.image_matrix;
            api.FPDFPageObj_Transform(image.handle, a, b, c, d, e, f);
            api.FPDFPage_InsertObject(self.page, image.handle);
            let count = api.FPDFPage_CountObjects(self.page);
            let inserted =
                (0..count).any(|index| api.FPDFPage_GetObject(self.page, index) == image.handle);
            ensure!(inserted, "Combined image insertion failed");
            image.handle = ptr::null_mut(); // The page now owns it.
            ensure!(
                api.FPDFPage_GenerateContent(self.page) != 0,
                "Cannot regenerate affected page"
            );
            Ok((self.save()?, geometry, [width, height], selected.len()))
        }
    }

    fn remove_combined(&self, geometry: &Geometry) -> Result<Vec<u8>> {
        let selected = self.select(geometry)?;
        ensure!(selected.len() == 1, "Expected one combined object");
        unsafe {
            ensure!(
                self.api.FPDFPageObj_GetType(selected[0]) == 3,
                "Combined object is not an image"
            );
            ensure!(
                self.api.FPDFPage_RemoveObject(self.page, selected[0]) != 0,
                "Cannot remove combined image"
            );
            self.api.FPDFPageObj_Destroy(selected[0]);
            ensure!(
                self.api.FPDFPage_GenerateContent(self.page) != 0,
                "Cannot regenerate removal probe"
            );
        }
        self.save()
    }

    fn save(&self) -> Result<Vec<u8>> {
        let mut writer = Writer {
            interface: FPDF_FILEWRITE {
                version: 1,
                WriteBlock: Some(write_block),
            },
            bytes: Vec::new(),
        };
        // Writer is repr(C), with the callback interface first. It stays pinned
        // on this stack for this synchronous call; callbacks cannot escape it.
        // Derive the pointer from the whole Writer, so the callback retains
        // provenance for its sibling byte-buffer field as well as the interface.
        let result = unsafe {
            self.api
                .FPDF_SaveAsCopy(self.handle, ptr::from_mut(&mut writer).cast(), 2)
        };
        ensure!(
            result != 0 && !writer.bytes.is_empty(),
            "Bounded nonincremental PDF save failed"
        );
        Ok(writer.bytes)
    }
}
impl Drop for Document<'_, '_> {
    fn drop(&mut self) {
        unsafe {
            self.api.FPDF_ClosePage(self.page);
            self.api.FPDF_CloseDocument(self.handle);
        }
    }
}
struct Bitmap<'a> {
    api: &'a dyn PdfiumLibraryBindings,
    handle: FPDF_BITMAP,
}
impl Drop for Bitmap<'_> {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                self.api.FPDFBitmap_Destroy(self.handle);
            }
        }
    }
}
struct PendingObject<'a> {
    api: &'a dyn PdfiumLibraryBindings,
    handle: FPDF_PAGEOBJECT,
}
impl Drop for PendingObject<'_> {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                self.api.FPDFPageObj_Destroy(self.handle);
            }
        }
    }
}

#[repr(C)]
struct Writer {
    interface: FPDF_FILEWRITE,
    bytes: Vec<u8>,
}
unsafe extern "C" fn write_block(
    this: *mut FPDF_FILEWRITE,
    data: *const c_void,
    size: c_ulong,
) -> c_int {
    if this.is_null() {
        return 0;
    }
    // PDFium passes the interface supplied to save(), which is the first field
    // of Writer. The buffer is borrowed only for this callback invocation.
    let writer = unsafe { &mut *this.cast::<Writer>() };
    let Ok(size) = usize::try_from(size) else {
        return 0;
    };
    if size == 0 {
        return 1;
    }
    if data.is_null()
        || size > MAX_SAVED_BYTES.saturating_sub(writer.bytes.len())
        || writer.bytes.try_reserve(size).is_err()
    {
        return 0;
    }
    writer
        .bytes
        .extend_from_slice(unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size) });
    1
}

#[derive(serde::Serialize)]
struct Geometry {
    normalized: [f64; 4],
    bounds: [f64; 4],
    image_matrix: [f64; 6],
    points: [f64; 2],
    block_points: [f64; 2],
}

fn pdf(objects: &[Vec<u8>]) -> Vec<u8> {
    let mut out = b"%PDF-1.7\n%generated dPDF fixture\n".to_vec();
    let mut offsets = vec![0];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        out.extend_from_slice(object);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets.iter().skip(1) {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}
fn stream(bytes: &[u8], extra: &str) -> Vec<u8> {
    let mut out = format!("<< /Length {} {extra} >>\nstream\n", bytes.len()).into_bytes();
    out.extend_from_slice(bytes);
    out.extend_from_slice(b"\nendstream");
    out
}

fn write_new(path: PathBuf, bytes: &[u8]) -> Result<()> {
    std::fs::File::options()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}

// Initial conservative catalog gate only. Page-tree/resource reachability and
// arbitrary-input qualification remain separate, unfinished work.
fn require_basic_catalog(document: &serde_json::Value) -> Result<()> {
    let objects = document["qpdf"][1]
        .as_object()
        .context("Missing catalog objects")?;
    let root = objects
        .get("trailer")
        .and_then(|record| record.get("value"))
        .and_then(|trailer| trailer.get("/Root"))
        .and_then(serde_json::Value::as_str)
        .context("Missing catalog reference")?;
    let catalog = objects
        .get(&format!("obj:{root}"))
        .and_then(|record| record.get("value"))
        .and_then(serde_json::Value::as_object)
        .context("Missing catalog dictionary")?;
    ensure!(
        catalog.get("/Type").and_then(serde_json::Value::as_str) == Some("/Catalog"),
        "REFUSED: unsupported catalog type"
    );
    ensure!(
        catalog
            .keys()
            .all(|key| matches!(key.as_str(), "/Type" | "/Pages")),
        "REFUSED: catalog references require unsupported reachability handling"
    );
    let pages = catalog
        .get("/Pages")
        .and_then(serde_json::Value::as_str)
        .context("Missing page-tree reference")?;
    ensure!(
        objects
            .get(&format!("obj:{pages}"))
            .and_then(|record| record.get("value"))
            .and_then(|value| value.get("/Type"))
            .and_then(serde_json::Value::as_str)
            == Some("/Pages"),
        "REFUSED: unsupported page-tree root"
    );
    Ok(())
}

// Catalog/page/resource keys are intentionally conservative until production
// handling is qualified. Alias checks also reject content retained through known keys.
fn preflight_graph(document: &serde_json::Value) -> Result<()> {
    use serde_json::Value;
    use std::collections::{HashMap, HashSet};
    require_basic_catalog(document)?;
    let objects = document["qpdf"][1]
        .as_object()
        .context("Missing source objects")?;
    let pages = document["pages"]
        .as_array()
        .context("Missing source pages")?;
    let mut owners = HashMap::new();
    for page in pages {
        let owner = page["object"].as_str().context("Missing source page")?;
        for content in page["contents"]
            .as_array()
            .context("Missing source contents")?
        {
            let reference = content
                .as_str()
                .context("Invalid source content reference")?;
            ensure!(
                objects
                    .get(&format!("obj:{reference}"))
                    .and_then(|record| record.get("stream"))
                    .and_then(Value::as_object)
                    .is_some(),
                "REFUSED: indirect content ownership is unsupported"
            );
            ensure!(
                owners.insert(reference, owner).is_none(),
                "REFUSED: shared page content requires unsupported ownership handling"
            );
        }
    }
    fn resolve<'a>(
        mut value: &'a Value,
        objects: &'a serde_json::Map<String, Value>,
    ) -> Result<&'a Value> {
        let mut seen = HashSet::new();
        while let Some(reference) = value.as_str() {
            ensure!(
                seen.len() < 16 && seen.insert(reference),
                "REFUSED: cyclic/deep resource reference"
            );
            value = objects
                .get(&format!("obj:{reference}"))
                .and_then(|record| record.get("value"))
                .context("REFUSED: unresolved source dictionary")?;
        }
        Ok(value)
    }
    for record in objects.values() {
        let Some(dictionary) = record.get("value").and_then(Value::as_object) else {
            continue;
        };
        let kind = dictionary.get("/Type").and_then(Value::as_str);
        if matches!(kind, Some("/Page" | "/Pages")) {
            let allowed: &[&str] = if kind == Some("/Pages") {
                &[
                    "/Type",
                    "/Parent",
                    "/Kids",
                    "/Count",
                    "/MediaBox",
                    "/CropBox",
                    "/Rotate",
                    "/Resources",
                ]
            } else {
                &[
                    "/Type",
                    "/Parent",
                    "/MediaBox",
                    "/CropBox",
                    "/Rotate",
                    "/Resources",
                    "/Contents",
                ]
            };
            ensure!(
                dictionary.keys().all(|key| allowed.contains(&key.as_str())),
                "REFUSED: page/tree properties require unsupported reachability handling"
            );
            if let Some(resources) = dictionary.get("/Resources") {
                let resources = resolve(resources, objects)?
                    .as_object()
                    .context("REFUSED: invalid resource dictionary")?;
                ensure!(
                    resources.keys().all(|key| matches!(
                        key.as_str(),
                        "/Font" | "/XObject" | "/ExtGState" | "/ProcSet"
                    )),
                    "REFUSED: resource properties require unsupported reachability handling"
                );
                for name in ["/Font", "/XObject", "/ExtGState"] {
                    if let Some(table) = resources.get(name) {
                        ensure!(
                            resolve(table, objects)?.is_object(),
                            "REFUSED: invalid resource table"
                        );
                    }
                }
            }
        }
        // Scan below separately, including stream dictionaries and arrays.
    }
    fn references(
        value: &Value,
        permitted: Option<&str>,
        owners: &HashMap<&str, &str>,
        budget: &mut usize,
    ) -> Result<()> {
        ensure!(
            *budget > 0,
            "REFUSED: reference graph exceeds its metadata budget"
        );
        *budget -= 1;
        if let Some(reference) = value.as_str()
            && let Some(owner) = owners.get(reference)
        {
            ensure!(
                permitted == Some(*owner),
                "REFUSED: page content is retained beyond its owner"
            );
        }
        match value {
            Value::Array(items) => {
                for item in items {
                    references(item, permitted, owners, budget)?;
                }
            }
            Value::Object(items) => {
                for item in items.values() {
                    references(item, permitted, owners, budget)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    let mut budget = 1_000_000;
    for (key, record) in objects {
        let value = record
            .get("value")
            .or_else(|| record.get("stream").and_then(|stream| stream.get("dict")));
        let Some(value) = value else {
            continue;
        };
        if let Some(dictionary) = value.as_object() {
            for (field, value) in dictionary {
                let permitted = if field == "/Contents" {
                    key.strip_prefix("obj:")
                } else {
                    None
                };
                references(value, permitted, &owners, &mut budget)?;
            }
        } else {
            references(value, None, &owners, &mut budget)?;
        }
    }
    Ok(())
}

fn fixture(case: &str) -> Result<Vec<u8>> {
    let index = case.parse::<u8>().ok().filter(|n| *n < 32);
    ensure!(
        index.is_some()
            || matches!(
                case,
                "crossing-path"
                    | "crossing-text"
                    | "marked"
                    | "text-clip"
                    | "path-clip"
                    | "annotation"
                    | "form"
                    | "empty"
                    | "catalog-stream"
                    | "catalog-attachment"
                    | "page-tree-stream"
                    | "untouched-page-pieceinfo"
                    | "untouched-page-resources"
                    | "shared-contents"
                    | "indirect-resources"
            ),
        "Choose a generated case 0..31 or a documented fixture"
    );
    let id = index.unwrap_or(0);
    let media = if id & 16 != 0 {
        "[-30 -20 370 280]"
    } else {
        "[0 0 400 300]"
    };
    let crop = if id & 8 != 0 {
        if id & 16 != 0 {
            "/CropBox [-10 0 350 260]"
        } else {
            "/CropBox [20 20 380 280]"
        }
    } else {
        ""
    };
    let boxes = format!(
        "/MediaBox {media} {crop} /Rotate {}",
        u16::from(id % 4) * 90
    );
    let inherited = id & 4 != 0;
    let standard=b"BT /F1 16 Tf 30 240 Td (OUTSIDE searchable text) Tj ET\nBT /F1 10 Tf 100 105 Td (BAKE_LABEL Authorized person:) Tj ET\n100 90 m 270 90 l S\nBT /F1 10 Tf 100 70 Td (BAKE_DATE 2026) Tj ET\nq 70 0 0 20 125 95 cm /Sig Do Q\nq 40 0 0 50 205 75 cm /Stamp Do Q\n";
    let content=match case {
        "crossing-path"=>b"20 90 m 380 90 l S".to_vec(),
        "crossing-text"=>b"BT /F1 10 Tf 20 100 Td (OUTSIDE BEFORE LABEL AFTER OUTSIDE) Tj ET".to_vec(),
        "text-clip"=>b"BT /F1 16 Tf 30 240 Td (OUTSIDE) Tj ET q BT /F1 20 Tf 100 100 Td 7 Tr (HIDDEN_CLIP) Tj ET 30 240 10 10 re f 100 80 100 50 re f Q".to_vec(),
        "path-clip"=>b"q 90 60 190 80 re W n 0 0 400 300 re f Q".to_vec(),
        "marked"=>[b"/Span << /ActualText (OUTSIDE BAKE_LABEL) >> BDC ".as_slice(),standard,b" EMC"].concat(),
        "form"=>b"q 1 0 0 1 100 80 cm /Fm Do Q".to_vec(),
        "empty"=>b"BT /F1 16 Tf 30 240 Td (OUTSIDE only) Tj ET".to_vec(),
        _=>standard.to_vec(),
    };
    let resources = format!(
        "/Resources << /Font << /F1 4 0 R >> /XObject << /Sig 8 0 R /Stamp 10 0 R {} >> >>",
        if case == "form" { "/Fm 12 0 R" } else { "" }
    );
    let mut objects=vec![b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        format!("<< /Type /Pages /Count 2 /Kids [3 0 R 6 0 R] {} >>",if inherited {boxes.as_str()} else {""}).into_bytes(),
        format!("<< /Type /Page /Parent 2 0 R {} {resources} /Contents 5 0 R {} >>",if inherited {""} else {boxes.as_str()},if case=="annotation" {"/Annots [13 0 R]"} else {""}).into_bytes(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),stream(&content,""),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 300] /CropBox [0 0 400 300] /Rotate 0 /Resources << /Font << /F1 4 0 R >> >> /Contents 7 0 R >>".to_vec(),
        stream(b"BT /F1 16 Tf 30 240 Td (UNTOUCHED second page) Tj ET",""),];
    for (stamp, color) in [(false, [20, 70, 160]), (true, [170, 25, 35])] {
        let mut rgb = Vec::new();
        let mut alpha = Vec::new();
        for y in 0i32..32 {
            for x in 0i32..32 {
                rgb.extend_from_slice(&color);
                let ink = if stamp {
                    let d = (x - 16).pow(2) + (y - 16).pow(2);
                    (140..200).contains(&d)
                } else {
                    (y - x / 2 - 5).abs() < 2 || (y + x / 2 - 24).abs() < 2
                };
                alpha.push(if ink { 170 } else { 0 });
            }
        }
        let mask = objects.len() + 2;
        objects.push(stream(&rgb,&format!("/Type /XObject /Subtype /Image /Width 32 /Height 32 /ColorSpace /DeviceRGB /BitsPerComponent 8 /SMask {mask} 0 R")));
        objects.push(stream(&alpha,"/Type /XObject /Subtype /Image /Width 32 /Height 32 /ColorSpace /DeviceGray /BitsPerComponent 8"));
    }
    objects.push(stream(
        b"BT /F1 10 Tf 0 0 Td (BAKE_LABEL) Tj ET",
        "/Type /XObject /Subtype /Form /BBox [0 -20 100 20] /Resources << /Font << /F1 4 0 R >> >>",
    ));
    objects.push(
        b"<< /Type /Annot /Subtype /Text /Rect [100 80 120 100] /Contents (Independent review) >>"
            .to_vec(),
    );
    if case == "catalog-stream" {
        objects[0] = b"<< /Type /Catalog /Pages 2 0 R /DPDFProbeRetainedContent 5 0 R >>".to_vec();
    } else if case == "catalog-attachment" {
        objects[0] = b"<< /Type /Catalog /Pages 2 0 R /Names << /EmbeddedFiles << /Names [(backup) 14 0 R] >> >> >>".to_vec();
        objects.push(b"<< /Type /Filespec /F (block-backup.txt) /EF << /F 15 0 R >> >>".to_vec());
        objects.push(stream(&content, "/Type /EmbeddedFile"));
    }
    if case == "page-tree-stream" {
        objects[1] = [
            b"<< /DPDFProbeRetainedContent 5 0 R ".as_slice(),
            &objects[1][3..],
        ]
        .concat();
    } else if case == "untouched-page-pieceinfo" {
        objects[5] = [
            b"<< /PieceInfo << /Probe << /LastModified (D:20261030000000Z) /Private 5 0 R >> >> "
                .as_slice(),
            &objects[5][3..],
        ]
        .concat();
    } else if case == "untouched-page-resources" {
        objects[5] = String::from_utf8(objects[5].clone())?
            .replacen(
                "/Resources << ",
                "/Resources << /DPDFProbeRetainedContent 5 0 R ",
                1,
            )
            .into_bytes();
    } else if case == "shared-contents" {
        objects[5] = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 300] {resources} /Contents 5 0 R >>"
        )
        .into_bytes();
    } else if case == "indirect-resources" {
        objects[2] =
            format!("<< /Type /Page /Parent 2 0 R {boxes} /Resources 14 0 R /Contents 5 0 R >>")
                .into_bytes();
        objects.push(
            resources
                .strip_prefix("/Resources ")
                .unwrap()
                .as_bytes()
                .to_vec(),
        );
    }
    Ok(pdf(&objects))
}

pub fn run_cli(mut args: impl Iterator<Item = std::ffi::OsString>) -> Result<()> {
    let output = PathBuf::from(
        args.next()
            .context("--probe-block NEW_DIRECTORY GENERATED_CASE")?,
    );
    let case = args
        .next()
        .context("Choose a generated case")?
        .into_string()
        .map_err(|_| anyhow::anyhow!("Case must be Unicode"))?;
    ensure!(
        args.next().is_none(),
        "--probe-block accepts no PDF/image input paths"
    );
    let bytes = fixture(&case)?;
    std::fs::create_dir(&output)
        .context("Choose a new directory; existing output is never overwritten")?;
    write_new(output.join("source.pdf"), &bytes)?;
    let mut report = serde_json::json!({"scope":"generated_rust_block_probe_only","qualified_for_secure_export":false,"dPDF_secure_export_enabled":false,"case":case,"status":"INCOMPLETE","binary_version":env!("CARGO_PKG_VERSION")});
    let report_path = output.join("report.json");
    crate::storage::atomic_write(&serde_json::to_vec_pretty(&report)?, &report_path)?;
    let engine = Engine::new()?;
    let document = Document::open(engine.api.as_ref(), &bytes)?;
    let metadata = crate::assembly::inspect_metadata(&output.join("source.pdf"))?;
    match preflight_graph(&metadata).and_then(|()| document.bake()) {
        Ok((baked, geometry, pixels, removed)) => {
            write_new(output.join("raw.pdf"), &baked)?;
            let clean = Document::open(engine.api.as_ref(), &baked)?;
            let mut removal_geometry = clean.geometry()?;
            for index in 0..4 {
                removal_geometry.bounds[index] += if index < 2 { -0.02 } else { 0.02 };
            }
            write_new(
                output.join("removed.pdf"),
                &clean.remove_combined(&removal_geometry)?,
            )?;
            report["status"] = "PROBE_COMPLETED".into();
            report["geometry"] = serde_json::to_value(geometry)?;
            report["block_pixels"] = serde_json::json!(pixels);
            report["removed_objects"] = removed.into();
            report["nominal_dpi"] = BLOCK_DPI.into();
        }
        Err(error) if error.to_string().starts_with("REFUSED:") => {
            report["status"] = "REFUSED".into();
            report["reason"] = error.to_string().into();
        }
        Err(error) => return Err(error),
    }
    ensure!(
        std::fs::read(output.join("source.pdf"))? == bytes,
        "Generated source changed"
    );
    report["source_unchanged"] = true.into();
    crate::storage::atomic_write(&serde_json::to_vec_pretty(&report)?, &report_path)?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph_metadata() -> serde_json::Value {
        serde_json::json!({"pages":[{"object":"3 0 R","contents":["4 0 R"]}],"qpdf":[{}, {
            "trailer":{"value":{"/Root":"1 0 R"}},
            "obj:1 0 R":{"value":{"/Type":"/Catalog","/Pages":"2 0 R"}},
            "obj:2 0 R":{"value":{"/Type":"/Pages","/Count":1,"/Kids":["3 0 R"]}},
            "obj:3 0 R":{"value":{"/Type":"/Page","/Parent":"2 0 R","/Contents":"4 0 R","/Resources":{"/Font":{"/F1":"5 0 R"}}}},
            "obj:4 0 R":{"stream":{"dict":{"/Length":0}}},
            "obj:5 0 R":{"value":{"/Type":"/Font","/Subtype":"/Type1","/BaseFont":"/Helvetica"}}
        }]})
    }

    #[test]
    fn content_aliases_under_known_resource_keys_are_refused() {
        let mut metadata = graph_metadata();
        assert!(preflight_graph(&metadata).is_ok());
        metadata["qpdf"][1]["obj:3 0 R"]["value"]["/Resources"]["/ExtGState"] =
            serde_json::json!({"/Unused":"4 0 R"});
        assert!(
            preflight_graph(&metadata)
                .unwrap_err()
                .to_string()
                .contains("beyond its owner")
        );
    }

    #[test]
    fn cyclic_resource_dictionaries_are_refused_without_recursing_forever() {
        let mut metadata = graph_metadata();
        metadata["qpdf"][1]["obj:3 0 R"]["value"]["/Resources"] = "6 0 R".into();
        metadata["qpdf"][1]["obj:6 0 R"] = serde_json::json!({"value":"6 0 R"});
        assert!(
            preflight_graph(&metadata)
                .unwrap_err()
                .to_string()
                .contains("cyclic/deep")
        );
    }

    #[test]
    fn save_callback_bounds_and_generated_input_contract_are_enforced() {
        let mut writer = Writer {
            interface: FPDF_FILEWRITE {
                version: 1,
                WriteBlock: Some(write_block),
            },
            bytes: vec![],
        };
        let bytes = [17u8, 42];
        unsafe {
            assert_eq!(
                write_block(ptr::from_mut(&mut writer).cast(), bytes.as_ptr().cast(), 2),
                1
            );
            assert_eq!(writer.bytes, bytes);
            assert_eq!(write_block(ptr::null_mut(), bytes.as_ptr().cast(), 2), 0);
            assert_eq!(
                write_block(ptr::from_mut(&mut writer).cast(), ptr::null(), 1),
                0
            );
            assert_eq!(
                write_block(
                    ptr::from_mut(&mut writer).cast(),
                    bytes.as_ptr().cast(),
                    (MAX_SAVED_BYTES + 1) as c_ulong
                ),
                0
            );
            assert_eq!(writer.bytes, bytes);
        }
        assert!(fixture("some-contract.pdf").is_err());
        assert!(fixture("32").is_err());
        for case in 0..32 {
            assert!(fixture(&case.to_string()).unwrap().starts_with(b"%PDF-1.7"));
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("retained.pdf");
        std::fs::write(&path, b"original").unwrap();
        assert!(write_new(path.clone(), b"replacement").is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"original");
    }
}
