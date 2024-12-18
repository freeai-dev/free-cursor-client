fn main() {
    use std::time::SystemTime;
    use winres::WindowsResource;

    // Get git commit hash
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("Failed to get git commit hash");
    let commit_hash = String::from_utf8(output.stdout)
        .expect("Invalid UTF-8")
        .trim()
        .to_string();
    println!("cargo:rustc-env=BUILD_ID={}", commit_hash);

    if cfg!(target_os = "windows") {
        let mut res = WindowsResource::new();

        let version = env!("CARGO_PKG_VERSION");
        // Parse version string into four numbers (major.minor.patch.build)
        let version_parts: Vec<u32> = version
            .split('.')
            .map(|s| s.parse().unwrap_or(0))
            .chain(std::iter::repeat(0))
            .take(4)
            .collect();

        // Version info needs to be packed into a single u64
        let make_version = |parts: &[u32]| -> u64 {
            ((parts[0] as u64) << 48)
                | ((parts[1] as u64) << 32)
                | ((parts[2] as u64) << 16)
                | (parts[3] as u64)
        };

        let version_num = make_version(&version_parts);
        res.set_version_info(winres::VersionInfo::FILEVERSION, version_num);
        res.set_version_info(winres::VersionInfo::PRODUCTVERSION, version_num);

        // Calculate current year for copyright notice
        let current_year = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            / 31557600
            + 1970;

        res.set("FileVersion", version)
            .set("ProductVersion", version)
            .set("FileDescription", "Free Cursor Client")
            .set("ProductName", "Free Cursor Client")
            .set("CompanyName", "Free Cursor")
            .set("LegalCopyright", &format!("Copyright © {}", current_year))
            .set("BuildId", &commit_hash);

        res.compile().unwrap();
    }
}
