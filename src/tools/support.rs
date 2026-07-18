//! Shared helpers for resolving a `loc`/`label` pair to screen coordinates,
//! used by Click, Type, Scroll, Move, MultiSelect, and MultiEdit.

use crate::params::ListOrString;
use crate::state;

/// Resolves a UI element `label` to coordinates, rejecting negative labels
/// up front (the Python reference silently wraps negative labels via
/// Python's list-negative-indexing; a label is never meant to be negative,
/// so this reports it as out of range instead).
pub fn resolve_label_checked(label: i64) -> Result<(i32, i32), String> {
    if label < 0 {
        return Err(format!("Label {label} out of range"));
    }
    state::resolve_label(label as usize)
}

/// Resolves multiple labels to coordinates in bulk.
pub fn resolve_labels_checked(labels: &[i64]) -> Result<Vec<(i32, i32)>, String> {
    let mut usize_labels = Vec::with_capacity(labels.len());
    for &label in labels {
        if label < 0 {
            return Err(format!("Label {label} out of range"));
        }
        usize_labels.push(label as usize);
    }
    state::resolve_labels(&usize_labels)
}

/// Converts an optional `loc` param into `Option<Vec<i32>>`, resolving the
/// JSON-stringified-array fallback.
pub fn as_loc_vec(loc: Option<ListOrString<i32>>) -> Result<Option<Vec<i32>>, String> {
    match loc {
        None => Ok(None),
        Some(v) => Ok(Some(v.into_list()?)),
    }
}

/// Resolves a required `loc`/`label` pair to a single `(x, y)` point.
/// `label`, when present, always takes priority over `loc` (matching the
/// Python reference).
pub fn resolve_point_required(
    loc: Option<ListOrString<i32>>,
    label: Option<i64>,
) -> Result<(i32, i32), String> {
    let loc_vec = as_loc_vec(loc)?;
    if loc_vec.is_none() && label.is_none() {
        return Err("Either loc or label must be provided.".to_string());
    }
    if let Some(label) = label {
        return resolve_label_checked(label);
    }
    let v = loc_vec.unwrap();
    if v.len() != 2 {
        return Err("Location must be a list of exactly 2 integers [x, y]".to_string());
    }
    Ok((v[0], v[1]))
}

/// Resolves an optional `loc`/`label` pair. Returns `None` when neither is
/// provided (used by Scroll, which defaults to the current cursor position).
pub fn resolve_point_optional(
    loc: Option<ListOrString<i32>>,
    label: Option<i64>,
) -> Result<Option<(i32, i32)>, String> {
    let loc_vec = as_loc_vec(loc)?;
    if let Some(label) = label {
        return Ok(Some(resolve_label_checked(label)?));
    }
    match loc_vec {
        None => Ok(None),
        Some(v) if v.len() == 2 => Ok(Some((v[0], v[1]))),
        Some(_) => Err("Location must be a list of exactly 2 integers [x, y]".to_string()),
    }
}
