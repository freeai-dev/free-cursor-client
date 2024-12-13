use anyhow::Result;
use std::sync::Once;
use windows::Win32::System::Console::{AllocConsole, AttachConsole, ATTACH_PARENT_PROCESS};

static CONSOLE_ATTACHED: Once = Once::new();

pub fn attach_console() -> Result<()> {
    unsafe {
        CONSOLE_ATTACHED.call_once(|| {
            if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
                let _ = AllocConsole();
            }
        });
    }
    Ok(())
} 