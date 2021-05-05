use std::io::prelude::*;
use std::io::{BufReader, BufWriter};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};

use crate::common::*;

pub struct HttpServer {
    listener: TcpListener,
    handler: Box<dyn Handler>,
}

impl HttpServer {
    pub fn new<T: ToSocketAddrs>(
        addr: T,
        handler: Box<dyn Handler>,
    ) -> Result<HttpServer, HttpError> {
        let listener = TcpListener::bind(addr).map_err(HttpError::from)?;
        Ok(HttpServer {
            listener: listener,
            handler: handler,
        })
    }

    pub fn listen(&self) -> Result<(), HttpError> {
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

pub trait Handler {
    fn handle(&self, req: &mut HttpRequest) -> Result<HttpResponse, HttpError>;
}

pub struct Router {
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
            version: req.version,
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
    pub fn new() -> Router {
        Router { rules: Vec::new() }
    }

    pub fn add(
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
