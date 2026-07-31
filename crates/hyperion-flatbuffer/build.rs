use std::{env, error::Error, path::PathBuf, process::Command};

fn main() -> Result<(), Box<dyn Error>> {
    let output = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?);
    let status = Command::new(flatc_fork::flatc())
        .args(["--rust", "--gen-onefile", "-o"])
        .arg(&output)
        .args(["schemas/hyperion_request.fbs", "schemas/hyperion_reply.fbs"])
        .status()?;

    if !status.success() {
        return Err(format!("flatc exited with {status}").into());
    }

    println!("cargo::rerun-if-changed=schemas/hyperion_request.fbs");
    println!("cargo::rerun-if-changed=schemas/hyperion_reply.fbs");
    Ok(())
}
