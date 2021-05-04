use std::collections::HashMap;
use std::env;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use thiserror::Error;

#[derive(Debug)]
struct HttpClient {
    stream: TcpStream,
}

impl HttpClient {
    fn new<T: ToSocketAddrs>(addr: T) -> Result<HttpClient, HttpError> {
        let stream = TcpStream::connect(addr).map_err(HttpError::from)?;
        let ret = HttpClient { stream: stream };
        Ok(ret)
    }

    fn send<T: Write>(&self, req: &HttpRequest, mut writer: BufWriter<T>) -> std::io::Result<()> {
        write!(
            writer,
            "{} {} {}\r\n",
            req.method.to_string(),
            req.path,
            req.version.string()
        )?;

        req.headers.write_to(&mut writer)?;
        if !req.headers.contains_key(&"content-length".to_string()) {
            write!(writer, "content-length: {}\r\n", req.body.len())?;
        }

        write!(writer, "\r\n")?;
        writer.write_all(&req.body)?;
        writer.flush()?;
        Ok(())
    }

    fn recv<T: Read>(&self, mut reader: BufReader<T>) -> Result<HttpResponse, HttpError> {
        let mut line = String::new();
        reader.read_line(&mut line).map_err(HttpError::from)?;
        let mut iter = line.splitn(3, " ");
        let version = HttpVersion::from(iter.next().ok_or_else(|| HttpError::HttpSyntaxError)?);
        if let HttpVersion::UNSUPPORTED = version {
            return Err(HttpError::HttpSyntaxError);
        }

        let status = HttpStatus::from(iter.next().ok_or_else(|| HttpError::HttpSyntaxError)?);
        if let HttpStatus::Invalid = status {
            return Err(HttpError::HttpSyntaxError);
        }

        let mut headers = HttpHeaders::new();
        headers.read_from(&mut reader)?;

        let body = match headers.content_length() {
            Some(0) => Vec::new(),
            Some(v) => {
                let mut body = Vec::with_capacity(v);
                reader.read_to_end(&mut body)?;
                body
            }
            None => {
                let mut body = Vec::new();
                reader.read_to_end(&mut body)?;
                body
            }
        };

        Ok(HttpResponse {
            version: version,
            status: status,
            headers: headers,
            body: body,
        })
    }

    fn request(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        self.send(request, BufWriter::new(&self.stream))
            .map_err(HttpError::from)?;
        self.recv(BufReader::new(&self.stream))
    }
}

#[derive(Debug, PartialEq, Eq)]
enum HttpMethod {
    GET,
    POST,
}

impl HttpMethod {
    fn to_string(&self) -> String {
        match self {
            HttpMethod::GET => "GET".to_string(),
            HttpMethod::POST => "POST".to_string(),
        }
    }
}

#[derive(Debug)]
enum HttpVersion {
    HTTP1_0,
    HTTP1_1,
    UNSUPPORTED,
}

impl HttpVersion {
    fn string(&self) -> &str {
        match self {
            Self::HTTP1_0 => "HTTP/1.0",
            Self::HTTP1_1 => "HTTP/1.1",
            _ => "UNSUPPORTED",
        }
    }
}

impl From<&str> for HttpVersion {
    fn from(v: &str) -> Self {
        match v {
            "HTTP/1.0" => Self::HTTP1_0,
            "HTTP/1.1" => Self::HTTP1_1,
            _ => Self::UNSUPPORTED,
        }
    }
}

#[derive(Debug)]
enum HttpStatus {
    Ok,
    NotFound,
    Invalid,
}

impl HttpStatus {
    fn code(&self) -> u32 {
        match self {
            Self::Ok => 200,
            Self::NotFound => 404,
            _ => 0,
        }
    }

    fn string(&self) -> &str {
        match self {
            Self::Ok => "OK",
            Self::NotFound => "Not Found",
            _ => "",
        }
    }
}

impl From<u32> for HttpStatus {
    fn from(v: u32) -> Self {
        match v {
            200 => Self::Ok,
            404 => Self::NotFound,
            _ => Self::Invalid,
        }
    }
}

impl From<&str> for HttpStatus {
    fn from(v: &str) -> Self {
        match v {
            "200" => Self::Ok,
            "404" => Self::NotFound,
            _ => Self::Invalid,
        }
    }
}

#[derive(Debug)]
struct HttpRequest {
    method: HttpMethod,
    path: String,
    version: HttpVersion,
    headers: HttpHeaders,
    body: Vec<u8>,
}

#[derive(Debug)]
struct HttpResponse {
    version: HttpVersion,
    status: HttpStatus,
    headers: HttpHeaders,
    body: Vec<u8>,
}

#[derive(Error, Debug)]
enum HttpError {
    #[error("ioerror : {source:?}")]
    IOError {
        #[from]
        source: std::io::Error,
    },
    #[error("syntax error")]
    HttpSyntaxError,
}

struct HttpServer {
    listener: TcpListener,
    handler: Box<dyn Handler>,
}

impl HttpServer {
    fn new<T: ToSocketAddrs>(addr: T, handler: Box<dyn Handler>) -> Result<HttpServer, HttpError> {
        let listener = TcpListener::bind(addr).map_err(HttpError::from)?;
        Ok(HttpServer {
            listener: listener,
            handler: handler,
        })
    }

    fn listen(&self) -> Result<(), HttpError> {
        for stream in self.listener.incoming() {
            self.handle(stream.map_err(HttpError::from)?)?;
        }
        Ok(())
    }

    fn handle(&self, stream: TcpStream) -> Result<(), HttpError> {
        let mut req = self.recv(BufReader::new(&stream))?;
        let resp = self.handler.handle(&mut req)?;
        self.send(&resp, BufWriter::new(&stream))
            .map_err(HttpError::from)?;
        Ok(())
    }

    fn recv<T: Read>(&self, mut reader: BufReader<T>) -> Result<HttpRequest, HttpError> {
        let mut line = String::new();
        reader.read_line(&mut line).map_err(HttpError::from)?;
        let mut iter = line.trim_end_matches("\r\n").splitn(3, " ");
        let method = match iter.next().ok_or_else(|| HttpError::HttpSyntaxError)? {
            "GET" => HttpMethod::GET,
            "POST" => HttpMethod::POST,
            _ => return Err(HttpError::HttpSyntaxError),
        };
        let path = iter
            .next()
            .ok_or_else(|| HttpError::HttpSyntaxError)?
            .to_string();

        let version = HttpVersion::from(iter.next().ok_or_else(|| HttpError::HttpSyntaxError)?);
        if let HttpVersion::UNSUPPORTED = version {
            return Err(HttpError::HttpSyntaxError);
        }
        let mut headers = HttpHeaders::new();

        headers.read_from(&mut reader)?;

        let body = match headers.content_length() {
            Some(0) => Vec::new(),
            Some(v) => {
                let mut body = Vec::with_capacity(v);
                reader.read_to_end(&mut body)?;
                body
            }
            None => Vec::new(),
        };

        Ok(HttpRequest {
            method: method,
            path: path,
            version: version,
            headers: headers,
            body: body,
        })
    }

    fn send<T: Write>(&self, resp: &HttpResponse, mut writer: BufWriter<T>) -> std::io::Result<()> {
        write!(
            writer,
            "{} {} {}\r\n",
            resp.version.string(),
            resp.status.code(),
            resp.status.string()
        )?;
        resp.headers.write_to(&mut writer)?;
        if !resp.headers.contains_key(&"content-length".to_string()) {
            write!(writer, "content-length: {}\r\n", resp.body.len())?;
        }

        write!(writer, "\r\n")?;
        writer.write_all(&resp.body)?;
        writer.flush()?;
        Ok(())
    }
}

trait Handler {
    fn handle(&self, req: &mut HttpRequest) -> Result<HttpResponse, HttpError>;
}

struct Router {
    rules: Vec<Rule>,
}

impl Handler for Router {
    fn handle(&self, req: &mut HttpRequest) -> Result<HttpResponse, HttpError> {
        for rule in &self.rules {
            if req.method == rule.method && req.path == rule.path {
                return rule.handler.handle(req);
            }
        }
        Ok(HttpResponse {
            version: match req.version {
                HttpVersion::HTTP1_0 => HttpVersion::HTTP1_0,
                HttpVersion::HTTP1_1 => HttpVersion::HTTP1_1,
                _ => HttpVersion::UNSUPPORTED,
            },
            status: HttpStatus::NotFound,
            headers: HttpHeaders::new(),
            body: Vec::new(),
        })
    }
}

impl Handler for fn(&mut HttpRequest) -> Result<HttpResponse, HttpError> {
    fn handle(&self, req: &mut HttpRequest) -> Result<HttpResponse, HttpError> {
        (self)(req)
    }
}

impl Router {
    fn add(
        &mut self,
        method: HttpMethod,
        path: String,
        handler: fn(&mut HttpRequest) -> Result<HttpResponse, HttpError>,
    ) {
        self.rules.push(Rule {
            method: method,
            path: path,
            handler: Box::new(handler),
        })
    }
}

struct Rule {
    method: HttpMethod,
    path: String,
    handler: Box<dyn Handler>,
}

#[derive(Debug)]
struct HttpHeaders {
    headers: HashMap<String, String>,
}

impl HttpHeaders {
    fn new() -> HttpHeaders {
        HttpHeaders {
            headers: HashMap::new(),
        }
    }

    fn write_to<T: Write>(&self, writer: &mut BufWriter<T>) -> std::io::Result<()> {
        for (key, value) in &self.headers {
            write!(writer, "{}: {}\r\n", key, value)?;
        }
        Ok(())
    }

    fn read_from<T: Read>(&mut self, reader: &mut BufReader<T>) -> Result<(), HttpError> {
        loop {
            let mut line = String::new();
            let size = reader.read_line(&mut line).map_err(HttpError::from)?;
            if size == 0 {
                return Err(HttpError::HttpSyntaxError);
            }

            let line_str = line.trim_end_matches("\r\n");
            if line_str == "" {
                break;
            }
            let mut iter = line_str.splitn(2, ":");
            let key = iter
                .next()
                .ok_or_else(|| HttpError::HttpSyntaxError)?
                .to_ascii_lowercase();
            let value = iter
                .next()
                .ok_or_else(|| HttpError::HttpSyntaxError)?
                .trim_start()
                .to_string();

            self.headers.insert(key, value);
        }
        Ok(())
    }

    fn content_length(&self) -> Option<usize> {
        match self.headers.get(&"content-length".to_string()) {
            Some(v) => match v.parse::<usize>() {
                Ok(s) => Some(s),
                Err(_) => None,
            },
            None => None,
        }
    }

    fn contains_key(&self, key: &String) -> bool {
        self.headers.contains_key(key)
    }
}

fn main() -> Result<(), HttpError> {
    let args = env::args().collect::<Vec<String>>();
    if &args[1] == "client" {
        let client = HttpClient::new("127.0.0.1:8080")?;
        let req = HttpRequest {
            method: HttpMethod::GET,
            path: (&"/").to_string(),
            version: HttpVersion::HTTP1_0,
            headers: HttpHeaders::new(),
            body: Vec::new(),
        };
        let resp = client.request(&req)?;
        println!("{:?}", resp);
        println!("{:?}", String::from_utf8(resp.body).unwrap());
    } else if &args[1] == "server" {
        let mut router = Router { rules: Vec::new() };
        router.add(HttpMethod::GET, "/".to_string(), |_| {
            Ok(HttpResponse {
                version: HttpVersion::HTTP1_1,
                status: HttpStatus::Ok,
                headers: HttpHeaders::new(),
                body: Vec::new(),
            })
        });
        let server = HttpServer::new("127.0.0.1:8080", Box::new(router))?;
        server.listen()?;
    }

    Ok(())
}
