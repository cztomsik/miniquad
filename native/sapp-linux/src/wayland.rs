mod egl;
pub mod gl;
mod wayland_client;
mod wayland_egl;
mod xdg_shell_protocol;

use xdg_shell_protocol::{xdg_surface, xdg_toplevel, xdg_wm_base};

use crate::wayland::wayland_client::*;

use egl::{eglGetDisplay, eglInitialize};

static mut COMPOSITOR: *mut wl_compositor = std::ptr::null_mut();
static mut XDG_WM_BASE: *mut xdg_wm_base = std::ptr::null_mut();
static mut SURFACE: *mut wl_surface = std::ptr::null_mut();

static mut CLOSED: bool = false;

unsafe extern "C" fn registry_add_object(
    data: *mut std::ffi::c_void,
    registry: *mut wl_registry,
    name: u32,
    interface: *const i8,
    version: u32,
) {
    if strcmp(interface, b"wl_compositor\x00" as *const u8 as *const _) == 0 {
        COMPOSITOR = wl_registry_bind(registry, name, &wl_compositor_interface, 1) as _;
    } else if strcmp(interface, b"xdg_wm_base\x00" as *const u8 as *const _) == 0 {
        XDG_WM_BASE = wl_registry_bind(
            registry,
            name,
            &xdg_shell_protocol::xdg_wm_base_interface,
            1,
        ) as _;
    }
}

unsafe extern "C" fn registry_remove_object(
    data: *mut std::ffi::c_void,
    registry: *mut wl_registry,
    name: u32,
) {
}

unsafe extern "C" fn xdg_surface_handle_configure(
    data: *mut std::ffi::c_void,
    xdg_surface: *mut xdg_surface,
    serial: u32,
) {
    xdg_surface_ack_configure(xdg_surface, serial);
    wl_surface_commit(SURFACE);
}

unsafe extern "C" fn xdg_toplevel_handle_close(
    data: *mut std::ffi::c_void,
    xdg_toplevel: *mut xdg_toplevel,
) {
    CLOSED = true;
}

#[no_mangle]
extern "C" {
    pub fn strcmp(_: *const i8, _: *const i8) -> i32;
}

unsafe fn wl_display_get_registry(display: *mut wl_display) -> *mut wl_registry {
    let registry: *mut wl_proxy;

    registry = wl_proxy_marshal_constructor(
        display as *mut _,
        WL_DISPLAY_GET_REGISTRY,
        &wl_registry_interface,
        std::ptr::null_mut::<std::ffi::c_void>(),
    );
    registry as *mut _ as *mut wl_registry
}

unsafe fn wl_registry_bind(
    wl_registry: *const wl_registry,
    name: u32,
    interface: *const wl_interface,
    version: u32,
) -> *mut std::ffi::c_void {
    let id: *mut wl_proxy;

    id = wl_proxy_marshal_constructor_versioned(
        wl_registry as _,
        WL_REGISTRY_BIND,
        interface as _,
        version,
        name,
        (*interface).name,
        version,
        std::ptr::null_mut::<std::ffi::c_void>(),
    );

    id as *mut _
}

unsafe fn wl_surface_commit(wl_surface: *const wl_surface) {
    wl_proxy_marshal(wl_surface as _, WL_SURFACE_COMMIT)
}

unsafe fn wl_registry_add_listener(
    wl_registry: *const wl_registry,
    listener: *const wl_registry_listener,
    data: *mut std::ffi::c_void,
) -> i32 {
    wl_proxy_add_listener(wl_registry as _, listener as _, data as _)
}

unsafe fn wl_compositor_create_surface(wl_compositor: *mut wl_compositor) -> *mut wl_surface {
    let id: *mut wl_proxy;

    id = wl_proxy_marshal_constructor(
        wl_compositor as _,
        WL_COMPOSITOR_CREATE_SURFACE,
        &wl_surface_interface as _,
        std::ptr::null_mut::<std::ffi::c_void>(),
    );

    id as *mut _
}

unsafe fn xdg_wm_base_get_xdg_surface(
    xdg_wm_base: *mut xdg_wm_base,
    surface: *mut wl_surface,
) -> *mut xdg_surface {
    let id: *mut wl_proxy;

    id = wl_proxy_marshal_constructor(
        xdg_wm_base as _,
        xdg_wm_base::get_xdg_surface,
        &xdg_shell_protocol::xdg_surface_interface as _,
        std::ptr::null_mut::<std::ffi::c_void>(),
        surface,
    );

    id as *mut _
}

unsafe fn xdg_surface_get_toplevel(xdg_surface: *mut xdg_surface) -> *mut xdg_toplevel {
    let id: *mut wl_proxy;

    id = wl_proxy_marshal_constructor(
        xdg_surface as _,
        xdg_surface::get_toplevel,
        &xdg_shell_protocol::xdg_toplevel_interface as _,
        std::ptr::null_mut::<std::ffi::c_void>(),
    );

    id as *mut _
}

unsafe fn xdg_surface_ack_configure(xdg_surface: *mut xdg_surface, serial: u32) {
    wl_proxy_marshal(xdg_surface as _, xdg_surface::ack_configure, serial);
}


pub fn init_window() {
    unsafe {
        let display = wl_display_connect(std::ptr::null_mut());
        if display.is_null() {
            panic!("Failed to connect to Wayland display.");
        }
        let registry = wl_display_get_registry(display);

        let mut registry_listener = wl_registry_listener {
            global: Some(registry_add_object),
            global_remove: Some(registry_remove_object),
        };
        wl_registry_add_listener(registry, &registry_listener, std::ptr::null_mut());
        wl_display_roundtrip(display);

        if COMPOSITOR.is_null() {
            panic!("No compositor!");
        }
        if XDG_WM_BASE.is_null() {
            panic!("No xdg_wm_base");
        }

        let egl_display = eglGetDisplay(display as _);
        eglInitialize(egl_display, std::ptr::null_mut(), std::ptr::null_mut());

        egl::eglBindAPI(egl::EGL_OPENGL_API);
        let attributes = [
            egl::EGL_RED_SIZE,
            8,
            egl::EGL_GREEN_SIZE,
            8,
            egl::EGL_BLUE_SIZE,
            8,
            egl::EGL_NONE,
        ];
        let mut config: egl::EGLConfig = std::mem::zeroed();
        let mut num_config = 0;

        egl::eglChooseConfig(
            egl_display,
            attributes.as_ptr() as _,
            &mut config,
            1,
            &mut num_config,
        );
        let egl_context = egl::eglCreateContext(
            egl_display,
            config,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );

        SURFACE = wl_compositor_create_surface(COMPOSITOR);
        assert!(SURFACE.is_null() == false);
        let xdg_surface = xdg_wm_base_get_xdg_surface(XDG_WM_BASE, SURFACE);
        assert!(xdg_surface.is_null() == false);
        let xdg_toplevel = xdg_surface_get_toplevel(xdg_surface);
        assert!(xdg_toplevel.is_null() == false);

        let mut xdg_surface_listener = xdg_shell_protocol::xdg_surface_listener {
            configure: Some(xdg_surface_handle_configure),
        };

        wl_proxy_add_listener(
            xdg_surface as _,
            std::mem::transmute(&mut xdg_surface_listener),
            std::ptr::null_mut(),
        );

        extern "C" fn noop(
            _: *mut std::ffi::c_void,
            _: *mut crate::wayland::xdg_toplevel,
            _: i32,
            _: i32,
            _: *mut crate::wayland::wl_array,
        ) -> () {
        }

        let mut xdg_toplevel_listener = xdg_shell_protocol::xdg_toplevel_listener {
            configure: Some(noop),
            close: Some(xdg_toplevel_handle_close),
        };

        wl_proxy_add_listener(
            xdg_toplevel as _,
            std::mem::transmute(&mut xdg_toplevel_listener),
            std::ptr::null_mut(),
        );

        wl_surface_commit(SURFACE);
        wl_display_roundtrip(display);

        let egl_window = wayland_egl::wl_egl_window_create(SURFACE as _, 512, 512);
        let egl_surface =
            egl::eglCreateWindowSurface(egl_display, config, egl_window as _, std::ptr::null_mut());
        egl::eglMakeCurrent(egl_display, egl_surface, egl_surface, egl_context);

        while CLOSED == false {
            wl_display_dispatch_pending(display);

            crate::_sapp_frame();

            egl::eglSwapBuffers(egl_display, egl_surface);
        }
    }
}
