# Targeted UIA Performance and Direct Invocation Design

## Goal

Reduce request-to-action latency and navigation errors by making Snapshot inspect the relevant application by default, exposing structured UI element identities, and invoking supported UI Automation actions without relying on screen coordinates.

## Snapshot scope

`Snapshot` gains three optional parameters:

- `scope`: `foreground` or `all`; defaults to `foreground`.
- `window`: a fuzzy window-title query. When present, Snapshot scans only the uniquely best matching window.
- `timeout_ms`: total UIA scan deadline in milliseconds; defaults to 2000 and must be between 100 and 30000.

`window` and `scope=all` are mutually exclusive. `scope=all` preserves the current whole-desktop inspection path for discovery and diagnostics. Window tables and desktop metadata remain available, but the UI tree and actionable state contain only elements from the selected scan scope.

If the UIA deadline expires, Snapshot returns the elements collected so far and includes a clear truncation notice. It does not sleep past the global deadline to retry background windows.

## Structured UI map

Each discovered element is represented by an `ElementNode` containing:

- a generation-scoped `element_id`;
- an optional `parent_id`;
- the owning window handle;
- name, control type, automation ID, bounds, and focus state;
- supported actions derived from cached UIA patterns.

Snapshot renders these fields in a compact, machine-readable line while retaining readable window hierarchy. The server state stores the current generation and element map. A new Snapshot invalidates IDs from the previous generation.

The UIA cache request includes RuntimeId and pattern-availability properties needed to relocate an element. The server does not retain live COM element objects across requests. Invocation reconstructs the owning window tree and resolves the element using its stored runtime identity, with automation ID, control type, and bounds used only as guarded fallback identity signals.

## Direct invocation

A new `InvokeElement` tool accepts:

```json
{
  "element_id": 57,
  "fallback_to_click": false
}
```

The tool rejects IDs from stale generations, closed windows, and elements that cannot be uniquely relocated. It executes the first supported semantic action in this order:

1. Invoke
2. SelectionItem.Select
3. Toggle
4. ExpandCollapse

Coordinate clicking is disabled by default. When `fallback_to_click=true`, an element with no supported semantic action is clicked at its last validated center only after confirming that the owning window still exists and its bounds still contain that point.

## Performance and operational changes

- Foreground-only UIA inspection is the default; all-window inspection is opt-in.
- UIA retries share the global deadline. Foreground/explicit targets may retry while all-window background scans do not extend the deadline.
- Existing Snapshot profiling remains the authoritative benchmark output.
- README run instructions use `cargo run --release` and explain that debug builds are unsuitable for 4K screenshot processing.
- Documentation recommends `WaitFor` rather than fixed `Wait` calls after UI actions.

No new dependency or generic query language is introduced. UIA object pooling and long-lived cross-request COM references are excluded from this change because their threading and invalidation complexity is not required to achieve the measured gains.

## Error behavior

Input errors distinguish invalid scope combinations and timeout ranges. Invocation errors distinguish stale element IDs, closed owner windows, ambiguous or missing relocation matches, unsupported semantic actions, and rejected click fallback. Snapshot reports deadline truncation in successful partial output rather than silently omitting the reason.

## Verification

- Unit tests cover scope parsing, parameter conflicts, timeout validation, target-window selection, deadline behavior, generation invalidation, parent relationships, supported-action ordering, and fallback validation.
- Existing tests continue to pass.
- Interactive UIA tests remain ignored unless an interactive Windows desktop is available.
- `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo build --release` validate the implementation.
- Before/after profiling records metadata-only Snapshot, foreground UI-tree Snapshot, all-window Snapshot, and Screenshot timings.
- A manual Claude E2E check uses structured IDs and `InvokeElement` to navigate from the profile menu to Settings, then verifies settings content with a foreground Snapshot.

## Success criteria

- A default Snapshot never includes actionable elements from unrelated background applications.
- `scope=all` still supports whole-desktop discovery.
- Direct invocation reaches supported controls without coordinate input.
- Stale or ambiguous identities fail safely.
- Release-mode foreground Snapshot is measurably faster and produces a materially smaller response than the previous all-window default.
- Release-mode Screenshot remains below one second on the measured 3840x2160 environment.
