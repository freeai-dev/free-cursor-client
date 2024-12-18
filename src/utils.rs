use anyhow::Result;
use std::sync::Once;

static CONSOLE_ATTACHED: Once = Once::new();

#[cfg(not(windows))]
pub fn attach_console() -> Result<()> {
    Ok(())
}

#[cfg(windows)]
pub fn attach_console() -> Result<()> {
    use windows::Win32::System::Console::{AllocConsole, AttachConsole, ATTACH_PARENT_PROCESS};

    unsafe {
        CONSOLE_ATTACHED.call_once(|| {
            if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
                let _ = AllocConsole();
            }
        });
    }
    Ok(())
}
