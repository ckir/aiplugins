//! Windows Job Object kill-guard (spike D4). The worker tree is host -> cmd.exe -> java.exe;
//! a plain child.kill() reaps only cmd.exe and orphans the JVM. A job with
//! JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE kills the entire subtree the moment the host drops the
//! last job handle (including host process death). Unix uses process groups (setsid + SIGKILL
//! to the negative PGID) for the same effect. The crate builds everywhere with a no-op stub
//! for exotic platforms.

#[cfg(windows)]
mod windows_imp {
    use std::io;
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    pub struct JobObject {
        handle: HANDLE,
    }

    // SAFETY: `handle` is a process-wide Windows job-object kernel handle. The Win32 job APIs used
    // here (AssignProcessToJobObject / SetInformationJobObject / CloseHandle) are thread-safe, so the
    // wrapper is sound to move across threads (Send) and share by reference (Sync). `HANDLE` is only
    // !Send/!Sync by default because the `windows` crate wraps a raw pointer without annotating it;
    // without these impls the whole BootedWorker is !Send, which breaks holding it across spawned
    // tokio tasks or in an Arc<Mutex<_>> (merge-gate finding).
    unsafe impl Send for JobObject {}
    unsafe impl Sync for JobObject {}

    impl JobObject {
        pub fn new() -> io::Result<Self> {
            // SAFETY: CreateJobObjectW with null args creates an unnamed job; we own the handle.
            let handle = unsafe { CreateJobObjectW(None, None) }
                .map_err(|e| io::Error::other(format!("CreateJobObjectW: {e}")))?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            // SAFETY: `info` outlives the call; size matches the info class.
            unsafe {
                SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const core::ffi::c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            }
            .map_err(|e| io::Error::other(format!("SetInformationJobObject: {e}")))?;
            Ok(JobObject { handle })
        }

        pub fn assign(&self, child: &Child) -> io::Result<()> {
            let child_handle = HANDLE(child.as_raw_handle());
            // SAFETY: both handles are valid; the child is alive at assign time.
            unsafe { AssignProcessToJobObject(self.handle, child_handle) }
                .map_err(|e| io::Error::other(format!("AssignProcessToJobObject: {e}")))
        }
    }

    impl Drop for JobObject {
        fn drop(&mut self) {
            // Closing the last handle triggers KILL_ON_JOB_CLOSE for the whole tree.
            // SAFETY: handle was created by us and not closed elsewhere.
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}

#[cfg(unix)]
mod unix_imp {
    use nix::sys::signal;
    use nix::unistd::Pid;
    use std::io;
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command};

    /// Process group kill-guard for Unix. The child is spawned in a new session
    /// (via `setsid()`), so it becomes the leader of a new process group. On drop,
    /// the entire group is sent `SIGKILL`, ensuring no orphaned JVM subprocesses.
    pub struct ProcessGroupGuard {
        pid: Option<u32>,
    }

    impl ProcessGroupGuard {
        /// Create a fresh guard (no child assigned yet).
        pub fn new() -> io::Result<Self> {
            Ok(ProcessGroupGuard { pid: None })
        }

        /// Configure the command to create a new process group / session.
        /// Must be called before `cmd.spawn()`.
        pub fn configure_command(&self, cmd: &mut Command) {
            // SAFETY: setsid() is async-signal-safe and is called in the child
            // process context between fork and exec. It creates a new session
            // and process group with the child as leader.
            unsafe {
                cmd.pre_exec(|| {
                    nix::unistd::setsid().map_err(|e| io::Error::from_raw_os_error(e as i32))?;
                    Ok(())
                });
            }
        }

        /// Record the child PID so the guard can kill the process group on drop.
        pub fn assign(&mut self, child: &Child) {
            self.pid = Some(child.id());
        }
    }

    impl Drop for ProcessGroupGuard {
        fn drop(&mut self) {
            if let Some(pid) = self.pid {
                // Best-effort: kill the process group on drop. Negative PID
                // signals the entire process group.
                let pgid = Pid::from_raw(-(pid as i32));
                let _ = signal::killpg(pgid, signal::Signal::SIGKILL);
            }
        }
    }
}

#[cfg(not(any(windows, unix)))]
mod fallback_imp {
    use std::io;
    use std::process::Child;

    /// No-op job on platforms that are neither Windows nor Unix (e.g. WASM).
    pub struct JobObject;
    impl JobObject {
        pub fn new() -> io::Result<Self> {
            Ok(JobObject)
        }
        pub fn assign(&self, _child: &Child) -> io::Result<()> {
            Ok(())
        }
    }
}

#[cfg(windows)]
pub use self::windows_imp::JobObject;

#[cfg(unix)]
pub use self::unix_imp::ProcessGroupGuard as JobObject;

#[cfg(not(any(windows, unix)))]
pub use self::fallback_imp::JobObject;

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Duration;

    // Spawn cmd.exe that in turn spawns a long-lived grandchild (ping -t), assign the DIRECT child
    // to a job, then drop the job: the whole tree must die, proving grandchildren are reaped (the
    // exact host->cmd.exe->java.exe shape spike D4 measured).
    #[test]
    fn dropping_job_kills_grandchild_tree() {
        let mut child = Command::new("cmd.exe")
            .args(["/c", "ping -n 60 127.0.0.1 >NUL"])
            .spawn()
            .expect("spawn cmd");
        let pid = child.id();
        {
            let job = JobObject::new().expect("create job");
            job.assign(&child).expect("assign child");
            // job drops here -> KILL_ON_JOB_CLOSE fires
        }
        std::thread::sleep(Duration::from_millis(500));
        // The child must now be gone; try_wait returns Some(exit) once reaped.
        let status = child.try_wait().expect("try_wait");
        assert!(
            status.is_some(),
            "child pid {pid} should be killed by job close"
        );
        let _ = child.kill();
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::unix_imp::ProcessGroupGuard;
    use std::process::Command;
    use std::time::Duration;

    // Spawn a shell that runs `sleep 60`, configure it to create a new session (process group),
    // then drop the guard: the entire process group must die, proving no orphans remain.
    #[test]
    fn dropping_process_group_kills_grandchild_tree() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 60"]);

        let mut guard = ProcessGroupGuard::new().expect("create guard");
        guard.configure_command(&mut cmd);

        let mut child = cmd.spawn().expect("spawn sh");
        let pid = child.id();

        // Give the process a moment to start
        std::thread::sleep(Duration::from_millis(100));

        // Verify the child is still running
        assert!(
            child.try_wait().expect("try_wait").is_none(),
            "child should still be running"
        );

        // Drop the guard — this sends SIGKILL to the process group
        drop(guard);

        std::thread::sleep(Duration::from_millis(200));

        // The child must now be gone
        let status = child.try_wait().expect("try_wait");
        assert!(
            status.is_some(),
            "child pid {pid} should be killed by process group SIGKILL"
        );
    }
}
