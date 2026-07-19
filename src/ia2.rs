//! Firefox DOM fallback through Microsoft Active Accessibility/IAccessible.

use std::mem::ManuallyDrop;

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::System::Com::IDispatch;
use windows::Win32::System::Variant::{
    VARIANT, VARIANT_0_0, VARIANT_0_0_0, VT_DISPATCH, VT_I4, VariantToUInt32,
};
use windows::Win32::UI::Accessibility::{
    AccessibleChildren, AccessibleObjectFromWindow, IAccessible, ROLE_SYSTEM_CHECKBUTTON,
    ROLE_SYSTEM_COMBOBOX, ROLE_SYSTEM_DOCUMENT, ROLE_SYSTEM_LINK, ROLE_SYSTEM_LISTITEM,
    ROLE_SYSTEM_MENUITEM, ROLE_SYSTEM_OUTLINEITEM, ROLE_SYSTEM_PAGETAB, ROLE_SYSTEM_PUSHBUTTON,
    ROLE_SYSTEM_RADIOBUTTON, ROLE_SYSTEM_STATICTEXT, ROLE_SYSTEM_TEXT,
};
use windows::Win32::UI::WindowsAndMessaging::{OBJID_CLIENT, STATE_SYSTEM_FOCUSED};
use windows::core::Interface;

const STATE_SYSTEM_UNAVAILABLE: u32 = 0x1;
const STATE_SYSTEM_INVISIBLE: u32 = 0x8000;
const STATE_SYSTEM_OFFSCREEN: u32 = 0x10000;
const STATE_SYSTEM_FOCUSABLE: u32 = 0x100000;
const MAX_NODES: usize = 5_000;

#[derive(Debug, Clone)]
pub struct Ia2Element {
    pub name: String,
    pub control_type: String,
    pub rect: RECT,
    pub interactive: bool,
    pub enabled: bool,
    pub offscreen: bool,
    pub focused: bool,
}

#[derive(Debug, Default)]
pub struct Ia2Result {
    pub dom_bounds: Option<RECT>,
    pub elements: Vec<Ia2Element>,
}

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

fn variant_type(variant: &VARIANT) -> windows::Win32::System::Variant::VARENUM {
    unsafe { variant.Anonymous.Anonymous.vt }
}

fn dispatch_from_variant(variant: &VARIANT) -> Option<IDispatch> {
    if variant_type(variant) != VT_DISPATCH {
        return None;
    }
    unsafe {
        let dispatch = &variant.Anonymous.Anonymous.Anonymous.pdispVal;
        dispatch.as_ref().cloned()
    }
}

fn role_name(role: u32) -> String {
    match role {
        ROLE_SYSTEM_CHECKBUTTON => "checkbox",
        ROLE_SYSTEM_COMBOBOX => "combobox",
        ROLE_SYSTEM_DOCUMENT => "document",
        ROLE_SYSTEM_LINK => "hyperlink",
        ROLE_SYSTEM_LISTITEM => "listitem",
        ROLE_SYSTEM_MENUITEM => "menuitem",
        ROLE_SYSTEM_OUTLINEITEM => "treeitem",
        ROLE_SYSTEM_PAGETAB => "tabitem",
        ROLE_SYSTEM_PUSHBUTTON => "button",
        ROLE_SYSTEM_RADIOBUTTON => "radiobutton",
        ROLE_SYSTEM_STATICTEXT => "text",
        ROLE_SYSTEM_TEXT => "text",
        _ => "control",
    }
    .to_string()
}

fn is_interactive(role: u32, state: u32) -> bool {
    matches!(
        role,
        ROLE_SYSTEM_CHECKBUTTON
            | ROLE_SYSTEM_COMBOBOX
            | ROLE_SYSTEM_LINK
            | ROLE_SYSTEM_LISTITEM
            | ROLE_SYSTEM_MENUITEM
            | ROLE_SYSTEM_OUTLINEITEM
            | ROLE_SYSTEM_PAGETAB
            | ROLE_SYSTEM_PUSHBUTTON
            | ROLE_SYSTEM_RADIOBUTTON
    ) || (role == ROLE_SYSTEM_TEXT && state & STATE_SYSTEM_FOCUSABLE != 0)
}

unsafe fn read_element(accessible: &IAccessible, child: &VARIANT) -> Option<(Ia2Element, u32)> {
    unsafe {
        let role = accessible
            .get_accRole(child)
            .ok()
            .and_then(|variant| VariantToUInt32(&variant).ok())?;
        let state = accessible
            .get_accState(child)
            .ok()
            .and_then(|variant| VariantToUInt32(&variant).ok())
            .unwrap_or(0);
        let name = accessible
            .get_accName(child)
            .map(|name| name.to_string())
            .unwrap_or_default();
        let mut left = 0;
        let mut top = 0;
        let mut width = 0;
        let mut height = 0;
        accessible
            .accLocation(&mut left, &mut top, &mut width, &mut height, child)
            .ok()?;
        Some((
            Ia2Element {
                name,
                control_type: role_name(role),
                rect: RECT {
                    left,
                    top,
                    right: left + width,
                    bottom: top + height,
                },
                interactive: is_interactive(role, state),
                enabled: state & STATE_SYSTEM_UNAVAILABLE == 0,
                offscreen: state & (STATE_SYSTEM_INVISIBLE | STATE_SYSTEM_OFFSCREEN) != 0,
                focused: state & STATE_SYSTEM_FOCUSED != 0,
            },
            role,
        ))
    }
}

unsafe fn walk(
    accessible: &IAccessible,
    child: &VARIANT,
    in_document: bool,
    result: &mut Ia2Result,
) {
    if result.elements.len() >= MAX_NODES {
        return;
    }
    let Some((element, role)) = (unsafe { read_element(accessible, child) }) else {
        return;
    };
    let in_document = in_document || role == ROLE_SYSTEM_DOCUMENT;
    if role == ROLE_SYSTEM_DOCUMENT && result.dom_bounds.is_none() {
        result.dom_bounds = Some(element.rect);
    }
    if in_document && role != ROLE_SYSTEM_DOCUMENT && !element.name.trim().is_empty() {
        result.elements.push(element);
    }

    if variant_type(child) != VT_I4 || unsafe { child.Anonymous.Anonymous.Anonymous.lVal } != 0 {
        return;
    }
    let count = unsafe { accessible.accChildCount() }.unwrap_or(0).max(0) as usize;
    if count == 0 {
        return;
    }
    let mut children = vec![VARIANT::default(); count.min(MAX_NODES - result.elements.len())];
    let mut obtained = 0;
    if unsafe { AccessibleChildren(accessible, 0, &mut children, &mut obtained) }.is_err() {
        return;
    }
    for child_variant in children.iter().take(obtained.max(0) as usize) {
        if let Some(dispatch) = dispatch_from_variant(child_variant) {
            if let Ok(child_accessible) = dispatch.cast::<IAccessible>() {
                unsafe { walk(&child_accessible, &variant_i4(0), in_document, result) };
            }
        } else if variant_type(child_variant) == VT_I4 {
            unsafe { walk(accessible, child_variant, in_document, result) };
        }
        if result.elements.len() >= MAX_NODES {
            break;
        }
    }
}

pub fn walk_firefox(hwnd: HWND) -> Result<Ia2Result, String> {
    unsafe {
        let mut raw = std::ptr::null_mut();
        AccessibleObjectFromWindow(hwnd, OBJID_CLIENT.0 as u32, &IAccessible::IID, &mut raw)
            .map_err(|e| format!("AccessibleObjectFromWindow failed: {e}"))?;
        if raw.is_null() {
            return Err("AccessibleObjectFromWindow returned no object".to_string());
        }
        let accessible = IAccessible::from_raw(raw);
        let mut result = Ia2Result::default();
        walk(&accessible, &variant_i4(0), false, &mut result);
        Ok(result)
    }
}
