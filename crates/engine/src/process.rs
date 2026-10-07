use anyhow::{bail, Context, Result};
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

pub fn run(
    program: &Path,
    args: &[String],
    stdin: Option<&[u8]>,
    cancel: &AtomicBool,
    timeout: Option<Duration>,
) -> Result<Vec<u8>> {
    if cancel.load(Ordering::Relaxed) {
        bail!("Canceled");
    }
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?)
        .env("RAYON_NUM_THREADS", "1");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().with_context(|| {
        format!(
            "Could not start bundled helper {}",
            program.file_name().unwrap_or_default().to_string_lossy()
        )
    })?;
    #[cfg(windows)]
    let _job = WindowsJob::attach(&child)?;
    if let Some(bytes) = stdin {
        child
            .stdin
            .take()
            .context("Helper stdin missing")?
            .write_all(bytes)?;
    }
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if cancel.load(Ordering::Relaxed) || timeout.is_some_and(|d| start.elapsed() > d) {
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            bail!(if cancel.load(Ordering::Relaxed) {
                "Canceled"
            } else {
                "Helper timed out"
            });
        }
        thread::sleep(Duration::from_millis(30));
    };
    use std::io::{Read, Seek, SeekFrom};
    stdout.seek(SeekFrom::Start(0))?;
    stderr.seek(SeekFrom::Start(0))?;
    let mut output = Vec::new();
    stdout.take(4 * 1024 * 1024).read_to_end(&mut output)?;
    if !status.success() {
        let mut error = String::new();
        stderr.take(16000).read_to_string(&mut error)?;
        bail!("Helper failed ({status}): {}", error.trim());
    }
    Ok(output)
}

#[cfg(windows)]
struct WindowsJob(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl WindowsJob {
    fn attach(child: &std::process::Child) -> Result<Self> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::*;
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                bail!("Cannot create cancellation job");
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let wrapper = Self(job);
            if SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
                || AssignProcessToJobObject(job, child.as_raw_handle() as _) == 0
            {
                bail!("Cannot isolate helper for cancellation");
            }
            Ok(wrapper)
        }
    }
}
#[cfg(windows)]
impl Drop for WindowsJob {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn cancellation_terminates_process_tree() {
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let signal = cancel.clone();
        let trigger = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            signal.store(true, Ordering::Relaxed);
        });
        let start = Instant::now();
        let result = run(
            Path::new("/bin/sh"),
            &["-c".into(), "sleep 20 & wait".into()],
            None,
            &cancel,
            None,
        );
        trigger.join().unwrap();
        assert!(result.is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
