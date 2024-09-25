
use std::{collections::HashMap, io::{Write, Read, Error, ErrorKind}};
use flate2::write::GzEncoder;
use flate2::Compression;
use nom::AsBytes;

#[derive(Debug)]
struct Request {
    pub version: String,
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

enum ParseState {
    StartLine,
    Headers,
}

impl Request {
    fn new() -> Request {
        Request {
            version: String::new(),
            method: String::new(),
            url: String::new(),
            headers: HashMap::new(),
            body: String::new(),
        }
    }

    fn readline(mut reader: impl FnMut(&mut [u8]) -> std::io::Result<usize>) -> std::io::Result<String>{
        let mut buf = Vec::new();
        let mut possible_end = false;
        loop {
            let mut byte = [0];
            reader(&mut byte)?;
            buf.push(byte[0]);
            match byte[0] {
                b'\n' if possible_end => break,
                b'\r' => possible_end = true,
                _ => possible_end = false,
            }
        }
        let s = String::from_utf8(buf).unwrap();
        Ok(s)
    }

    fn parse_request_line(&mut self, line: &str) -> std::io::Result<()> {
        let mut tokens =  line.split_whitespace();

        let str: &str = match tokens.next() {
            Some(str) => str,
            None => return Err(Error::new(ErrorKind::InvalidInput, "Invalid input")),
        };
        self.method = str.to_string();

        let str: &str = match tokens.next() {
            Some(str) => str,
            None => return Err(Error::new(ErrorKind::InvalidInput, "Invalid input")),
        };
        self.url = str.to_string();

        let str: &str = match tokens.next() {
            Some(str) => str,
            None => return Err(Error::new(ErrorKind::InvalidInput, "Invalid input")),
        };
        self.version = str.to_string();
        Ok(())
    }

    fn parse_header(&mut self, line: &str) -> std::io::Result<()> {
        let mut parts = line.splitn(2, ':');
        let key = parts.next().unwrap().trim().to_string();
        let mut value = parts.next().unwrap().trim().to_string();

        if self.headers.contains_key(&key) {
            value = format!("{},{}", self.headers.get(&key).unwrap(), value).to_string();
        }
        self.headers.insert(key, value);
        Ok(())
    }

    fn from_stream(mut reader: impl FnMut(&mut [u8]) -> std::io::Result<usize>) -> std::io::Result<Self> {
        let mut state: ParseState = ParseState::StartLine;
        let mut req = Request::new();

        while let Ok(line) = Request::readline(&mut reader) {
            match state {
                ParseState::StartLine => {
                    let _ = req.parse_request_line(&line)
                    .or_else(|e| return Err(e));
                    state = ParseState::Headers;
                },
                ParseState::Headers => {
                    if line == "\r\n" {
                        break;
                    } else {
                        let _ = req.parse_header(&line)
                        .or_else(|e| return Err(e));
                    }
                },
            }
        }
        let content_length = req.headers
                                        .get("Content-Length")
                                        .unwrap_or(&"0".to_string())
                                        .parse::<usize>()
                                        .unwrap();

        for _ in 0..content_length {
            let mut byte = [0];
            reader(&mut byte)?;
            req.body.push(byte[0] as char);
        }
        Ok(req)
    }
}

struct Response {
    version: String,
    status: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
    encoding: String,
}

impl Response {
    pub fn new() -> Response {
        Response {
            version: "HTTP/1.1".to_string(),
            status: String::new(),
            headers: HashMap::new(),
            body: Vec::new(),
            encoding: String::new(),
        }
    }
    fn set_status(&mut self, status: &str) {
        self.status.push_str(status);
    }
    fn set_header(&mut self, name: &str, value: &str) {
        self.headers.insert(name.to_string(), value.to_string());
    }
    fn set_body(&mut self, body: &str) {
        self.body.extend(body.as_bytes());
    }
    fn set_encoding(&mut self, encoding: &str) {
        self.encoding.push_str(encoding);
    }

    fn handle_encoding(&mut self, msg: &mut Vec<u8>) -> () {
        if self.encoding.len() > 0 && self.body.len() > 0 {
            msg.extend(format!("Content-Encoding: {}\r\n", self.encoding).as_bytes());
            match self.encoding.as_str() {
                "gzip" => {
                    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                    std::io::copy(&mut self.body.as_slice(), &mut encoder).unwrap();
                    let output = encoder.finish().unwrap();
                    msg.extend(format!("Content-Length: {}\r\n\r\n", output.len()).as_bytes());
                    msg.extend(output.iter());
                },
                _ => {
                    msg.extend(format!("Content-Length: {}\r\n\r\n", self.body.len()).as_bytes());
                    msg.extend(self.body.as_bytes());
                }
            }
        } else {
            msg.extend(format!("Content-Length: {}\r\n\r\n", self.body.len()).as_bytes());
            msg.extend(self.body.as_bytes());
        }
    }
    fn to_string(&mut self) -> std::io::Result<Vec<u8>> {
        let mut msg: Vec<u8> = Vec::new();
        msg.extend(format!("{} {}\r\n", self.version, self.status).as_bytes());

        for (key,value) in self.headers.iter() {
            msg.extend(format!("{}: {}\r\n",key, value).as_bytes());
        }
        self.handle_encoding(&mut msg);
        Ok(msg)
    }
}


struct Transaction {
    request: Request,
    response: Response,
}

impl Transaction {
    pub fn new(request: Request, response: Response) -> Transaction {
        Transaction {
            request: request,
            response: response,
        }
    }
}

pub struct SessionConfig {
    pub download_dir: String,
    pub upload_dir: String,
    pub supported_encoding: Vec<String>,
}

pub struct Session {
    stream: std::net::TcpStream,
    config: SessionConfig,
}

impl Session {
    pub fn new(config: SessionConfig, stream: std::net::TcpStream) -> Session {
        Session {
            stream: stream,
            config: config,
        }
    }

    pub fn close(&mut self) -> std::io::Result<()> {
        self.stream.shutdown(std::net::Shutdown::Both)?;
        Ok(())
    }

    fn send(&mut self, response: Vec<u8>) -> std::io::Result<()> {
        self.stream.write(&response)?;
        self.stream.flush()?;
        Ok(())
    }

    fn receive(&mut self) -> std::io::Result<Request> {
        Request::from_stream(|buf| self.stream.read(buf))
    }

    pub fn handle_client(&mut self) -> std::io::Result<()> {
        let request = self.receive()?;
        let mut transaction = Transaction::new(request, Response::new());

        match transaction.request.method.as_str() {
            "GET" => {
                self.process_get_request(&mut transaction)?;
            },
            "POST" => {
                self.process_post_request(&mut transaction)?;
            },
            _ => {
                transaction.response.set_status(&"405 Method Not Allowed");
                transaction.response.set_header("Allow", "GET, POST");
                self.send(transaction.response.to_string().unwrap())?;
            }
        }
        Ok(())
    }

    fn process_content_encoding(&mut self, transaction: &mut Transaction) -> std::io::Result<()> {
        if transaction.request.headers.contains_key("Accept-Encoding") {
            let encoder_options = transaction.request.headers.get("Accept-Encoding").unwrap().split(",").collect::<Vec<&str>>();
            for requested_option in encoder_options {
                for supported_option in self.config.supported_encoding.iter() {
                    if requested_option.trim() == supported_option.trim() {
                        println!("Setting encoding to {:?}", requested_option);
                        transaction.response.set_encoding(&requested_option.trim());
                        break;
                    }
                }
            }
        }
        Ok(())
    }
    fn process_echo_request(&mut self, transaction: &mut Transaction) -> std::io::Result<()> {
        println!("{:?}", transaction.request.url);
        self.process_content_encoding(transaction)?;
        transaction.response.set_status(&"200 OK");
        transaction.response.set_body(transaction.request.url.split("/").collect::<Vec<&str>>()[2]);
        transaction.response.set_header("Content-Type", "text/plain");
        self.send(transaction.response.to_string().unwrap())?;
        Ok(())
    }

    fn process_user_agent_request(&mut self, transaction: &mut Transaction) -> std::io::Result<()> {
        self.process_content_encoding(transaction)?;
        transaction.response.set_status(&"200 OK");
        transaction.response.set_body(&transaction.request.headers.get("User-Agent").unwrap_or(&"".to_string()));
        transaction.response.set_header("Content-Type", "text/plain");
        self.send(transaction.response.to_string().unwrap())?;
        Ok(())
    }

    fn process_file_download_request(&mut self, transaction: &mut Transaction) -> std::io::Result<()> {
        self.process_content_encoding(transaction)?;
        let filename = transaction.request.url.split("/").collect::<Vec<&str>>()[2];
        let dirname = self.config.download_dir.as_str();
        let result = std::fs::read_to_string(format!("{}/{}", dirname, filename));
        match result {
            Ok(content) => {
                transaction.response.set_status(&"200 OK");
                transaction.response.set_body(&content);
                transaction.response.set_header("Content-Type", "application/octet-stream");
                self.send(transaction.response.to_string().unwrap())?;
            },
            Err(_) => {
                transaction.response.set_status(&"404 Not Found");
                self.send(transaction.response.to_string().unwrap())?;
            }
        }
        Ok(())
    }

    fn process_get_request(&mut self, transaction: &mut Transaction) -> std::io::Result<()> {

        if transaction.request.url.starts_with("/echo/") {
            self.process_echo_request(transaction)?;
        } else if transaction.request.url.starts_with("/user-agent") {
            self.process_user_agent_request(transaction)?;
        } else if transaction.request.url.starts_with("/files"){
            self.process_file_download_request(transaction)?;
        } else if transaction.request.url == "/" {
            transaction.response.set_status(&"200 OK");
            self.send(transaction.response.to_string().unwrap())?;
        } else {
            transaction.response.set_status(&"404 Not Found");
            self.send(transaction.response.to_string().unwrap())?;
        }
        Ok(())
    }

    fn process_post_request(&mut self, transaction: &mut Transaction) -> std::io::Result<()> {
        if transaction.request.url.starts_with("/files/") {
            let filename = transaction.request.url.split("/").collect::<Vec<&str>>()[2];
            let dirname = self.config.upload_dir.as_str();
            let mut file = std::fs::File::create(format!("{}/{}", dirname, filename)).unwrap();
            transaction.response.set_status(&"201 Created");
            file.write_all(transaction.request.body.as_bytes())?;
            self.send(transaction.response.to_string().unwrap())?;
        } else {
            transaction.response.set_status(&"404 Not Found");
            self.send(transaction.response.to_string().unwrap())?;
        }
        Ok(())
    }
}