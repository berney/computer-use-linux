use computer_use_linux::{
    atspi_tree::{AccessibilityNode, snapshot_tree},
    diagnostics::{Check, DoctorReport, doctor_report, hydrate_session_bus_env},
    screenshot::{RawScreenshotCapture, capture_screenshot_raw},
};

#[test]
fn exposes_record_replay_library_surface() {
    let _doctor: fn() -> DoctorReport = doctor_report;
    let _hydrate: fn() = hydrate_session_bus_env;
    let _snapshot_tree = snapshot_tree;
    let _capture_screenshot_raw = capture_screenshot_raw;
    let _check: Option<Check> = None;
    let _report: Option<DoctorReport> = None;
    let _node: Option<AccessibilityNode> = None;
    let _capture: Option<RawScreenshotCapture> = None;
}
