use std::collections::HashMap;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter};
use thiserror::Error;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HttpMethod {
    GET,
    POST,
}

impl HttpMethod {
    pub fn to_string(&self) -> String {
        match self {
            HttpMethod::GET => "GET".to_string(),
            HttpMethod::POST => "POST".to_string(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HttpVersion {
    HTTP1_0,
    HTTP1_1,
    UNSUPPORTED,
}

impl HttpVersion {
    pub fn string(&self) -> &str {
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HttpStatus {
    Ok,
    NotFound,
    Invalid,
}

impl HttpStatus {
    pub fn code(&self) -> u32 {
        match self {
            Self::Ok => 200,
            Self::NotFound => 404,
            _ => 0,
        }
    }

    pub fn string(&self) -> &str {
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
pub struct HttpRequest {
    pub method: HttpMethod,
    pub path: String,
    pub version: HttpVersion,
    pub headers: HttpHeaders,
    pub body: Vec<u8>,
}

#[derive(Debug)]
pub struct HttpResponse {
    pub version: HttpVersion,
    pub status: HttpStatus,
    pub headers: HttpHeaders,
    pub body: Vec<u8>,
}

#[derive(Error, Debug)]
pub enum HttpError {
    #[error("ioerror : {source:?}")]
    IOError {
        #[from]
        source: std::io::Error,
    },
    #[error("syntax error")]
    HttpSyntaxError,
}

#[derive(Debug)]
pub struct HttpHeaders {
    headers: HashMap<String, String>,
}

impl HttpHeaders {
    pub fn new() -> HttpHeaders {
        HttpHeaders {
            headers: HashMap::new(),
        }
    }

    pub fn write_to<T: Write>(&self, writer: &mut BufWriter<T>) -> std::io::Result<()> {
        for (key, value) in &self.headers {
            write!(writer, "{}: {}\r\n", key, value)?;
        }
        Ok(())
    }

    pub fn read_from<T: Read>(&mut self, reader: &mut BufReader<T>) -> Result<(), HttpError> {
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

    pub fn content_length(&self) -> Option<usize> {
        match self.headers.get(&"content-length".to_string()) {
            Some(v) => match v.parse::<usize>() {
                Ok(s) => Some(s),
                Err(_) => None,
            },
            None => None,
        }
    }

    pub fn contains_key(&self, key: &String) -> bool {
        self.headers.contains_key(key)
    }
}
