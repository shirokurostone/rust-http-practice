use std::collections::HashMap;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter};
use std::net::{TcpStream, ToSocketAddrs};
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

    fn send(&self, req: &HttpRequest) -> std::io::Result<()> {
        let mut writer = BufWriter::new(&self.stream);
        match &req.method {
            HttpMethod::GET => write!(writer, "GET {} HTTP/1.0\r\n", req.path)?,
            HttpMethod::POST => write!(writer, "POST {} HTTP/1.0\r\n", req.path)?,
        }
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

    fn recv(&self) -> Result<HttpResponse, HttpError> {
        let mut reader = BufReader::new(&self.stream);

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
        self.send(request).map_err(HttpError::from)?;
        self.recv()
    }
}

#[derive(Debug)]
enum HttpMethod {
    GET,
    POST,
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

fn main() -> Result<(), HttpError> {
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
    Ok(())
}
