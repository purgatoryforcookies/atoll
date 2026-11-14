//! Where the island lives: the screen with the menu bar, its notch if it has one, and the
//! window's place on it. Also the macOS glue that only the window needs: the pointer, watched
//! while the window lets clicks through, and the menu bar item that quits it.

use std::sync::{Mutex, OnceLock};
use rune::Waker;
use crate::{EAR, WING};

/// The resting pill on a screen without a notch: its width, height and gap from the top edge.
pub const PILL_CORE: f32 = 104.0;
pub const PILL_H: f32 = 38.0;
/// The height the pill's content is sized for; the pill stands taller, with room around it.
pub const PILL_LINE: f32 = 26.0;
pub const PILL_TOP: f32 = 4.0;
/// How much taller the pill stretches while it is resizing fast.
pub const STRETCH: f32 = 3.0;
/// With no notch to fit, the pill's content line draws this tall on the glass, in millimetres,
/// so it sits at the menu bar's scale whatever the monitor: about 1.05× on a 32" 4K screen at
/// 2560 points wide, 1.25× on a 27" one.
pub const PILL_MM: f32 = 7.5;
/// The pill's zoom when the display doesn't report its size.
const ZOOM: f32 = 1.25;
/// How far the expanded card reaches below the island's top row.
pub const CARD_H: f32 = 122.0;
/// How much further it reaches for the row of session pills, when there are two or more.
pub const PILLS: f32 = 16.0;

/// The screen with the menu bar, where the island lives.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Screen {
    /// Points wide and tall, and pixels per point.
    pub width: f32,
    pub height: f32,
    pub scale: f64,
    /// The notch's size in points; on a screen without one the island floats as a pill.
    pub notch: Option<[f32; 2]>,
    /// Millimetres per point on the glass, or 0 when the display doesn't say.
    pub mm: f32,
    /// The menu bar's height where it sits beside a notch, in points; 0 on a flat screen.
    pub inset: f32,
}

impl Screen {
    /// The island's resting body: the notch, or the pill.
    pub fn core(&self) -> [f32; 2] { self.notch.unwrap_or([PILL_CORE, PILL_H]) }

    /// How much larger than its layout the island draws.
    pub fn zoom(&self) -> f32 {
        match (self.notch, self.mm) {
            (Some(_), _) => 1.0,
            // A notched screen is a laptop's, close to you: its pill stays at notch scale.
            (None, _) if self.inset > 0.0 => 1.0,
            (None, mm) if mm > 0.0 => (PILL_MM / (PILL_LINE * mm)).clamp(1.0, 2.0),
            _ => ZOOM,
        }
    }

    /// The window in layout points, wide enough for both wings at full reach and tall enough
    /// for the expanded card; it opens [`Screen::zoom`] times this.
    pub fn layout(&self) -> (f32, f32) {
        let [w, h] = self.core();
        let body = if self.notch.is_some() { h } else { PILL_TOP + PILL_H + STRETCH };
        (w + 2.0 * (WING + EAR), body + CARD_H + PILLS + 6.0)
    }

    pub fn window(&self) -> (f32, f32) {
        let ((w, h), k) = (self.layout(), self.zoom());
        (w * k, h * k)
    }

    /// The window's top-left in points from the screen's top-left. A pill on a notched screen
    /// hangs below the notch rather than behind its camera.
    fn origin(&self) -> (f32, f32) {
        ((self.width - self.window().0) * 0.5, if self.notch.is_some() { 0.0 } else { self.inset })
    }

    /// The same in desktop pixels: the menu bar's screen is the desktop's origin.
    pub fn position(&self) -> (i32, i32) {
        let (x, y) = self.origin();
        ((x as f64 * self.scale).round() as i32, (y as f64 * self.scale).round() as i32)
    }
}

/// What the main thread's watchers hand the running app, and what they need of it: a new
/// screen after the displays change, the pointer over the window, the waker, the window.
#[derive(Default)]
pub struct Shared {
    pub screen: Mutex<Option<Screen>>,
    /// The screen the window is placed for, which the pointer watcher measures against.
    pub placed: Mutex<Screen>,
    /// The pointer in window points from its top-left, while it is over the window.
    pub pointer: Mutex<Option<[f32; 2]>>,
    pub waker: OnceLock<Waker>,
    pub window: OnceLock<rune::WindowHandle>,
    /// The menu asked for the setup guide.
    pub guide: std::sync::atomic::AtomicBool,
}

impl Shared {
    pub fn wake(&self) { if let Some(wake) = self.waker.get() { wake() } }
}

/// The screen with the menu bar: its size, and its notch, which is the screen's width less
/// the menu bar on either side of it by its top safe-area inset. Headless runs get a notch.
pub fn menu_screen() -> Screen {
    let fallback = Screen { width: 1512.0, height: 982.0, scale: 2.0, notch: Some([185.0, 32.0]), mm: 0.0, inset: 0.0 };
    #[cfg(target_os = "macos")]
    if !rune::is_headless()
        && let Some(mtm) = objc2_foundation::MainThreadMarker::new()
        // SAFETY: an NSArray getter and plain geometry getters on live screens, on the main thread.
        && let Some(screen) = unsafe { objc2_app_kit::NSScreen::screens(mtm).firstObject() }
    {
        // SAFETY: plain geometry getters on a live screen, on the main thread.
        let (top, l, r) = unsafe { (screen.safeAreaInsets().top, screen.auxiliaryTopLeftArea(), screen.auxiliaryTopRightArea()) };
        let frame = screen.frame();
        let w = frame.size.width - l.size.width - r.size.width;
        // `ATOLL_PILL=1` floats the pill even on a screen with a notch.
        let pill = std::env::var("ATOLL_PILL").is_ok_and(|v| !v.is_empty() && v != "0");
        let notch = (top > 0.0 && w > 0.0 && !pill).then_some([w as f32, top as f32]);
        #[repr(C)]
        struct Size { w: f64, _h: f64 }
        #[link(name = "CoreGraphics", kind = "framework")]
        unsafe extern "C" { fn CGMainDisplayID() -> u32; fn CGDisplayScreenSize(display: u32) -> Size; }
        // SAFETY: plain getters; the main display is the one with the menu bar, as is `screens[0]`.
        // A display without size data (some projectors, older EDIDs) reports zero.
        let glass = unsafe { CGDisplayScreenSize(CGMainDisplayID()) };
        let mm = if glass.w > 0.0 && frame.size.width > 0.0 { (glass.w / frame.size.width) as f32 } else { 0.0 };
        return Screen {
            width: frame.size.width as f32, height: frame.size.height as f32, scale: screen.backingScaleFactor(),
            notch, mm, inset: top as f32,
        };
    }
    fallback
}

/// Follows the pointer while it is over the window, though the window lets clicks through and
/// hears nothing itself: it hands `shared` the pointer and wakes the app as it moves over the
/// window and once as it leaves, and costs nothing while it is elsewhere. Main thread.
pub fn watch_pointer(shared: std::sync::Arc<Shared>) {
    #[cfg(target_os = "macos")]
    mac::watch_pointer(shared);
    #[cfg(not(target_os = "macos"))]
    let _ = shared;
}

/// Opens a file in VS Code, or without it in the shell's `$EDITOR` (nano when unset) in Terminal,
/// rather than whatever claims `.json`, which is often Xcode; TextEdit if even Terminal won't
/// open. A missing file is made first, as an empty JSON object.
pub fn open(path: &std::path::Path) {
    #[cfg(target_os = "macos")]
    {
        let path = path.to_owned();
        // Off the main thread: `open` takes a moment to answer whether an app took the file.
        std::thread::spawn(move || {
            if !path.exists() && let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&path, "{}\n"));
            }
            let run = |args: &[&std::ffi::OsStr]| std::process::Command::new("open").args(args).status().is_ok_and(|s| s.success());
            if run(&["-b".as_ref(), "com.microsoft.VSCode".as_ref(), path.as_os_str()]) { return }
            // A `.command` file opens in Terminal without asking to control it, as a script would.
            let script = std::env::temp_dir().join("atoll-settings.command");
            let quoted = format!("'{}'", path.to_string_lossy().replace('\'', r"'\''"));
            let wrote = std::fs::write(&script, format!("#!/bin/sh\nexec ${{EDITOR:-nano}} {quoted}\n")).and_then(|()| {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700))
            });
            if wrote.is_ok() && run(&["-a".as_ref(), "Terminal".as_ref(), script.as_os_str()]) { return }
            run(&["-t".as_ref(), path.as_os_str()]);
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = path;
}

/// Puts text on the clipboard.
pub fn copy(text: &str) {
    #[cfg(target_os = "macos")]
    {
        use std::io::Write;
        let text = text.to_owned();
        std::thread::spawn(move || {
            let Ok(mut pb) = std::process::Command::new("pbcopy").stdin(std::process::Stdio::piped()).spawn() else { return };
            if let Some(mut stdin) = pb.stdin.take() { let _ = stdin.write_all(text.as_bytes()); }
            let _ = pb.wait();
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = text;
}

/// A capsule in the menu bar whose menu reopens the setup guide, opens the app at login and quits
/// it: with no Dock icon and a window that lets clicks through, it's the way out. Main thread.
// shortcut: `rune::Tray` has no macOS backend yet; this is AppKit directly. Move to `rune::Tray`
// once it supports macOS.
pub fn menu_bar_item(shared: std::sync::Arc<Shared>) {
    #[cfg(target_os = "macos")]
    mac::menu_bar_item(shared);
    #[cfg(not(target_os = "macos"))]
    let _ = shared;
}

#[cfg(target_os = "macos")]
mod mac {
    use super::Shared;
    use objc2::{class, msg_send, msg_send_id, rc::Retained, runtime::{AnyObject, Bool}, sel};
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

    pub(super) fn menu_bar_item(shared: Arc<Shared>) {
        if MainThreadMarker::new().is_none() { return }
        let _ = SHARED.set(shared);
        // SAFETY: main thread; AppKit's status bar, an image drawn by a block AppKit keeps and calls
        // on the main thread, and a menu whose one item sends `terminate:` up the responder chain to
        // the application. The item and its menu live for the process, so they are leaked.
        unsafe {
            let bar: *mut AnyObject = msg_send![class!(NSStatusBar), systemStatusBar];
            // NSVariableStatusItemLength.
            let item: Retained<AnyObject> = msg_send_id![bar, statusItemWithLength: -1.0f64];
            let button: *mut AnyObject = msg_send![&*item, button];
            // The island in miniature: a hairline capsule with its dot, drawn as a template so it
            // takes the menu bar's own colour, light or dark.
            let draw = block2::RcBlock::new(|_: NSRect| -> Bool {
                let outline: *mut AnyObject = msg_send![class!(NSBezierPath), bezierPathWithRoundedRect: NSRect::new(NSPoint::new(1.0, 5.0), NSSize::new(17.0, 8.0)), xRadius: 4.0f64, yRadius: 4.0f64];
                let black: *mut AnyObject = msg_send![class!(NSColor), blackColor];
                let _: () = msg_send![black, set];
                let _: () = msg_send![outline, setLineWidth: 1.2f64];
                let _: () = msg_send![outline, stroke];
                let dot: *mut AnyObject = msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: NSRect::new(NSPoint::new(4.0, 7.6), NSSize::new(2.8, 2.8))];
                let _: () = msg_send![dot, fill];
                // Three bars of the waveform, so it reads as the island rather than a switch.
                for (x, h) in [(10.6, 2.6), (12.4, 4.2), (14.2, 2.6)] {
                    let bar: *mut AnyObject = msg_send![class!(NSBezierPath), bezierPath];
                    let _: () = msg_send![bar, moveToPoint: NSPoint::new(x, 9.0 - h / 2.0)];
                    let _: () = msg_send![bar, lineToPoint: NSPoint::new(x, 9.0 + h / 2.0)];
                    let _: () = msg_send![bar, setLineWidth: 1.1f64];
                    // NSLineCapStyleRound.
                    let _: () = msg_send![bar, setLineCapStyle: 1usize];
                    let _: () = msg_send![bar, stroke];
                }
                Bool::YES
            });
            let image: *mut AnyObject = msg_send![class!(NSImage), imageWithSize: NSSize::new(19.0, 18.0), flipped: Bool::NO, drawingHandler: &*draw];
            let _: () = msg_send![image, setTemplate: Bool::YES];
            let name = NSString::from_str("Atoll");
            let _: () = msg_send![image, setAccessibilityDescription: &*name];
            let _: () = msg_send![button, setImage: image];
            let menu: Retained<AnyObject> = msg_send_id![class!(NSMenu), new];
            if let Some(guide) = guide_item() {
                let _: () = msg_send![&*menu, addItem: &*guide];
                std::mem::forget(guide);
            }
            if let Some(login) = login_item() {
                let _: () = msg_send![&*menu, addItem: &*login];
                let sep: *mut AnyObject = msg_send![class!(NSMenuItem), separatorItem];
                let _: () = msg_send![&*menu, addItem: sep];
                std::mem::forget(login);
            }
            let (title, key) = (NSString::from_str("Quit Atoll"), NSString::from_str("q"));
            let quit: Retained<AnyObject> = msg_send_id![msg_send_id![class!(NSMenuItem), alloc],
                initWithTitle: &*title, action: sel!(terminate:), keyEquivalent: &*key];
            let _: () = msg_send![&*menu, addItem: &*quit];
            let _: () = msg_send![&*item, setMenu: &*menu];
            std::mem::forget((item, menu, quit));
        }
    }

    /// What the menu's actions hand to the app; AppKit calls them with no way to carry it.
    static SHARED: std::sync::OnceLock<Arc<Shared>> = std::sync::OnceLock::new();

    /// "Connect Claude Code…", which opens the card on the setup guide again.
    fn guide_item() -> Option<Retained<AnyObject>> {
        // SAFETY: main thread; a target class registered once with one method of the signature
        // AppKit sends an action with.
        unsafe {
            extern "C" fn guide(_: &AnyObject, _: objc2::runtime::Sel, _: *mut AnyObject) {
                if let Some(shared) = SHARED.get() {
                    shared.guide.store(true, std::sync::atomic::Ordering::Relaxed);
                    shared.wake();
                }
            }
            let mut builder = objc2::declare::ClassBuilder::new("AtollGuideTarget", class!(NSObject))?;
            builder.add_method(sel!(guide:), guide as extern "C" fn(_, _, _));
            let target: Retained<AnyObject> = msg_send_id![builder.register(), new];
            let title = NSString::from_str("Connect Claude Code…");
            let item: Retained<AnyObject> = msg_send_id![msg_send_id![class!(NSMenuItem), alloc],
                initWithTitle: &*title, action: sel!(guide:), keyEquivalent: &*NSString::from_str("")];
            let _: () = msg_send![&*item, setTarget: &*target];
            // The menu item holds its target weakly; it lives for the process.
            std::mem::forget(target);
            Some(item)
        }
    }

    #[link(name = "ServiceManagement", kind = "framework")]
    unsafe extern "C" {}

    /// The main app service's `SMAppServiceStatus`: 1 enabled, 2 waiting for the user's approval.
    fn status(service: *mut AnyObject) -> isize {
        // SAFETY: a getter on the service.
        unsafe { msg_send![service, status] }
    }

    /// "Open at Login", ticked while macOS opens the app at login, or None before macOS 13. It
    /// lists the bundle under System Settings → Login Items, so it only takes from the installed
    /// app; run from a build, macOS refuses and the tick stays off.
    fn login_item() -> Option<Retained<AnyObject>> {
        // SAFETY: main thread; a class getter, and a target class registered once with one
        // method of the signature AppKit sends an action with.
        unsafe {
            let service: *mut AnyObject = msg_send![objc2::runtime::AnyClass::get("SMAppService")?, mainAppService];
            extern "C" fn toggle(_: &AnyObject, _: objc2::runtime::Sel, item: *mut AnyObject) {
                // SAFETY: AppKit calls this on the main thread with the menu item that was chosen.
                unsafe {
                    let service: *mut AnyObject = msg_send![class!(SMAppService), mainAppService];
                    let mut err: *mut AnyObject = std::ptr::null_mut();
                    if status(service) == 1 {
                        let _: Bool = msg_send![service, unregisterAndReturnError: &mut err];
                    } else {
                        let _: Bool = msg_send![service, registerAndReturnError: &mut err];
                        // SMAppServiceStatusRequiresApproval: the user turned it off in Settings before, so send them there.
                        if status(service) == 2 { let _: () = msg_send![class!(SMAppService), openSystemSettingsLoginItems]; }
                    }
                    let _: () = msg_send![item, setState: (status(service) == 1) as isize];
                }
            }
            let mut builder = objc2::declare::ClassBuilder::new("AtollLoginTarget", class!(NSObject))?;
            builder.add_method(sel!(toggle:), toggle as extern "C" fn(_, _, _));
            let target: Retained<AnyObject> = msg_send_id![builder.register(), new];
            let title = NSString::from_str("Open at Login");
            let item: Retained<AnyObject> = msg_send_id![msg_send_id![class!(NSMenuItem), alloc],
                initWithTitle: &*title, action: sel!(toggle:), keyEquivalent: &*NSString::from_str("")];
            let _: () = msg_send![&*item, setTarget: &*target];
            let _: () = msg_send![&*item, setState: (status(service) == 1) as isize];
            // The menu item holds its target weakly; it lives for the process.
            std::mem::forget(target);
            Some(item)
        }
    }

    use std::{cell::Cell, rc::Rc, sync::Arc};

    pub(super) fn watch_pointer(shared: Arc<Shared>) {
        if MainThreadMarker::new().is_none() { return }
        let inside = Rc::new(Cell::new(false));
        let moved = move || {
            // SAFETY: a class getter for the pointer's place, in global points from the bottom-left
            // of the menu bar's screen, on the main thread.
            let at: NSPoint = unsafe { msg_send![class!(NSEvent), mouseLocation] };
            let screen = *shared.placed.lock().unwrap();
            let ((x, y), (w, h)) = (screen.origin(), screen.window());
            let p = [at.x as f32 - x, screen.height - at.y as f32 - y];
            let now = p[0] >= 0.0 && p[1] >= 0.0 && p[0] <= w && p[1] <= h;
            if now || inside.replace(now) {
                *shared.pointer.lock().unwrap() = now.then_some(p);
                shared.wake();
            }
        };
        let (global, local) = (moved.clone(), moved);
        let global = block2::RcBlock::new(move |_: *mut AnyObject| global());
        let local = block2::RcBlock::new(move |event: *mut AnyObject| -> *mut AnyObject { local(); event });
        // Mouse moved and left-mouse dragged: other apps' events reach the global monitor, and
        // this app's own, once the window takes clicks, the local one.
        let mask: u64 = 1 << 5 | 1 << 6;
        // SAFETY: main thread; AppKit keeps both monitors for the process, and their tokens are
        // leaked as it never removes them.
        unsafe {
            let g: Option<Retained<AnyObject>> = msg_send_id![class!(NSEvent), addGlobalMonitorForEventsMatchingMask: mask, handler: &*global];
            let l: Option<Retained<AnyObject>> = msg_send_id![class!(NSEvent), addLocalMonitorForEventsMatchingMask: mask, handler: &*local];
            std::mem::forget((g, l));
        }
    }
}
