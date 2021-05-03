use std::collections::HashMap;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter};
use std::net::{TcpStream, ToSocketAddrs};

#[derive(Debug)]
struct HttpClient {
    stream: TcpStream,
}

impl HttpClient {
    fn new<T: ToSocketAddrs>(addr: T) -> std::io::Result<HttpClient> {
        let stream = TcpStream::connect(addr)?;
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

    fn recv(&self) -> std::io::Result<HttpResponse> {
        let mut reader = BufReader::new(&self.stream);

        let mut line = String::new();
        reader.read_line(&mut line)?;
        let mut iter = line.splitn(3, " ");
        iter.next();
        let status: u16 = iter.next().unwrap().parse().unwrap();
        let mut headers: HashMap<String, String> = HashMap::new();

        loop {
            let mut line = String::new();
            let size = reader.read_line(&mut line)?;
            if size == 0 {
                panic!();
            }

            let line_str = line.trim_end_matches("\r\n");
            if line_str == "" {
                break;
            }
            let mut iter = line_str.splitn(2, ":");
            let key = iter.next().unwrap().to_ascii_lowercase();
            let value = iter.next().unwrap().trim_start().to_string();

            headers.insert(key, value);
        }

        let body = match headers.get(&"content-length".to_string()) {
            Some(v) => {
                let size = v.parse().unwrap();
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

    fn request(&self, request: &HttpRequest) -> std::io::Result<HttpResponse> {
        self.send(request)?;
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

fn main() -> std::io::Result<()> {
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
