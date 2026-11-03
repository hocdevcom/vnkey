#[cfg(not(target_os = "linux"))]
compile_error!(
    "viet-config is Linux-only; build the library without the config-app feature on Windows"
);

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = viet_core::control::run(&args) {
        eprintln!("viet-config: {error}");
        std::process::exit(1);
    }
}
