mod abs_pointer;
mod accessibility_guard;
#[path = "atspi_tree.rs"]
mod atspi_tree_impl;
mod cli;
mod command_runner;
mod cosmic_helper;
mod diagnostics_cache;
#[path = "diagnostics.rs"]
mod diagnostics_impl;
mod ei_input;
mod gnome_extension;
mod identity;
pub mod indicator;
mod keyboard_keymap;
mod pointer_path;
mod remote_desktop;
#[path = "screenshot.rs"]
mod screenshot_impl;
mod server;
mod terminal;
mod tree_spill;
mod windowing;
mod windows;
mod x11_display;
mod ydotool;

pub mod atspi_tree {
    pub use crate::atspi_tree_impl::{
        AccessibilityAction, AccessibilityNode, AccessibilityText, AccessibilityTextSelection,
        AccessibilityValue, Bounds, snapshot_tree,
    };
    pub(crate) use crate::atspi_tree_impl::{
        AccessibleAppSummary, EditableTextSetOutcome, FocusProbe, FocusedElementSummary,
        UNKNOWN_ROLE, ValueSetInvocation, accessibility_chrome_context,
        focused_element_summary_in_app, list_accessible_apps, object_ref_owner_pid, perform_action,
        perform_named_action, probe_focused_element, set_element_value,
        snapshot_accessibility_tree, snapshot_limits,
    };
}

pub mod diagnostics {
    pub use crate::diagnostics_impl::{
        AccessibilityReport, CapabilityMap, Check, DoctorReport, InputReport, PlatformReport,
        PortalReport, PreferredBackends, ReadinessReport, ScreenshotCaptureStatus, WindowingReport,
        doctor_report, hydrate_session_bus_env,
    };
    pub(crate) use crate::diagnostics_impl::{
        SetupReport, setup_accessibility_report, wtype_compatible_wayland_desktop,
    };
}

pub mod screenshot {
    pub use crate::screenshot_impl::{RawScreenshotCapture, capture_screenshot_raw};
    pub(crate) use crate::screenshot_impl::{
        ScreenshotCapture, ScreenshotOutputFormat, ScreenshotPayloadOptions, capture_screenshot,
        prepare_screenshot_payload,
    };
}

#[doc(hidden)]
pub async fn run_cli_from_env() -> anyhow::Result<()> {
    cli::run_from_env().await
}
