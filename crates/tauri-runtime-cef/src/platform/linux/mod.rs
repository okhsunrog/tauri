// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

mod event_loop;
mod monitor;
mod taskbar;
mod utils;
mod webview;
mod window;

pub(crate) use webview::LinuxWebviewState;
pub(crate) use window::CefNativeHost;

/// Use the display GTK actually opened, including an explicit GDK_BACKEND override.
pub(crate) fn is_wayland() -> bool {
  #[cfg(target_os = "linux")]
  {
    use gtk::prelude::*;
    gtk::gdk::Display::default().is_some_and(|d| d.is::<gdk4_wayland::WaylandDisplay>())
  }
  #[cfg(not(target_os = "linux"))]
  {
    false
  }
}

pub(crate) fn initialize_display() -> tauri_runtime::Result<&'static str> {
  #[cfg(target_os = "linux")]
  {
    use gtk::prelude::*;
    if let Some(display) =
      gtk::gdk::Display::default().and_then(|d| d.downcast::<gdk4_wayland::WaylandDisplay>().ok())
    {
      let raw = display
        .wl_display_raw()
        .ok_or(tauri_runtime::Error::WebviewRuntimeNotInstalled)?;
      // GTK owns the connection throughout the event loop and CEF shutdown.
      unsafe {
        cef::sys::cef_set_wayland_display(raw.as_ptr().cast());
      }
      log::info!("CEF native backend: wayland (shared GTK display)");
      return Ok("wayland");
    }
  }
  log::info!("CEF native backend: x11");
  Ok("x11")
}
