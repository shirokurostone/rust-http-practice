use std::env;

mod client;
mod common;
mod server;
use crate::client::*;
use crate::common::*;
use crate::server::*;

fn main() -> Result<(), HttpError> {
    let args = env::args().collect::<Vec<String>>();
    if &args[1] == "client" {
        let resp = HttpClient::get(String::from(&args[2]))?;
        println!("{:?}", resp);
        println!("{:?}", String::from_utf8(resp.body).unwrap());
    } else if &args[1] == "server" {
        let mut router = Router::new();
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
