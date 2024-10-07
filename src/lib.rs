
use std::{collections::HashMap, io::{Error, ErrorKind, Read, Write}, str::SplitWhitespace};
//use flate2::write::GzEncoder;
//use flate2::Compression;

#[derive(Debug)]
pub struct HeaderMap {
    pub map: HashMap<String, Vec<String>>,
}
impl HeaderMap {
    pub fn new() -> HeaderMap {
        HeaderMap {
            map: HashMap::new(),
        }
    }
}

pub enum HttpError {
    Io(std::io::Error),
    ParseError(&'static str),
}

#[derive(Debug, Clone)]
pub enum Method {
    GET,
    POST,
    PUT,
    DELETE,
    HEAD,
    OPTIONS,
    CONNECT,
    TRACE,
}
impl Method {
    pub fn from_string(method: &str) -> Result<Method, HttpError> {
        match method{
            "GET" => Ok(Method::GET),
            "POST" => Ok(Method::POST),
            "PUT" => Ok(Method::PUT),
            "DELETE" => Ok(Method::DELETE),
            "HEAD" => Ok(Method::HEAD),
            "OPTIONS" => Ok(Method::OPTIONS),
            "CONNECT" => Ok(Method::CONNECT),
            "TRACE" => Ok(Method::TRACE),
            _ => Err(HttpError::ParseError("Invalid HTTP method")),
        }
    }
    pub fn to_string(method: &Method) -> String {
        let str = match method {
            Method::GET => "GET",
            Method::POST => "POST",
            Method::PUT => "PUT",
            Method::DELETE => "DELETE",
            Method::HEAD => "HEAD",
            Method::OPTIONS => "OPTIONS",
            Method::CONNECT => "CONNECT",
            Method::TRACE => "TRACE",
        };
        str.to_string()
    }
}

#[derive(Debug)]
pub enum Version {
    Http1_0,
    Http1_1,
    Http2_0,
    Http3_0,
}

impl Version {
    pub fn from_str(version: &str) -> Result<Version, HttpError> {
        match version {
            "HTTP/1.0" => Ok(Version::Http1_0),
            "HTTP/1.1" => Ok(Version::Http1_1),
            "HTTP/2.0" => Ok(Version::Http2_0),
            "HTTP/3.0" => Ok(Version::Http3_0),
            _ => Err(HttpError::ParseError("Invalid HTTP version")),
        }
    }
    pub fn to_str(version: Version) -> String {
        let str = match version {
            Version::Http1_0 => "HTTP/1.0",
            Version::Http1_1 => "HTTP/1.1",
            Version::Http2_0 => "HTTP/2.0",
            Version::Http3_0 => "HTTP/3.0",
        };
        str.to_string()
    }
}
type StatusCode = (u16, String);

//Inspirations: https://tokio.rs/tokio/tutorial/framing
#[derive(Debug)]
pub enum HttpFrame {
    RequestHead {
        method: Method,
        uri: String,
        version: Version,
        headers: HeaderMap,
    },
    ResponseHead {
        status: StatusCode,
        version: Version,
        headers: HeaderMap,
    },
    BodyChunk {
        chunk: Vec<u8>,
    },
}

impl HttpFrame {
    pub fn get_uri(&self) -> Result<String, Error> {
        match self {
            HttpFrame::RequestHead { uri, .. } => Ok(uri.clone()),
            _ => Err(Error::new(ErrorKind::InvalidData, "Invalid frame type")),
        }
    }
    fn line_from_stream<'a>(data: &mut impl Iterator<Item = &'a u8>) -> Result<Vec<u8>, HttpError> {
        let mut line: Vec<u8> = Vec::new();
        let mut found_carriage_return = false;

        for byte in data {
            line.push(*byte);
            match byte {
                b'\n' if found_carriage_return => break,
                b'\r' => found_carriage_return = true,
                _ => (),
            }
        }
        if found_carriage_return && line.last() == Some(&&b'\n') {
            return Ok(line);
        }
        Err(HttpError::ParseError("Error parsing line"))
    }

    fn process_request_line(mut tokens: SplitWhitespace) -> Result<(String, Version), HttpError> {

        let str: &str = match tokens.next() {
            Some(str) => str,
            None => return Err(HttpError::ParseError("Error in parsing request line - No uri found")),
        };
        let uri = str.to_string();

        let str: &str = match tokens.next() {
            Some(str) => str,
            None => return Err(HttpError::ParseError("Error in parsing request line - No version found"))
        };
        let version = Version::from_str(str)?;
        Ok((uri, version))
    }

    fn process_status_line(mut tokens: SplitWhitespace) -> Result<StatusCode, HttpError> {
        let str: &str = match tokens.next() {
            Some(str) => str,
            None => return Err(HttpError::ParseError("Error in parsing request line - status code found")),
        };
        let status = str.parse::<u16>().unwrap();
        let reason = tokens.collect::<String>();
        Ok((status, reason))
    }

    fn process_msg_headers<'a>(data: & mut impl Iterator<Item = &'a u8>) -> Result<HeaderMap, HttpError> {
        let mut headers = HeaderMap::new();
        loop {
            let line = HttpFrame::line_from_stream(data)?;
            if line == b"\r\n" {
                break;
            }
            let mut parts = std::str::from_utf8(&line).unwrap().splitn(2, ':');

            let key = parts.next().unwrap().trim().to_string();
            let values = parts
                                        .next()
                                        .unwrap()
                                        .trim()
                                        .to_string()
                                        .split(",")
                                        .map(|s| s.trim().to_string())
                                        .collect::<Vec<String>>();
            headers.map.insert(key, values);
        }
        Ok(headers)
    }

    fn message_frame_from_stream<'a>(data: &mut impl Iterator<Item = &'a u8>) -> Result<HttpFrame, HttpError> {
        let line = String::from_utf8(HttpFrame::line_from_stream(data)?).unwrap();
        let mut tokens =  line.split_whitespace();

        let str: &str = match tokens.next() {
            Some(str) => str,
            None => return Err(HttpError::ParseError("Error in parsing request line - No method found")),
        };
        match str {
            "GET" | "POST | PUT" | "DELETE" | "HEAD" | "OPTIONS" | "CONNECT" | "TRACE" => {
                let (uri, version) = HttpFrame::process_request_line(tokens)?;
                return Ok(HttpFrame::RequestHead {
                    method: Method::from_string(str)?,
                    uri: uri,
                    version: version,
                    headers: HttpFrame::process_msg_headers(data)?,
                });
            },
            "HTTP/1.0" | "HTTP/1.1" | "HTTP/2.0" | "HTTP/3.0" => {
                let version = Version::from_str(str)?;
                let status = HttpFrame::process_status_line(tokens)?;
                return Ok(HttpFrame::ResponseHead {
                    version: version,
                    status: (status.0, status.1),
                    headers: HttpFrame::process_msg_headers(data)?,
                });
            },
            _ => return Err(HttpError::ParseError("Invalid HTTP method")),
        }
    }

    fn frame_to_stream(message: HttpFrame) -> Result<Vec<u8>, HttpError> {
        let mut data = Vec::new();
        match message {
            HttpFrame::RequestHead { method, uri, version, headers } => {
                data.extend(format!("{} {} {}\r\n",
                                            Method::to_string(&method),
                                            uri,
                                            Version::to_str(version)
                                        ).as_bytes());
                for (key, values) in headers.map.iter() {
                    data.extend(format!("{}: {}\r\n", key, values.join(", ")).as_bytes());
                }
                data.extend(b"\r\n");
            },
            HttpFrame::ResponseHead { version, status, headers } => {
                data.extend(format!("{} {} {}\r\n", Version::to_str(version), status.0, status.1).as_bytes());
                for (key, values) in headers.map.iter() {
                    data.extend(format!("{}: {}\r\n", key, values.join(", ")).as_bytes());
                }
                data.extend(b"\r\n");
            },
            HttpFrame::BodyChunk { chunk } => {
                data.extend(chunk);
            },
        }
        Ok(data)
    }

    pub fn body_frame_from_stream<'a>(length: u32, mut data: impl Iterator<Item = &'a u8>) -> Result<HttpFrame, HttpError> {
        let mut body = HttpFrame::BodyChunk { chunk: Vec::new() };

        let chunk = match body {
            HttpFrame::BodyChunk{ref mut chunk} => chunk,
            _ => unreachable!(),
        };

        for _ in 1..length {
            let r = data.next();
            match r {
                Some(byte) => {
                    chunk.push(*byte);
                },
                None => return Err(HttpError::ParseError("Error reading body")),
            }
        }
        Ok(body)
    }

    pub fn from_stream<'a>(data: & mut impl Iterator<Item = &'a u8>) -> Result<Vec<HttpFrame>, HttpError> {
        let mut frames: Vec<HttpFrame> = Vec::new();
        let frame = HttpFrame::message_frame_from_stream(data)?;

        let content_length:u32 = match frame {
            HttpFrame::RequestHead { ref headers, .. } => {
                headers.map.get("Content-Length").unwrap_or(&vec![0.to_string()])[0].parse::<u32>().unwrap()
            },
            HttpFrame::ResponseHead { ref headers, .. } => {
                headers.map.get("Content-Length").unwrap_or(&vec![0.to_string()])[0].parse::<u32>().unwrap()
            },
            _ => 0,
        };

        frames.push(frame);

        if content_length > 0 {
            let body = HttpFrame::body_frame_from_stream(content_length, data)?;
            frames.push(body);
        }
        Ok(frames)
    }

    pub fn to_stream(mut frames: Vec<HttpFrame>) -> Result<Vec<u8>, HttpError> {
       let mut data: Vec<u8>;

        if frames.len() > 1 {
            let chunk = match frames.pop().unwrap() {
                HttpFrame::BodyChunk { chunk } => chunk,
                _ => unreachable!(),
            };
            //Update the content length in the headers
            let mut message = frames.pop().unwrap();

            let headers = match message {
                HttpFrame::RequestHead { ref mut headers, .. } =>  headers,
                HttpFrame::ResponseHead { ref mut headers, .. } => headers,
                _ => unreachable!(),
            };
            headers.map.insert("Content-Length".to_string(), vec![chunk.len().to_string()]);
            data = HttpFrame::frame_to_stream(frames.pop().unwrap())?;
            data.extend(chunk);
        } else {
            data = HttpFrame::frame_to_stream(frames.pop().unwrap())?;
        }
        Ok(data)
    }

}

struct Route {
    method: Method,
    uri: String,
    handler: Box<dyn Fn(Vec<HttpFrame>) -> Result<Vec<HttpFrame>, HttpError>>,
}

struct RouteConfig {
    config: Vec<Route>,
}

struct ServerConfig {
    listen_address: String,
    listen_port: i32,
}

pub struct HttpServer {
    config: ServerConfig,
    routes: RouteConfig
}

impl HttpServer {
    pub fn new(listen_address: &str, listen_port: i32) -> HttpServer {
        HttpServer {
            config : ServerConfig {
                listen_address: listen_address.to_string(),
                listen_port: listen_port,
            },
            routes: RouteConfig {
                config: Vec::new(),
            },
        }
    }
    pub fn add_route<F>(&mut self, method: Method, uri: String, handler: F)  -> &mut Self
        where F: Fn(Vec<HttpFrame>) -> Result<Vec<HttpFrame>, HttpError> + 'static
    {

        match method {
            Method::GET => {
                self.routes.config.push(Route{method: Method::GET, uri: uri, handler: Box::new(handler)});
            },
            Method::POST => {
                self.routes.config.push(Route{method: Method::POST, uri: uri, handler: Box::new(handler)});
            },
            _ => {
                unimplemented!();
            }
        }
        self
    }

    pub fn listen(&mut self) -> Result<(), HttpError> {

        let listen_addr = format!("{}:{}", self.config.listen_address, self.config.listen_port);

        let listener = match std::net::TcpListener::bind(listen_addr) {
            Ok(listener) => listener,
            Err(e) => {
                return Err(HttpError::Io(e));
            }
        };

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {

                    std::thread::spawn(move || {
                        HttpServer::handle_client(stream);
                    });
                },
                Err(e) => {
                    println!("error: {}", e);
                }
            }
        }
        Ok(())
    }
    fn handle_client(mut stream: std::net::TcpStream) -> () {
        let mut frame: Vec<u8> = Vec::new();

        stream.set_read_timeout(Some(std::time::Duration::from_millis(10))).unwrap();
        loop {
            let mut buf: Vec<u8> = Vec::with_capacity(1024);
            let count = stream.read(&mut buf).unwrap();
            if count == 0 {
                break;
            }
            frame.extend(buf);
        }
        let frames = match HttpFrame::from_stream(&mut frame.iter()) {
            Ok(frames) => frames,
            Err(_e) => {
                println!("Error parsing frame");
                stream.write(b"HTTP/1.1 400 Bad Request\r\n\r\n").unwrap();
                stream.shutdown(std::net::Shutdown::Both).unwrap();
                return;
            }
        };
        let (uri, method) = match frames[0] {
            HttpFrame::RequestHead { ref uri, ref method, .. } => (uri, method),
            _ => {
                stream.write(b"HTTP/1.1 400 Bad Request\r\n\r\n").unwrap();
                stream.shutdown(std::net::Shutdown::Both).unwrap();
                return;
            },
        };
        println!("URI: {}, method: {}", uri, Method::to_string(method));
        stream.write(b"HTTP/1.1 400 Bad Request\r\n\r\n").unwrap();
        stream.shutdown(std::net::Shutdown::Both).unwrap();
        return;
    }
}




