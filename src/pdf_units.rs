//! Physical page-unit preflight, independent of visual marks and review remarks.
use crate::assembly::{
    SourceSnapshot, inspect_metadata_cancellable, inspect_snapshot, path_string, qpdf_executable,
};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Map, Value};
use std::{collections::HashSet, process::Command};

fn unit<'a>(mut value: &'a Value, objects: &'a Map<String, Value>) -> Result<f64> {
    let mut seen = HashSet::new();
    for _ in 0..16 {
        if value.is_null() {
            return Ok(1.0); // A null dictionary entry has the same meaning as omission.
        }
        if let Some(number) = value.as_f64() {
            ensure!(number.is_finite() && number > 0.0, "Invalid PDF UserUnit");
            return Ok(number);
        }
        let reference = value.as_str().context("Invalid PDF UserUnit value")?;
        ensure!(seen.insert(reference), "Cyclic PDF UserUnit reference");
        value = objects
            .get(&format!("obj:{reference}"))
            .and_then(|record| record.get("value"))
            .context("Unresolved PDF UserUnit reference")?;
    }
    bail!("PDF UserUnit reference depth exceeds 16")
}

pub(crate) fn from_metadata(document: &Value, expected_pages: usize) -> Result<Vec<f64>> {
    let pages = document
        .get("pages")
        .and_then(Value::as_array)
        .context("Missing PDF page metadata")?;
    ensure!(
        pages.len() == expected_pages && expected_pages > 0,
        "PDF page metadata differs from the opened document"
    );
    let objects = document
        .get("qpdf")
        .and_then(|value| value.get(1))
        .and_then(Value::as_object)
        .context("Missing PDF object metadata")?;
    pages
        .iter()
        .map(|page| {
            let reference = page
                .get("object")
                .and_then(Value::as_str)
                .context("Missing PDF page reference")?;
            let dictionary = objects
                .get(&format!("obj:{reference}"))
                .and_then(|record| record.get("value"))
                .and_then(Value::as_object)
                .context("Missing PDF page dictionary")?;
            // UserUnit is a page entry, not an inheritable page-tree attribute.
            dictionary
                .get("/UserUnit")
                .map_or(Ok(1.0), |value| unit(value, objects))
        })
        .collect()
}

pub fn load(bytes: &[u8]) -> Result<Vec<f64>> {
    let document = inspect_snapshot(bytes)?;
    let count = document
        .get("pages")
        .and_then(Value::as_array)
        .context("Missing PDF page metadata")?
        .len();
    from_metadata(&document, count)
}

pub fn mark_units(source: &SourceSnapshot, expected_pages: usize) -> Result<Vec<f64>> {
    let units = load(source.bytes.as_slice())?;
    ensure!(
        units.len() == expected_pages,
        "PDF page metadata differs from the opened document"
    );
    Ok(units)
}

/// Retain a replacement page's coordinate units without regenerating its image streams.
pub fn set_image_page_unit(bytes: Vec<u8>, unit: f64) -> Result<Vec<u8>> {
    set_image_page_unit_cancellable(bytes, unit, None)
}

pub(crate) fn set_image_page_unit_cancellable(
    bytes: Vec<u8>,
    unit: f64,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Vec<u8>> {
    crate::background_jobs::check_cancelled(cancel)?;
    ensure!(unit.is_finite() && unit > 0.0, "Invalid image page unit");
    if unit == 1.0 {
        return Ok(bytes);
    }
    let directory = tempfile::Builder::new()
        .prefix("dpdf-image-units-")
        .tempdir()?;
    let input = directory.path().join("image.pdf");
    let patch = directory.path().join("units.json");
    let output = directory.path().join("scaled.pdf");
    std::fs::write(&input, bytes)?;
    let document = inspect_metadata_cancellable(&input, cancel)?;
    ensure!(
        document["pages"].as_array().map(Vec::len) == Some(1),
        "Image unit update needs exactly one page"
    );
    let reference = document["pages"][0]["object"]
        .as_str()
        .context("Missing image page reference")?;
    let key = format!("obj:{reference}");
    let mut page = document["qpdf"][1][&key]["value"]
        .as_object()
        .context("Missing image page dictionary")?
        .clone();
    page.insert("/UserUnit".into(), serde_json::json!(unit));
    let mut updates = Map::new();
    updates.insert(key, serde_json::json!({"value":page}));
    std::fs::write(
        &patch,
        serde_json::to_vec(&serde_json::json!({"qpdf":[{"jsonversion":2},updates]}))?,
    )?;
    crate::assembly::run_qpdf_cancellable(
        Command::new(qpdf_executable())
            .arg(path_string(&input)?.as_ref())
            .arg(format!("--update-from-json={}", path_string(&patch)?))
            .arg("--stream-data=preserve")
            .arg("--object-streams=preserve")
            .arg(path_string(&output)?.as_ref()),
        "Preserve image page units",
        cancel,
    )?;
    ensure!(
        from_metadata(&inspect_metadata_cancellable(&output, cancel)?, 1)? == [unit],
        "Image page units did not survive output"
    );
    crate::background_jobs::check_cancelled(cancel)?;
    std::fs::read(&output).context("Read scaled image page")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture(value: Value) -> Value {
        json!({"pages":[{"object":"3 0 R"}],"qpdf":[{}, {
            "obj:3 0 R":{"value":{"/Type":"/Page","/UserUnit":value}},
            "obj:7 0 R":{"value":1},"obj:8 0 R":{"value":"8 0 R"}
        }]})
    }

    #[test]
    fn physical_units_resolve_defaults_references_and_reject_invalid_metadata() {
        for value in [Value::Null, json!(1), json!("7 0 R")] {
            assert_eq!(from_metadata(&fixture(value), 1).unwrap(), [1.0]);
        }
        let mut absent = fixture(json!(2));
        absent["qpdf"][1]["obj:3 0 R"]["value"]
            .as_object_mut()
            .unwrap()
            .remove("/UserUnit");
        absent["qpdf"][1]["obj:3 0 R"]["value"]["/Parent"] = json!("9 0 R");
        absent["qpdf"][1]["obj:9 0 R"] = json!({"value":{"/UserUnit":2}});
        assert_eq!(from_metadata(&absent, 1).unwrap(), [1.0]);
        for value in [
            json!(0),
            json!(-2),
            json!(true),
            json!("8 0 R"),
            json!("99 0 R"),
        ] {
            assert!(from_metadata(&fixture(value), 1).is_err());
        }
        assert!(from_metadata(&fixture(json!(1)), 2).is_err());
    }

    #[test]
    fn valid_custom_units_remain_physical_metadata() {
        assert_eq!(from_metadata(&fixture(json!(2)), 1).unwrap(), [2.0]);
        assert_eq!(from_metadata(&fixture(json!(0.5)), 1).unwrap(), [0.5]);
    }
}
