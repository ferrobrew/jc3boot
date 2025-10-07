use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::Parser;
use re_utilities_injector as injector;

use windows::Win32::System::Threading::{ResumeThread, TerminateProcess};

#[derive(Debug, Parser)]
struct Args {
    #[clap(short, long)]
    spawn: bool,
    path: Option<PathBuf>,
    #[clap(short, long)]
    payload: Option<String>,
    #[clap(short, long)]
    dont_resume: bool,

    #[clap(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
    _args: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let payload_name = args
        .payload
        .unwrap_or_else(|| "jc3boot_payload.dll".to_string());

    let file_name = args
        .path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|p| p.to_string_lossy())
        .unwrap_or("JustCause3.exe".into());

    if args.spawn {
        for (process_id, process) in injector::get_processes_by_name(&file_name)? {
            println!("Terminating process with PID {process_id}");
            unsafe { TerminateProcess(*process, 0)? };
        }
    }

    let cmd_args = args._args.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    let pi = match &args.path {
        Some(path) => injector::spawn::arbitrary_process(
            path.parent().expect("no parent directory for executable"),
            path,
            std::iter::empty(),
            cmd_args,
            true,
        ),
        None => {
            injector::spawn::steam_process(225540, |p| p.join("JustCause3.exe"), cmd_args, true)
        }
    }?;

    println!("Spawned process with PID {}", pi.process_id);

    let payload_path = std::env::current_exe()?
        .parent()
        .context("failed to find launcher executable directory")?
        .join(payload_name);

    if let Err(err) = inject(&pi, &payload_path, args.dont_resume) {
        println!("Failed to inject: {err}");
        unsafe { TerminateProcess(*pi.process, 0)? };
        return Err(err);
    }

    Ok(())
}

fn inject(
    pi: &injector::spawn::ProcessInformation,
    payload_path: &Path,
    dont_resume: bool,
) -> anyhow::Result<()> {
    println!("Injecting into process with PID {}", pi.process_id);
    let payload_path = injector::inject(*pi.process, payload_path).context("failed to inject")?;

    println!("Running payload");
    let payload_base = injector::get_remote_module_base(pi.process_id, &payload_path)
        .context("failed to get payload base")?
        .context("payload base is null")?;
    injector::call_remote_export(
        *pi.process,
        payload_base,
        "run",
        Some(std::time::Duration::from_secs(10)),
    )
    .context("failed to call payload run")?;

    // Processes are resumed after injection to ensure the payload is loaded before the game
    if !dont_resume {
        unsafe { ResumeThread(*pi.thread) };
    }

    Ok(())
}
