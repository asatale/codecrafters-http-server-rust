use std::{collections::HashMap, env};
use http_server_starter_rust::{ HeaderMap, HttpFrame, HttpServer, Method, Version };



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
    let listen_addr = "127.0.0.1:4221";
    let listen_port = 4221;
    let dirname = get_serving_directory();
    let _supported_encoding = vec!("gzip".to_string(), "deflate".to_string());

    let mut server = HttpServer::new(listen_addr, listen_port, );

    let _= server.add_route(Method::GET, "/".to_string(), |_request:Vec<HttpFrame>| {
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
    }).add_route(Method::GET, "/user-agent".to_string(), |request:Vec<HttpFrame>| {
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
        let user_agent = match headers.map.get("User-Agent").unwrap().get(0) {
            Some(user_agent) => user_agent,
            None => &"".to_string(),
        };
        if user_agent.len() > 0 {
            let response_body = HttpFrame::BodyChunk {
                chunk: Vec::<u8>::from(user_agent.as_bytes()),
            };
            return Ok(vec![response, response_body]);
        }
        Ok(vec![response])
    }).add_route(Method::GET, "/echo/*".to_string(), |request:Vec<HttpFrame>| {
        let response = HttpFrame::ResponseHead{
            status: (200,"OK".to_string()),
            version: Version::Http1_1,
            headers: HeaderMap{
                map: HashMap::from([
                                    ("Content-Type".to_string(), vec!["text/html".to_string()]),
                                ])
                            },
        };
        let result = request.get(0).unwrap().get_uri().unwrap();
        let (prefix, remaining) = result.split_at("/echo/".len());
        assert_eq!(prefix, "/echo/");

        let response_body = HttpFrame::BodyChunk { chunk: Vec::<u8>::from(remaining.as_bytes()) };
        Ok(vec![response, response_body])
    }).add_route(Method::GET, "/files/*".to_string(), move|request:Vec<HttpFrame>| {
        let result = request.get(0).unwrap().get_uri().unwrap();
        let (prefix, filename) = result.split_at("/files/".len());
        assert_eq!(prefix, "/files/");

        std::fs::read(format!("{}/{}", dirname, filename)).map(|content| {

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
    }).listen();

}
