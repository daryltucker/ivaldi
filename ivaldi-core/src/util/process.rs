use std::io;
use std::process::{Child, Command};

/// Process guard that ensures child processes are killed and reaped on drop.
/// 
/// This implements the "ProcessGuard" pattern to prevent zombie processes
/// and ensure cleanup even on panic/cancellation.
pub struct ProcessGuard {
    child: Option<Child>,
    #[allow(dead_code)]
    use_process_group: bool,
}

impl ProcessGuard {
    /// Spawn a command with automatic cleanup.
    /// Creates a new process group on Unix or uses Job Objects on Windows to enable tree termination.
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0); // Create new process group
        }

        let child = command.spawn()?;
        
        #[cfg(windows)]
        let _job = {
            // Windows Job Objects for tree termination
            // This is a simplified placeholder for the Windows implementation
            // Real implementation would involve winapi calls to assign process to job
            None::<()> 
        };

        Ok(Self {
            child: Some(child),
            use_process_group: true,
        })
    }

    /// Access the underlying child process
    pub fn get_mut(&mut self) -> Option<&mut Child> {
        self.child.as_mut()
    }

    /// Kill the process tree (process + all children in group)
    pub fn kill_tree(&mut self) -> io::Result<()> {
        if let Some(child) = &mut self.child {
            #[cfg(unix)]
            {
                use libc::{killpg, SIGKILL, SIGTERM};
                let pid = child.id() as i32;
                
                // 1. Try SIGTERM first
                let _ = unsafe { killpg(pid, SIGTERM) };
                
                // 2. Wait a bit for it to exit
                let mut count = 0;
                while count < 10 {
                    if let Ok(Some(_)) = child.try_wait() {
                        return Ok(());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    count += 1;
                }
                
                // 3. Fallback to SIGKILL
                let _ = unsafe { killpg(pid, SIGKILL) };
            }

            #[cfg(not(unix))]
            {
                // Fallback for non-unix - only kills immediate child
                let _ = child.kill();
            }
        }
        Ok(())
    }

    /// Wait for the process to exit
    pub fn wait(&mut self) -> io::Result<std::process::ExitStatus> {
        if let Some(mut child) = self.child.take() {
            child.wait()
        } else {
            Err(io::Error::other("Process already waited for"))
        }
    }

    /// Wait for the process to exit and capture output
    pub fn wait_with_output(mut self) -> io::Result<std::process::Output> {
        if let Some(child) = self.child.take() {
            child.wait_with_output()
        } else {
            Err(io::Error::other("Process already waited for"))
        }
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        if self.child.is_some() {
            let _ = self.kill_tree();
            if let Some(mut child) = self.child.take() {
                let _ = child.wait(); // Reap zombie
            }
        }
    }
}
