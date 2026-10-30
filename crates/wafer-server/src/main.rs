use wafer_server::logging;

fn main() {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::Media::timeBeginPeriod(1);
    }
    print!("Starting net.minecraft.server.Main{}", if cfg!(windows) { "\r\n" } else { "\n" });
    let args: Vec<String> = std::env::args().skip(1).collect();
    let server_main = std::thread::Builder::new()
        .name(wafer_server::server::main::SERVER_MAIN.into())
        .spawn(move || {
            logging::init("logs");
            wafer_server::server::main::main(args);
        })
        .expect("server main thread");
    let _ = server_main.join();
    logging::shutdown();
    std::process::exit(wafer_server::server::dedicated::exit_status())
}
