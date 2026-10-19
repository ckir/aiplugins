//! Process kill-guard (spike D4). The worker tree is host -> cmd.exe -> java.exe; a plain
//! child.kill() reaps only cmd.exe and orphans the JVM. On Windows a job with
//! JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE kills the entire subtree the moment the host drops the last
//! job handle (including host process death). On Unix, process groups (setsid + SIGKILL to the
//! process-group id) give the same effect. The crate builds everywhere with a no-op stub for
//! exotic platforms.

#[cfg(windows)]
mod windows_imp {
    use std::io;
    use std::os::windows::io::{AsRawHandle, RawHandle};
    use std::process::Child;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    pub struct JobObject {
        handle: Option<HANDLE>,
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
            Ok(JobObject {
                handle: Some(handle),
            })
        }

        pub fn assign(&self, child: &Child) -> io::Result<()> {
            self.assign_raw_handle(child.as_raw_handle())
        }

        /// Assign a bare process handle to the job. After `kill_group` the job handle is gone, so
        /// there is nothing left to assign into: report `InvalidInput` rather than panic.
        pub fn assign_raw_handle(&self, handle: RawHandle) -> io::Result<()> {
            let job = self
                .handle
                .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
            let child_handle = HANDLE(handle);
            // SAFETY: both handles are valid; the child is alive at assign time.
            unsafe { AssignProcessToJobObject(job, child_handle) }
                .map_err(|e| io::Error::other(format!("AssignProcessToJobObject: {e}")))
        }

        /// Terminate every process in the job and release the handle, so a second call and the
        /// `Drop` are no-ops.
        pub fn kill_group(&mut self) -> io::Result<()> {
            let Some(handle) = self.handle.take() else {
                return Ok(());
            };
            // SAFETY: `handle` is a live job handle we own and are about to close.
            let res = unsafe { TerminateJobObject(handle, 1) }
                .map_err(|e| io::Error::other(format!("TerminateJobObject: {e}")));
            // SAFETY: `handle` was created by us and is taken, so it is not closed elsewhere.
            unsafe {
                let _ = CloseHandle(handle);
            }
            res
        }

        /// Whether a job handle is still held (i.e. the guard has something to kill/close).
        pub fn is_armed(&self) -> bool {
            self.handle.is_some()
        }
    }

    impl Drop for JobObject {
        fn drop(&mut self) {
            // Closing the last handle triggers KILL_ON_JOB_CLOSE for the whole tree. `kill_group`
            // takes the handle, so a guard that was killed explicitly is a no-op here.
            if let Some(handle) = self.handle.take() {
                // SAFETY: handle was created by us and not closed elsewhere.
                unsafe {
                    let _ = CloseHandle(handle);
                }
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

        /// Record a bare PID (the leader of the process group) to kill on drop.
        pub fn assign_pid(&mut self, pid: u32) {
            self.pid = Some(pid);
        }

        /// Kill the process group with SIGKILL. `killpg` takes a POSITIVE process-group id: the
        /// leader's PID *is* the group id (the child became group leader via `setsid`), and a
        /// negated id is rejected with `EINVAL`. `ESRCH` means the group is already gone — the
        /// normal case after a clean exit — so it maps to `Ok(())`. The pid is taken so the guard
        /// is disarmed and a second call, and the `Drop`, are no-ops.
        pub fn kill_group(&mut self) -> io::Result<()> {
            let Some(pid) = self.pid else {
                return Ok(());
            };
            let res = match signal::killpg(Pid::from_raw(pid as i32), signal::Signal::SIGKILL) {
                Ok(()) => Ok(()),
                Err(nix::errno::Errno::ESRCH) => Ok(()),
                Err(e) => Err(io::Error::from_raw_os_error(e as i32)),
            };
            self.pid.take();
            res
        }

        /// Whether a pid is recorded and the guard can still act.
        pub fn is_armed(&self) -> bool {
            self.pid.is_some()
        }
    }

    impl Drop for ProcessGroupGuard {
        fn drop(&mut self) {
            // Best-effort: kill the process group. A caller that cares about the error calls
            // `kill_group` explicitly first (this crate has no log sink).
            let _ = self.kill_group();
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
        /// No-op: this platform has no process groups.
        pub fn assign_pid(&mut self, _pid: u32) {}
        /// No-op: there is never anything to kill.
        pub fn kill_group(&mut self) -> io::Result<()> {
            Ok(())
        }
        /// A no-op guard is never armed.
        pub fn is_armed(&self) -> bool {
            false
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
    use std::time::{Duration, Instant};

    // Spawn a shell that runs `sleep 60`, configure it to create a new session (process group),
    // assign the child to the guard, then drop the guard: the group leader must die, proving the
    // guard actually signals the group (assert the LEADER only; grandchild death is
    // `assign_pid_kills_group_on_drop`'s job).
    #[test]
    fn dropping_process_group_kills_leader() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 60"]);

        let mut guard = ProcessGroupGuard::new().expect("create guard");
        guard.configure_command(&mut cmd);

        let mut child = cmd.spawn().expect("spawn sh");
        let pid = child.id();

        guard.assign(&child);

        // Give the process a moment to start.
        std::thread::sleep(Duration::from_millis(100));

        // Verify the child is still running.
        assert!(
            child.try_wait().expect("try_wait").is_none(),
            "child should still be running"
        );

        // Drop the guard — this sends SIGKILL to the process group.
        drop(guard);

        // The leader must be gone within 1 s (poll every 50 ms).
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            if child.try_wait().expect("try_wait").is_some() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "child pid {pid} should be killed when the guard drops"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    // Spawn a shell that spawns a grandchild and waits; assign the LEADER pid to the guard, read
    // the grandchild pid from stdout, then drop the guard: the whole group — leader and grandchild
    // — must die. This is the guarantee the guard exists for.
    #[test]
    fn assign_pid_kills_group_on_drop() {
        use nix::errno::Errno;
        use nix::sys::signal;
        use nix::unistd::Pid;
        use std::io::{BufRead, BufReader};
        use std::process::Stdio;

        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("sleep 30 & echo $!; wait");
        cmd.stdout(Stdio::piped());

        let mut guard = ProcessGroupGuard::new().expect("create guard");
        guard.configure_command(&mut cmd);

        let mut child = cmd.spawn().expect("spawn sh");

        let stdout = child.stdout.take().expect("piped stdout");
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line).expect("read grandchild pid");
        let grandchild: i32 = line.trim().parse().expect("parse grandchild pid");

        guard.assign_pid(child.id());

        drop(guard);

        // Within 1 s the grandchild must be gone: `kill(pid, 0)` must fail with ESRCH, or (Linux
        // only) /proc must report it a zombie — a non-reaping PID 1 leaves one behind. A missing or
        // unreadable /proc entry counts as "not a zombie", so macOS relies on ESRCH alone.
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match signal::kill(Pid::from_raw(grandchild), None) {
                Err(Errno::ESRCH) => break,
                _ =>
                {
                    #[cfg(target_os = "linux")]
                    if is_zombie(grandchild) {
                        break;
                    }
                }
            }
            assert!(
                Instant::now() < deadline,
                "grandchild pid {grandchild} should die with its process group"
            );
            std::thread::sleep(Duration::from_millis(50));
        }

        // Reap the (already group-killed) leader so the test leaves no zombie and clippy is happy.
        let _ = child.wait();
    }

    // A group that is already gone answers ESRCH; that is the NORMAL path after a clean exit, so it
    // must map to `Ok(())`. Asserting the `Ok` pair alone would pass for a body that never clears
    // the pid, so also assert `is_armed()` flips false after the FIRST call (the `take()`).
    #[test]
    fn kill_group_esrch_is_ok() {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("exit 0");

        let mut guard = ProcessGroupGuard::new().expect("create guard");
        guard.configure_command(&mut cmd);

        let mut child = cmd.spawn().expect("spawn sh");
        let pid = child.id();
        // Reap the leader FIRST: an exited-but-unreaped leader still answers `killpg(pgid, 0) == 0`.
        child.wait().expect("wait for leader");

        guard.assign_pid(pid);
        assert!(guard.is_armed(), "guard should be armed after assign_pid");

        assert!(
            guard.kill_group().is_ok(),
            "first kill_group on a gone group maps ESRCH to Ok"
        );
        assert!(!guard.is_armed(), "kill_group must clear the pid");
        assert!(
            guard.kill_group().is_ok(),
            "second kill_group is a no-op Ok"
        );
    }

    #[cfg(target_os = "linux")]
    fn is_zombie(pid: i32) -> bool {
        match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            // `pid (comm) state …`; split on the LAST `") "` so a comm containing it cannot fool us.
            Ok(stat) => stat
                .rsplit(") ")
                .next()
                .is_some_and(|rest| rest.starts_with('Z')),
            Err(_) => false,
        }
    }
}
