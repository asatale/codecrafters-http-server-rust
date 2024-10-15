use std::{collections::HashMap, env, fs::OpenOptions, io::Write};
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

fn handle_default_path(request:Vec<HttpFrame>) -> Result<Vec<HttpFrame>, HttpError> {
    println!("Handling default path");
    let uri = request.get(0).unwrap().get_uri();
    if uri == "/" {
        let response = HttpFrame::ResponseHead {
            status: (200,"OK".to_string()),
            version: Version::Http1_1,
            headers: HeaderMap{
                map: HashMap::new(),
            },
        };
        Ok(vec![response])
    } else {
        let response = HttpFrame::ResponseHead {
            status: (404,"Not Found".to_string()),
            version: Version::Http1_1,
            headers: HeaderMap{
                map: HashMap::new(),
            },
        };
        Ok(vec![response])
    }
}

fn handle_user_agent(request:Vec<HttpFrame>) -> Result<Vec<HttpFrame>, HttpError> {
    println!("Handling user-agent");
    let response = HttpFrame::ResponseHead {
        status: (200,"OK".to_string()),
        version: Version::Http1_1,
        headers: HeaderMap{
            map: HashMap::from([
                                ("Content-Type".to_string(), vec!["text/plain".to_string()]),
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
                                ("Content-Type".to_string(), vec!["text/plain".to_string()]),
                            ])
                        },
    };
    let result = request.get(0).unwrap().get_uri();
    let (prefix, remaining) = result.split_at("/echo/".len());
    assert_eq!(prefix, "/echo/");

    let response_body = HttpFrame::BodyChunk { chunk: Vec::<u8>::from(remaining.as_bytes()) };
    Ok(vec![response, response_body])
}

fn handle_files_reads(request:Vec<HttpFrame>) -> Result<Vec<HttpFrame>, HttpError> {
    println!("Handling files reads");
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
                                    ("Content-Type".to_string(), vec!["application/octet-stream".to_string()]),
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


fn handle_files_writes(request:Vec<HttpFrame>) -> Result<Vec<HttpFrame>, HttpError> {
    println!("Handling files writes");
    let dirname = get_serving_directory();
    let result = request.get(0).unwrap().get_uri();
    let (prefix, filename) = result.split_at("/files/".len());
    assert_eq!(prefix, "/files/");

    let chunk = match request.get(1).unwrap() {
        HttpFrame::BodyChunk { chunk } => chunk,
        _ => panic!("Invalid request type"),
    };

    let mut file = OpenOptions::new().write(true).truncate(true).create(true).open(format!("{}/{}",dirname,filename)).unwrap();

    match file.write(&chunk) {
        Ok(_) => {
            let response = HttpFrame::ResponseHead {
                status: (201,"Created".to_string()),
                version: Version::Http1_1,
                headers: HeaderMap{
                    map: HashMap::new(),
                }
            };
            file.sync_all().unwrap();
            Ok(vec![response])
        },
        Err(e) => {
            println!("Error {}, Writing file: {}", e, format!("{}/{}",dirname,filename));
            let response = HttpFrame::ResponseHead {
                status: (500,"Internal Server Error".to_string()),
                version: Version::Http1_1,
                headers: HeaderMap{
                    map: HashMap::new()
                }
            };
            file.sync_all().unwrap();
            Ok(vec![response])
        },
    }
}


fn main() {
    let listen_addr = "127.0.0.1";
    let listen_port = 4221;
    let _supported_encoding = vec!("gzip".to_string(), "deflate".to_string());
    let mut server = HttpServer::new(listen_addr, listen_port, );

    server.add_route(Method::GET, "/".to_string(), &handle_default_path);
    server.add_route(Method::GET, "/user-agent".to_string(),&handle_user_agent);
    server.add_route(Method::GET, "/echo/".to_string(), &handle_echo);
    server.add_route(Method::GET, "/files/".to_string(), &handle_files_reads);
    server.add_route(Method::POST, "/files/".to_string(), &handle_files_writes);

    match server.listen() {
        Ok(_) => println!("Server started at http://{}", listen_addr),
        Err(_e) => println!("Error starting server"),
    }

}
