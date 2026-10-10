//! 有界 Git 子进程；整次任务共享时限，并负责终止、等待所有辅助进程。
use crate::operation::{self, Operation, Phase};
use anyhow::{Result, bail};
use std::{
    cell::RefCell,
    ffi::{OsStr, OsString},
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread::{self, JoinHandle},
    time::Duration,
};

const OUTPUT_LIMIT: usize = 64 * 1024;
thread_local! {
    static EXECUTABLE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}
struct ExecutableScope(Option<PathBuf>);
impl Drop for ExecutableScope {
    fn drop(&mut self) {
        EXECUTABLE.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}

/// 为隔离测试注入假 Git 程序，不修改全局 PATH。
#[doc(hidden)]
pub fn with_git_executable<T>(executable: &Path, f: impl FnOnce() -> T) -> T {
    let _scope = ExecutableScope(EXECUTABLE.with(|s| s.replace(Some(executable.into()))));
    f()
}

pub(crate) fn task<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    if operation::current().is_some() {
        return f();
    }
    let op = Operation::new();
    operation::with_operation(&op, || {
        let result = f();
        op.finish();
        result
    })
}

pub struct GitRunner {
    home: PathBuf,
}
impl GitRunner {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }

    pub fn run<I, S>(&self, args: I, label: &str) -> Result<Vec<u8>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args: Vec<OsString> = args.into_iter().map(|s| s.as_ref().into()).collect();
        task(|| self.run_inner(&args, label))
    }

    fn run_inner(&self, args: &[OsString], label: &str) -> Result<Vec<u8>> {
        let op = operation::current().expect("Git task has an operation");
        op.check()?;
        op.set_phase(label, Phase::Network);
        op.check()?;
        let executable = EXECUTABLE
            .with(|s| s.borrow().clone())
            .unwrap_or_else(|| "git".into());
        let mut command = Command::new(executable);
        command
            .args([
                "-c",
                "credential.helper=",
                "-c",
                "credential.interactive=never",
                "-c",
                "core.hooksPath=",
                "-c",
                "core.fsmonitor=false",
                "-c",
                "protocol.allow=never",
                "-c",
                "protocol.https.allow=always",
                "-c",
                "protocol.file.allow=always",
            ])
            .args(args)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            )
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "")
            .env("SSH_ASKPASS", "")
            .env("GCM_INTERACTIVE", "never")
            .env("GIT_LFS_SKIP_SMUDGE", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // 不继承调用者指定的仓库、注入配置或可能泄露凭证的跟踪输出。
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_") {
                command.env_remove(key);
            }
        }
        command
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            )
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "")
            .env("GIT_LFS_SKIP_SMUDGE", "1");
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // 先挂入关闭即终止的 Job，再恢复子进程，避免辅助进程逃逸。
            command.creation_flags(0x0000_0004 | 0x0800_0000); // 挂起启动且无控制台窗口
        }
        let child = command
            .spawn()
            .map_err(|_| anyhow::anyhow!("Git 未安装或无法启动"))?;
        let mut process = Process::new(child)?;
        let stdout = process.child.stdout.take().expect("piped stdout");
        let stderr = process.child.stderr.take().expect("piped stderr");
        process.readers.push(drain(stdout, OUTPUT_LIMIT)?);
        process.readers.push(drain(stderr, 0)?);
        #[cfg(windows)]
        process.job.resume(&process.child)?;
        let status = loop {
            op.check()?;
            match process.child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => op.wait(Duration::from_millis(20)),
                Err(_) => bail!("无法等待 Git 进程结束"),
            }
        };
        // Git 结束后仍持管道的辅助进程不能导致读取线程永久等待。
        process.terminate();
        let mut output = Vec::new();
        for (index, reader) in process.readers.drain(..).enumerate() {
            let data = reader
                .join()
                .map_err(|_| anyhow::anyhow!("读取 Git 输出失败"))??;
            if index == 0 {
                output = data;
            }
        }
        op.check()?;
        if !status.success() {
            // 错误不得包含原始 stderr、命令参数、认证 URL 或环境值。
            bail!("Git 操作失败，请检查来源、分支或网络连接");
        }
        Ok(output)
    }
}

fn drain(
    mut pipe: impl Read + Send + 'static,
    limit: usize,
) -> Result<JoinHandle<Result<Vec<u8>>>> {
    thread::Builder::new()
        .name("chu-git-output".into())
        .spawn(move || {
            let mut captured = Vec::new();
            let mut buffer = [0; 8192];
            loop {
                match pipe.read(&mut buffer) {
                    Ok(0) => return Ok(captured),
                    Ok(n) => {
                        let keep = n.min(limit.saturating_sub(captured.len()));
                        captured.extend_from_slice(&buffer[..keep]);
                        // 捕获达到上限仍持续排空，避免管道回压阻塞 Git。
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => bail!("读取 Git 输出失败"),
                }
            }
        })
        .map_err(|_| anyhow::anyhow!("无法启动 Git 输出读取线程"))
}

struct Process {
    child: Child,
    readers: Vec<JoinHandle<Result<Vec<u8>>>>,
    terminated: bool,
    #[cfg(windows)]
    job: windows_job::Job,
}
impl Process {
    fn new(mut child: Child) -> Result<Self> {
        #[cfg(windows)]
        let job = match windows_job::Job::new(&child) {
            Ok(job) => job,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
        };
        #[cfg(not(windows))]
        let _ = &mut child;
        Ok(Self {
            child,
            readers: Vec::new(),
            terminated: false,
            #[cfg(windows)]
            job,
        })
    }
    fn terminate(&mut self) {
        if self.terminated {
            return;
        }
        self.terminated = true;
        #[cfg(unix)]
        {
            unsafe extern "C" {
                fn kill(pid: i32, signal: i32) -> i32;
            }
            // 子进程独占进程组；Git 网络、凭证辅助进程继承同一组。
            unsafe {
                kill(-(self.child.id() as i32), 9);
            }
        }
        #[cfg(windows)]
        self.job.terminate();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        self.terminate();
        for reader in self.readers.drain(..) {
            let _ = reader.join();
        }
    }
}

#[cfg(windows)]
mod windows_job {
    use anyhow::{Result, bail};
    use std::{ffi::c_void, os::windows::io::AsRawHandle, process::Child};
    type Handle = *mut c_void;
    #[repr(C)]
    #[derive(Default)]
    struct BasicLimit {
        process_time: i64,
        job_time: i64,
        flags: u32,
        min_working_set: usize,
        max_working_set: usize,
        active_processes: u32,
        affinity: usize,
        priority: u32,
        scheduling: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct ExtendedLimit {
        basic: BasicLimit,
        io_counters: [u64; 6],
        process_memory: usize,
        job_memory: usize,
        peak_process_memory: usize,
        peak_job_memory: usize,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateJobObjectW(attributes: Handle, name: *const u16) -> Handle;
        fn SetInformationJobObject(job: Handle, class: i32, info: Handle, length: u32) -> i32;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
        fn TerminateJobObject(job: Handle, code: u32) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
    }
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtResumeProcess(process: Handle) -> i32;
    }
    pub(super) struct Job(Handle);
    impl Job {
        pub fn new(child: &Child) -> Result<Self> {
            let handle = unsafe { CreateJobObjectW(std::ptr::null_mut(), std::ptr::null()) };
            if handle.is_null() {
                bail!("无法创建 Git 进程组");
            }
            let job = Self(handle);
            let mut limit = ExtendedLimit::default();
            limit.basic.flags = 0x2000; // 关闭 Job 时终止其所有进程
            let configured = unsafe {
                SetInformationJobObject(
                    handle,
                    9,
                    (&mut limit as *mut ExtendedLimit).cast(),
                    std::mem::size_of::<ExtendedLimit>() as u32,
                ) != 0
                    && AssignProcessToJobObject(handle, child.as_raw_handle()) != 0
            };
            if !configured {
                bail!("无法隔离 Git 辅助进程");
            }
            Ok(job)
        }
        pub fn resume(&self, child: &Child) -> Result<()> {
            if unsafe { NtResumeProcess(child.as_raw_handle()) } < 0 {
                bail!("无法启动 Git 进程");
            }
            Ok(())
        }
        pub fn terminate(&self) {
            unsafe {
                TerminateJobObject(self.0, 1);
            }
        }
    }
    impl Drop for Job {
        fn drop(&mut self) {
            self.terminate();
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}
