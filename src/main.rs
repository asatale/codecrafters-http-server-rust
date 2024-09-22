use std::{env, io::{Read, Write}, net::{TcpListener, TcpStream}, thread};
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
        response.set_header("Content-Type", "text/plain");
        send_response(stream,&response.to_string().unwrap())?;
    } else if request.url.starts_with("/user-agent") {
        response.set_body(&request.headers.get("User-Agent").unwrap_or(&"".to_string()));
        response.set_header("Content-Type", "text/plain");
        send_response(stream,&response.to_string().unwrap())?;
    } else if request.url.starts_with("/files"){
        let filename = request.url.split("/").collect::<Vec<&str>>()[2];
        let dirname = get_serving_directory();

        let result = std::fs::read_to_string(format!("{}/{}", dirname, filename));
        match result {
            Ok(content) => {
                response.set_body(&content);
                response.set_header("Content-Type", "application/octet-stream");
                send_response(stream,&response.to_string().unwrap())?;
            },
            Err(_) => {
                send_not_found(stream, request)?;
            }
        }
    } else if request.url == "/" {
        send_response(stream,&response.to_string().unwrap())?;
    } else {
        send_not_found(stream, request)?;
    }
    Ok(())
}

fn process_post_request(stream: &mut TcpStream, request: HttpRequest) -> std::io::Result<()> {
    let mut response: HttpResponse = HttpResponse::new();
    response.set_version(&request.version);
    response.set_status(&"201 Created");

    if request.url.starts_with("/files/") {
        let filename = request.url.split("/").collect::<Vec<&str>>()[2];
        let dirname = get_serving_directory();
        let mut file = std::fs::File::create(format!("{}/{}", dirname, filename))?;
        file.write_all(request.body.as_bytes())?;
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
        "POST" => {
            process_post_request(stream, request)?;
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
    
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                thread::spawn(move || {
                    handle_client(&mut stream).unwrap_or(());
                    stream.shutdown(std::net::Shutdown::Both)
                        .unwrap_or_else(|e| println!("error: {} in closing connection", e));
                });
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
