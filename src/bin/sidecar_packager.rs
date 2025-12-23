use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Deserialize;
use tar::Builder;

#[derive(Deserialize)]
struct ProductJson {
    #[serde(rename = "serverApplicationName")]
    server_application_name: Option<String>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("packager error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let args = Args::parse(&manifest_dir)?;

    if !args.build_dir.is_dir() {
        return Err(format!(
            "sidecar build directory not found at {}",
            args.build_dir.display()
        )
        .into());
    }
    if !args.launcher_bin.is_file() {
        return Err(format!(
            "launcher binary not found at {}",
            args.launcher_bin.display()
        )
        .into());
    }

    let bin_dir = args.build_dir.join("bin");
    if !bin_dir.is_dir() {
        return Err(format!("sidecar bin directory missing at {}", bin_dir.display()).into());
    }

    let server_bin_path = bin_dir.join(&args.server_app_name);
    fs::copy(&args.launcher_bin, &server_bin_path)?;
    set_executable(&server_bin_path, &args.launcher_bin)?;

    if let Some(parent) = args.out_path.parent() {
        fs::create_dir_all(parent)?;
    }

    write_tar_gz(&args.build_dir, &args.out_path)?;
    println!("Wrote sidecar archive to {}", args.out_path.display());

    Ok(())
}

struct Args {
    build_dir: PathBuf,
    out_path: PathBuf,
    launcher_bin: PathBuf,
    server_app_name: String,
}

impl Args {
    fn parse(manifest_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mut build_dir: Option<PathBuf> = None;
        let mut out_path: Option<PathBuf> = None;
        let mut launcher_bin: Option<PathBuf> = None;
        let mut server_app_name: Option<String> = None;

        let mut iter = env::args();
        iter.next();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--build-dir" => {
                    build_dir = Some(PathBuf::from(next_value(&mut iter, "--build-dir")?));
                }
                "--out" => {
                    out_path = Some(PathBuf::from(next_value(&mut iter, "--out")?));
                }
                "--launcher-bin" => {
                    launcher_bin = Some(PathBuf::from(next_value(&mut iter, "--launcher-bin")?));
                }
                "--server-app-name" => {
                    server_app_name = Some(next_value(&mut iter, "--server-app-name")?);
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                _ => {
                    if let Some(value) = arg.strip_prefix("--build-dir=") {
                        build_dir = Some(PathBuf::from(value));
                    } else if let Some(value) = arg.strip_prefix("--out=") {
                        out_path = Some(PathBuf::from(value));
                    } else if let Some(value) = arg.strip_prefix("--launcher-bin=") {
                        launcher_bin = Some(PathBuf::from(value));
                    } else if let Some(value) = arg.strip_prefix("--server-app-name=") {
                        server_app_name = Some(value.to_string());
                    } else {
                        return Err(format!("unknown argument: {arg}").into());
                    }
                }
            }
        }

        let build_dir = build_dir.unwrap_or_else(|| manifest_dir.join("vscode-server-linux-arm64"));
        let out_path = out_path.unwrap_or_else(|| {
            manifest_dir
                .parent()
                .unwrap_or(manifest_dir)
                .join("extention/resources/sidecar/sidecar.tar.gz")
        });
        let launcher_bin = launcher_bin.unwrap_or_else(|| {
            manifest_dir
                .join("target")
                .join("release")
                .join("uplink-server")
        });

        let server_app_name = match server_app_name {
            Some(name) => name,
            None => load_server_app_name(&build_dir, manifest_dir)
                .unwrap_or_else(|| "uplink-server".to_string()),
        };

        Ok(Self {
            build_dir,
            out_path,
            launcher_bin,
            server_app_name,
        })
    }
}

fn next_value(
    iter: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    iter.next()
        .ok_or_else(|| format!("missing value for {flag}").into())
}

fn load_server_app_name(build_dir: &Path, manifest_dir: &Path) -> Option<String> {
    let candidates = [build_dir.join("product.json"), manifest_dir.join("sidecar/product.json")];
    for candidate in candidates {
        if let Ok(contents) = fs::read_to_string(candidate) {
            if let Ok(product) = serde_json::from_str::<ProductJson>(&contents) {
                if let Some(name) = product.server_application_name {
                    return Some(name);
                }
            }
        }
    }
    None
}

fn set_executable(path: &Path, source: &Path) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(source)?.permissions().mode();
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
    Ok(())
}

fn write_tar_gz(build_dir: &Path, out_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let top_level = build_dir
        .file_name()
        .ok_or("failed to determine sidecar folder name")?;

    let file = fs::File::create(out_path)?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut tar = Builder::new(enc);
    tar.append_dir_all(top_level, build_dir)?;
    tar.finish()?;
    let enc = tar.into_inner()?;
    enc.finish()?;
    Ok(())
}

fn print_usage() {
    println!(
        "Usage: sidecar-packager [--build-dir PATH] [--out PATH] [--launcher-bin PATH] [--server-app-name NAME]\n\
        \n\
        Defaults:\n\
          --build-dir    server/vscode-server-linux-arm64\n\
          --out          extention/resources/sidecar/sidecar.tar.gz\n\
          --launcher-bin server/target/release/uplink-server\n"
    );
}
