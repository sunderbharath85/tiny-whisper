use anyhow::{anyhow, Result};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::sync::mpsc::sync_channel;

/// Paste one segment: `write` puts `text` on the clipboard on this thread,
/// `dispatch` runs the `inject` keystroke where it must run (the main thread),
/// and this waits for the keystroke to finish. Pasting segments one call at a
/// time therefore can't let segment N+1 overwrite the clipboard before
/// segment N's Ctrl/Cmd+V has landed. Don't call it on the main thread: it
/// blocks until the dispatched keystroke has run.
pub fn paste_serialized<W, I, D>(text: &str, write: W, inject: I, dispatch: D) -> Result<()>
where
    W: FnOnce(&str) -> Result<()>,
    I: FnOnce() -> Result<()> + Send + 'static,
    D: FnOnce(Box<dyn FnOnce() + Send>) -> Result<()>,
{
    write(text)?;
    let (done_tx, done_rx) = sync_channel(1);
    dispatch(Box::new(move || {
        let _ = done_tx.send(inject());
    }))?;
    done_rx
        .recv()
        .map_err(|_| anyhow!("the paste keystroke was dropped before it ran"))?
}

/// Write `text` to the clipboard and wait for it to propagate.
/// Safe to call from any thread.
pub fn prepare(text: &str) -> Result<()> {
    let mut cb = arboard::Clipboard::new()?;
    cb.set_text(text.to_string())?;
    std::thread::sleep(std::time::Duration::from_millis(30));
    Ok(())
}

/// Send the Cmd/Ctrl+V keystroke sequence.
/// Must be called from the main thread on macOS — enigo calls
/// TSMGetInputSourceProperty which asserts it runs on the main dispatch queue.
pub fn inject_keys() -> Result<()> {
    // Without Accessibility permission macOS drops synthetic keystrokes
    // without any error, so the paste would silently do nothing.
    #[cfg(target_os = "macos")]
    if !macos::accessibility_trusted() {
        return Err(anyhow!(
            "Pasting needs Accessibility permission. Allow tiny-whisper in System Settings → \
             Privacy & Security → Accessibility, then try again. The text is on the clipboard."
        ));
    }
    let mut enigo = Enigo::new(&Settings::default())?;
    #[cfg(target_os = "macos")]
    let mod_key = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let mod_key = Key::Control;

    enigo.key(mod_key, Direction::Press)?;
    enigo.key(Key::Unicode('v'), Direction::Click)?;
    enigo.key(mod_key, Direction::Release)?;
    Ok(())
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};

    type CFTypeRef = *const c_void;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        static kAXTrustedCheckOptionPrompt: CFTypeRef;
        fn AXIsProcessTrustedWithOptions(options: CFTypeRef) -> bool;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFBooleanTrue: CFTypeRef;
        static kCFTypeDictionaryKeyCallBacks: c_void;
        static kCFTypeDictionaryValueCallBacks: c_void;
        fn CFDictionaryCreate(
            allocator: CFTypeRef,
            keys: *const CFTypeRef,
            values: *const CFTypeRef,
            count: isize,
            key_callbacks: *const c_void,
            value_callbacks: *const c_void,
        ) -> CFTypeRef;
        fn CFRelease(cf: CFTypeRef);
    }

    /// Whether this process may send keystrokes to other apps. The first
    /// check that finds it untrusted also shows the system prompt that leads
    /// to System Settings; later checks stay quiet.
    pub fn accessibility_trusted() -> bool {
        static PROMPTED: AtomicBool = AtomicBool::new(false);
        let prompt = !PROMPTED.swap(true, Ordering::SeqCst);
        unsafe {
            if !prompt {
                return AXIsProcessTrustedWithOptions(std::ptr::null());
            }
            let keys = [kAXTrustedCheckOptionPrompt];
            let values = [kCFBooleanTrue];
            let options = CFDictionaryCreate(
                std::ptr::null(),
                keys.as_ptr(),
                values.as_ptr(),
                1,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            );
            let trusted = AXIsProcessTrustedWithOptions(options);
            if !options.is_null() {
                CFRelease(options);
            }
            trusted
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{channel, Sender};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    type Job = Box<dyn FnOnce() + Send>;

    /// Stands in for the app's main thread: runs dispatched jobs one at a
    /// time, each a little late, like a busy event loop.
    fn slow_main_thread() -> Sender<Job> {
        let (tx, rx) = channel::<Job>();
        std::thread::spawn(move || {
            for job in rx {
                std::thread::sleep(Duration::from_millis(20));
                job();
            }
        });
        tx
    }

    fn send_to(main: &Sender<Job>) -> impl FnOnce(Job) -> Result<()> + '_ {
        move |job| main.send(job).map_err(|_| anyhow!("main thread gone"))
    }

    /// A fake clipboard, and what each Ctrl/Cmd+V pasted from it.
    #[derive(Default)]
    struct Desktop {
        clipboard: Arc<Mutex<String>>,
        pasted: Arc<Mutex<Vec<String>>>,
    }

    impl Desktop {
        fn paste(&self, text: &str, main: &Sender<Job>) -> Result<()> {
            let clipboard = self.clipboard.clone();
            let (from, pasted) = (self.clipboard.clone(), self.pasted.clone());
            paste_serialized(
                text,
                |t| {
                    *clipboard.lock().unwrap() = t.to_string();
                    Ok(())
                },
                move || {
                    pasted.lock().unwrap().push(from.lock().unwrap().clone());
                    Ok(())
                },
                send_to(main),
            )
        }
    }

    #[test]
    fn each_segment_is_pasted_before_the_next_clipboard_write() {
        let main = slow_main_thread();
        let desktop = Desktop::default();
        for text in ["one ", "two ", "three "] {
            desktop.paste(text, &main).unwrap();
        }
        assert_eq!(*desktop.pasted.lock().unwrap(), ["one ", "two ", "three "]);
    }

    #[test]
    fn keystroke_failure_is_returned() {
        let main = slow_main_thread();
        let err = paste_serialized("hi", |_| Ok(()), || Err(anyhow!("no keyboard")), send_to(&main))
            .unwrap_err();
        assert_eq!(err.to_string(), "no keyboard");
    }

    #[test]
    fn clipboard_failure_skips_the_keystroke() {
        let dispatched = AtomicBool::new(false);
        let err = paste_serialized(
            "hi",
            |_| Err(anyhow!("clipboard busy")),
            || Ok(()),
            |_job| {
                dispatched.store(true, Ordering::SeqCst);
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "clipboard busy");
        assert!(!dispatched.load(Ordering::SeqCst));
    }

    #[test]
    fn dispatch_failure_is_returned() {
        let err = paste_serialized("hi", |_| Ok(()), || Ok(()), |_job| Err(anyhow!("event loop closed")))
            .unwrap_err();
        assert_eq!(err.to_string(), "event loop closed");
    }

    #[test]
    fn dropped_keystroke_is_an_error_not_a_hang() {
        let err = paste_serialized("hi", |_| Ok(()), || Ok(()), |job| {
            drop(job);
            Ok(())
        })
        .unwrap_err();
        assert!(err.to_string().contains("dropped"), "{err}");
    }
}
