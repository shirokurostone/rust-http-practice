use std::io::prelude::*;
use std::io::{BufReader, BufWriter};
use std::net::{TcpStream, ToSocketAddrs};
use url::Url;

use crate::common::*;

#[derive(Debug)]
pub struct HttpClient {
    stream: TcpStream,
}

impl HttpClient {
    pub fn new<T: ToSocketAddrs>(addr: T) -> Result<HttpClient, HttpError> {
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

    pub fn request(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        self.send(request, BufWriter::new(&self.stream))
            .map_err(HttpError::from)?;
        self.recv(BufReader::new(&self.stream))
    }

    pub fn get(url: String) -> Result<HttpResponse, HttpError> {
        let url = Url::parse(&url).map_err(HttpError::from)?;
        let addrs = url
            .socket_addrs(|| Some(80))
            .map_err(|_| HttpError::UrlFormatError)?;
        let addr = addrs.first().ok_or_else(|| HttpError::UrlFormatError)?;

        let client = HttpClient::new(addr)?;
        let req = HttpRequest {
            method: HttpMethod::GET,
            path: (url.path()).to_string(),
            version: HttpVersion::HTTP1_0,
            headers: HttpHeaders::new(),
            body: Vec::new(),
        };
        let resp = client.request(&req)?;
        Ok(resp)
    }
}
