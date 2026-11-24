//! Native process-tree ownership. All runner FFI stays in this module.

#[cfg(unix)]
pub(super) struct Group {
    pid: Option<libc::pid_t>,
}

#[cfg(unix)]
impl Group {
    pub(super) fn prepare(command: &mut tokio::process::Command) -> std::io::Result<Self> {
        command.process_group(0);
        Ok(Self { pid: None })
    }

    pub(super) fn attach(&mut self, child: &tokio::process::Child) -> std::io::Result<()> {
        self.pid = Some(
            child
                .id()
                .and_then(|id| id.try_into().ok())
                .ok_or_else(|| std::io::Error::other("process has no usable group ID"))?,
        );
        Ok(())
    }

    fn signal(&self, signal: i32) -> std::io::Result<()> {
        let Some(pid) = self.pid else { return Ok(()) };
        // SAFETY: pid is a positive child PID in its own group; a negative PID
        // targets only that group. kill takes integers and no pointers.
        if unsafe { libc::kill(-pid, signal) } == 0 {
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        // The group is our own child's, so EPERM can only mean that every member has
        // already exited and is waiting to be reaped (macOS reports that case as EPERM
        // instead of ESRCH). There is nothing left to signal either way.
        if matches!(error.raw_os_error(), Some(libc::ESRCH | libc::EPERM)) {
            Ok(())
        } else {
            Err(error)
        }
    }

    pub(super) fn terminate(&self) -> std::io::Result<()> {
        self.signal(libc::SIGTERM)
    }

    pub(super) fn kill(&self) -> std::io::Result<()> {
        self.signal(libc::SIGKILL)
    }
}

#[cfg(unix)]
impl Drop for Group {
    fn drop(&mut self) {
        let _ = self.kill();
    }
}

#[cfg(windows)]
pub(super) struct Group {
    // Store the owned handle as an integer so the guard can move with the future.
    // It is private, closed once in Drop, and never dereferenced by Rust.
    job: isize,
}

#[cfg(windows)]
impl Group {
    pub(super) fn prepare(_: &mut tokio::process::Command) -> std::io::Result<Self> {
        use windows_sys::Win32::System::JobObjects::*;
        // SAFETY: null security attributes/name request a private unnamed job.
        let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if job.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let group = Self { job: job as isize };
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: job is owned and valid; limits is initialized, and its address
        // and exact size remain valid throughout this synchronous call.
        let set = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
        };
        if set == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(group)
    }

    pub(super) fn attach(&mut self, child: &tokio::process::Child) -> std::io::Result<()> {
        let process = child
            .raw_handle()
            .ok_or_else(|| std::io::Error::other("process has no handle"))?;
        // SAFETY: both handles are live for the call; ownership stays with the
        // guard and Tokio child, respectively. No Rust memory is dereferenced.
        let assigned = unsafe {
            windows_sys::Win32::System::JobObjects::AssignProcessToJobObject(
                self.job as _,
                process as _,
            )
        };
        if assigned == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub(super) fn terminate(&self) -> std::io::Result<()> {
        self.kill()
    }

    pub(super) fn kill(&self) -> std::io::Result<()> {
        // SAFETY: the job remains owned until Drop closes it.
        let killed =
            unsafe { windows_sys::Win32::System::JobObjects::TerminateJobObject(self.job as _, 1) };
        if killed == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[cfg(windows)]
impl Drop for Group {
    fn drop(&mut self) {
        // SAFETY: this guard owns exactly this job handle and closes it once;
        // KILL_ON_JOB_CLOSE also terminates remaining descendants.
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.job as _) };
    }
}

#[cfg(all(test, unix))]
// Used by the integration-test inclusion of this module, not by lib tests.
#[allow(dead_code)]
pub fn is_dead(pid: i32) -> bool {
    // SAFETY: signal zero only probes an integer process ID; no pointers are used.
    unsafe {
        libc::kill(pid, 0) == -1
            && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
    }
}
