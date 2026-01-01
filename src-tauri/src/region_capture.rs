//! Native region capture for Windows.
//!
//! Captures all monitors into a single canvas, opens a full-screen overlay window,
//! allows user to select a region with resize handles, and returns the cropped image.

use log::{debug, error};
use specta::Type;
use tauri::{AppHandle, Manager};
use tokio::sync::oneshot;

#[cfg(target_os = "windows")]
use tauri::WebviewWindowBuilder;

/// Information about the virtual screen (all monitors combined).
#[derive(Debug, Clone, serde::Serialize, Type)]
pub struct VirtualScreenInfo {
    /// Minimum X coordinate (can be negative if monitors are left of primary)
    pub offset_x: i32,
    /// Minimum Y coordinate
    pub offset_y: i32,
    /// Total width spanning all monitors
    pub total_width: u32,
    /// Total height spanning all monitors
    pub total_height: u32,
    /// Scale factor of primary monitor (for coordinate conversion)
    pub scale_factor: f64,
}

/// Region selected by the user (in screen coordinates).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Type)]
pub struct SelectedRegion {
    /// X coordinate in virtual screen space
    pub x: i32,
    /// Y coordinate in virtual screen space
    pub y: i32,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
}

/// Result of a region capture operation.
#[derive(Debug)]
pub enum RegionCaptureResult {
    /// User selected a region successfully
    Selected {
        region: SelectedRegion,
        image_data: Vec<u8>, // PNG bytes
    },
    /// User cancelled (pressed Escape)
    Cancelled,
    /// An error occurred
    Error(String),
}

/// State for tracking ongoing region capture operations.
pub struct RegionCaptureState {
    /// Channel to receive the result from the overlay window
    pub result_sender: Option<oneshot::Sender<RegionCaptureResult>>,
    /// Screenshot data (PNG bytes of entire virtual screen)
    pub screenshot_data: Option<Vec<u8>>,
    /// Virtual screen info for coordinate conversion
    pub virtual_info: Option<VirtualScreenInfo>,
}

impl Default for RegionCaptureState {
    fn default() -> Self {
        Self {
            result_sender: None,
            screenshot_data: None,
            virtual_info: None,
        }
    }
}

pub type ManagedRegionCaptureState = std::sync::Mutex<RegionCaptureState>;

/// Captures a screenshot of all monitors and returns it as PNG bytes along with virtual screen info.
#[cfg(target_os = "windows")]
pub fn capture_all_monitors() -> Result<(Vec<u8>, VirtualScreenInfo), String> {
    use screenshots::image::{self, ImageEncoder};
    use screenshots::Screen;

    let screens = Screen::all().map_err(|e| format!("Failed to enumerate screens: {}", e))?;

    if screens.is_empty() {
        return Err("No screens found".to_string());
    }

    // Find virtual screen boundaries
    let min_x = screens.iter().map(|s| s.display_info.x).min().unwrap_or(0);
    let min_y = screens.iter().map(|s| s.display_info.y).min().unwrap_or(0);
    let max_x = screens
        .iter()
        .map(|s| s.display_info.x + s.display_info.width as i32)
        .max()
        .unwrap_or(0);
    let max_y = screens
        .iter()
        .map(|s| s.display_info.y + s.display_info.height as i32)
        .max()
        .unwrap_or(0);

    let total_width = (max_x - min_x) as u32;
    let total_height = (max_y - min_y) as u32;

    debug!(
        "Virtual screen: offset=({}, {}), size={}x{}",
        min_x, min_y, total_width, total_height
    );

    // Create canvas for combined screenshot
    let mut canvas = image::RgbaImage::new(total_width, total_height);

    // Get scale factor from first screen (primary)
    let scale_factor = screens
        .first()
        .map(|s| s.display_info.scale_factor as f64)
        .unwrap_or(1.0);

    // Capture each screen and composite onto canvas
    for screen in screens {
        let img = screen
            .capture()
            .map_err(|e| format!("Failed to capture screen: {}", e))?;

        let offset_x = (screen.display_info.x - min_x) as u32;
        let offset_y = (screen.display_info.y - min_y) as u32;

        debug!(
            "Screen {} at ({}, {}): {}x{}, placing at ({}, {})",
            screen.display_info.id,
            screen.display_info.x,
            screen.display_info.y,
            screen.display_info.width,
            screen.display_info.height,
            offset_x,
            offset_y
        );

        // Copy pixels from captured image to canvas
        for y in 0..img.height().min(total_height - offset_y) {
            for x in 0..img.width().min(total_width - offset_x) {
                let pixel = img.get_pixel(x, y);
                canvas.put_pixel(offset_x + x, offset_y + y, *pixel);
            }
        }
    }

    // Encode to PNG using ImageEncoder trait
    let mut png_bytes: Vec<u8> = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
    encoder
        .write_image(
            canvas.as_raw(),
            total_width,
            total_height,
            image::ColorType::Rgba8,
        )
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;

    let info = VirtualScreenInfo {
        offset_x: min_x,
        offset_y: min_y,
        total_width,
        total_height,
        scale_factor,
    };

    Ok((png_bytes, info))
}

#[cfg(not(target_os = "windows"))]
pub fn capture_all_monitors() -> Result<(Vec<u8>, VirtualScreenInfo), String> {
    Err("Native region capture is only supported on Windows".to_string())
}

/// Crops a region from the full screenshot and returns it as PNG bytes.
#[cfg(target_os = "windows")]
pub fn crop_region(screenshot_data: &[u8], region: &SelectedRegion) -> Result<Vec<u8>, String> {
    use screenshots::image::{self, ImageEncoder};

    // Decode the PNG
    let img = image::load_from_memory(screenshot_data)
        .map_err(|e| format!("Failed to decode screenshot: {}", e))?
        .to_rgba8();

    // Validate region bounds
    if region.x < 0 || region.y < 0 {
        return Err("Invalid region: negative coordinates".to_string());
    }
    let x = region.x as u32;
    let y = region.y as u32;

    if x + region.width > img.width() || y + region.height > img.height() {
        return Err(format!(
            "Region out of bounds: ({}, {}) + {}x{} exceeds {}x{}",
            x,
            y,
            region.width,
            region.height,
            img.width(),
            img.height()
        ));
    }

    // Crop the region
    let cropped = image::imageops::crop_imm(&img, x, y, region.width, region.height).to_image();

    // Encode to PNG using ImageEncoder trait
    let mut png_bytes: Vec<u8> = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
    encoder
        .write_image(
            cropped.as_raw(),
            region.width,
            region.height,
            image::ColorType::Rgba8,
        )
        .map_err(|e| format!("Failed to encode cropped PNG: {}", e))?;

    Ok(png_bytes)
}

#[cfg(not(target_os = "windows"))]
pub fn crop_region(_screenshot_data: &[u8], _region: &SelectedRegion) -> Result<Vec<u8>, String> {
    Err("Native region capture is only supported on Windows".to_string())
}

/// Opens the region capture overlay and returns when user selects a region or cancels.
#[cfg(target_os = "windows")]
pub async fn open_region_picker(app: &AppHandle) -> RegionCaptureResult {
    // First, capture all monitors
    let (screenshot_data, virtual_info) = match capture_all_monitors() {
        Ok(result) => result,
        Err(e) => return RegionCaptureResult::Error(e),
    };

    debug!(
        "Screenshot captured: {} bytes, virtual screen: {:?}",
        screenshot_data.len(),
        virtual_info
    );

    // Create a channel for receiving the result
    let (tx, rx) = oneshot::channel::<RegionCaptureResult>();

    // Store state for the overlay to access
    {
        let state = app.state::<ManagedRegionCaptureState>();
        let mut guard = state.lock().unwrap();
        guard.result_sender = Some(tx);
        guard.screenshot_data = Some(screenshot_data.clone());
        guard.virtual_info = Some(virtual_info.clone());
    }

    // Calculate window position and size based on virtual screen
    // We need to account for scale factor when setting window position/size
    let scale = virtual_info.scale_factor;
    let x = virtual_info.offset_x as f64 / scale;
    let y = virtual_info.offset_y as f64 / scale;
    let width = virtual_info.total_width as f64 / scale;
    let height = virtual_info.total_height as f64 / scale;

    debug!(
        "Creating overlay window at ({}, {}) size {}x{} (logical)",
        x, y, width, height
    );

    // Create the overlay window
    let window_result = WebviewWindowBuilder::new(
        app,
        "region_capture",
        tauri::WebviewUrl::App("src/region-capture/index.html".into()),
    )
    .title("Region Capture")
    .position(x, y)
    .inner_size(width, height)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .focused(true)
    .visible(false) // Start hidden, show after ready
    .build();

    match window_result {
        Ok(window) => {
            debug!("Region capture overlay window created");

            // Show the window - frontend will fetch data via command when ready
            let _ = window.show();
            let _ = window.set_focus();

            // Force topmost
            force_overlay_topmost(&window);
        }
        Err(e) => {
            error!("Failed to create region capture window: {}", e);
            // Clean up state
            let state = app.state::<ManagedRegionCaptureState>();
            let mut guard = state.lock().unwrap();
            guard.result_sender = None;
            guard.screenshot_data = None;
            guard.virtual_info = None;
            return RegionCaptureResult::Error(format!("Failed to create overlay: {}", e));
        }
    }

    // Wait for result from overlay
    match rx.await {
        Ok(result) => result,
        Err(_) => {
            RegionCaptureResult::Error("Region capture channel closed unexpectedly".to_string())
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub async fn open_region_picker(_app: &AppHandle) -> RegionCaptureResult {
    RegionCaptureResult::Error("Native region capture is only supported on Windows".to_string())
}

/// Called from the overlay when user selects a region.
pub fn on_region_selected(app: &AppHandle, region: SelectedRegion) {
    let state = app.state::<ManagedRegionCaptureState>();
    let mut guard = state.lock().unwrap();

    // Get the screenshot data and crop the region
    if let (Some(screenshot_data), Some(sender)) =
        (guard.screenshot_data.take(), guard.result_sender.take())
    {
        match crop_region(&screenshot_data, &region) {
            Ok(cropped_data) => {
                let _ = sender.send(RegionCaptureResult::Selected {
                    region,
                    image_data: cropped_data,
                });
            }
            Err(e) => {
                let _ = sender.send(RegionCaptureResult::Error(e));
            }
        }
    }

    guard.virtual_info = None;

    // Close the overlay window
    if let Some(window) = app.get_webview_window("region_capture") {
        let _ = window.close();
    }
}

/// Called from the overlay when user cancels.
pub fn on_region_cancelled(app: &AppHandle) {
    let state = app.state::<ManagedRegionCaptureState>();
    let mut guard = state.lock().unwrap();

    if let Some(sender) = guard.result_sender.take() {
        let _ = sender.send(RegionCaptureResult::Cancelled);
    }

    guard.screenshot_data = None;
    guard.virtual_info = None;

    // Close the overlay window
    if let Some(window) = app.get_webview_window("region_capture") {
        let _ = window.close();
    }
}

/// Forces a window to be topmost using Win32 API (Windows only).
#[cfg(target_os = "windows")]
fn force_overlay_topmost(overlay_window: &tauri::webview::WebviewWindow) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    };

    let overlay_clone = overlay_window.clone();

    let _ = overlay_clone.clone().run_on_main_thread(move || {
        if let Ok(hwnd) = overlay_clone.hwnd() {
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
        }
    });
}

/// Encode bytes to base64 string.
pub fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

        result.push(ALPHABET[b0 >> 2] as char);
        result.push(ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(ALPHABET[b2 & 0x3f] as char);
        } else {
            result.push('=');
        }
    }

    result
}
