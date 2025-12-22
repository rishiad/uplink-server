use std::env;
use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("launcher error: {err}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<i32, Box<dyn std::error::Error>> {
    let mut args: Vec<OsString> = env::args_os().collect();
    if !args.is_empty() {
        args.remove(0);
    }

    let inspect_arg = if let Some(first) = args.first() {
        let first_str = first.to_string_lossy();
        if first_str.starts_with("--inspect") {
            Some(args.remove(0))
        } else {
            None
        }
    } else {
        None
    };

    let exe_path = env::current_exe()?.canonicalize()?;
    let bin_dir = exe_path
        .parent()
        .ok_or("failed to resolve launcher binary directory")?;
    let root = bin_dir.parent().ok_or("failed to resolve server root")?;

    let node_path = root.join("node");
    let server_main = root.join("out").join("server-main.js");
    if !node_path.exists() {
        return Err(format!("node binary not found at {}", node_path.display()).into());
    }
    if !server_main.exists() {
        return Err(format!("server entrypoint not found at {}", server_main.display()).into());
    }

    maybe_patch_glibc(&node_path);

    let mut cmd = Command::new(&node_path);
    if let Some(inspect) = inspect_arg {
        cmd.arg(inspect);
    }
    cmd.arg(server_main).args(args);

    let status = cmd.status()?;
    Ok(status.code().unwrap_or(1))
}

fn maybe_patch_glibc(node_path: &Path) {
    let glibc_linker = env::var_os("VSCODE_SERVER_CUSTOM_GLIBC_LINKER");
    let glibc_path = env::var_os("VSCODE_SERVER_CUSTOM_GLIBC_PATH");
    let patchelf_path = env::var_os("VSCODE_SERVER_PATCHELF_PATH");

    let (Some(glibc_linker), Some(glibc_path), Some(patchelf_path)) =
        (glibc_linker, glibc_path, patchelf_path)
    else {
        return;
    };

    println!(
        "Patching glibc from {} with {}...",
        glibc_path.to_string_lossy(),
        patchelf_path.to_string_lossy()
    );
    if let Err(err) = Command::new(&patchelf_path)
        .arg("--set-rpath")
        .arg(&glibc_path)
        .arg(node_path)
        .status()
    {
        eprintln!("patchelf --set-rpath failed: {err}");
    }

    println!(
        "Patching linker from {} with {}...",
        glibc_linker.to_string_lossy(),
        patchelf_path.to_string_lossy()
    );
    if let Err(err) = Command::new(&patchelf_path)
        .arg("--set-interpreter")
        .arg(&glibc_linker)
        .arg(node_path)
        .status()
    {
        eprintln!("patchelf --set-interpreter failed: {err}");
    }

    println!("Patching complete.");
}
