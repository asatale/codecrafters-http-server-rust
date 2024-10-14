
use std::{collections::HashMap, io::{Read, Write}, str::SplitWhitespace, sync::Arc};
use std::result::Result::Ok;
use std::fmt;
use flate2::write::GzEncoder;
use flate2::Compression;
use flate2::write::DeflateEncoder;

struct DataStream {
    active: bool,
    stream: std::net::TcpStream,
    data: [u8; 1024],
    rptr: usize,
    wptr: usize,
}

impl DataStream  {
    pub fn new(stream: std::net::TcpStream) -> DataStream {
        DataStream {
            active: true,
            stream: stream,
            data: [0; 1024],
            rptr: 0,
            wptr: 0,
        }
    }
    pub fn close(&mut self) -> () {
        self.active = false;
        self.stream.shutdown(std::net::Shutdown::Both).unwrap();
    }

    fn consume_byte(&mut self) -> u8 {
        let byte = self.data[self.rptr];
        self.rptr += 1;
        byte
    }
    pub fn write(&mut self, data: &[u8]) -> Result<usize, HttpError> {
        match self.stream.write(data) {
            Ok(count) => Ok(count),
            Err(error) => {
                println!("Error writing to stream: {}", error);
                Err(HttpError::new(HttpErrorKind::IOError, "I/O Error", None))
            },
        }
    }
    fn next(&mut self) -> Option<u8> {
        if self.active == false {
            println!("Stream is closed");
            return None;
        }
        if self.rptr < self.wptr {
            return Some(self.consume_byte());
        } else {
            self.rptr = 0;
            self.wptr = 0;
        }

        match self.stream.read(&mut self.data) {
            Ok(count) => {
                self.wptr = count;
                if count == 0 {
                    return None;
                }
                return Some(self.consume_byte());
            }
            Err(_) => {
                println!("Error reading from socket");
                return None;
            }
        }
    }
}

impl Iterator for DataStream {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        self.next()
    }
}


#[derive(Debug, Clone)]
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


#[derive(Debug, Clone)]
pub enum HttpErrorKind {
    RequestError,
    ResponseError,
    ParseError,
    IOError,
}

#[derive(Debug, Clone)]
pub struct HttpError {
    kind: HttpErrorKind,
    err_msg: String,
    err_code: u32,
}

impl HttpError {
    pub fn new(kind: HttpErrorKind, msg: &str, code: Option<u32>) -> HttpError {
        HttpError {
            kind: kind,
            err_msg: msg.to_string(),
            err_code: code.unwrap_or(0),
        }
    }
}
impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "HttpError: {}", self.err_msg)
    }
}



#[derive(PartialEq, Eq, Debug, Clone)]
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
            _ => Err(HttpError::new(HttpErrorKind::RequestError,"Bad Request", Some(400))),
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

#[derive(Debug, Clone)]
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
            _ => Err(HttpError::new(HttpErrorKind::ParseError, "Error parsing version", None)),
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
#[derive(Debug, Clone)]
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
    pub fn get_uri(&self) -> String {
        match self {
            HttpFrame::RequestHead { uri, .. } => uri.clone(),
            _ => {panic!("No uri found for frame")},
        }
    }
    pub fn get_method(&self) -> Method {
        match self {
            HttpFrame::RequestHead { method, .. } => method.clone(),
            _ => panic!("No method found for frame"),
        }
    }
    fn line_from_stream(data: &mut impl Iterator<Item = u8>) -> Result<Vec<u8>, HttpError> {
        let mut line: Vec<u8> = Vec::new();
        let mut found_carriage_return = false;

        for byte in data {
            line.push(byte);
            match byte {
                b'\n' if found_carriage_return => break,
                b'\r' => found_carriage_return = true,
                _ => (),
            }
        }
        if found_carriage_return && line.last() == Some(&&b'\n') {
            return Ok(line);
        }
        println!("Error in parsing request line - No CRLF found");
        Err(HttpError::new(HttpErrorKind::ParseError, "Error parsing message", None))
    }

    fn process_request_line(mut tokens: SplitWhitespace) -> Result<(String, Version), HttpError> {
        let str: &str = match tokens.next() {
            Some(str) => str,
            None => {
                println!("Error in parsing request line - No URI found");
                return Err(HttpError::new(HttpErrorKind::RequestError, "Bad Request", Some(400)));
            }
        };
        let uri = str.to_string();

        let str: &str = match tokens.next() {
            Some(str) => str,
            None => {
                println!("Error in parsing request line - No version found");
                return Err(HttpError::new(HttpErrorKind::RequestError, "Bad Request", Some(400)));
            }
        };
        let version = Version::from_str(str)?;
        Ok((uri, version))
    }

    fn process_status_line(mut tokens: SplitWhitespace) -> Result<StatusCode, HttpError> {
        let str: &str = match tokens.next() {
            Some(str) => str,
            None => {
                println!("Error in parsing response line - No status found");
                return Err(HttpError::new(HttpErrorKind::ResponseError, "Parse Error", None));
            }
        };
        let status = str.parse::<u16>().unwrap();
        let reason = tokens.collect::<String>();
        Ok((status, reason))
    }

    fn process_msg_headers(data: & mut impl Iterator<Item = u8>) -> Result<HeaderMap, HttpError> {
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

    fn message_frame_from_stream(data: &mut impl Iterator<Item = u8>) -> Result<HttpFrame, HttpError> {
        let line = String::from_utf8(HttpFrame::line_from_stream(data)?).unwrap();
        let mut tokens =  line.split_whitespace();

        let str: &str = match tokens.next() {
            Some(str) => str,
            None => {
                println!("Error in parsing request line - No method found");
                return Err(HttpError::new(HttpErrorKind::ParseError, "Bad Request", None));
            },
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
            _ => return Err(HttpError::new(HttpErrorKind::ParseError,"Bad Request", None)),
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

    pub fn body_frame_from_stream(length: u32, mut data: impl Iterator<Item = u8>) -> Result<HttpFrame, HttpError> {
        let mut body = HttpFrame::BodyChunk { chunk: Vec::new() };

        let chunk = match body {
            HttpFrame::BodyChunk{ref mut chunk} => chunk,
            _ => unreachable!(),
        };

        for _ in 1..length {
            let r = data.next();
            match r {
                Some(byte) => {
                    chunk.push(byte);
                },
                None => return Err(HttpError::new(HttpErrorKind::RequestError, "Bad Request", Some(400))),
            }
        }
        Ok(body)
    }

    pub fn from_stream(data: &mut impl Iterator<Item = u8>) -> Result<Vec<HttpFrame>, HttpError> {
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
            let mut chunk = match frames.pop().unwrap() {
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
            if headers.map.contains_key("Content-Encoding") {
                let encoding = headers.map.get("Content-Encoding").unwrap().get(0).unwrap();
                if encoding == "gzip" {
                    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                    encoder.write_all(&chunk).unwrap();
                    chunk = encoder.finish().unwrap();
                } else if encoding == "deflate" {
                    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
                    encoder.write_all(&chunk).unwrap();
                    chunk = encoder.finish().unwrap();
                }
            }
            headers.map.insert("Content-Length".to_string(), vec![chunk.len().to_string()]);
            data = HttpFrame::frame_to_stream(message)?;
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
    handler: Arc<Box<dyn Fn(Vec<HttpFrame>) -> Result<Vec<HttpFrame>, HttpError> + 'static + Send + Sync>>,
}

struct RouteConfig {
    config: Vec<Route>,
}

impl Clone for RouteConfig {
    fn clone(&self) -> RouteConfig {
        let mut config: Vec<Route> = Vec::new();
        for route in self.config.iter() {
            config.push(Route {
                method: route.method.clone(),
                uri: route.uri.clone(),
                handler: route.handler.clone(),
            });
        }
        RouteConfig {
            config: config,
        }
    }
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
    pub fn add_route<F>(&mut self, method: Method, uri: String, handler: F)  -> ()
        where F: Fn(Vec<HttpFrame>) -> Result<Vec<HttpFrame>, HttpError> + 'static + Send + Sync
    {
        match method {
            Method::GET => {
                self.routes.config.push(Route{method: Method::GET, uri: uri, handler: Arc::new(Box::new(handler))});
            },
            Method::POST => {
                self.routes.config.push(Route{method: Method::POST, uri: uri, handler: Arc::new(Box::new(handler))});
            },
            _ => {
                unimplemented!();
            }
        }
        // Sort the resultant vector by uri length
        self.routes.config.sort_by(|a, b| b.uri.len().cmp(&a.uri.len()));
    }

    pub fn listen(&mut self) -> Result<(), HttpError> {

        let listen_addr = format!("{}:{}", self.config.listen_address, self.config.listen_port);

        let listener = match std::net::TcpListener::bind(listen_addr) {
            Ok(listener) => listener,
            Err(e) => {
                println!("Error binding to address: {}", e);
                return Err(HttpError::new(HttpErrorKind::IOError, "I/O Error", None));
            }
        };

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let config = self.routes.clone();
                    std::thread::spawn( move || {
                        HttpServer::handle_client(stream, config);
                    });
                },
                Err(e) => {
                    println!("error: {}", e);
                }
            }
        }
        Ok(())
    }

    fn handle_client(stream: std::net::TcpStream, route_cfg: RouteConfig) -> () {
        let mut data_stream = DataStream::new(stream);

        let frame_buf = match HttpFrame::from_stream(&mut data_stream) {
            Ok(frame_buf) => frame_buf,
            Err(e) => {
                match e.kind {
                    HttpErrorKind::RequestError => {
                        data_stream.write(format!("HTTP/1.1 {} {}\r\n\r\n", e.err_code, e.err_msg).as_bytes()).unwrap();
                    },
                    _ => {
                        println!("Error reading from stream: {}", e.err_msg);
                    }
                }
                data_stream.close();
                return;
            }
        };
        println!("Received frames: {:?}", frame_buf);
        match HttpServer::handle_transaction(&mut data_stream, route_cfg, frame_buf){
            Ok(_) => (),
            Err(e) => {
                match e.kind {
                    HttpErrorKind::RequestError => {
                        data_stream.write(format!("HTTP/1.1 {} {}\r\n\r\n", e.err_code, e.err_msg).as_bytes()).unwrap();
                    },
                    _ => {
                        println!("Internal Server Error: {}", e.err_msg);
                        data_stream.write(b"HTTP/1.1 500 Internal Server Error \r\n\r\n").unwrap();
                    }
                }
                data_stream.close();
                ()
            }
        };
    }

    fn process_compression_headers(request: &HttpFrame) -> Result<String, HttpError> {
        let request_hdrs = match request {
            HttpFrame::RequestHead { headers, .. } => headers,
            _ => unreachable!(),
        };
        if request_hdrs.map.contains_key("Accept-Encoding") {
            println!("Compression headers found");
            let requested_algos = request_hdrs.map.get("Accept-Encoding").unwrap();
            for encoding in requested_algos.iter() {
                println!("Encoding: {}", encoding);
                if encoding == "gzip" || encoding == "deflate" {
                    println!("Found matching encryption {}", encoding);
                    return Ok(encoding.clone())
                }
            }
        }
        Err(HttpError::new(HttpErrorKind::RequestError, "No matching compression algorithm", None))
    }

    fn handle_transaction(data_stream: &mut DataStream, route_cfg: RouteConfig, frames: Vec<HttpFrame>) -> Result<(), HttpError> {

        let request = frames[0].clone();
        let (msg_method, msg_uri) = (request.get_method(), request.get_uri());

        for route in route_cfg.config.iter(){
            if route.method == msg_method && msg_uri.starts_with(route.uri.as_str()) {
                let handler = route.handler.clone();
                match handler(frames) {
                    Ok(mut response) => {
                       match HttpServer::process_compression_headers(&request) {
                            Ok(encoding) => {
                                let header = match response[0] {
                                    HttpFrame::ResponseHead { ref mut headers, .. } => headers,
                                    _ => unreachable!(),
                                };
                                header.map.insert("Content-Encoding".to_string(), vec![encoding]);
                            },
                            _ => (),
                        };
                        let data = HttpFrame::to_stream(response).unwrap();
                        data_stream.write(&data).unwrap();
                    },
                    Err(e) => {
                        println!("Error processing request: {:?}", e);
                        data_stream.write(b"HTTP/1.1 500 Internal Server Error\r\n\r\n").unwrap();
                    }
                }
                return Ok(());
            }
        }
        data_stream.write(b"HTTP/1.1 400 Bad Request\r\n\r\n").unwrap();
        data_stream.close();
        return Ok(());
    }
}




