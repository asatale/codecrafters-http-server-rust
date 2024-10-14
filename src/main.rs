use std::{collections::HashMap, env};
use http_server_starter_rust::{ HeaderMap, HttpError, HttpFrame, HttpServer, Method, Version };



fn get_serving_directory() -> String {
    let args: Vec<String> = env::args().collect();
    for arg in args.iter() {
        if arg == "--directory" {
            return args.get(args.iter().position(|x| x == "--directory").unwrap() + 1).unwrap().to_string();
        }
    }
    return ".".to_string();
}

fn handle_default_path(_request:Vec<HttpFrame>) -> Result<Vec<HttpFrame>, HttpError> {
    println!("Handling default path");
    let response = HttpFrame::ResponseHead {
        status: (200,"OK".to_string()),
        version: Version::Http1_1,
        headers: HeaderMap{
            map: HashMap::from([
                                ("Content-Type".to_string(), vec!["text/html".to_string()]),
                            ])
                        },
    };
    Ok(vec![response])
}

fn handle_user_agent(request:Vec<HttpFrame>) -> Result<Vec<HttpFrame>, HttpError> {
    println!("Handling user-agent");
    let response = HttpFrame::ResponseHead {
        status: (200,"OK".to_string()),
        version: Version::Http1_1,
        headers: HeaderMap{
            map: HashMap::from([
                                ("Content-Type".to_string(), vec!["text/html".to_string()]),
                            ])
                        },
    };
    let headers = match request.get(0).unwrap() {
        HttpFrame::RequestHead { headers, .. } => headers,
        _ => panic!("Invalid request type"),
    };
    match headers.map.get("User-Agent").unwrap().get(0) {
        Some(user_agent) => {
            let response_body = HttpFrame::BodyChunk {
                chunk: Vec::<u8>::from(user_agent.as_bytes()),
            };
            return Ok(vec![response, response_body]);
        },
        None => (),
    };
    Ok(vec![response])

}

fn handle_echo(request:Vec<HttpFrame>) -> Result<Vec<HttpFrame>, HttpError> {
    println!("Handling echo");
    let response = HttpFrame::ResponseHead{
        status: (200,"OK".to_string()),
        version: Version::Http1_1,
        headers: HeaderMap{
            map: HashMap::from([
                                ("Content-Type".to_string(), vec!["text/html".to_string()]),
                            ])
                        },
    };
    let result = request.get(0).unwrap().get_uri();
    let (prefix, remaining) = result.split_at("/echo/".len());
    assert_eq!(prefix, "/echo/");

    let response_body = HttpFrame::BodyChunk { chunk: Vec::<u8>::from(remaining.as_bytes()) };
    Ok(vec![response, response_body])
}

fn handle_files(request:Vec<HttpFrame>) -> Result<Vec<HttpFrame>, HttpError> {
    println!("Handling files");
    let dirname = get_serving_directory();
    let result = request.get(0).unwrap().get_uri();
    let (prefix, filename) = result.split_at("/files/".len());
    assert_eq!(prefix, "/files/");

    std::fs::read(format!("{}/{}",dirname,filename)).map(|content| {

        let response = HttpFrame::ResponseHead {
            status: (200,"OK".to_string()),
            version: Version::Http1_1,
            headers: HeaderMap{
                map: HashMap::from([
                                    ("Content-Type".to_string(), vec!["text/html".to_string()]),
                                ])
                            },
        };
        let response_body = HttpFrame::BodyChunk {
            chunk: content,
        };
        Ok(vec![response, response_body])
    }).unwrap_or_else(|_| {
        let response = HttpFrame::ResponseHead {
            status: (404,"Not Found".to_string()),
            version: Version::Http1_1,
            headers: HeaderMap{
                map: HashMap::new()
            }
        };
        Ok(vec![response])
    })
}

fn main() {
    let listen_addr = "127.0.0.1";
    let listen_port = 4221;
    let _supported_encoding = vec!("gzip".to_string(), "deflate".to_string());
    let mut server = HttpServer::new(listen_addr, listen_port, );

    server.add_route(Method::GET, "/".to_string(), &handle_default_path);
    server.add_route(Method::GET, "/user-agent".to_string(),&handle_user_agent);
    server.add_route(Method::GET, "/echo/".to_string(), &handle_echo);
    server.add_route(Method::GET, "/files/".to_string(), &handle_files);

    match server.listen() {
        Ok(_) => println!("Server started at http://{}", listen_addr),
        Err(_e) => println!("Error starting server"),
    }

}
