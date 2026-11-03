fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = viet_core::control::run(&args) {
        eprintln!("viet-config: {error}");
        std::process::exit(1);
    }
}
