// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use cef::ImplBrowserHost;
use std::os::raw::c_ulong;
use tauri_runtime::dpi::{PhysicalPosition, PhysicalSize, Rect};
use tauri_utils::config::Color;
use x11_dl::xlib;

use crate::{webview::AppWebview, window::AppWindow};

use super::utils::{atom, with_cef_display};

/// Requested layout is cached because Wayland has no XGetGeometry equivalent.
pub(crate) struct LinuxWebviewState {
  bounds: std::cell::RefCell<Rect>,
  offset: std::sync::Arc<std::sync::Mutex<(i32, i32)>>,
  area: gtk::Box,
}

impl LinuxWebviewState {
  pub(crate) fn new(parent: &AppWindow, bounds: Rect) -> Self {
    Self {
      bounds: std::cell::RefCell::new(bounds),
      offset: parent.cef_host.offset.clone(),
      area: parent.cef_host.default_vbox(),
    }
  }
}

impl AppWebview {
  pub(crate) fn native_parent_matches(&self, parent: &AppWindow) -> Option<bool> {
    if super::is_wayland() {
      return None;
    }
    let xid = self.host.window_handle();
    if xid == 0 {
      return None;
    }
    with_cef_display(None, |xlib, display| unsafe {
      let mut root = 0;
      let mut native_parent = 0;
      let mut children = std::ptr::null_mut();
      let mut count = 0;
      let status = (xlib.XQueryTree)(
        display,
        xid as xlib::Window,
        &mut root,
        &mut native_parent,
        &mut children,
        &mut count,
      );
      if !children.is_null() {
        (xlib.XFree)(children.cast());
      }
      // Browsers are created under (and reparented into) the X11 host, not the GTK toplevel.
      (status != 0).then_some(native_parent == parent.cef_host_handle() as xlib::Window)
    })
  }

  pub(crate) fn native_visible(&self) -> Option<bool> {
    if super::is_wayland() {
      return None;
    }
    let xid = self.host.window_handle();
    if xid == 0 {
      return None;
    }
    with_cef_display(None, |xlib, display| unsafe {
      let mut attributes = std::mem::MaybeUninit::<xlib::XWindowAttributes>::uninit();
      if (xlib.XGetWindowAttributes)(display, xid as xlib::Window, attributes.as_mut_ptr()) == 0 {
        return None;
      }
      // XGetWindowAttributes initializes the complete structure on success.
      Some(attributes.assume_init().map_state == xlib::IsViewable)
    })
  }

  fn xid(&self) -> xlib::Window {
    let xid = self.host.window_handle();
    assert_ne!(xid, 0, "failed to get XID");
    xid as xlib::Window
  }

  pub(crate) fn set_background_color(&self, color: Option<Color>) {
    let _ = (self, color);
    // Native child-window background is not equivalent to Chromium's rendered
    // background. Creation still applies BrowserSettings.
  }

  pub(crate) fn bounds(&self) -> Option<Rect> {
    if let Some(view) = &self.offscreen {
      return Some(view.bounds());
    }
    if super::is_wayland() {
      return Some(*self.linux_native.bounds.borrow());
    }
    let xid = self.xid();

    with_cef_display(None, |xlib, display| unsafe {
      let mut root: xlib::Window = 0;
      let mut x: i32 = 0;
      let mut y: i32 = 0;
      let mut width: u32 = 0;
      let mut height: u32 = 0;
      let mut border_width: u32 = 0;
      let mut depth: u32 = 0;

      if (xlib.XGetGeometry)(
        display,
        xid,
        &mut root,
        &mut x,
        &mut y,
        &mut width,
        &mut height,
        &mut border_width,
        &mut depth,
      ) == 0
      {
        return None;
      }

      Some(Rect {
        position: PhysicalPosition::new(x, y).into(),
        size: PhysicalSize::new(width, height).into(),
      })
    })
  }

  pub(crate) fn reparent(&self, parent: &AppWindow) {
    debug_assert!(!super::is_wayland());
    let xid = self.xid();
    // Linux reparents into the GTK content-area X11 host, unlike Windows/macOS
    // where the CEF host handle is the native window/view.
    let parent_xid = parent.cef_host_handle();

    with_cef_display((), |xlib, display| unsafe {
      (xlib.XReparentWindow)(display, xid, parent_xid as xlib::Window, 0, 0);
      (xlib.XMapRaised)(display, xid);
    });
  }

  pub(crate) fn apply_visible(&self, visible: bool) {
    if super::is_wayland() {
      if !visible {
        log::warn!(
          "hiding an individual native Wayland webview is not supported; hide its window instead"
        );
      }
      return;
    }
    let xid = self.xid();

    with_cef_display((), |xlib, display| unsafe {
      let net_wm_state = atom(xlib, display, "_NET_WM_STATE");
      const PROP_MODE_REPLACE: i32 = 0;

      if visible {
        (xlib.XChangeProperty)(
          display,
          xid,
          net_wm_state,
          xlib::XA_ATOM,
          32,
          PROP_MODE_REPLACE,
          std::ptr::null(),
          0,
        );
        (xlib.XMapWindow)(display, xid);
      } else {
        let hidden: [c_ulong; 1] = [atom(xlib, display, "_NET_WM_STATE_HIDDEN")];
        (xlib.XChangeProperty)(
          display,
          xid,
          net_wm_state,
          xlib::XA_ATOM,
          32,
          PROP_MODE_REPLACE,
          hidden.as_ptr() as *const u8,
          1,
        );
        (xlib.XUnmapWindow)(display, xid);
      }
    });
  }

  pub(crate) fn apply_physical_bounds(&self, scale: f64, x: i32, y: i32, width: i32, height: i32) {
    if super::is_wayland() {
      use gtk::prelude::*;
      *self.linux_native.bounds.borrow_mut() = Rect {
        position: PhysicalPosition::new(x, y).into(),
        size: PhysicalSize::new(width.max(1) as u32, height.max(1) as u32).into(),
      };
      let (ox, oy) = *self.linux_native.offset.lock().unwrap();
      self.host.set_window_bounds(Some(&cef::Rect {
        x: (x as f64 / scale).round() as i32 + ox,
        y: (y as f64 / scale).round() as i32 + oy,
        width: (width as f64 / scale).round().max(1.) as i32,
        height: (height as f64 / scale).round().max(1.) as i32,
      }));
      // Subsurface positions become visible with the parent's next commit.
      self.linux_native.area.queue_draw();
      return;
    }
    let xid = self.xid();

    with_cef_display((), |xlib, display| unsafe {
      (xlib.XMoveResizeWindow)(
        display,
        xid,
        x,
        y,
        width.max(1) as u32,
        height.max(1) as u32,
      );
      // `with_cef_display` issues an `XFlush` once the closure returns, so a
      // blocking `XSync` round-trip here just stalls every resize frame.
    });
  }
}
