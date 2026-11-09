use cosmic_applet_disk_space::storage;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    match arguments.as_slice() {
        [argument] if argument == "--help" || argument == "-h" => {
            println!(
                "COSMIC Disk Space {}\n\n\
                 Usage: cosmic-applet-disk-space [--dump | --help | --version]\n\n\
                 No arguments: start the COSMIC panel applet.\n\
                 --dump: print one live storage snapshot as JSON without opening a window.",
                env!("CARGO_PKG_VERSION")
            );
        }
        [argument] if argument == "--version" || argument == "-V" => {
            println!("cosmic-applet-disk-space {}", env!("CARGO_PKG_VERSION"));
        }
        [argument] if argument == "--dump" => {
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| error.to_string())
                .and_then(|runtime| runtime.block_on(storage::scan()));
            match result {
                Ok(snapshot) => match serde_json::to_string_pretty(&snapshot) {
                    Ok(json) => println!("{json}"),
                    Err(error) => {
                        eprintln!("Cannot encode storage snapshot: {error}");
                        return ExitCode::FAILURE;
                    }
                },
                Err(error) => {
                    eprintln!("Cannot read disk usage: {error}");
                    return ExitCode::FAILURE;
                }
            }
        }
        [] => return run_applet(),
        _ => {
            eprintln!("Unknown arguments. Use --help for usage.");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

#[cfg(feature = "applet")]
fn run_applet() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cosmic_applet_disk_space=info,warn".into()),
        )
        .init();
    match cosmic::applet::run::<cosmic_applet_disk_space::app::DiskSpace>(()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Cannot start COSMIC Disk Space: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(feature = "applet"))]
fn run_applet() -> ExitCode {
    eprintln!(
        "This diagnostic-only build supports --dump. Build with default features for the applet."
    );
    ExitCode::FAILURE
}
