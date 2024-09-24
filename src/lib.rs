
use std::{collections::HashMap, io::{Write, Read, Error, ErrorKind}};
use flate2::write::GzEncoder;
use flate2::Compression;

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
    body: String,
    encoding: String,
}

impl Response {
    pub fn new() -> Response {
        Response {
            version: "HTTP/1.1".to_string(),
            status: String::new(),
            headers: HashMap::new(),
            body: String::new(),
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
        self.body.push_str(body);
    }
    fn set_encoding(&mut self, encoding: &str) {
        self.encoding.push_str(encoding);
    }

    fn handle_encoding(&mut self, msg: &mut String) -> () {
        if self.encoding.len() > 0 && self.body.len() > 0 {
            msg.push_str("Content-Encoding: ");
            msg.push_str(&self.encoding);
            msg.push_str("\r\n");

            match self.encoding.as_str() {
                "gzip" => {
                    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                    std::io::copy(&mut self.body.as_bytes(), &mut encoder).unwrap();
                    let output = encoder.finish().unwrap();

                    msg.push_str("Content-Length: ");
                    msg.push_str(&output.len().to_string());
                    msg.push_str("\r\n\r\n");
                    msg.push_str(&output.iter().map(|&c| c as char).collect::<String>());
                },
                _ => {
                    msg.push_str("Content-Length: ");
                    msg.push_str(&self.body.len().to_string());
                    msg.push_str("\r\n\r\n");
                    if self.body.len() > 0 {
                        msg.push_str(&self.body);
                    }
                }
            }
        } else {
            msg.push_str("Content-Length: ");
            msg.push_str(&self.body.len().to_string());
            msg.push_str("\r\n\r\n");
            if self.body.len() > 0 {
                msg.push_str(&self.body);
            }
        }
    }
    fn to_string(&mut self) -> std::io::Result<String> {
        let mut msg = String::new();
        msg.push_str(&self.version);
        msg.push_str(" ");
        msg.push_str(&self.status);
        msg.push_str("\r\n");

        for (key,value) in self.headers.iter() {
            msg.push_str(&key);
            msg.push_str(": ");
            msg.push_str(&value);
            msg.push_str("\r\n");
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

    fn send(&mut self, response: &str) -> std::io::Result<()> {
        self.stream.write(response.as_bytes())?;
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
                self.send(&transaction.response.to_string().unwrap())?;
            }
        }
        Ok(())
    }

    fn process_content_encoding(&mut self, transaction: &mut Transaction) -> std::io::Result<()> {
        if transaction.request.headers.contains_key("Accept-Encoding") {
            println!("{:?}", transaction.request.headers.get("Accept-Encoding").unwrap());
            let encoder_options = transaction.request.headers.get("Accept-Encoding").unwrap().split(",").collect::<Vec<&str>>();
            for requested_option in encoder_options {
                for supported_option in self.config.supported_encoding.iter() {
                    if requested_option == supported_option {
                        println!("Selection encoding{:?}", requested_option);
                        transaction.response.set_encoding(&requested_option);
                    }
                }
            }
        }
        Ok(())
    }
    fn process_echo_request(&mut self, transaction: &mut Transaction) -> std::io::Result<()> {
        self.process_content_encoding(transaction)?;
        transaction.response.set_status(&"200 OK");
        transaction.response.set_body(transaction.request.url.split("/").collect::<Vec<&str>>()[2]);
        transaction.response.set_header("Content-Type", "text/plain");
        self.send(&transaction.response.to_string().unwrap())?;
        Ok(())
    }

    fn process_user_agent_request(&mut self, transaction: &mut Transaction) -> std::io::Result<()> {
        self.process_content_encoding(transaction)?;
        transaction.response.set_status(&"200 OK");
        transaction.response.set_body(&transaction.request.headers.get("User-Agent").unwrap_or(&"".to_string()));
        transaction.response.set_header("Content-Type", "text/plain");
        self.send(&transaction.response.to_string().unwrap())?;
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
                self.send(&transaction.response.to_string().unwrap())?;
            },
            Err(_) => {
                transaction.response.set_status(&"404 Not Found");
                self.send(&transaction.response.to_string().unwrap())?;
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
            self.send(&transaction.response.to_string().unwrap())?;
        } else {
            transaction.response.set_status(&"404 Not Found");
            self.send(&transaction.response.to_string().unwrap())?;
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
            self.send(&transaction.response.to_string().unwrap())?;
        } else {
            transaction.response.set_status(&"404 Not Found");
            self.send(&transaction.response.to_string().unwrap())?;
        }
        Ok(())
    }
}