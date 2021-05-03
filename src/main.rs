use std::collections::HashMap;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter};
use std::net::TcpStream;

fn main() -> std::io::Result<()> {
    let stream = TcpStream::connect("127.0.0.1:8080")?;
    let mut writer = BufWriter::new(&stream);
    write!(writer, "GET / HTTP/1.0\r\n\r\n")?;
    writer.flush()?;

    let mut reader = BufReader::new(&stream);

    let mut line = String::new();
    let _ = reader.read_line(&mut line);
    let mut resp_headers: HashMap<String, String> = HashMap::new();

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

        resp_headers.insert(key, value);
    }

    let mut body = String::from("");
    match resp_headers.get(&"content-length".to_string()) {
        Some(v) => {
            let size = v.parse().unwrap();
            let mut buf = Vec::with_capacity(size);
            reader.read_to_end(&mut buf)?;
            body = String::from_utf8(buf).unwrap();
        }
        None => {
            reader.read_to_string(&mut body)?;
        }
    }

    println!("headers: {:#?}", resp_headers);
    println!("body : {:#?}", body);

    Ok(())
}
