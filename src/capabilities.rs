use std::io;

#[cfg(target_os = "linux")]
pub fn check_network_capture_capability() -> Result<bool, io::Error> {
    use std::{fs, os::unix::fs::PermissionsExt};

    // Check if we're running as root
    if unsafe { libc::geteuid() } == 0 {
        return Ok(true);
    }

    // Check if the binary has CAP_NET_RAW capability
    // This is a simplified check - a full implementation would use libcap
    let exe_path = std::env::current_exe()?;
    let metadata = fs::metadata(&exe_path)?;

    // This is a heuristic - proper capability checking requires libcap
    // For now, we assume if the file has special permissions, capabilities might be set
    Ok(metadata.permissions().mode() & 0o7000 != 0)
}

#[cfg(not(target_os = "linux"))]
pub fn check_network_capture_capability() -> Result<bool, io::Error> {
    // On non-Linux systems, we can't easily check capabilities
    Ok(true)
}

pub fn get_capability_error_message() -> String {
    format!(
        "Network capture requires elevated privileges. You have several options:\n\n\
         1. Run with sudo:\n   \
            sudo {}\n\n\
         2. Set capabilities on the binary (recommended):\n   \
            sudo setcap cap_net_raw=eip {}\n\n\
         3. Use 'make build' which automatically sets capabilities:\n   \
            make build\n\n\
         4. To analyze existing packet captures, use -f option:\n   \
            {} -f capture.pcap",
        std::env::args().collect::<Vec<_>>().join(" "),
        std::env::current_exe().unwrap_or_default().display(),
        std::env::current_exe().unwrap_or_default().display()
    )
}
