//! Incoming local URI drops for the Slint-compatible Winit 0.30 backend.
//! No clipboard data is read and no transfer is started before an actual drop.

use std::fs::File;
use std::io::{ErrorKind, Read};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sctk::data_device_manager::data_device::{DataDeviceData, DataDeviceHandler};
use sctk::data_device_manager::data_offer::{DataOfferHandler, DragOffer};
use sctk::data_device_manager::data_source::DataSourceHandler;
use sctk::data_device_manager::WritePipe;
use sctk::reexports::calloop::timer::{TimeoutAction, Timer};
use sctk::reexports::client::protocol::{
    wl_data_device::WlDataDevice, wl_data_device_manager::DndAction, wl_data_source::WlDataSource,
    wl_surface::WlSurface,
};
use sctk::reexports::client::{Connection, Proxy, QueueHandle};

use super::state::WinitState;
use super::{make_wid, WindowId};
use crate::event::WindowEvent;

const MIME: &str = "text/uri-list";
#[path = "uri_list.rs"]
mod uri_list;
use uri_list::{local_paths, MAX_BYTES};
const MAX_TRANSFERS: usize = 4;

/// Keep each transfer's lifetime tied to the event loop, including shutdown.
struct Transfer {
    file: File,
    offer: DragOffer,
    window: WindowId,
    bytes: Vec<u8>,
    deadline: Instant,
    count: Arc<AtomicUsize>,
}

impl Drop for Transfer {
    fn drop(&mut self) {
        self.offer.destroy();
        self.count.fetch_sub(1, Ordering::Relaxed);
    }
}

impl DataDeviceHandler for WinitState {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        device: &WlDataDevice,
        _: f64,
        _: f64,
        surface: &WlSurface,
    ) {
        let Some(offer) = device.data::<DataDeviceData>().and_then(|d| d.drag_offer()) else {
            return;
        };
        let accepted = self.windows.borrow().contains_key(&make_wid(surface))
            && offer.with_mime_types(|types| types.iter().any(|mime| mime == MIME));
        offer.accept_mime_type(offer.serial, accepted.then(|| MIME.to_owned()));
        offer.set_actions(
            if accepted {
                DndAction::Copy
            } else {
                DndAction::empty()
            },
            if accepted {
                DndAction::Copy
            } else {
                DndAction::empty()
            },
        );
    }

    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}
    fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice, _: f64, _: f64) {}
    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}

    fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, device: &WlDataDevice) {
        let Some(offer) = device.data::<DataDeviceData>().and_then(|d| d.drag_offer()) else {
            return;
        };
        let window = make_wid(&offer.surface);
        if !self.windows.borrow().contains_key(&window)
            || !offer.with_mime_types(|types| types.iter().any(|mime| mime == MIME))
            || (offer.inner().version() >= 3 && offer.selected_action != DndAction::Copy)
            || self.pending_file_drops.load(Ordering::Relaxed) >= MAX_TRANSFERS
        {
            offer.destroy();
            return;
        }
        let Ok(pipe) = offer.receive(MIME.to_owned()) else {
            offer.destroy();
            return;
        };
        let file = File::from(std::os::fd::OwnedFd::from(pipe));
        let Ok(flags) = rustix::fs::fcntl_getfl(&file) else {
            offer.destroy();
            return;
        };
        if rustix::fs::fcntl_setfl(&file, flags | rustix::fs::OFlags::NONBLOCK).is_err() {
            offer.destroy();
            return;
        }
        self.pending_file_drops.fetch_add(1, Ordering::Relaxed);
        let mut transfer = Transfer {
            file,
            offer,
            window,
            bytes: Vec::new(),
            deadline: Instant::now() + Duration::from_secs(10),
            count: self.pending_file_drops.clone(),
        };
        // A bounded nonblocking timer avoids blocking the Wayland/UI thread when
        // a drag source stalls or sends only part of its URI list.
        let result = self.loop_handle.insert_source(
            Timer::from_duration(Duration::ZERO),
            move |_, _, state| {
                if Instant::now() >= transfer.deadline
                    || !state.windows.borrow().contains_key(&transfer.window)
                {
                    return TimeoutAction::Drop;
                }
                let mut chunk = [0u8; 4096];
                for _ in 0..16 {
                    match transfer.file.read(&mut chunk) {
                        Ok(0) => {
                            if let Some(paths) = local_paths(&transfer.bytes) {
                                for path in paths {
                                    state.events_sink.push_window_event(
                                        WindowEvent::DroppedFile(path),
                                        transfer.window,
                                    );
                                }
                                state.dispatched_events = true;
                                transfer.offer.finish();
                            }
                            return TimeoutAction::Drop;
                        }
                        Ok(n) if transfer.bytes.len() + n <= MAX_BYTES => {
                            transfer.bytes.extend_from_slice(&chunk[..n])
                        }
                        Ok(_) => return TimeoutAction::Drop,
                        Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                        Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                        Err(_) => return TimeoutAction::Drop,
                    }
                }
                TimeoutAction::ToDuration(Duration::from_millis(10))
            },
        );
        if let Err(error) = result {
            tracing::warn!("Could not register URI drop receive: {error}");
        }
    }
}

impl DataOfferHandler for WinitState {
    fn source_actions(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &mut DragOffer,
        _: DndAction,
    ) {
    }
    fn selected_action(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &mut DragOffer,
        _: DndAction,
    ) {
    }
}

// This backend never creates a data source; dispatch is required by SCTK's macro.
impl DataSourceHandler for WinitState {
    fn accept_mime(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlDataSource,
        _: Option<String>,
    ) {
    }
    fn send_request(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlDataSource,
        _: String,
        _: WritePipe,
    ) {
    }
    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}
    fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}
    fn dnd_finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}
    fn action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: DndAction) {}
}

sctk::delegate_data_device!(WinitState);
