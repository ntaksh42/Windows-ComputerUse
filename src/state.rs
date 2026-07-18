//! Global desktop state, populated by the Snapshot tool (a later phase) and
//! consumed here so Click/Type/Scroll/Move/MultiSelect/MultiEdit can resolve
//! a UI element `label` to screen coordinates without re-querying the
//! accessibility tree.
#![allow(dead_code)] // TODO: drop once the Snapshot tool writes real state.

use std::sync::{Mutex, OnceLock};

/// A single UI element discovered by Snapshot's accessibility-tree walk.
///
/// Kept intentionally minimal — the Snapshot implementation is expected to
/// grow this with additional fields (bounding box, metadata, window name,
/// etc.) as it lands.
#[derive(Debug, Clone)]
pub struct ElementNode {
    pub name: String,
    pub control_type: String,
    pub center: (i32, i32),
}

/// Flat lists of interactive/scrollable elements from the most recent
/// Snapshot call. Labels index into `interactive_nodes` first, then
/// `scrollable_nodes` for the remainder.
#[derive(Debug, Clone, Default)]
pub struct DesktopState {
    pub interactive_nodes: Vec<ElementNode>,
    pub scrollable_nodes: Vec<ElementNode>,
}

static STATE: OnceLock<Mutex<Option<DesktopState>>> = OnceLock::new();

fn state_lock() -> &'static Mutex<Option<DesktopState>> {
    STATE.get_or_init(|| Mutex::new(None))
}

/// Replaces the current desktop state (called by Snapshot after a capture).
pub fn set_state(state: DesktopState) {
    *state_lock().lock().unwrap() = Some(state);
}

/// Clears the desktop state.
pub fn clear_state() {
    *state_lock().lock().unwrap() = None;
}

const EMPTY_STATE_ERROR: &str = "Desktop state is empty. Please call Snapshot first.";

/// Resolves a single UI element `label` to screen coordinates.
///
/// Labels index `interactive_nodes` first; values beyond that range index
/// into `scrollable_nodes` as an offset.
pub fn resolve_label(label: usize) -> Result<(i32, i32), String> {
    let guard = state_lock().lock().unwrap();
    let state = guard
        .as_ref()
        .ok_or_else(|| EMPTY_STATE_ERROR.to_string())?;
    if label < state.interactive_nodes.len() {
        Ok(state.interactive_nodes[label].center)
    } else {
        let idx = label - state.interactive_nodes.len();
        state
            .scrollable_nodes
            .get(idx)
            .map(|n| n.center)
            .ok_or_else(|| format!("Label {label} out of range"))
    }
}

/// Resolves multiple UI element labels to screen coordinates in bulk.
pub fn resolve_labels(labels: &[usize]) -> Result<Vec<(i32, i32)>, String> {
    let guard = state_lock().lock().unwrap();
    let state = guard
        .as_ref()
        .ok_or_else(|| EMPTY_STATE_ERROR.to_string())?;
    let mut out = Vec::with_capacity(labels.len());
    for &label in labels {
        if label < state.interactive_nodes.len() {
            out.push(state.interactive_nodes[label].center);
        } else {
            let idx = label - state.interactive_nodes.len();
            match state.scrollable_nodes.get(idx) {
                Some(n) => out.push(n.center),
                None => return Err(format!("Label {label} out of range")),
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    // Tests share process-global STATE; serialize them so they don't clobber
    // each other under `cargo test`'s default multi-threaded runner.
    static TEST_GUARD: StdMutex<()> = StdMutex::new(());

    fn node(x: i32, y: i32) -> ElementNode {
        ElementNode {
            name: "n".into(),
            control_type: "Button".into(),
            center: (x, y),
        }
    }

    #[test]
    fn empty_state_errors() {
        let _g = TEST_GUARD.lock().unwrap();
        clear_state();
        assert_eq!(resolve_label(0), Err(EMPTY_STATE_ERROR.to_string()));
        assert_eq!(resolve_labels(&[0]), Err(EMPTY_STATE_ERROR.to_string()));
    }

    #[test]
    fn resolves_interactive_then_scrollable_offset() {
        let _g = TEST_GUARD.lock().unwrap();
        set_state(DesktopState {
            interactive_nodes: vec![node(1, 1), node(2, 2)],
            scrollable_nodes: vec![node(3, 3)],
        });
        assert_eq!(resolve_label(0), Ok((1, 1)));
        assert_eq!(resolve_label(1), Ok((2, 2)));
        assert_eq!(resolve_label(2), Ok((3, 3)));
        assert_eq!(resolve_label(3), Err("Label 3 out of range".to_string()));
        clear_state();
    }

    #[test]
    fn resolves_labels_in_bulk() {
        let _g = TEST_GUARD.lock().unwrap();
        set_state(DesktopState {
            interactive_nodes: vec![node(1, 1)],
            scrollable_nodes: vec![node(2, 2)],
        });
        assert_eq!(resolve_labels(&[0, 1]), Ok(vec![(1, 1), (2, 2)]));
        assert_eq!(
            resolve_labels(&[0, 5]),
            Err("Label 5 out of range".to_string())
        );
        clear_state();
    }
}
