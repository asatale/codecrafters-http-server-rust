// Uncomment this block to pass the first stage
use std::{io::Write, net::TcpListener};

fn handle_client(mut stream: std::net::TcpStream) -> std::io::Result<()> {
    let response = "HTTP/1.1 200 OK\r\n\r\n";
    stream.write(response.as_bytes())?;
    stream.flush()?;
    stream.shutdown(std::net::Shutdown::Both)?;
    Ok(())
}

fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    // println!("Logs from your program will appear here!");

    // Uncomment this block to pass the first stage

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();
    
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("accepted new connection");
                handle_client(stream).unwrap_or_else(|_x| println!("error handling client"));
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
