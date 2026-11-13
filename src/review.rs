//! Independent review revisions and interoperable PDF annotations. Never signature content.
use crate::{
    assembly::{ExportResult, SourceSnapshot, path_string, qpdf_executable},
    document::NormalizedRect,
};
use anyhow::{Context, Result, bail};
use pdfium_render::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

const MAX_MODEL: usize = 1024 * 1024;
const MAX_REMARKS: usize = 1000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revision {
    pub id: u32,
    pub label: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Note,
    Highlight,
    Underline,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Remark {
    pub id: u32,
    pub page: usize,
    pub revision: u32,
    pub kind: Kind,
    pub bounds: NormalizedRect,
    pub text: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Filter {
    All,
    Current,
    Selected(Vec<u32>),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub schema_version: u32,
    pub workspace: String,
    pub revisions: Vec<Revision>,
    pub current: u32,
    pub visible: bool,
    pub filter: Filter,
    pub remarks: Vec<Remark>,
}
impl Default for Review {
    fn default() -> Self {
        Self {
            schema_version: 1,
            workspace: uuid::Uuid::new_v4().to_string(),
            revisions: vec![Revision {
                id: 1,
                label: "Revision 1".into(),
            }],
            current: 1,
            visible: true,
            filter: Filter::All,
            remarks: vec![],
        }
    }
}
fn valid_bounds(b: NormalizedRect) -> bool {
    [b.x, b.y, b.width, b.height].iter().all(|n| n.is_finite())
        && b.x >= 0.0
        && b.y >= 0.0
        && b.width > 0.0
        && b.height > 0.0
        && b.x + b.width <= 1.00001
        && b.y + b.height <= 1.00001
}
impl Review {
    /// Content of `snapshot` (revisions and remarks) with the view state of `present`:
    /// visibility, the active revision and the filter stay as the user last set them,
    /// dropping references to revisions the snapshot does not contain.
    pub fn restored_from(snapshot: &Review, present: &Review) -> Review {
        let exists = |id: u32| snapshot.revisions.iter().any(|r| r.id == id);
        let mut restored = snapshot.clone();
        restored.visible = present.visible;
        restored.schema_version = snapshot.schema_version.max(present.schema_version);
        // A step that adds/removes revisions also restores which one is active.
        if snapshot.revisions == present.revisions {
            restored.current = present.current;
        }
        restored.filter = match &present.filter {
            Filter::Selected(ids) => {
                let kept: Vec<u32> = ids.iter().copied().filter(|id| exists(*id)).collect();
                if kept.is_empty() {
                    Filter::All
                } else {
                    Filter::Selected(kept)
                }
            }
            other => other.clone(),
        };
        restored
    }
    /// True when revisions or remarks differ (view state is not document content).
    pub fn content_differs(&self, other: &Review) -> bool {
        self.revisions != other.revisions || self.remarks != other.remarks
    }
    pub fn validate(&self, pages: usize) -> Result<()> {
        if !matches!(self.schema_version, 1 | 2)
            || uuid::Uuid::try_parse(&self.workspace).is_err()
            || self.revisions.is_empty()
            || self.revisions.len() > 64
            || self.remarks.len() > MAX_REMARKS
        {
            bail!("Unsupported or oversized review model");
        }
        if matches!(self.filter, Filter::Current) && self.schema_version < 2 {
            bail!("Current-revision filters require review metadata version 2");
        }
        let mut revisions = HashSet::new();
        let mut labels = HashSet::new();
        for revision in &self.revisions {
            if revision.id == 0
                || revision.id > 1_000_000
                || revision.label.trim().is_empty()
                || revision.label.len() > 128
                || !revisions.insert(revision.id)
                || !labels.insert(&revision.label)
            {
                bail!("Invalid or duplicate review revision");
            }
        }
        if !revisions.contains(&self.current) {
            bail!("Active revision does not exist");
        }
        if let Filter::Selected(ids) = &self.filter {
            let mut seen = HashSet::new();
            if ids
                .iter()
                .any(|id| !revisions.contains(id) || !seen.insert(*id))
            {
                bail!("Invalid review visibility filter");
            }
        }
        let mut remarks = HashSet::new();
        for mark in &self.remarks {
            if mark.id == 0
                || mark.id > 1_000_000
                || !remarks.insert(mark.id)
                || mark.page >= pages
                || !revisions.contains(&mark.revision)
                || !valid_bounds(mark.bounds)
                || mark.text.len() > 4096
            {
                bail!("Invalid or oversized review remark");
            }
        }
        if serde_json::to_vec(self)?.len() > MAX_MODEL {
            bail!("Review metadata exceeds 1 MiB");
        }
        Ok(())
    }
    pub fn new_revision(&mut self, label: &str) -> Result<u32> {
        let label = label.trim();
        if self.revisions.len() >= 64
            || label.is_empty()
            || label.len() > 128
            || self.revisions.iter().any(|r| r.label == label)
        {
            bail!("Choose a unique revision label of 1–128 bytes (up to 64 revisions)");
        }
        let id = self.revisions.iter().map(|r| r.id).max().unwrap_or(0) + 1;
        if id > 1_000_000 {
            bail!("Revision identifier limit reached");
        }
        self.revisions.push(Revision {
            id,
            label: label.into(),
        });
        self.current = id;
        Ok(id)
    }
    pub fn add(
        &mut self,
        page: usize,
        bounds: NormalizedRect,
        kind: Kind,
        text: &str,
        pages: usize,
    ) -> Result<u32> {
        if kind == Kind::Note && text.trim().is_empty() {
            bail!("Enter a note");
        }
        let id = self.remarks.iter().map(|r| r.id).max().unwrap_or(0) + 1;
        let remark = Remark {
            id,
            page,
            revision: self.current,
            kind,
            bounds,
            text: text.into(),
        };
        self.remarks.push(remark);
        if let Err(error) = self.validate(pages) {
            self.remarks.pop();
            return Err(error);
        }
        Ok(id)
    }
    pub fn filter_for_positions(&self, positions: &str) -> Result<Filter> {
        let mut selected = vec![];
        for part in positions.split(',') {
            let position = part
                .trim()
                .parse::<usize>()
                .context("Enter revision positions such as 1,3")?;
            let revision = position
                .checked_sub(1)
                .and_then(|i| self.revisions.get(i))
                .context("Selected revision does not exist")?;
            if !selected.contains(&revision.id) {
                selected.push(revision.id);
            }
        }
        Ok(Filter::Selected(selected))
    }
    pub fn is_visible(&self, remark: &Remark) -> bool {
        self.visible
            && match &self.filter {
                Filter::All => true,
                Filter::Current => remark.revision == self.current,
                Filter::Selected(ids) => ids.contains(&remark.revision),
            }
    }
    fn prefix(&self) -> String {
        format!("dpdf-review:{}:", self.workspace)
    }
    pub fn annotation_name(&self, id: u32) -> String {
        format!("{}{id}", self.prefix())
    }
}
#[derive(Clone, Copy)]
pub enum ExportMode {
    Visible,
    Current,
    Clean,
}
impl ExportMode {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "visible" => Ok(Self::Visible),
            "current" => Ok(Self::Current),
            "clean" => Ok(Self::Clean),
            _ => bail!("Choose visible, current or clean review export"),
        }
    }
}
#[derive(Clone)]
pub struct NativeGeometry {
    rect: [f32; 4],
    quad: [f32; 8],
}
fn native_geometry(page: &PdfPage<'_>, bounds: NormalizedRect) -> Result<NativeGeometry> {
    if !valid_bounds(bounds) {
        bail!("Review bounds must lie inside the page");
    }
    let config = PdfRenderConfig::new().set_fixed_size(1_000_000, 1_000_000);
    let corners = [
        (bounds.x, bounds.y),
        (bounds.x + bounds.width, bounds.y),
        (bounds.x, bounds.y + bounds.height),
        (bounds.x + bounds.width, bounds.y + bounds.height),
    ];
    let mut quad = [0.0; 8];
    for (index, (x, y)) in corners.into_iter().enumerate() {
        let (x, y) = page.pixels_to_points(
            (x * 1_000_000.0).round() as i32,
            (y * 1_000_000.0).round() as i32,
            &config,
        )?;
        quad[index * 2] = x.value;
        quad[index * 2 + 1] = y.value;
    }
    let xs = [quad[0], quad[2], quad[4], quad[6]];
    let ys = [quad[1], quad[3], quad[5], quad[7]];
    Ok(NativeGeometry {
        rect: [
            xs.into_iter().fold(f32::INFINITY, f32::min),
            ys.into_iter().fold(f32::INFINITY, f32::min),
            xs.into_iter().fold(f32::NEG_INFINITY, f32::max),
            ys.into_iter().fold(f32::NEG_INFINITY, f32::max),
        ],
        quad,
    })
}
fn view_bounds(page: &PdfPage<'_>, rect: [f32; 4]) -> Result<NormalizedRect> {
    let config = PdfRenderConfig::new().set_fixed_size(1_000_000, 1_000_000);
    let corners = [
        (rect[0], rect[1]),
        (rect[0], rect[3]),
        (rect[2], rect[1]),
        (rect[2], rect[3]),
    ];
    let (mut left, mut top, mut right, mut bottom) = (1.0f32, 1.0f32, 0.0f32, 0.0f32);
    for (x, y) in corners {
        let (x, y) = page.points_to_pixels(PdfPoints::new(x), PdfPoints::new(y), &config)?;
        let (x, y) = (x as f32 / 1_000_000.0, y as f32 / 1_000_000.0);
        left = left.min(x);
        top = top.min(y);
        right = right.max(x);
        bottom = bottom.max(y);
    }
    let bounds = NormalizedRect {
        x: left.max(0.0),
        y: top.max(0.0),
        width: right.min(1.0) - left.max(0.0),
        height: bottom.min(1.0) - top.max(0.0),
    };
    if !valid_bounds(bounds) {
        bail!("Native remark moved outside the page");
    }
    Ok(bounds)
}

fn checked(command: &mut Command, action: &str) -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        checked_with_limits(
            command,
            action,
            crate::helper_process::Limits {
                timeout: std::time::Duration::from_secs(60),
                stdout: 64 * 1024,
                stderr: 0,
            },
        )
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let output = command
            .output()
            .with_context(|| format!("Start qpdf to {action}"))?;
        if !output.status.success() {
            bail!(
                "qpdf could not {action} (exit {}); previous files preserved",
                output.status
            );
        }
        Ok(())
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn checked_with_limits(
    command: &mut Command,
    action: &str,
    limits: crate::helper_process::Limits,
) -> Result<()> {
    let output = crate::helper_process::run(command, b"", limits, None)
        .with_context(|| format!("Run qpdf to {action}"))?;
    if !output.status.success() {
        bail!(
            "qpdf could not {action} (exit {}); previous files preserved",
            output.status
        );
    }
    Ok(())
}

fn inspect(path: &Path) -> Result<Value> {
    crate::assembly::inspect_metadata(path)
}
fn objects(document: &Value) -> Result<&Map<String, Value>> {
    document["qpdf"][1]
        .as_object()
        .context("Missing qpdf objects")
}
fn catalog(objects: &Map<String, Value>) -> Result<(&str, &Value)> {
    let root = objects
        .get("trailer")
        .and_then(|t| t["value"]["/Root"].as_str())
        .context("Missing PDF catalog reference")?;
    let catalog = objects
        .get(&format!("obj:{root}"))
        .map(|o| &o["value"])
        .context("Missing PDF catalog")?;
    if !catalog.is_object() {
        bail!("Invalid PDF catalog");
    }
    Ok((root, catalog))
}
fn resolve<'a>(objects: &'a Map<String, Value>, mut value: &'a Value) -> Result<&'a Value> {
    for _ in 0..8 {
        let Some(reference) = value.as_str().filter(|s| s.ends_with(" R")) else {
            return Ok(value);
        };
        value = objects
            .get(&format!("obj:{reference}"))
            .map(|o| &o["value"])
            .context("Missing indirect review object")?;
    }
    bail!("Cyclic or deeply indirect review object")
}
fn text(value: &Value) -> Option<&str> {
    value.as_str()?.strip_prefix("u:")
}
fn stored(document: &Value) -> Result<Option<Review>> {
    let (_, root) = catalog(objects(document)?)?;
    let Some(value) = root.get("/dPDFReview") else {
        return Ok(None);
    };
    let source = text(value).context("Unsupported dPDF review metadata string")?;
    if source.len() > MAX_MODEL {
        bail!("Review metadata exceeds 1 MiB");
    }
    let model: Review = serde_json::from_str(source).context("Read dPDF review revisions")?;
    model.validate(100_000)?;
    Ok(Some(model))
}
fn annotations(document: &Value, page: usize) -> Result<Vec<&Value>> {
    let objects = objects(document)?;
    let reference = document["pages"][page]["object"]
        .as_str()
        .context("Missing page reference")?;
    let page = objects
        .get(&format!("obj:{reference}"))
        .map(|v| &v["value"])
        .context("Missing page dictionary")?;
    let Some(annotations) = page.get("/Annots") else {
        return Ok(vec![]);
    };
    Ok(resolve(objects, annotations)?
        .as_array()
        .context("Invalid page annotation array")?
        .iter()
        .collect())
}
pub(crate) fn from_metadata(document: &Value, pdf: &PdfDocument<'_>) -> Result<Option<Review>> {
    let Some(mut model) = stored(document)? else {
        return Ok(None);
    };
    let objects = objects(document)?;
    let prefix = model.prefix();
    let mut native = HashMap::new();
    for page in 0..pdf.pages().len() as usize {
        for annotation in annotations(document, page)? {
            let value = resolve(objects, annotation)?;
            if let Some(name) = text(&value["/NM"]).filter(|name| name.starts_with(&prefix))
                && native.insert(name.to_string(), (page, value)).is_some()
            {
                bail!("Duplicate native remark identifiers; existing annotations were preserved");
            }
        }
    }
    model.remarks.retain_mut(|remark| {
        let Some((page, value)) = native.get(&format!("{prefix}{}", remark.id)) else {
            return false;
        };
        remark.page = *page;
        if let Some(contents) = text(&value["/Contents"]) {
            remark.text = contents.into();
        }
        true
    });
    for remark in &mut model.remarks {
        let (_, value) = native[&format!("{prefix}{}", remark.id)];
        let numbers = value["/Rect"]
            .as_array()
            .filter(|a| a.len() == 4)
            .context("Invalid native remark rectangle")?;
        let mut rect = [0.0; 4];
        for i in 0..4 {
            rect[i] = numbers[i].as_f64().context("Invalid remark coordinate")? as f32;
        }
        remark.bounds = view_bounds(&pdf.pages().get(remark.page as i32)?, rect)?;
    }
    model.validate(pdf.pages().len() as usize)?;
    Ok(Some(model))
}
pub fn read(source: &SourceSnapshot, pdf: &PdfDocument<'_>) -> Result<Option<Review>> {
    from_metadata(
        &crate::assembly::inspect_snapshot(source.bytes.as_slice())?,
        pdf,
    )
}

fn annotation_flags(value: &Value, visible: bool) -> u64 {
    let original = value["/F"].as_u64().unwrap_or(0);
    if visible {
        (original & !(2 | 32)) | 4
    } else {
        (original & !4) | 2 | 32
    }
}

fn update(
    document: &Value,
    model: &Review,
    mode: ExportMode,
    geometry: &[NativeGeometry],
) -> Result<Value> {
    let objects = objects(document)?;
    let (root, catalog) = catalog(objects)?;
    let pages = document["pages"].as_array().context("Missing PDF pages")?;
    model.validate(pages.len())?;
    let prior = stored(document)?;
    if let Some(prior) = &prior
        && prior.workspace != model.workspace
    {
        bail!("Review workspace differs from the saved document; load its revisions first");
    }
    if !matches!(mode, ExportMode::Clean) && geometry.len() != model.remarks.len() {
        bail!("Missing review geometry");
    }
    let clean = matches!(mode, ExportMode::Clean);
    let mut saved = model.clone();
    if matches!(mode, ExportMode::Visible) {
        saved.visible = true;
        saved.filter = Filter::All;
    }
    let wanted: HashMap<_, _> = if clean {
        HashMap::new()
    } else {
        saved
            .remarks
            .iter()
            .map(|r| (saved.annotation_name(r.id), r))
            .collect()
    };
    let prefix = model.prefix();
    let mut owners: HashMap<String, String> = HashMap::new();
    let mut main = HashMap::new();
    let mut all = vec![];
    for page in 0..pages.len() {
        for (index, annotation) in annotations(document, page)?.into_iter().enumerate() {
            let value = resolve(objects, annotation)?;
            if let Some(name) = text(&value["/NM"]).filter(|name| name.starts_with(&prefix)) {
                let reference = annotation.as_str().map(str::to_string);
                if let Some(reference) = &reference {
                    owners.insert(reference.clone(), name.into());
                }
                if let Some(popup) = value["/Popup"].as_str() {
                    owners.insert(popup.into(), name.into());
                }
                if main
                    .insert(name.to_string(), (reference, value.clone()))
                    .is_some()
                {
                    bail!("Duplicate native remark identifiers; input preserved");
                }
            }
            all.push((page, index, annotation, value));
        }
    }
    loop {
        let before = owners.len();
        for &(_, _, annotation, value) in &all {
            let owner = ["/Parent", "/IRT"]
                .iter()
                .find_map(|key| value[*key].as_str().and_then(|r| owners.get(r)))
                .cloned();
            if let Some(owner) = owner {
                if let Some(reference) = annotation.as_str() {
                    owners.insert(reference.into(), owner.clone());
                }
                if let Some(popup) = value["/Popup"].as_str() {
                    owners.insert(popup.into(), owner);
                }
            }
        }
        if owners.len() == before {
            break;
        }
        if owners.len() > 10_000 {
            bail!("Review reply graph exceeds its limit");
        }
    }
    let mut changes = Map::new();
    // Keep retained native object references stable so popup/reply links survive edits.
    // Deleted dictionaries become null, including objects reachable outside /Annots.
    for (reference, owner) in &owners {
        if let Some(remark) = wanted.get(owner) {
            if let Some(original) = objects
                .get(&format!("obj:{reference}"))
                .map(|o| &o["value"])
            {
                let mut value = original.clone();
                value
                    .as_object_mut()
                    .context("Invalid native review dictionary")?;
                value["/F"] = json!(annotation_flags(&value, saved.is_visible(remark)));
                value["/P"] = pages[remark.page]["object"].clone();
                changes.insert(format!("obj:{reference}"), json!({"value":value}));
            }
        } else {
            changes.insert(format!("obj:{reference}"), json!({"value":null}));
        }
    }
    let mut arrays = vec![vec![]; pages.len()];
    for &(page, _, annotation, value) in &all {
        if text(&value["/NM"]).is_some_and(|name| name.starts_with(&prefix)) {
            continue;
        }
        let owner = annotation.as_str().and_then(|r| owners.get(r)).or_else(|| {
            ["/Parent", "/IRT"]
                .iter()
                .find_map(|key| value[*key].as_str().and_then(|r| owners.get(r)))
        });
        if let Some(owner) = owner {
            if let Some(remark) = wanted.get(owner) {
                let kept = if annotation.is_string() {
                    annotation.clone()
                } else {
                    let mut value = value.clone();
                    value["/F"] = json!(annotation_flags(&value, saved.is_visible(remark)));
                    value["/P"] = pages[remark.page]["object"].clone();
                    value
                };
                arrays[remark.page].push(kept);
            }
        } else {
            arrays[page].push(annotation.clone());
        }
    }
    let mut next = document["qpdf"][0]["maxobjectid"]
        .as_u64()
        .context("Missing PDF object count")?
        + 1;
    if next > 10_000_000 {
        bail!("PDF object identifier range is too large for review editing");
    }
    if !matches!(mode, ExportMode::Clean) {
        for (remark, g) in saved.remarks.iter().zip(geometry) {
            let name = saved.annotation_name(remark.id);
            let existing = main.get(&name);
            let reference = if let Some((Some(reference), _)) = existing {
                reference.clone()
            } else {
                let reference = format!("{next} 0 R");
                next += 1;
                reference
            };
            let subtype = match remark.kind {
                Kind::Note => "/Text",
                Kind::Highlight => "/Highlight",
                Kind::Underline => "/Underline",
            };
            let revision = saved
                .revisions
                .iter()
                .find(|r| r.id == remark.revision)
                .unwrap();
            let managed = json!({"/Type":"/Annot","/Subtype":subtype,"/P":pages[remark.page]["object"],"/Rect":g.rect,
                "/Contents":format!("u:{}",remark.text),"/NM":format!("u:{name}"),"/Subj":format!("u:{}",revision.label),
                "/C":if remark.kind==Kind::Underline {json!([0.15,0.35,0.85])}else{json!([1.0,0.8,0.0])},"/CA":if remark.kind==Kind::Highlight {0.35}else{1.0}});
            let mut annotation = existing
                .map(|(_, value)| value.clone())
                .unwrap_or_else(|| json!({"/T":"u:dPDF"}));
            let dictionary = annotation
                .as_object_mut()
                .context("Invalid native review dictionary")?;
            dictionary.remove("/AP");
            dictionary.remove("/QuadPoints");
            dictionary.extend(managed.as_object().unwrap().clone());
            annotation["/F"] = json!(annotation_flags(&annotation, saved.is_visible(remark)));
            if remark.kind == Kind::Note {
                annotation["/Name"] = json!("/Comment");
            } else {
                annotation["/QuadPoints"] = json!(g.quad);
            }
            changes.insert(format!("obj:{reference}"), json!({"value":annotation}));
            arrays[remark.page].push(json!(reference));
        }
    }
    let mut catalog = catalog.clone();
    if matches!(mode, ExportMode::Clean) {
        catalog.as_object_mut().unwrap().remove("/dPDFReview");
    } else {
        catalog["/dPDFReview"] = json!(format!("u:{}", serde_json::to_string(&saved)?));
    }
    changes.insert(format!("obj:{root}"), json!({"value":catalog}));
    for (page, annotations) in arrays.into_iter().enumerate() {
        let reference = pages[page]["object"]
            .as_str()
            .context("Missing page reference")?;
        let mut dictionary = objects[&format!("obj:{reference}")]["value"].clone();
        if annotations.is_empty() {
            dictionary
                .as_object_mut()
                .context("Invalid page dictionary")?
                .remove("/Annots");
        } else {
            dictionary["/Annots"] = json!(annotations);
        }
        changes.insert(format!("obj:{reference}"), json!({"value":dictionary}));
    }
    Ok(json!({"qpdf":[{"jsonversion":2},changes]}))
}

pub struct Prepared {
    source: SourceSnapshot,
    model: Review,
    mode: ExportMode,
    geometry: Vec<NativeGeometry>,
    pages: usize,
}
pub fn prepare(
    source: SourceSnapshot,
    pdf: &PdfDocument<'_>,
    model: &Review,
    mode: ExportMode,
) -> Result<Prepared> {
    let pages = pdf.pages().len() as usize;
    model.validate(pages)?;
    let geometry = if matches!(mode, ExportMode::Clean) {
        vec![]
    } else {
        model
            .remarks
            .iter()
            .map(|r| native_geometry(&pdf.pages().get(r.page as i32)?, r.bounds))
            .collect::<Result<_>>()?
    };
    Ok(Prepared {
        source,
        model: model.clone(),
        mode,
        geometry,
        pages,
    })
}
impl Prepared {
    pub fn set_source_bytes(&mut self, bytes: Arc<Vec<u8>>) {
        self.source.bytes = bytes;
    }
    pub fn save(self, destination: &Path) -> Result<ExportResult> {
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .canonicalize()?;
        let destination = parent.join(
            destination
                .file_name()
                .context("Choose review output filename")?,
        );
        crate::storage::ensure_separate_output(&destination, &[&self.source.path])?;
        let scratch = tempfile::tempdir()?;
        let input = scratch.path().join("source.pdf");
        let patch = scratch.path().join("review.json");
        std::fs::write(&input, self.source.bytes.as_slice())?;
        let document = inspect(&input)?;
        if document["pages"].as_array().map(Vec::len) != Some(self.pages) {
            bail!("Review page count differs from its opened snapshot");
        }
        std::fs::write(
            &patch,
            serde_json::to_vec(&update(&document, &self.model, self.mode, &self.geometry)?)?,
        )?;
        let output = tempfile::Builder::new()
            .prefix(".dpdf-review-")
            .suffix(".pdf")
            .tempfile_in(&parent)?
            .into_temp_path();
        checked(
            Command::new(qpdf_executable())
                .arg(path_string(&input)?.as_ref())
                .arg(format!("--update-from-json={}", path_string(&patch)?))
                .arg("--stream-data=preserve")
                .arg("--object-streams=preserve")
                .arg(path_string(&output)?.as_ref()),
            "write review annotations",
        )?;
        checked(
            Command::new(qpdf_executable())
                .arg("--check")
                .arg(path_string(&output)?.as_ref()),
            "validate review output",
        )?;
        let verify = inspect(&output)?;
        if verify["pages"].as_array().map(Vec::len) != Some(self.pages) {
            bail!("Review output lost pages");
        }
        let saved = stored(&verify)?;
        if matches!(self.mode, ExportMode::Clean) && saved.is_some()
            || !matches!(self.mode, ExportMode::Clean) && saved.is_none()
        {
            bail!("Review metadata did not survive output");
        }
        std::fs::OpenOptions::new()
            .write(true)
            .open(&output)?
            .sync_all()?;
        crate::storage::ensure_separate_output(&destination, &[&self.source.path])?;
        output
            .persist(&destination)
            .map_err(|e| e.error)
            .context("Publish review copy")?;
        Ok(ExportResult {
            path: destination,
            pages: self.pages,
        })
    }
}
pub fn run_cli(mut args: impl Iterator<Item = std::ffi::OsString>) -> Result<()> {
    let action = args
        .next()
        .context("--review inspect INPUT | export INPUT OUTPUT MODEL visible|current|clean")?;
    let input = PathBuf::from(args.next().context("Missing input PDF")?).canonicalize()?;
    let source = SourceSnapshot {
        path: input.clone(),
        bytes: Arc::new(std::fs::read(&input)?),
    };
    let pdfium = crate::pdf::bind_pdfium()?;
    let pdf = pdfium.load_pdf_from_byte_slice(source.bytes.as_slice(), None)?;
    if action == "inspect" {
        if args.next().is_some() {
            bail!("--review inspect INPUT");
        }
        println!("{}", serde_json::to_string(&read(&source, &pdf)?)?);
        return Ok(());
    }
    if action != "export" {
        bail!("Choose inspect or export");
    }
    let output = PathBuf::from(args.next().context("Missing review output")?);
    let recipe = PathBuf::from(args.next().context("Missing review model JSON")?);
    let mode = ExportMode::parse(
        &args
            .next()
            .context("Missing export mode")?
            .to_string_lossy(),
    )?;
    if args.next().is_some() {
        bail!("--review export INPUT OUTPUT MODEL visible|current|clean");
    }
    crate::storage::ensure_separate_output(&output, &[&input, &recipe])?;
    let mut bytes = Vec::new();
    std::fs::File::open(&recipe)?
        .take(MAX_MODEL as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_MODEL {
        bail!("Review model exceeds 1 MiB");
    }
    let model: Review = serde_json::from_slice(&bytes)?;
    let prepared = prepare(source.clone(), &pdf, &model, mode)?;
    crate::storage::ensure_separate_output(&output, &[&input, &recipe])?;
    let result = prepared.save(&output)?;
    println!(
        "saved_pages={} output={}",
        result.pages,
        result.path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    #[test]
    fn review_helper_errors_and_stalls_are_bounded_and_retryable() {
        use std::{
            process::Command,
            time::{Duration, Instant},
        };
        let policy = || crate::helper_process::Limits {
            timeout: Duration::from_millis(150),
            stdout: 4096,
            stderr: 0,
        };
        let start = Instant::now();
        let error = super::checked_with_limits(
            Command::new("sh").args(["-c", "sleep 30"]),
            "test review",
            policy(),
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("deadline"));
        assert!(start.elapsed() < Duration::from_secs(3));
        for command in [
            "printf 'private helper detail' >&2; exit 3",
            "cat /dev/zero",
        ] {
            let mut limits = policy();
            limits.timeout = Duration::from_secs(3);
            let error = super::checked_with_limits(
                Command::new("sh").args(["-c", command]),
                "test review",
                limits,
            )
            .unwrap_err();
            assert!(!format!("{error:#}").contains("private helper detail"));
        }
        let mut limits = policy();
        limits.timeout = Duration::from_secs(3);
        super::checked_with_limits(
            Command::new("sh").args(["-c", "exit 0"]),
            "test review",
            limits,
        )
        .unwrap();
    }
    use super::*;
    #[test]
    fn undo_restores_content_but_keeps_view_state() {
        let page = NormalizedRect {
            x: 0.1,
            y: 0.1,
            width: 0.2,
            height: 0.05,
        };
        let mut before = Review::default();
        before.add(0, page, Kind::Highlight, "first", 3).unwrap();
        let mut present = before.clone();
        let second = present.new_revision("Second pass").unwrap();
        present.add(1, page, Kind::Note, "second", 3).unwrap();
        present.visible = false;
        present.filter = Filter::Selected(vec![1, second]);
        let restored = Review::restored_from(&before, &present);
        assert_eq!(restored.remarks, before.remarks);
        assert_eq!(restored.revisions, before.revisions);
        assert!(!restored.visible);
        assert_eq!(restored.current, 1);
        assert_eq!(restored.filter, Filter::Selected(vec![1]));
        assert!(present.content_differs(&before));
        assert!(!restored.content_differs(&before));
        present.filter = Filter::Selected(vec![second]);
        assert_eq!(Review::restored_from(&before, &present).filter, Filter::All);
        // Redoing the new revision makes it active again; an unrelated step keeps the choice.
        assert_eq!(Review::restored_from(&present, &before).current, second);
        let mut chosen = present.clone();
        chosen.current = 1;
        let mut edited = present.clone();
        edited.remarks.pop();
        assert_eq!(Review::restored_from(&edited, &chosen).current, 1);
    }
    fn bounds() -> NormalizedRect {
        NormalizedRect {
            x: 0.1,
            y: 0.2,
            width: 0.3,
            height: 0.05,
        }
    }
    #[test]
    fn current_filter_tracks_active_revision_while_selected_sets_remain_fixed() {
        let mut review = Review::default();
        review.add(0, bounds(), Kind::Note, "First", 1).unwrap();
        let second = review.new_revision("Second").unwrap();
        review.add(0, bounds(), Kind::Note, "Second", 1).unwrap();
        review.schema_version = 2;
        review.filter = Filter::Current;
        assert!(!review.is_visible(&review.remarks[0]));
        assert!(review.is_visible(&review.remarks[1]));
        review.current = 1;
        assert!(review.is_visible(&review.remarks[0]));
        assert!(!review.is_visible(&review.remarks[1]));
        review.filter = Filter::Selected(vec![second]);
        review.current = second;
        review.current = 1;
        assert!(!review.is_visible(&review.remarks[0]));
        assert!(review.is_visible(&review.remarks[1]));
        review.filter = Filter::Current;
        review.schema_version = 1;
        assert!(review.validate(1).is_err());
        review.schema_version = 2;
        review.validate(1).unwrap();
    }
    #[test]
    fn selected_revision_positions_map_stable_ids_and_reject_invalid_entries() {
        let mut review = Review::default();
        review.new_revision("Second").unwrap();
        review.new_revision("Third").unwrap();
        review.revisions[2].id = 42;
        review.current = 42;
        assert_eq!(
            review.filter_for_positions(" 3,1,3 ").unwrap(),
            Filter::Selected(vec![42, 1])
        );
        for invalid in ["", "0", "4", "1,no", "1,"] {
            assert!(review.filter_for_positions(invalid).is_err());
        }
        assert_eq!(review.filter, Filter::All);
        assert!(review.visible);
    }
    #[test]
    fn revisions_and_filters_remain_independent_and_invalid_add_is_atomic() {
        let mut review = Review::default();
        review
            .add(0, bounds(), Kind::Note, "Review note", 3)
            .unwrap();
        let second = review.new_revision("Revision 2").unwrap();
        review.add(1, bounds(), Kind::Underline, "", 3).unwrap();
        review.filter = Filter::Selected(vec![second]);
        assert!(!review.is_visible(&review.remarks[0]));
        assert!(review.is_visible(&review.remarks[1]));
        review.visible = false;
        assert!(!review.is_visible(&review.remarks[1]));
        let before = review.clone();
        assert!(
            review
                .add(4, bounds(), Kind::Note, "Outside document", 3)
                .is_err()
        );
        assert_eq!(review, before);
        assert!(review.new_revision(" Revision 2 ").is_err());
        assert_eq!(review, before);
        review.new_revision("Empty revision").unwrap();
        let decoded: Review =
            serde_json::from_slice(&serde_json::to_vec(&review).unwrap()).unwrap();
        assert_eq!(review, decoded);
    }
    #[test]
    fn namespace_ids_metadata_and_geometry_are_bounded() {
        let mut review = Review::default();
        review.revisions.push(review.revisions[0].clone());
        assert!(review.validate(1).is_err());
        review.revisions.pop();
        assert!(
            review
                .add(
                    0,
                    NormalizedRect {
                        x: f32::NAN,
                        ..bounds()
                    },
                    Kind::Highlight,
                    "",
                    1
                )
                .is_err()
        );
        assert!(
            review
                .add(0, bounds(), Kind::Note, &"x".repeat(4097), 1)
                .is_err()
        );
        assert!(review.remarks.is_empty());
        review.workspace = "invalid namespace".into();
        assert!(review.validate(1).is_err());
    }
}
