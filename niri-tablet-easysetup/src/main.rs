mod apps;
mod catalog;
mod discovery;
mod export;
mod kdl;
mod model;
mod niri;
mod store;
mod ui;

const HELP: &str = "\
niri-tablet-easysetup - friendly GUI configurator for niri-tablet gestures

Usage: niri-tablet-easysetup [OPTION]

Options:
  (no option)    open the GUI
  --check        print what would be loaded (discovery + current scheme)
  --version      print version
  -h, --help     this help
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version") {
        println!("niri-tablet-easysetup {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|a| a == "--check") {
        run_check();
        return;
    }
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{HELP}");
        return;
    }
    ui::run();
}

/// Headless startup report: same discovery/load path as the GUI.
fn run_check() {
    let niri_bin = niri::find_niri();
    match &niri_bin {
        Some(p) => {
            let (ok, msg) = niri::gestures_supported(p);
            if ok {
                println!("niri: {} (gestures config supported)", p.display());
            } else {
                println!("niri: {} (NO gesture config support!)\n  {msg}", p.display());
            }
            println!("ipc: {}", if niri::ipc_available() { "available" } else { "not found" });
        }
        None => println!("niri: not found on PATH"),
    }

    let loaded = store::load();
    match &loaded.discovery.config_path {
        Some(p) => println!("config: {}", p.display()),
        None => println!("config: none found"),
    }
    match &loaded.discovery.source {
        discovery::Source::ManagedFile(p) => println!("gestures: managed file {}", p.display()),
        discovery::Source::ManagedFileAmbiguous(p, others) => {
            println!("gestures: managed file {} (also present: {:?})", p.display(), others)
        }
        discovery::Source::Inline(p) => println!("gestures: inline in {}", p.display()),
        discovery::Source::None => println!("gestures: not configured anywhere (first run)"),
    }
    for w in loaded.warnings.iter().chain(&loaded.model.warnings) {
        println!("warning: {w}");
    }
    for n in loaded.discovery.notices() {
        println!("notice: {n:?}");
    }
    println!("--- current scheme ---");
    println!("{}", export::scheme_to_json(&loaded.model));
}
