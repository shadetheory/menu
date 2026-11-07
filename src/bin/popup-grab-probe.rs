//! Tries an xdg_popup grab parented on a pill-sized layer surface.
//! The only serial available at an Alt+Space launch is the keyboard-enter
//! serial from layer-shell exclusive focus. This probe records whether Sway
//! accepts that grab. Usage: popup-grab-probe RESULT_FILE

use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::client::globals::registry_queue_init;
use smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard;
use smithay_client_toolkit::reexports::client::protocol::{
    wl_output, wl_pointer, wl_seat, wl_shm, wl_surface,
};
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::keyboard::{KeyboardHandler, Keysym};
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind, PointerHandler};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
    LayerSurfaceConfigure,
};
use smithay_client_toolkit::shell::xdg::popup::{Popup, PopupConfigure, PopupHandler};
use smithay_client_toolkit::shell::xdg::window::{Window, WindowConfigure, WindowHandler};
use smithay_client_toolkit::shell::xdg::XdgPositioner;
use smithay_client_toolkit::shell::xdg::XdgShell;
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shm::slot::SlotPool;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm, delegate_xdg_popup, delegate_xdg_shell,
    registry_handlers,
};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

const PILL_W: i32 = 680;
const PILL_H: i32 = 74;

fn main() {
    let path = PathBuf::from(std::env::args().nth(1).expect("result file"));
    let conn = Connection::connect_to_env().expect("wayland");
    let (globals, mut queue) = registry_queue_init(&conn).expect("globals");
    let qh = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh).expect("compositor");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("layer");
    let xdg = XdgShell::bind(&globals, &qh).expect("xdg");
    let shm = Shm::bind(&globals, &qh).expect("shm");
    let surface = compositor.create_surface(&qh);
    let layer =
        layer_shell.create_layer_surface(&qh, surface, Layer::Top, Some("omarchy-menu-rust"), None);
    layer.set_anchor(Anchor::TOP);
    layer.set_margin(36, 0, 0, 0);
    layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
    layer.set_exclusive_zone(0);
    layer.set_size(PILL_W as u32, PILL_H as u32);
    layer.commit();
    let pool = SlotPool::new((PILL_W * PILL_H * 4) as usize, &shm).expect("pool");
    let mut app = Probe {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        compositor,
        xdg,
        shm,
        pool,
        layer,
        keyboard: None,
        pointer: None,
        seat: None,
        path,
        enter_serial: None,
        grab_sent: false,
        popup: None,
        exit: false,
    };
    app.log("started");
    while !app.exit {
        if let Err(err) = queue.blocking_dispatch(&mut app) {
            app.log(&format!("dispatch-error {err}"));
            break;
        }
    }
}

struct Probe {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor: CompositorState,
    xdg: XdgShell,
    shm: Shm,
    pool: SlotPool,
    layer: LayerSurface,
    keyboard: Option<WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    seat: Option<wl_seat::WlSeat>,
    path: PathBuf,
    enter_serial: Option<u32>,
    grab_sent: bool,
    popup: Option<Popup>,
    exit: bool,
}

impl Probe {
    fn log(&self, line: &str) {
        eprintln!("popup-grab-probe: {line}");
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = writeln!(file, "{line}");
        }
    }

    fn paint_layer(&mut self, qh: &QueueHandle<Self>) {
        let surface = self.layer.wl_surface().clone();
        self.paint_into(&surface, qh, false);
        self.layer.commit();
    }

    fn paint_into(&mut self, surface: &wl_surface::WlSurface, qh: &QueueHandle<Self>, popup: bool) {
        let stride = PILL_W * 4;
        let Ok((buffer, canvas)) =
            self.pool
                .create_buffer(PILL_W, PILL_H, stride, wl_shm::Format::Argb8888)
        else {
            return;
        };
        for px in canvas.as_chunks_mut::<4>().0 {
            if popup {
                px.copy_from_slice(&[0, 0, 0, 0]);
            } else {
                px.copy_from_slice(&[20, 24, 28, 196]);
            }
        }
        surface.damage_buffer(0, 0, PILL_W, PILL_H);
        surface.frame(qh, surface.clone());
        let _ = buffer.attach_to(surface);
        surface.commit();
    }

    fn try_grab(&mut self, qh: &QueueHandle<Self>, serial: u32, source: &str) {
        if self.grab_sent {
            return;
        }
        let Some(seat) = self.seat.clone() else {
            self.log("grab-skipped no-seat");
            return;
        };
        let Ok(positioner) = XdgPositioner::new(&self.xdg) else {
            self.log("grab-skipped no-positioner");
            return;
        };
        positioner.set_size(PILL_W, PILL_H);
        positioner.set_anchor_rect(0, 0, PILL_W, PILL_H);
        let surface = self.compositor.create_surface(qh);
        let popup = match Popup::from_surface(None, &positioner, qh, surface, &self.xdg) {
            Ok(popup) => popup,
            Err(err) => {
                self.log(&format!("grab-skipped popup-create {err}"));
                return;
            }
        };
        self.layer.get_popup(popup.xdg_popup());
        popup.xdg_popup().grab(&seat, serial);
        popup.wl_surface().commit();
        self.popup = Some(popup);
        self.grab_sent = true;
        self.log(&format!("grab-sent serial={serial} source={source}"));
    }
}

impl CompositorHandler for Probe {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _: u32,
    ) {
        if self
            .popup
            .as_ref()
            .is_some_and(|popup| popup.wl_surface() == surface)
        {
            return;
        }
        self.paint_layer(qh);
    }
    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for Probe {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl LayerShellHandler for Probe {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.log("layer-closed");
        self.exit = true;
    }
    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &LayerSurface,
        _: LayerSurfaceConfigure,
        _: u32,
    ) {
        self.paint_layer(qh);
    }
}

impl SeatHandler for Probe {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        self.seat = Some(seat);
    }
    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        self.seat = Some(seat.clone());
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            if let Ok(keyboard) = self.seat_state.get_keyboard(qh, &seat, None) {
                self.keyboard = Some(keyboard);
            }
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            if let Ok(pointer) = self.seat_state.get_pointer(qh, &seat) {
                self.pointer = Some(pointer);
            }
        }
    }
    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            if let Some(keyboard) = self.keyboard.take() {
                keyboard.release();
            }
        }
        if capability == Capability::Pointer {
            if let Some(pointer) = self.pointer.take() {
                pointer.release();
            }
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for Probe {
    fn enter(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: &wl_surface::WlSurface,
        serial: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
        self.enter_serial = Some(serial);
        self.log(&format!("keyboard-enter serial={serial}"));
        self.try_grab(qh, serial, "keyboard-enter");
    }
    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
    }
    fn press_key(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &WlKeyboard,
        serial: u32,
        event: smithay_client_toolkit::seat::keyboard::KeyEvent,
    ) {
        self.log(&format!("key serial={serial} sym={:?}", event.keysym));
        if !self.grab_sent {
            self.try_grab(qh, serial, "key-press");
        }
    }
    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: u32,
        _: smithay_client_toolkit::seat::keyboard::KeyEvent,
    ) {
    }
    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: u32,
        _: smithay_client_toolkit::seat::keyboard::Modifiers,
        _: u32,
    ) {
    }
}

impl PointerHandler for Probe {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            if let PointerEventKind::Press { serial, .. } = event.kind {
                self.log(&format!(
                    "pointer-press serial={serial} at {},{}",
                    event.position.0, event.position.1
                ));
            }
        }
    }
}

impl PopupHandler for Probe {
    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        popup: &Popup,
        config: PopupConfigure,
    ) {
        self.log(&format!(
            "popup-configure {}x{} serial={}",
            config.width, config.height, config.serial
        ));
        let surface = popup.wl_surface().clone();
        self.paint_into(&surface, qh, true);
    }
    fn done(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Popup) {
        self.log("popup-done");
        self.popup = None;
    }
}

impl WindowHandler for Probe {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {}
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &Window,
        _: WindowConfigure,
        _: u32,
    ) {
    }
}

impl ShmHandler for Probe {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_compositor!(Probe);
delegate_output!(Probe);
delegate_shm!(Probe);
delegate_seat!(Probe);
delegate_keyboard!(Probe);
delegate_pointer!(Probe);
delegate_layer!(Probe);
delegate_xdg_shell!(Probe);
delegate_xdg_popup!(Probe);
delegate_registry!(Probe);

impl ProvidesRegistryState for Probe {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}
