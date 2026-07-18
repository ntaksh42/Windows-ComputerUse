//! UIA (UI Automation) COM wrapper used by the `Snapshot`/`WaitFor` tools.
//!
//! Performance-critical: every window's subtree is fetched with a single
//! `FindAllBuildCache` call driven by an `IUIAutomationCacheRequest` that
//! pre-registers the properties/patterns we need. Callers only ever read
//! `Cached*` members — never `Current*` — so there are no per-element
//! cross-process COM round trips (docs/SPEC.md "Snapshot 性能設計").
//!
//! COM must be initialized MTA (`COINIT_MULTITHREADED`) on the calling
//! thread before any function here is used; see [`ensure_com_initialized`].

use std::cell::Cell;
use std::mem::ManuallyDrop;

use windows::Win32::Foundation::{HWND, RECT, VARIANT_FALSE, VARIANT_TRUE};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::System::Variant::{VARIANT, VARIANT_0_0, VARIANT_0_0_0, VT_BOOL, VT_I4};
use windows::Win32::UI::Accessibility::*;
use windows::core::Result as WinResult;

/// A UI element read from the UIA cache after a `FindAllBuildCache` call.
/// Every field here is a `Cached*` read — no COM round trip per field.
#[derive(Debug, Clone)]
pub struct RawElement {
    pub control_type: i32,
    pub name: String,
    pub rect: RECT,
    pub is_enabled: bool,
    pub is_offscreen: bool,
    pub has_keyboard_focus: bool,
    /// Only meaningful when `control_type == UIA_WindowControlTypeId`.
    pub is_modal: bool,
    /// Whether `IUIAutomationScrollPattern` is present on this element at all
    /// (presence, not scrollability direction — matches the task's
    /// "ScrollPattern の有無で判定" instruction).
    pub is_scrollable: bool,
}

thread_local! {
    static COM_INITIALIZED: Cell<bool> = const { Cell::new(false) };
}

/// Initializes COM as MTA on the current thread, once. Safe to call
/// repeatedly (including from multiple functions on the same thread).
pub fn ensure_com_initialized() -> Result<(), String> {
    COM_INITIALIZED.with(|initialized| {
        if initialized.get() {
            return Ok(());
        }
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr.is_ok() {
            initialized.set(true);
            Ok(())
        } else {
            Err(format!("CoInitializeEx failed: {hr:?}"))
        }
    })
}

/// Creates the `IUIAutomation` root object (`CUIAutomation`).
pub fn create_automation() -> WinResult<IUIAutomation> {
    unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }
}

/// Control-type ids considered "interactive" (docs/SPEC.md §6 item 4):
/// Button/CheckBox/ComboBox/Edit/Hyperlink/ListItem/MenuItem/RadioButton/
/// TabItem/TreeItem/SplitButton.
pub const INTERACTIVE_CONTROL_TYPES: &[i32] = &[
    UIA_ButtonControlTypeId.0,
    UIA_CheckBoxControlTypeId.0,
    UIA_ComboBoxControlTypeId.0,
    UIA_EditControlTypeId.0,
    UIA_HyperlinkControlTypeId.0,
    UIA_ListItemControlTypeId.0,
    UIA_MenuItemControlTypeId.0,
    UIA_RadioButtonControlTypeId.0,
    UIA_TabItemControlTypeId.0,
    UIA_TreeItemControlTypeId.0,
    UIA_SplitButtonControlTypeId.0,
];

pub const WINDOW_CONTROL_TYPE: i32 = UIA_WindowControlTypeId.0;

/// Lower-cased display name for a `UIA_*ControlTypeId` value, used to render
/// UI Tree lines (`(x,y) controltype "name" [action: ...]`).
#[allow(non_upper_case_globals)] // matching the windows crate's UIA_*ControlTypeId consts
pub fn control_type_name(control_type: i32) -> String {
    let id = UIA_CONTROLTYPE_ID(control_type);
    let name = match id {
        UIA_ButtonControlTypeId => "button",
        UIA_CalendarControlTypeId => "calendar",
        UIA_CheckBoxControlTypeId => "checkbox",
        UIA_ComboBoxControlTypeId => "combobox",
        UIA_EditControlTypeId => "edit",
        UIA_HyperlinkControlTypeId => "hyperlink",
        UIA_ImageControlTypeId => "image",
        UIA_ListItemControlTypeId => "listitem",
        UIA_ListControlTypeId => "list",
        UIA_MenuBarControlTypeId => "menubar",
        UIA_MenuControlTypeId => "menu",
        UIA_MenuItemControlTypeId => "menuitem",
        UIA_ProgressBarControlTypeId => "progressbar",
        UIA_RadioButtonControlTypeId => "radiobutton",
        UIA_ScrollBarControlTypeId => "scrollbar",
        UIA_SliderControlTypeId => "slider",
        UIA_SpinnerControlTypeId => "spinner",
        UIA_StatusBarControlTypeId => "statusbar",
        UIA_TabControlTypeId => "tab",
        UIA_TabItemControlTypeId => "tabitem",
        UIA_TextControlTypeId => "text",
        UIA_ToolBarControlTypeId => "toolbar",
        UIA_ToolTipControlTypeId => "tooltip",
        UIA_TreeControlTypeId => "tree",
        UIA_TreeItemControlTypeId => "treeitem",
        UIA_CustomControlTypeId => "custom",
        UIA_GroupControlTypeId => "group",
        UIA_ThumbControlTypeId => "thumb",
        UIA_DataGridControlTypeId => "datagrid",
        UIA_DataItemControlTypeId => "dataitem",
        UIA_DocumentControlTypeId => "document",
        UIA_SplitButtonControlTypeId => "splitbutton",
        UIA_WindowControlTypeId => "window",
        UIA_PaneControlTypeId => "pane",
        UIA_HeaderControlTypeId => "header",
        UIA_HeaderItemControlTypeId => "headeritem",
        UIA_TableControlTypeId => "table",
        UIA_TitleBarControlTypeId => "titlebar",
        UIA_SeparatorControlTypeId => "separator",
        UIA_SemanticZoomControlTypeId => "semanticzoom",
        UIA_AppBarControlTypeId => "appbar",
        _ => "control",
    };
    name.to_string()
}

/// Builds a `VARIANT` holding a `VT_I4` value (used for property conditions).
fn variant_i4(value: i32) -> VARIANT {
    let mut variant = VARIANT::default();
    variant.Anonymous.Anonymous = ManuallyDrop::new(VARIANT_0_0 {
        vt: VT_I4,
        wReserved1: 0,
        wReserved2: 0,
        wReserved3: 0,
        Anonymous: VARIANT_0_0_0 { lVal: value },
    });
    variant
}

/// Builds a `VARIANT` holding a `VT_BOOL` value (used for property conditions).
fn variant_bool(value: bool) -> VARIANT {
    let mut variant = VARIANT::default();
    variant.Anonymous.Anonymous = ManuallyDrop::new(VARIANT_0_0 {
        vt: VT_BOOL,
        wReserved1: 0,
        wReserved2: 0,
        wReserved3: 0,
        Anonymous: VARIANT_0_0_0 {
            boolVal: if value { VARIANT_TRUE } else { VARIANT_FALSE },
        },
    });
    variant
}

/// Builds the `IUIAutomationCacheRequest` used for every window walk:
/// registers the properties and patterns docs/SPEC.md §6 item 2 calls out
/// (Name/ControlType/BoundingRectangle/IsEnabled/IsOffscreen/
/// IsKeyboardFocusable/HasKeyboardFocus/AutomationId/ClassName, plus
/// Invoke/Value/Toggle/Scroll/SelectionItem/ExpandCollapse/Window pattern
/// availability) so every element read below is a `Cached*` read.
pub fn build_cache_request(automation: &IUIAutomation) -> WinResult<IUIAutomationCacheRequest> {
    let cache = unsafe { automation.CreateCacheRequest()? };
    unsafe {
        cache.SetAutomationElementMode(AutomationElementMode_Full)?;
        for property in [
            UIA_NamePropertyId,
            UIA_ControlTypePropertyId,
            UIA_BoundingRectanglePropertyId,
            UIA_IsEnabledPropertyId,
            UIA_IsOffscreenPropertyId,
            UIA_IsKeyboardFocusablePropertyId,
            UIA_HasKeyboardFocusPropertyId,
            UIA_AutomationIdPropertyId,
            UIA_ClassNamePropertyId,
        ] {
            cache.AddProperty(property)?;
        }
        for pattern in [
            UIA_InvokePatternId,
            UIA_ValuePatternId,
            UIA_TogglePatternId,
            UIA_ScrollPatternId,
            UIA_SelectionItemPatternId,
            UIA_ExpandCollapsePatternId,
            UIA_WindowPatternId,
        ] {
            cache.AddPattern(pattern)?;
        }
    }
    Ok(cache)
}

/// Builds the `FindAllBuildCache` filter condition: any of the interactive
/// control types, any element exposing `ScrollPattern`, or a `Window`
/// element (needed to detect nested modal dialogs). Filtering server-side
/// keeps the marshaled element count down instead of fetching the whole
/// subtree and discarding most of it client-side.
pub fn build_condition(automation: &IUIAutomation) -> WinResult<IUIAutomationCondition> {
    unsafe {
        let mut conditions: Vec<Option<IUIAutomationCondition>> =
            Vec::with_capacity(INTERACTIVE_CONTROL_TYPES.len() + 2);
        for &control_type in INTERACTIVE_CONTROL_TYPES {
            conditions.push(Some(automation.CreatePropertyCondition(
                UIA_ControlTypePropertyId,
                &variant_i4(control_type),
            )?));
        }
        conditions.push(Some(automation.CreatePropertyCondition(
            UIA_IsScrollPatternAvailablePropertyId,
            &variant_bool(true),
        )?));
        conditions.push(Some(automation.CreatePropertyCondition(
            UIA_ControlTypePropertyId,
            &variant_i4(WINDOW_CONTROL_TYPE),
        )?));
        automation.CreateOrConditionFromNativeArray(&conditions)
    }
}

/// Reads every `Cached*` member of `element` into a [`RawElement`]. Each
/// property read is independently best-effort (defaults on failure) so one
/// missing property doesn't drop the whole element.
unsafe fn read_element(element: &IUIAutomationElement) -> RawElement {
    unsafe {
        let control_type = element.CachedControlType().map(|c| c.0).unwrap_or_default();
        let name = element
            .CachedName()
            .map(|b| b.to_string())
            .unwrap_or_default();
        let rect = element.CachedBoundingRectangle().unwrap_or_default();
        let is_enabled = element
            .CachedIsEnabled()
            .map(|b| b.as_bool())
            .unwrap_or(false);
        let is_offscreen = element
            .CachedIsOffscreen()
            .map(|b| b.as_bool())
            .unwrap_or(true);
        let has_keyboard_focus = element
            .CachedHasKeyboardFocus()
            .map(|b| b.as_bool())
            .unwrap_or(false);

        let is_modal = if control_type == WINDOW_CONTROL_TYPE {
            element
                .GetCachedPatternAs::<IUIAutomationWindowPattern>(UIA_WindowPatternId)
                .ok()
                .and_then(|pattern| pattern.CachedIsModal().ok())
                .map(|b| b.as_bool())
                .unwrap_or(false)
        } else {
            false
        };

        let is_scrollable = element
            .GetCachedPatternAs::<IUIAutomationScrollPattern>(UIA_ScrollPatternId)
            .is_ok();

        RawElement {
            control_type,
            name,
            rect,
            is_enabled,
            is_offscreen,
            has_keyboard_focus,
            is_modal,
            is_scrollable,
        }
    }
}

/// Walks one top-level window's subtree in a single `FindAllBuildCache`
/// call, returning the window's own (root) element plus every matching
/// descendant, in document order.
///
/// This is the hot path the CacheRequest design exists for: exactly two COM
/// calls per window (`ElementFromHandleBuildCache` + `FindAllBuildCache`),
/// then only `Cached*` reads.
pub fn walk_window(
    automation: &IUIAutomation,
    cache_request: &IUIAutomationCacheRequest,
    condition: &IUIAutomationCondition,
    hwnd: HWND,
) -> WinResult<(RawElement, Vec<RawElement>)> {
    unsafe {
        let root = automation.ElementFromHandleBuildCache(hwnd, cache_request)?;
        let root_raw = read_element(&root);
        let array = root.FindAllBuildCache(TreeScope_Subtree, condition, cache_request)?;
        let len = array.Length()?.max(0) as usize;
        let mut elements = Vec::with_capacity(len);
        for i in 0..len as i32 {
            let element = array.GetElement(i)?;
            elements.push(read_element(&element));
        }
        Ok((root_raw, elements))
    }
}
