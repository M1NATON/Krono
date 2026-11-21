mod protocol;
mod tools;

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use krono_core::mcp_policy::McpPolicy;
use tools::VaultSandbox;

struct CliArgs {
    vault: PathBuf,
    allow_dirs: Vec<String>,
    allow_all: bool,
    allow_write: bool,
}

fn print_usage() {
    eprintln!(
        "Usage: krono-mcp [mcp] --vault <path> [--allow-dir <rel_dir>]... [--allow-all] [--allow-write]"
    );
    eprintln!("Options:");
    eprintln!("  --vault <path>        Path to the Krono vault directory (required)");
    eprintln!("  --allow-dir <dir>     Allow access to a relative subfolder (repeatable)");
    eprintln!("  --allow-all           Allow access to the entire vault (except .md.enc/.krono/.git)");
    eprintln!("  --allow-write         Enable the `append` write tool (disabled by default)");
    eprintln!("  -h, --help            Print this help message");
}

fn parse_args() -> Result<CliArgs, i32> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("mcp") {
        args.remove(0);
    }

    let mut vault: Option<PathBuf> = None;
    let mut allow_dirs = Vec::new();
    let mut allow_all = false;
    let mut allow_write = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_usage();
                return Err(0);
            }
            "--vault" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    eprintln!("krono-mcp: missing value for --vault");
                    return Err(2);
                };
                vault = Some(PathBuf::from(val));
            }
            "--allow-dir" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    eprintln!("krono-mcp: missing value for --allow-dir");
                    return Err(2);
                };
                allow_dirs.push(val.clone());
            }
            "--allow-all" => {
                allow_all = true;
            }
            "--allow-write" => {
                allow_write = true;
            }
            other => {
                eprintln!("krono-mcp: unknown argument '{other}'");
                print_usage();
                return Err(2);
            }
        }
        i += 1;
    }

    let Some(vault) = vault else {
        eprintln!("krono-mcp: --vault <path> is required");
        print_usage();
        return Err(2);
    };

    Ok(CliArgs {
        vault,
        allow_dirs,
        allow_all,
        allow_write,
    })
}

fn main() {
    let cli = match parse_args() {
        Ok(c) => c,
        Err(code) => std::process::exit(code),
    };

    let policy = match McpPolicy::new(&cli.allow_dirs, cli.allow_all, cli.allow_write) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("krono-mcp: invalid policy: {e}");
            std::process::exit(2);
        }
    };

    let sandbox = match VaultSandbox::new(cli.vault.clone(), policy) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("krono-mcp: failed to open vault: {e}");
            std::process::exit(1);
        }
    };

    eprintln!(
        "krono-mcp ready (vault={}, allow_all={}, allow_dirs={:?}, allow_write={})",
        cli.vault.display(),
        sandbox.policy().allow_all(),
        sandbox.policy().allowed_dirs(),
        sandbox.policy().allow_write()
    );

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    for line_res in stdin.lock().lines() {
        let line = match line_res {
            Ok(l) => l,
            Err(e) => {
                eprintln!("krono-mcp: stdin read error: {e}");
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(resp) = protocol::handle_json_line(&line, &sandbox) {
            if let Err(e) = writeln!(out, "{resp}").and_then(|_| out.flush()) {
                eprintln!("krono-mcp: stdout write error: {e}");
                break;
            }
        }
    }
}
