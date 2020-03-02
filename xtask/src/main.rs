use std::env;
use std::process::Command;
use std::ffi::OsString;

type DynError = Box<dyn std::error::Error>;

fn main() {
    if let Err(e) = try_main() {
        eprintln!("{}", e);
        std::process::exit(-1);
    }
}

fn try_main() -> Result<(), DynError> {
    let task = env::args().nth(1);
    match task.as_ref().map(|it| it.as_str()) {
        Some("nsight") => nsight(),
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        "Tasks:
nsight            runs nv-nsight-gfx on the debug executable
"
    )
}

fn nsight() {
    let cd = env::current_dir().unwrap();

    let mut dir_arg = OsString::from("--dir=");
    dir_arg.push(cd.as_os_str());

    let mut exe_arg = OsString::from("--exe=");
    exe_arg.push(cd.join("target/debug/phasor"));

    Command::new("nv-nsight-launcher")
        .args(&[OsString::from("--activity=Frame Debugger"), dir_arg, exe_arg])
        .spawn()
        .expect("failed to launch nv-nsight-launcher")
        .wait()
        .expect("waiting for child process failed");
}
