//! One virtual-pointer click at an absolute output position.
//! Usage: seat-click X Y WIDTH HEIGHT

use smithay_client_toolkit::reexports::client::protocol::wl_pointer::ButtonState;
use smithay_client_toolkit::reexports::client::protocol::{wl_registry, wl_seat};
use smithay_client_toolkit::reexports::client::{Connection, Dispatch, Proxy, QueueHandle};
use smithay_client_toolkit::reexports::protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
    zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
};

struct Clicker {
    manager: Option<ZwlrVirtualPointerManagerV1>,
    seat: Option<wl_seat::WlSeat>,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let first = args.next();
    let hold = first.as_deref() == Some("hold");
    let x: u32 = (if hold { args.next() } else { first })
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    let y: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(400);
    let width: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1280);
    let height: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(720);
    let conn = Connection::connect_to_env().expect("wayland");
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let _registry = conn.display().get_registry(&qh, ());
    let mut state = Clicker {
        manager: None,
        seat: None,
    };
    queue.roundtrip(&mut state).expect("registry");
    let manager = state.manager.take().expect("virtual pointer");
    let seat = state.seat.take().expect("seat");
    let pointer = manager.create_virtual_pointer(Some(&seat), &qh, ());
    queue.roundtrip(&mut state).expect("pointer");
    let time = 1u32;
    pointer.motion_absolute(time, x, y, width, height);
    pointer.frame();
    queue.roundtrip(&mut state).expect("move");
    if hold {
        println!("holding {x},{y}");
        let _ = queue.roundtrip(&mut state);
        std::thread::park();
        return;
    }
    pointer.button(time, 0x110, ButtonState::Pressed);
    pointer.frame();
    pointer.button(time, 0x110, ButtonState::Released);
    pointer.frame();
    queue.roundtrip(&mut state).expect("click");
    println!("clicked {x},{y} on {width}x{height}");
}

impl Dispatch<wl_registry::WlRegistry, ()> for Clicker {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == ZwlrVirtualPointerManagerV1::interface().name {
                state.manager = Some(registry.bind(name, version.clamp(1, 2), qh, ()));
            } else if interface == wl_seat::WlSeat::interface().name {
                state.seat = Some(registry.bind(name, version.clamp(1, 7), qh, ()));
            }
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for Clicker {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrVirtualPointerManagerV1, ()> for Clicker {
    fn event(
        _: &mut Self,
        _: &ZwlrVirtualPointerManagerV1,
        _: <ZwlrVirtualPointerManagerV1 as smithay_client_toolkit::reexports::client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrVirtualPointerV1, ()> for Clicker {
    fn event(
        _: &mut Self,
        _: &ZwlrVirtualPointerV1,
        _: <ZwlrVirtualPointerV1 as smithay_client_toolkit::reexports::client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
