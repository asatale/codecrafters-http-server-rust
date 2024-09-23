use std::{env, net::{TcpListener}, thread};
use http_server_starter_rust::{Session, SessionConfig};



fn get_serving_directory() -> String {
    let args: Vec<String> = env::args().collect();
    for arg in args.iter() {
        if arg == "--directory" {
            return args.get(args.iter().position(|x| x == "--directory").unwrap() + 1).unwrap().to_string();
        }
    }
    return ".".to_string();
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").expect("Failed to create listener");
    let dirname = get_serving_directory();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let mut session = Session::new(SessionConfig{
                                                                    download_dir: dirname.clone(),
                                                                    upload_dir: dirname.clone(),},
                                                        stream);
                thread::spawn(move || {
                    session.handle_client().unwrap_or(());
                    session.close().unwrap_or(());
                });
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
