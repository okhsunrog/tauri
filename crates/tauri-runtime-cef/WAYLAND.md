# Native Linux Wayland fork

This branch integrates the CEF 154 Wayland embedding API ported from
chromiumembedded/cef#4233. It requires the matching custom cef-rs bindings and
SDK, rather than stock CEF 154. See the smoke application's Cargo patches for
local development: both `cef` and `cef-dll-sys` must resolve to the same fork.

GTK selects the display backend, honoring `GDK_BACKEND`. Its actual Wayland
connection is passed to CEF before initialization; CEF children use the GTK
surface and receive content offsets and bounds in logical pixels. X11 keeps
its 24-bit child host. The chosen Ozone backend is passed to subprocesses too.

The fork's Linux x86_64 bindings require experimental API 999999. Startup checks
both the API version and the generated platform hash before accessing CEF
vtables. Selecting a different table version is rejected.

GTK does not expose the parent xdg_surface through a public API, so menus and
select popups use CEF's degraded subsurface fallback. They may be clipped and
lack outside-click dismissal. Cross-window webview reparenting is rejected.
Individual native webview hiding is not implemented; whole-window hiding uses
GTK. Global cursor coordinates and X11 taskbar/workspace hints are unavailable
on Wayland. Snapshot bounds are requested layout, while native parent and
visibility observations report unknown.

Validation so far covers frontend build and Rust type checks, including normal
build scripts against custom CEF headers. Hardware Wayland embedding, resize,
input, menus and Vulkan WebGPU still require runtime acceptance with the actual
custom libcef. A successful compile does not establish those results.
