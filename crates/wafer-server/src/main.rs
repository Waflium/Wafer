use wafer_server::logging;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let server_main = std::thread::Builder::new()
        .name("main".into())
        .spawn(move || {
            logging::init("logs");
            wafer_server::server::main::main(args);
        })
        .expect("server main thread");
    let _ = server_main.join();
    wafer_server::server::dedicated::run_shutdown_hooks();
    logging::shutdown();
    std::process::exit(wafer_server::server::dedicated::exit_status())
}
