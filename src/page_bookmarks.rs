//! Keep primary outline navigation attached to a replaced logical page.
use crate::{
    assembly::{inspect_metadata, path_string, qpdf_executable},
    pages::PagePlan,
};
use anyhow::{Context, Result, ensure};
use serde_json::{Map, Value, json};
use std::{collections::HashMap, path::Path, process::Command};

pub(crate) struct Target {
    route: Vec<usize>,
    title: Value,
    page: usize,
    destination: Vec<Value>,
}

fn scalar(mut value: Value, objects: &Value) -> Result<Value> {
    for _ in 0..16 {
        if value.is_null()
            || value.is_number()
            || value.as_str().is_some_and(|text| text.starts_with('/'))
        {
            return Ok(value);
        }
        let reference = value.as_str().context("Unsupported bookmark coordinate")?;
        value = objects
            .get(format!("obj:{reference}"))
            .and_then(|record| record.get("value"))
            .context("Unresolved bookmark coordinate")?
            .clone();
    }
    anyhow::bail!("Bookmark coordinate reference exceeds 16 levels")
}

fn visit<'a>(
    nodes: &'a Value,
    route: &mut Vec<usize>,
    output: &mut Vec<(Vec<usize>, &'a Value)>,
) -> Result<()> {
    ensure!(route.len() < 64, "Bookmark nesting exceeds 64 levels");
    for (index, node) in nodes
        .as_array()
        .context("Invalid bookmark list")?
        .iter()
        .enumerate()
    {
        ensure!(output.len() < 10_000, "Bookmark count exceeds 10,000");
        route.push(index);
        output.push((route.clone(), node));
        if node["kids"].as_array().is_some_and(|kids| !kids.is_empty()) {
            visit(&node["kids"], route, output)?;
        }
        route.pop();
    }
    Ok(())
}

pub(crate) fn capture(path: &Path, plan: &PagePlan) -> Result<Vec<Target>> {
    let mut replacements = HashMap::new();
    for (output, page) in plan.pages().iter().enumerate() {
        if let Some(origin) = page.primary_origin
            && (page.source != 0 || page.page != origin)
        {
            replacements.entry(origin).or_insert(output);
        }
    }
    if replacements.is_empty() {
        return Ok(Vec::new());
    }
    let document = inspect_metadata(path)?;
    let mut outlines = Vec::new();
    visit(&document["outlines"], &mut Vec::new(), &mut outlines)?;
    let mut targets = Vec::new();
    for (route, node) in outlines {
        let Some(origin) = node["destpageposfrom1"]
            .as_u64()
            .and_then(|page| page.checked_sub(1))
        else {
            continue;
        };
        let Some(&page) = replacements.get(&(origin as usize)) else {
            continue;
        };
        let mut destination = node["dest"]
            .as_array()
            .context("Missing resolved bookmark destination")?
            .clone();
        ensure!(
            (2..=6).contains(&destination.len()),
            "Unsupported bookmark destination"
        );
        for value in &mut destination[1..] {
            *value = scalar(value.clone(), &document["qpdf"][1])?;
        }
        targets.push(Target {
            route,
            title: node["title"].clone(),
            page,
            destination,
        });
    }
    Ok(targets)
}

fn node_at<'a>(document: &'a Value, route: &[usize]) -> Result<&'a Value> {
    let mut list = &document["outlines"];
    let mut node = None;
    for &index in route {
        let current = list
            .get(index)
            .context("Bookmark structure changed during page assembly")?;
        node = Some(current);
        list = &current["kids"];
    }
    node.context("Missing bookmark route")
}

pub(crate) fn restore(targets: &[Target], input: &Path, scratch: &Path) -> Result<()> {
    if targets.is_empty() {
        return Ok(());
    }
    let document = inspect_metadata(input)?;
    let mut changes = Map::new();
    for target in targets {
        let node = node_at(&document, &target.route)?;
        ensure!(
            node["title"] == target.title,
            "Bookmark identity changed during page assembly"
        );
        let reference = node["object"]
            .as_str()
            .context("Missing assembled bookmark reference")?;
        let key = format!("obj:{reference}");
        let mut dictionary = document["qpdf"][1][&key]["value"]
            .as_object()
            .context("Missing assembled bookmark dictionary")?
            .clone();
        let mut destination = target.destination.clone();
        destination[0] = document["pages"]
            .get(target.page)
            .and_then(|page| page.get("object"))
            .context("Missing replacement destination page")?
            .clone();
        // Keep GoTo action chains and styling; named bookmarks get a direct target.
        // Other catalog destinations and page links are outside this repair's scope.
        if let Some(original) = dictionary.get("/A") {
            let mut action = original.clone();
            for _ in 0..16 {
                if action.is_object() {
                    break;
                }
                let reference = action.as_str().context("Invalid bookmark action")?;
                action = document["qpdf"][1]
                    .get(format!("obj:{reference}"))
                    .and_then(|record| record.get("value"))
                    .context("Unresolved bookmark action")?
                    .clone();
            }
            ensure!(
                action.is_object() && action["/S"] == "/GoTo",
                "Unsupported replacement bookmark action"
            );
            action["/D"] = json!(destination);
            dictionary.remove("/Dest");
            dictionary.insert("/A".into(), action);
        } else {
            dictionary.insert("/Dest".into(), json!(destination));
        }
        changes.insert(key, json!({"value":dictionary}));
    }
    let patch = scratch.join("bookmarks.json");
    let output = scratch.join("bookmarked.pdf");
    std::fs::write(
        &patch,
        serde_json::to_vec(&json!({"qpdf":[{"jsonversion":2},changes]}))?,
    )?;
    crate::assembly::run_qpdf(
        Command::new(qpdf_executable())
            .arg(path_string(input)?.as_ref())
            .arg(format!("--update-from-json={}", path_string(&patch)?))
            .arg("--stream-data=preserve")
            .arg("--object-streams=preserve")
            .arg(path_string(&output)?.as_ref()),
        "Restore replacement bookmarks",
    )?;
    let verified = inspect_metadata(&output)?;
    for target in targets {
        let node = node_at(&verified, &target.route)?;
        ensure!(
            node["title"] == target.title
                && node["destpageposfrom1"].as_u64() == Some(target.page as u64 + 1),
            "Replacement bookmark validation failed"
        );
    }
    std::fs::copy(output, input).context("Stage restored replacement bookmarks")?;
    Ok(())
}
