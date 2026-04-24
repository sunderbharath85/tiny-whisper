use anyhow::Result;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

pub fn paste(text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }
    {
        let mut cb = arboard::Clipboard::new()?;
        cb.set_text(text.to_string())?;
    }
    // Small delay so the clipboard propagates before we send the keystroke.
    std::thread::sleep(std::time::Duration::from_millis(30));

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
