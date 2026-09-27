use anyhow::Result;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

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
