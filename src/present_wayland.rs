//! Panel-sized wlr-layer-shell surface. The buffer is premultiplied ARGB8888.
//!
//! A visible dim layer covers the focused output and takes presses outside the
//! pill. Sway 1.9 unconstrains a layer-shell popup before the client's first
//! commit, which logs an uninitialized xdg_surface configure. This client does
//! not create that popup. The dim is the disclosed surface, and it is destroyed
//! with this process.

use super::{paint_model, pill_size, prepare_model, Menu, Options};
use crate::place::{place_panel, OutputBox};
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::client::globals::registry_queue_init;
use smithay_client_toolkit::reexports::client::protocol::{
    wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface,
};
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::keyboard::{KeyEvent, KeyboardHandler, Keysym};
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind, PointerHandler};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
    LayerSurfaceConfigure,
};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shm::slot::SlotPool;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm, registry_handlers,
};

pub fn run(menu: Menu, opts: Options) -> Result<(), String> {
    eprintln!(
        "omarchy-menu-rust: Wayland layer-shell namespace omarchy-menu-rust. The panel is centered on the focused output. A visible dim layer omarchy-menu-rust-dim sits behind it on that output only and takes a press outside the pill. No xdg_popup is created."
    );
    let conn = Connection::connect_to_env().map_err(|err| err.to_string())?;
    let (globals, mut queue) = registry_queue_init(&conn).map_err(|err| err.to_string())?;
    let qh = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh).map_err(|err| err.to_string())?;
    let layer_shell = LayerShell::bind(&globals, &qh).map_err(|err| err.to_string())?;
    let shm = Shm::bind(&globals, &qh).map_err(|err| err.to_string())?;
    let surface = compositor.create_surface(&qh);
    let layer = layer_shell.create_layer_surface(
        &qh,
        surface,
        Layer::Overlay,
        Some("omarchy-menu-rust"),
        None,
    );
    let model = prepare_model(menu, &opts);
    let (width, height) = pill_size(&model);
    // Top and left, not opposite edges. Opposite edges would stretch the panel.
    // exclusive zone -1 asks the compositor not to shove the panel below a bar.
    layer.set_anchor(Anchor::TOP | Anchor::LEFT);
    layer.set_margin(0, 0, 0, 0);
    layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
    layer.set_exclusive_zone(-1);
    layer.set_size(width, height);
    layer.commit();
    let pool = SlotPool::new((width * height * 4) as usize, &shm).map_err(|err| err.to_string())?;
    let mut app = Pill {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        compositor,
        layer_shell,
        shm,
        exit: false,
        first: true,
        pool,
        width,
        height,
        layer,
        dim: None,
        focus_output: None,
        output_logical: None,
        output_scale: 1,
        centered_logged: false,
        keyboard: None,
        pointer: None,
        model,
        opts,
        wrote_shot: false,
        traced: false,
    };
    while !app.exit {
        queue
            .blocking_dispatch(&mut app)
            .map_err(|err| err.to_string())?;
        if app.model.should_close {
            app.exit = true;
        }
    }
    Ok(())
}

struct Pill {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    exit: bool,
    first: bool,
    pool: SlotPool,
    width: u32,
    height: u32,
    layer: LayerSurface,
    dim: Option<LayerSurface>,
    focus_output: Option<wl_output::WlOutput>,
    output_logical: Option<(i32, i32)>,
    output_scale: i32,
    centered_logged: bool,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    model: super::Model,
    opts: Options,
    wrote_shot: bool,
    traced: bool,
}

impl Pill {
    fn apply_geometry(&mut self) {
        let (panel_w, panel_h) = pill_size(&self.model);
        let (logical_w, logical_h) = self
            .output_logical
            .unwrap_or((panel_w as i32, panel_h as i32));
        let placed = place_panel(
            OutputBox {
                origin_x: 0,
                origin_y: 0,
                logical_w: logical_w.max(1),
                logical_h: logical_h.max(1),
            },
            panel_w,
            panel_h,
        );
        self.layer.set_anchor(Anchor::TOP | Anchor::LEFT);
        self.layer.set_exclusive_zone(-1);
        self.layer.set_margin(placed.y, 0, 0, placed.x);
        self.layer.set_size(placed.width, placed.height);
        self.width = placed.width.max(64);
        self.height = placed.height.max(72);
    }

    fn draw(&mut self, qh: &QueueHandle<Self>) {
        self.apply_geometry();
        let scale = u32::try_from(self.output_scale.max(1)).unwrap_or(1);
        let _ = self.layer.set_buffer_scale(scale);
        let buf_w = self.width.saturating_mul(scale).max(1);
        let buf_h = self.height.saturating_mul(scale).max(1);
        let stride = buf_w as i32 * 4;
        let (buffer, canvas) = match self.pool.create_buffer(
            buf_w as i32,
            buf_h as i32,
            stride,
            wl_shm::Format::Argb8888,
        ) {
            Ok(buf) => buf,
            Err(err) => {
                eprintln!("omarchy-menu-rust: shm buffer failed: {err}");
                return;
            }
        };
        let pixmap = paint_model(&self.model, self.width, self.height);
        for (i, px) in pixmap.pixels().iter().enumerate() {
            let x = (i as u32) % self.width;
            let y = (i as u32) / self.width;
            for dy in 0..scale {
                for dx in 0..scale {
                    let start = (((y * scale + dy) * buf_w + (x * scale + dx)) * 4) as usize;
                    if start + 3 >= canvas.len() {
                        continue;
                    }
                    canvas[start] = px.blue();
                    canvas[start + 1] = px.green();
                    canvas[start + 2] = px.red();
                    canvas[start + 3] = px.alpha();
                }
            }
        }
        if let Some(path) = &self.opts.screenshot {
            if !self.wrote_shot {
                let _ = super::write_png(path, &pixmap);
                self.wrote_shot = true;
            }
        }
        self.layer
            .wl_surface()
            .damage_buffer(0, 0, buf_w as i32, buf_h as i32);
        self.layer
            .wl_surface()
            .frame(qh, self.layer.wl_surface().clone());
        let _ = buffer.attach_to(self.layer.wl_surface());
        self.layer.commit();
        if self.opts.trace_state && !self.traced {
            self.traced = true;
            super::emit_state_trace(&self.model);
        }
        if self.opts.exit_after_first_frame && (self.opts.screenshot.is_none() || self.wrote_shot) {
            self.exit = true;
        }
    }

    fn trace_state(&self) {
        if self.opts.trace_state {
            super::emit_state_trace(&self.model);
        }
    }

    fn note_output(&mut self, qh: &QueueHandle<Self>, output: &wl_output::WlOutput) {
        let Some(info) = self.output_state.info(output) else {
            return;
        };
        let Some((origin_x, origin_y, logical_w, logical_h, scale)) = output_box(&info) else {
            return;
        };
        self.output_logical = Some((logical_w, logical_h));
        self.output_scale = scale;
        if !self.centered_logged {
            let (panel_w, panel_h) = pill_size(&self.model);
            let placed = place_panel(
                OutputBox {
                    origin_x,
                    origin_y,
                    logical_w,
                    logical_h,
                },
                panel_w,
                panel_h,
            );
            eprintln!(
                "omarchy-menu-rust: center output {:?} logical {}x{} origin ({},{}) scale {} panel {}x{} at ({},{})",
                info.name,
                logical_w,
                logical_h,
                origin_x,
                origin_y,
                scale,
                placed.width,
                placed.height,
                placed.x,
                placed.y
            );
            self.centered_logged = true;
        }
        self.spawn_dim(qh, output);
        self.draw(qh);
    }

    fn spawn_dim(&mut self, qh: &QueueHandle<Self>, output: &wl_output::WlOutput) {
        if self.dim.is_some() {
            return;
        }
        let surface = self.compositor.create_surface(qh);
        let dim = self.layer_shell.create_layer_surface(
            qh,
            surface,
            Layer::Top,
            Some("omarchy-menu-rust-dim"),
            Some(output),
        );
        dim.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        dim.set_exclusive_zone(-1);
        dim.set_keyboard_interactivity(KeyboardInteractivity::None);
        dim.set_size(0, 0);
        dim.commit();
        eprintln!(
            "omarchy-menu-rust: visible dim layer omarchy-menu-rust-dim on the focused output. It is behind the menu. A press on it closes the menu. It is removed when this process exits."
        );
        self.dim = Some(dim);
    }

    fn paint_dim(&mut self, qh: &QueueHandle<Self>, size: (u32, u32)) {
        let Some(dim) = self.dim.as_ref() else {
            return;
        };
        let surface = dim.wl_surface().clone();
        let (mut width, mut height) = size;
        if width == 0 || height == 0 {
            let Some((logical_w, logical_h)) = self.output_logical else {
                return;
            };
            width = logical_w.max(1) as u32;
            height = logical_h.max(1) as u32;
        }
        let stride = width as i32 * 4;
        let Ok((buffer, canvas)) = self.pool.create_buffer(
            width as i32,
            height as i32,
            stride,
            wl_shm::Format::Argb8888,
        ) else {
            return;
        };
        for px in canvas.as_chunks_mut::<4>().0 {
            px.copy_from_slice(&[0, 0, 0, 112]);
        }
        surface.damage_buffer(0, 0, width as i32, height as i32);
        surface.frame(qh, surface.clone());
        let _ = buffer.attach_to(&surface);
        if let Some(dim) = self.dim.as_ref() {
            dim.commit();
        }
    }
}

impl CompositorHandler for Pill {
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
        self.draw(qh);
    }
    fn surface_enter(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        output: &wl_output::WlOutput,
    ) {
        if surface != self.layer.wl_surface() {
            return;
        }
        self.focus_output = Some(output.clone());
        self.note_output(qh, output);
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

impl OutputHandler for Pill {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if self
            .focus_output
            .as_ref()
            .is_some_and(|focus| focus == &output)
        {
            self.note_output(qh, &output);
        }
    }
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl LayerShellHandler for Pill {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.exit = true;
    }
    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _: u32,
    ) {
        let dim_surface = self
            .dim
            .as_ref()
            .is_some_and(|dim| dim.wl_surface() == layer.wl_surface());
        if dim_surface {
            self.paint_dim(qh, configure.new_size);
            return;
        }
        if self.first {
            self.first = false;
            self.draw(qh);
        }
    }
}

impl SeatHandler for Pill {
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

impl KeyboardHandler for Pill {
    fn enter(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        serial: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
        eprintln!("omarchy-menu-rust: keyboard-enter serial={serial}");
        let _ = qh;
    }
    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
    }
    fn press_key(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        let sym = format!("{:?}", event.keysym);
        let named = match event.keysym {
            Keysym::Escape => "Escape",
            Keysym::Return | Keysym::KP_Enter => "Return",
            Keysym::Up => "Up",
            Keysym::Down => "Down",
            Keysym::BackSpace => "BackSpace",
            _ if sym.contains("Return") || sym.contains("KP_Enter") => "Return",
            _ if sym.contains("Escape") => "Escape",
            _ if sym.contains("BackSpace") || sym.contains("Backspace") => "BackSpace",
            _ if sym.contains("Down") => "Down",
            _ if sym.contains("Up") => "Up",
            _ => "",
        };
        let named = if named.is_empty() && matches!(event.utf8.as_deref(), Some("\r" | "\n")) {
            "Return"
        } else {
            named
        };
        if named.is_empty() {
            self.model.wayland_key("", event.utf8.as_deref(), false);
        } else {
            self.model.wayland_key(named, None, false);
        }
        self.trace_state();
        if self.model.should_close {
            self.exit = true;
        }
        self.draw(qh);
    }
    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }
    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: smithay_client_toolkit::seat::keyboard::Modifiers,
        _: u32,
    ) {
    }
}

impl PointerHandler for Pill {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            let on_dim = self
                .dim
                .as_ref()
                .is_some_and(|dim| dim.wl_surface() == &event.surface);
            if on_dim {
                if let PointerEventKind::Press { .. } = event.kind {
                    eprintln!("omarchy-menu-rust: dim press dismissed the menu");
                    self.model.should_close = true;
                    self.exit = true;
                }
                continue;
            }
            if &event.surface != self.layer.wl_surface() {
                continue;
            }
            let (x, y) = (event.position.0, event.position.1);
            match event.kind {
                PointerEventKind::Motion { .. } | PointerEventKind::Enter { .. } => {
                    self.model.highlight_at(
                        x as f32,
                        y as f32,
                        self.width as f32,
                        self.height as f32,
                    );
                }
                PointerEventKind::Press { .. } => {
                    let width = self.width as f32;
                    let height = self.height as f32;
                    if let Some(focus) =
                        super::confirm_button_at(&self.model, x as f32, y as f32, width, height)
                    {
                        if let Some(confirm) = &mut self.model.confirm {
                            confirm.focus = focus;
                        }
                        self.model.enter(|_| {});
                    } else if let Some(index) =
                        super::row_index_at(&self.model, x as f32, y as f32, width, height)
                    {
                        self.model.cursor = index;
                        self.model.confirm = None;
                        self.model.enter(|_| {});
                    }
                    self.trace_state();
                    if self.model.should_close {
                        self.exit = true;
                    }
                    self.draw(qh);
                }
                _ => {}
            }
        }
    }
}

impl ShmHandler for Pill {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

fn output_box(
    info: &smithay_client_toolkit::output::OutputInfo,
) -> Option<(i32, i32, i32, i32, i32)> {
    let scale = info.scale_factor.max(1);
    if let Some(size) = info.logical_size {
        let origin = info.logical_position.unwrap_or(info.location);
        return Some((origin.0, origin.1, size.0.max(1), size.1.max(1), scale));
    }
    let mode = info
        .modes
        .iter()
        .find(|mode| mode.current)
        .or_else(|| info.modes.first())?;
    let origin = info.logical_position.unwrap_or(info.location);
    Some((
        origin.0,
        origin.1,
        crate::place::logical_len(mode.dimensions.0, scale as f64),
        crate::place::logical_len(mode.dimensions.1, scale as f64),
        scale,
    ))
}

delegate_compositor!(Pill);
delegate_output!(Pill);
delegate_shm!(Pill);
delegate_seat!(Pill);
delegate_keyboard!(Pill);
delegate_pointer!(Pill);
delegate_layer!(Pill);
delegate_registry!(Pill);

impl ProvidesRegistryState for Pill {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}
