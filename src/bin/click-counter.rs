//! Underlying xdg window that counts pointer clicks.
//! Usage: click-counter COUNT_FILE

use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::client::globals::registry_queue_init;
use smithay_client_toolkit::reexports::client::protocol::{
    wl_output, wl_pointer, wl_seat, wl_shm, wl_surface,
};
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind, PointerHandler};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::shell::xdg::window::{
    Window, WindowConfigure, WindowDecorations, WindowHandler,
};
use smithay_client_toolkit::shell::xdg::XdgShell;
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shm::slot::SlotPool;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{
    delegate_compositor, delegate_output, delegate_pointer, delegate_registry, delegate_seat,
    delegate_shm, delegate_xdg_shell, delegate_xdg_window, registry_handlers,
};
use std::fs;
use std::path::PathBuf;

fn main() {
    let path = PathBuf::from(std::env::args().nth(1).expect("count file"));
    let conn = Connection::connect_to_env().expect("wayland");
    let (globals, mut queue) = registry_queue_init(&conn).expect("globals");
    let qh = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh).expect("compositor");
    let xdg = XdgShell::bind(&globals, &qh).expect("xdg");
    let shm = Shm::bind(&globals, &qh).expect("shm");
    let surface = compositor.create_surface(&qh);
    let window = xdg.create_window(surface, WindowDecorations::None, &qh);
    window.set_title("click-counter");
    window.set_app_id("omarchy-menu-rust.click-counter");
    window.commit();
    let pool = SlotPool::new(1280 * 720 * 4, &shm).expect("pool");
    let mut app = Counter {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,
        pool,
        window,
        pointer: None,
        path,
        clicks: 0,
        width: 1280,
        height: 720,
        exit: false,
        configured: false,
    };
    while !app.exit {
        if queue.blocking_dispatch(&mut app).is_err() {
            break;
        }
    }
}

struct Counter {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    pool: SlotPool,
    window: Window,
    pointer: Option<wl_pointer::WlPointer>,
    path: PathBuf,
    clicks: u32,
    width: u32,
    height: u32,
    exit: bool,
    configured: bool,
}

impl Counter {
    fn paint(&mut self, qh: &QueueHandle<Self>) {
        let stride = self.width as i32 * 4;
        let Ok((buffer, canvas)) = self.pool.create_buffer(
            self.width as i32,
            self.height as i32,
            stride,
            wl_shm::Format::Argb8888,
        ) else {
            return;
        };
        for px in canvas.as_chunks_mut::<4>().0 {
            px.copy_from_slice(&[48, 64, 96, 255]);
        }
        let surface = self.window.wl_surface();
        surface.damage_buffer(0, 0, self.width as i32, self.height as i32);
        surface.frame(qh, surface.clone());
        let _ = buffer.attach_to(surface);
        surface.commit();
    }

    fn write(&self) {
        let _ = fs::write(&self.path, format!("{}\n", self.clicks));
    }
}

impl CompositorHandler for Counter {
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
    fn frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        self.paint(qh);
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

impl OutputHandler for Counter {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl WindowHandler for Counter {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.exit = true;
    }
    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &Window,
        configure: WindowConfigure,
        _: u32,
    ) {
        if let Some(w) = configure.new_size.0 {
            self.width = w.get();
        }
        if let Some(h) = configure.new_size.1 {
            self.height = h.get();
        }
        if !self.configured {
            self.configured = true;
            eprintln!("click-counter: configured {}x{}", self.width, self.height);
            self.write();
        }
        self.paint(qh);
    }
}

impl SeatHandler for Counter {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
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
        if capability == Capability::Pointer {
            if let Some(pointer) = self.pointer.take() {
                pointer.release();
            }
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for Counter {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    eprintln!(
                        "click-counter: enter {},{}",
                        event.position.0, event.position.1
                    );
                }
                PointerEventKind::Press { .. } => {
                    self.clicks += 1;
                    self.write();
                    eprintln!(
                        "click-counter: {} at {},{}",
                        self.clicks, event.position.0, event.position.1
                    );
                }
                _ => {}
            }
        }
    }
}

impl ShmHandler for Counter {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_compositor!(Counter);
delegate_output!(Counter);
delegate_shm!(Counter);
delegate_seat!(Counter);
delegate_pointer!(Counter);
delegate_xdg_shell!(Counter);
delegate_xdg_window!(Counter);
delegate_registry!(Counter);

impl ProvidesRegistryState for Counter {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}
