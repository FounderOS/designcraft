//! Host mode: a page that frames DesignCraft (`?host=parent`) or runs it in its own document
//! (`data-host="page"` on the canvas) opens documents in it and stores what it saves instead of
//! the browser downloading. The messages and both transports are `craft-web-host`'s, the same
//! contract as the other Craft apps' bridges; this module is DesignCraft's part of it.
//!
//! - `designcraft:ready` once the app is running and listening. DesignCraft reads whole files, so
//!   it says `file: false` and a host sends `bytes`.
//! - `designcraft:open` `{ name, bytes }` goes into the shell's [`Inbox`] as a dropped file does,
//!   and opens as File › Open would.
//! - `designcraft:save` `{ name, bytes, saveAs }` for every file the app would download: Save and
//!   exports with `saveAs: false` (an export under its own extension), Save As… with `true`.
//! - `designcraft:dirty` whenever unsaved changes appear or go away, and `designcraft:failed` when
//!   the app couldn't start.

use std::rc::Rc;

use craft_web_host::web::{FileData, Host, Message};
use designcraft_ui_egui::{ImportRequest, ImportedFile, Inbox};

/// The app id in the host messages (`designcraft:ready`).
const APP: &str = "designcraft";

/// The import purpose of a file the host sent: like a dropped one it is not tied to the document
/// in front, and a document file opens as one.
const OPEN_PURPOSE: &str = "host";

#[derive(Clone)]
pub struct Bridge {
    host: Rc<Host>,
    inbox: Inbox,
}

/// The bridge for this page, or `None` when DesignCraft isn't hosted (no `?host=parent` or
/// `data-host="page"`, no framing parent, or a parent the page doesn't allow): then saves stay
/// browser downloads. Files the host sends go to `inbox` once [`Bridge::listen`] runs.
pub fn connect(canvas: &web_sys::Element, inbox: &Inbox) -> Option<Bridge> {
    Some(Bridge { host: Rc::new(Host::connect(APP, canvas)?), inbox: inbox.clone() })
}

impl Bridge {
    /// Hear the host's files. `ctx` wakes the app: a message is no input event, so egui would not
    /// paint (and drain the inbox) before the next one.
    pub fn listen(&self, ctx: egui::Context) {
        let inbox = self.inbox.clone();
        self.host.listen(move |message| {
            if let Message::Open { name, data } = message {
                deliver_open(&inbox, &ctx, name, &data);
            }
        });
    }

    pub fn ready(&self) {
        let _ = self.host.post_ready(env!("CARGO_PKG_VERSION"), false);
    }

    /// The app couldn't start (no WebGPU or WebGL2, for example): let the host say so.
    pub fn failed(&self, error: &str) {
        let _ = self.host.post_failed(error, None);
    }

    /// Hand the host a file the app would otherwise download. The host names it by the last
    /// component of `name`, so an export keeps its own extension.
    pub fn save(&self, name: &str, bytes: &[u8], save_as: bool) -> Result<(), String> {
        self.host.post_save(name, bytes, None, save_as)
    }

    /// Report unsaved changes; the crate sends only changes of state, so this can run every frame.
    pub fn dirty(&self, dirty: bool) {
        self.host.post_dirty(dirty);
    }
}

fn deliver_open(inbox: &Inbox, ctx: &egui::Context, name: String, data: &FileData) {
    let FileData::Bytes(array) = data else {
        log::warn!("host file {name} came as a File; DesignCraft reads bytes only; not opened");
        return;
    };
    let request = ImportRequest { purpose: OPEN_PURPOSE.into(), target: None };
    inbox.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(ImportedFile { request, name, bytes: array.to_vec() });
    ctx.request_repaint();
}
