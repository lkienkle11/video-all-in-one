use std::{
    io::{self, Write},
    process::{Command, Stdio},
    time::Duration,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match args.first().and_then(|v| v.to_str()).unwrap_or("") {
        "lines" => {
            for i in 0..100 {
                println!("out {i}");
                eprintln!("err {i}");
            }
        }
        "exit" => {
            eprintln!("chosen failure");
            std::process::exit(23);
        }
        "echo" => {
            for arg in &args[1..] {
                println!("{}", arg.to_string_lossy());
            }
        }
        "stderr" => {
            for arg in &args[1..] {
                eprintln!("{}", arg.to_string_lossy());
            }
        }
        "bytes" => {
            io::stdout().write_all(&[b'a', 0xff, b'\n'])?;
            io::stderr().write_all("界".repeat(30_000).as_bytes())?;
        }
        "boundary" => {
            io::stderr().write_all("x".repeat(65_533).as_bytes())?;
            io::stderr().write_all(args[2].to_string_lossy().as_bytes())?;
            io::stderr().write_all(b"\n")?;
        }
        "tree" | "orphan" => {
            let mut child = Command::new(std::env::current_exe()?)
                .args([std::ffi::OsStr::new("sleep"), args[1].as_os_str()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            // The child publishes readiness only after it is running.
            while !std::path::Path::new(&args[1]).exists() {
                std::thread::sleep(Duration::from_millis(5));
            }
            println!("ready");
            io::stdout().flush()?;
            if args[0] == "orphan" {
                return Ok(());
            }
            let _ = child.wait()?;
        }
        "sleep" => {
            std::fs::write(&args[1], std::process::id().to_string())?;
            std::thread::sleep(Duration::from_secs(60));
        }
        "cwd" => println!("{}", std::env::current_dir()?.display()),
        _ => return Err("unknown helper mode".into()),
    }
    Ok(())
}
