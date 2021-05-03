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
            "{} {} HTTP/1.0\r\n",
            req.method.to_string(),
            req.path
        )?;
        for (key, value) in &req.headers {
            write!(writer, "{}: {}\r\n", key, value)?;
        }
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
        iter.next();
        let status: u16 = iter
            .next()
            .ok_or_else(|| HttpError::HttpSyntaxError)?
            .parse()
            .map_err(|_| HttpError::HttpSyntaxError)?;
        let mut headers: HashMap<String, String> = HashMap::new();

        loop {
            let mut line = String::new();
            let size = reader.read_line(&mut line).map_err(HttpError::from)?;
            if size == 0 {
                panic!();
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

            headers.insert(key, value);
        }

        let body = match headers.get(&"content-length".to_string()) {
            Some(v) => {
                let size = v.parse::<usize>().map_err(|_| HttpError::HttpSyntaxError)?;
                let mut body = Vec::with_capacity(size);
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

#[derive(Debug)]
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
struct HttpRequest {
    method: HttpMethod,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

#[derive(Debug)]
struct HttpResponse {
    status: u16,
    headers: HashMap<String, String>,
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

#[derive(Debug)]
struct HttpServer {
    listener: TcpListener,
}

impl HttpServer {
    fn new<T: ToSocketAddrs>(addr: T) -> Result<HttpServer, HttpError> {
        let listener = TcpListener::bind(addr).map_err(HttpError::from)?;
        Ok(HttpServer { listener: listener })
    }

    fn listen(&self) -> Result<(), HttpError> {
        for stream in self.listener.incoming() {
            self.handle(stream.map_err(HttpError::from)?)?;
        }
        Ok(())
    }

    fn handle(&self, stream: TcpStream) -> Result<(), HttpError> {
        let req = self.recv(BufReader::new(&stream))?;
        let resp = HttpResponse {
            status: 200,
            headers: HashMap::new(),
            body: Vec::new(),
        };
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
        let version = match iter.next().ok_or_else(|| HttpError::HttpSyntaxError)? {
            "HTTP/1.0" => "1.0",
            "HTTP/1.1" => "1.1",
            _ => return Err(HttpError::HttpSyntaxError),
        };

        let mut headers: HashMap<String, String> = HashMap::new();

        loop {
            let mut line = String::new();
            let size = reader.read_line(&mut line).map_err(HttpError::from)?;
            if size == 0 {
                panic!();
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

            headers.insert(key, value);
        }

        let body = match headers.get(&"content-length".to_string()) {
            Some(v) => {
                if v == "0" {
                    Vec::new()
                } else {
                    let size = v.parse::<usize>().map_err(|_| HttpError::HttpSyntaxError)?;
                    let mut body = Vec::with_capacity(size);
                    reader.read_to_end(&mut body)?;
                    body
                }
            }
            None => {
                let mut body = Vec::new();
                reader.read_to_end(&mut body)?;
                body
            }
        };

        Ok(HttpRequest {
            method: method,
            path: path,
            headers: headers,
            body: body,
        })
    }

    fn send<T: Write>(&self, resp: &HttpResponse, mut writer: BufWriter<T>) -> std::io::Result<()> {
        write!(writer, "HTTP/1.1 200 OK\r\n",)?;
        for (key, value) in &resp.headers {
            write!(writer, "{}: {}\r\n", key, value)?;
        }
        if !resp.headers.contains_key(&"content-length".to_string()) {
            write!(writer, "content-length: {}\r\n", resp.body.len())?;
        }

        write!(writer, "\r\n")?;
        writer.write_all(&resp.body)?;
        writer.flush()?;
        Ok(())
    }
}

fn main() -> Result<(), HttpError> {
    let args = env::args().collect::<Vec<String>>();
    if &args[1] == "client" {
        let client = HttpClient::new("127.0.0.1:8080")?;
        let req = HttpRequest {
            method: HttpMethod::GET,
            path: (&"/").to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        };
        let resp = client.request(&req)?;
        println!("{:?}", resp);
        println!("{:?}", String::from_utf8(resp.body).unwrap());
    } else if &args[1] == "server" {
        let server = HttpServer::new("127.0.0.1:8080")?;
        server.listen()?;
    }

    Ok(())
}
