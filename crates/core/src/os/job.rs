//! Windows Job Object wrapper so timeout/cancel/app-exit kill the whole installer process tree.

/// Guard that owns a Windows Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
/// Dropping it (or closing the handle) terminates every process assigned to the job.
pub struct KillOnCloseJob {
    #[cfg(windows)]
    handle: *mut std::ffi::c_void,
}

// Job handles are usable across threads; we only ever touch them from the command task.
unsafe impl Send for KillOnCloseJob {}

impl KillOnCloseJob {
    pub fn create() -> Option<Self> {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::JobObjects::{
                CreateJobObjectW, SetInformationJobObject, JobObjectExtendedLimitInformation,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            };

            unsafe {
                let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if handle.is_null() {
                    return None;
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &mut info as *mut _ as *mut _,
                    std::mem::size_of_val(&info) as u32,
                );
                if ok == 0 {
                    let _ = windows_sys::Win32::Foundation::CloseHandle(handle);
                    return None;
                }
                Some(Self { handle })
            }
        }
        #[cfg(not(windows))]
        {
            Some(Self {})
        }
    }

    /// Assigns an already-spawned process to this job. Returns false on failure.
    pub fn assign_process(&self, process_handle: *mut std::ffi::c_void) -> bool {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
            if process_handle.is_null() {
                return false;
            }
            unsafe { AssignProcessToJobObject(self.handle, process_handle) != 0 }
        }
        #[cfg(not(windows))]
        {
            let _ = process_handle;
            true
        }
    }
}

impl Drop for KillOnCloseJob {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            let _ = windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_can_be_created_and_dropped() {
        let job = KillOnCloseJob::create();
        assert!(job.is_some());
        drop(job);
    }
}
