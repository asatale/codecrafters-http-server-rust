use std::{io::{Read, Write}, net::{TcpListener, TcpStream}};
use http_server_starter_rust::{Request as HttpRequest, Response as HttpResponse};

// Send response string to the client
fn send_response(stream: &mut std::net::TcpStream, response: &str) -> std::io::Result<()> {
    stream.write(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

// Send 200 OK response to the client
fn send_ok(stream: &mut TcpStream, request: HttpRequest) -> std::io::Result<()> {
    let mut response: HttpResponse = HttpResponse::new();
    response.set_version(&request.version);
    response.set_status(&"200 OK");
    send_response(stream,&response.to_string().unwrap())
}

// Send 404 Not Found response to the client
fn send_not_found(stream: &mut TcpStream, request: HttpRequest)-> std::io::Result<()> {
    let mut response: HttpResponse = HttpResponse::new();
    response.set_version(&request.version);
    response.set_status(&"404 Not Found");
    send_response(stream,&response.to_string().unwrap())
}

fn handle_request(stream: &mut TcpStream) -> std::io::Result<HttpRequest> {
    let reader = |b: &mut[u8]| {stream.read(b)};
    HttpRequest::from_stream(reader)
}


fn handle_client(stream: &mut TcpStream) -> std::io::Result<()> {
    let result: Result<HttpRequest, std::io::Error> = handle_request(stream);
    match result {
        Ok(request) => {
            if request.url == "/" {
                send_ok(stream, request)?;
            } else {
                send_not_found(stream, request)?;
            }
        }
        Err(e) => {
            println!("Error parsing request: {}", e);
        }
    }
    Ok(())
}

fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    // println!("Logs from your program will appear here!");

    // Uncomment this block to pass the first stage

    let listener = TcpListener::bind("127.0.0.1:4221").expect("Failed to create listener");
    
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                handle_client(&mut stream).unwrap_or(());
                stream.shutdown(std::net::Shutdown::Both)
                    .unwrap_or_else(|e| println!("error: {} in closing connection", e));
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
