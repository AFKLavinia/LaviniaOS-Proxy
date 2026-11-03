//! Split-tunneling controller via Linux cgroups v2 and routing mark bypass.
use std::fs;
use std::path::Path;

#[allow(dead_code)]
pub const CGROUP_BYPASS_DIR: &str = "/sys/fs/cgroup/lavinia_bypass";

#[allow(dead_code)]
pub struct SplitTunnel;

impl SplitTunnel {
    /// Prepares cgroup v2 for bypass if supported on this system.
    #[allow(dead_code)]
    pub fn init_cgroup() -> std::io::Result<()> {
        let p = Path::new(CGROUP_BYPASS_DIR);
        if !p.exists() {
            fs::create_dir_all(p)?;
        }
        Ok(())
    }

    /// Adds a process PID to the bypass cgroup so its packets route directly to the Internet.
    #[allow(dead_code)]
    pub fn bypass_pid(pid: u32) -> std::io::Result<()> {
        let procs_file = format!("{}/cgroup.procs", CGROUP_BYPASS_DIR);
        fs::write(procs_file, pid.to_string())
    }
}

