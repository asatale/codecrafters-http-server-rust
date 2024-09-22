use std::{io::{Read, Write}, net::{TcpListener, TcpStream}};
use http_server_starter_rust::{Request as HttpRequest, Response as HttpResponse};


// Send response string to the client
fn send_response(stream: &mut std::net::TcpStream, response: &str) -> std::io::Result<()> {
    stream.write(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

// Process GET request
fn process_get_request(stream: &mut TcpStream, request: HttpRequest) -> std::io::Result<()> {
    let mut response: HttpResponse = HttpResponse::new();
    response.set_version(&request.version);
    response.set_status(&"200 OK");

    if request.url.starts_with("/echo/") {
        response.set_body(request.url.split("/").collect::<Vec<&str>>()[2]);
        response.set_header("Content Type", "text/plain");
        send_response(stream,&response.to_string().unwrap())?;
    } else {
       send_not_found(stream, request)?;
    }
    Ok(())
}


// Send 404 Not Found response to the client
fn send_not_found(stream: &mut TcpStream, request: HttpRequest)-> std::io::Result<()> {
    let mut response: HttpResponse = HttpResponse::new();
    response.set_version(&request.version);
    response.set_status(&"404 Not Found");
    send_response(stream,&response.to_string().unwrap())
}

// Send 405 Method Not Allowed response to the client
fn send_not_allowed(stream: &mut TcpStream, request: HttpRequest)-> std::io::Result<()> {
    let mut response: HttpResponse = HttpResponse::new();
    response.set_version(&request.version);
    response.set_status(&"405 Method Not Allowed");
    response.set_header("Allow", "GET");
    send_response(stream,&response.to_string().unwrap())
}

fn parse_request(stream: &mut TcpStream) -> std::io::Result<HttpRequest> {
    let reader = |b: &mut[u8]| {stream.read(b)};
    HttpRequest::from_stream(reader)
}

fn process_request(stream: &mut TcpStream, request: HttpRequest) -> std::io::Result<()> {
    match request.method.as_str() {
        "GET" => {
            process_get_request(stream, request)?;
        },
        _ => {
            send_not_allowed(stream, request)?;
        }
    }
    Ok(())
}

fn handle_client(stream: &mut TcpStream) -> std::io::Result<()> {
    let result: Result<HttpRequest, std::io::Error> = parse_request(stream);
    match result {
        Ok(request) => {
            process_request(stream, request)?;
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
