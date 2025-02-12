use std::sync::{Arc, Mutex};

use crate::event::{
    ElementState, ModifiersState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent,
};

use super::{event_loop::WindowEventsSink, window::WindowStore, DeviceId};

use smithay_client_toolkit::reexports::client::protocol::{
    wl_pointer::{self, Event as PtrEvent, WlPointer},
    wl_seat,
};

pub fn implement_pointer(
    seat: &wl_seat::WlSeat,
    sink: Arc<Mutex<WindowEventsSink>>,
    store: Arc<Mutex<WindowStore>>,
    modifiers_tracker: Arc<Mutex<ModifiersState>>,
) -> WlPointer {
    let mut mouse_focus = None;
    let mut axis_buffer = None;
    let mut axis_discrete_buffer = None;
    let mut axis_state = TouchPhase::Ended;

    seat.get_pointer(|pointer| {
        pointer.implement_closure(
            move |evt, pointer| {
                let mut sink = sink.lock().unwrap();
                let store = store.lock().unwrap();
                match evt {
                    PtrEvent::Enter {
                        surface,
                        surface_x,
                        surface_y,
                        ..
                    } => {
                        let wid = store.find_wid(&surface);
                        if let Some(wid) = wid {
                            mouse_focus = Some(wid);
                            sink.send_event(
                                WindowEvent::CursorEntered {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Wayland(DeviceId),
                                    ),
                                },
                                wid,
                            );
                            sink.send_event(
                                WindowEvent::CursorMoved {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Wayland(DeviceId),
                                    ),
                                    position: (surface_x, surface_y).into(),
                                    modifiers: modifiers_tracker.lock().unwrap().clone(),
                                },
                                wid,
                            );
                        }
                    },
                    PtrEvent::Leave { surface, .. } => {
                        mouse_focus = None;
                        let wid = store.find_wid(&surface);
                        if let Some(wid) = wid {
                            sink.send_event(
                                WindowEvent::CursorLeft {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Wayland(DeviceId),
                                    ),
                                },
                                wid,
                            );
                        }
                    },
                    PtrEvent::Motion {
                        surface_x,
                        surface_y,
                        ..
                    } => {
                        if let Some(wid) = mouse_focus {
                            sink.send_event(
                                WindowEvent::CursorMoved {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Wayland(DeviceId),
                                    ),
                                    position: (surface_x, surface_y).into(),
                                    modifiers: modifiers_tracker.lock().unwrap().clone(),
                                },
                                wid,
                            );
                        }
                    },
                    PtrEvent::Button { button, state, .. } => {
                        if let Some(wid) = mouse_focus {
                            let state = match state {
                                wl_pointer::ButtonState::Pressed => ElementState::Pressed,
                                wl_pointer::ButtonState::Released => ElementState::Released,
                                _ => unreachable!(),
                            };
                            let button = match button {
                                0x110 => MouseButton::Left,
                                0x111 => MouseButton::Right,
                                0x112 => MouseButton::Middle,
                                // TODO figure out the translation ?
                                _ => return,
                            };
                            sink.send_event(
                                WindowEvent::MouseInput {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Wayland(DeviceId),
                                    ),
                                    state,
                                    button,
                                    modifiers: modifiers_tracker.lock().unwrap().clone(),
                                },
                                wid,
                            );
                        }
                    },
                    PtrEvent::Axis { axis, value, .. } => {
                        if let Some(wid) = mouse_focus {
                            if pointer.as_ref().version() < 5 {
                                let (mut x, mut y) = (0.0, 0.0);
                                // old seat compatibility
                                match axis {
                                    // wayland vertical sign convention is the inverse of winit
                                    wl_pointer::Axis::VerticalScroll => y -= value as f32,
                                    wl_pointer::Axis::HorizontalScroll => x += value as f32,
                                    _ => unreachable!(),
                                }
                                sink.send_event(
                                    WindowEvent::MouseWheel {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Wayland(DeviceId),
                                        ),
                                        delta: MouseScrollDelta::PixelDelta(
                                            (x as f64, y as f64).into(),
                                        ),
                                        phase: TouchPhase::Moved,
                                        modifiers: modifiers_tracker.lock().unwrap().clone(),
                                    },
                                    wid,
                                );
                            } else {
                                let (mut x, mut y) = axis_buffer.unwrap_or((0.0, 0.0));
                                match axis {
                                    // wayland vertical sign convention is the inverse of winit
                                    wl_pointer::Axis::VerticalScroll => y -= value as f32,
                                    wl_pointer::Axis::HorizontalScroll => x += value as f32,
                                    _ => unreachable!(),
                                }
                                axis_buffer = Some((x, y));
                                axis_state = match axis_state {
                                    TouchPhase::Started | TouchPhase::Moved => TouchPhase::Moved,
                                    _ => TouchPhase::Started,
                                }
                            }
                        }
                    },
                    PtrEvent::Frame => {
                        let axis_buffer = axis_buffer.take();
                        let axis_discrete_buffer = axis_discrete_buffer.take();
                        if let Some(wid) = mouse_focus {
                            if let Some((x, y)) = axis_discrete_buffer {
                                sink.send_event(
                                    WindowEvent::MouseWheel {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Wayland(DeviceId),
                                        ),
                                        delta: MouseScrollDelta::LineDelta(x as f32, y as f32),
                                        phase: axis_state,
                                        modifiers: modifiers_tracker.lock().unwrap().clone(),
                                    },
                                    wid,
                                );
                            } else if let Some((x, y)) = axis_buffer {
                                sink.send_event(
                                    WindowEvent::MouseWheel {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Wayland(DeviceId),
                                        ),
                                        delta: MouseScrollDelta::PixelDelta(
                                            (x as f64, y as f64).into(),
                                        ),
                                        phase: axis_state,
                                        modifiers: modifiers_tracker.lock().unwrap().clone(),
                                    },
                                    wid,
                                );
                            }
                        }
                    },
                    PtrEvent::AxisSource { .. } => (),
                    PtrEvent::AxisStop { .. } => {
                        axis_state = TouchPhase::Ended;
                    },
                    PtrEvent::AxisDiscrete { axis, discrete } => {
                        let (mut x, mut y) = axis_discrete_buffer.unwrap_or((0, 0));
                        match axis {
                            // wayland vertical sign convention is the inverse of winit
                            wl_pointer::Axis::VerticalScroll => y -= discrete,
                            wl_pointer::Axis::HorizontalScroll => x += discrete,
                            _ => unreachable!(),
                        }
                        axis_discrete_buffer = Some((x, y));
                        axis_state = match axis_state {
                            TouchPhase::Started | TouchPhase::Moved => TouchPhase::Moved,
                            _ => TouchPhase::Started,
                        }
                    },
                    _ => unreachable!(),
                }
            },
            (),
        )
    })
    .unwrap()
}
