use openssl::ssl::{SslConnector, SslMethod};
use std::collections::HashMap;
use std::env;
use std::io::{self, BufRead, BufReader, Cursor, Read, Write};
use std::net::TcpStream;

#[derive(Debug)]
struct URL {
    scheme: String,
    hostname: String,
    path: String,
    port: String,
}

struct HTTPRequest {
    http_version: String,
    method: String,
    hostname: String,
    path: String,
    headers: HashMap<String, String>,
    data: String,
}

impl HTTPRequest {
    fn build(&self) -> String {
        format!(
            "{} {} HTTP/{}\r\nHost:{}\r\n\r\n",
            self.method, self.path, self.http_version, self.hostname
        )
    }
}

struct HTTPResponse {
    http_version: String,
    status_code: u16,
    status_messaage: String,
    headers: HashMap<String, String>,
    data: String,
}

fn split_url(full_url: &str) -> URL {
    let _schemes = ["http", "https", "file", "data"];

    let scheme_url: Vec<&str> = full_url.split("://").collect();

    assert!(_schemes.contains(&scheme_url[0]));

    let mut port = if scheme_url[0] == "http" { "80" } else { "443" };

    let hostname_and_path = scheme_url[1].split_once('/').unwrap_or((scheme_url[1], ""));

    let mut hostname = hostname_and_path.0;

    if hostname.contains(":") {
        let hostname_port: Vec<&str> = hostname.split(":").collect();
        hostname = hostname_port[0];
        port = hostname_port[1];
    }

    URL {
        scheme: String::from(scheme_url[0]),
        hostname: String::from(hostname),
        path: format!("/{}", hostname_and_path.1),
        port: String::from(port),
    }
}

fn request(full_url: &str) -> io::Result<HTTPResponse> {
    let url = split_url(full_url);

    let request = HTTPRequest {
        http_version: String::from("1.0"),
        path: String::from(&url.path),
        headers: HashMap::new(),
        hostname: String::from(&url.hostname),
        method: String::from("GET"),
        data: String::from(""),
    };

    let mut res = vec![];

    if url.scheme == "https" {
        let base_stream = TcpStream::connect(format!("{}:{}", &url.hostname, &url.port))
            .expect("Couldn't connect to the server...");
        let connector = SslConnector::builder(SslMethod::tls()).unwrap().build();

        let mut stream = connector.connect(&url.hostname, base_stream).unwrap();

        stream
            .write_all(request.build().as_bytes())
            .expect("Couldn't send data to server");

        stream.flush()?;

        stream.read_to_end(&mut res).unwrap();
    } else {
        let mut stream = TcpStream::connect(format!("{}:{}", &url.hostname, &url.port))
            .expect("Couldn't connect to the server...");
        stream
            .write_all(request.build().as_bytes())
            .expect("Couldn't send data to server");

        stream.flush()?;

        stream.read_to_end(&mut res).unwrap();
    }

    let mut res_stream = Cursor::new(res);

    let mut reader = io::BufReader::new(&mut res_stream);

    let mut status_line: String = String::new();
    reader.read_line(&mut status_line)?;

    let status_parts: Vec<&str> = status_line.split(" ").collect();

    assert!(status_parts[0].contains("HTTP/"));

    let mut headers = HashMap::new();

    loop {
        let mut current_line: String = String::new();
        reader.read_line(&mut current_line)?;

        if current_line == String::from("\r\n") {
            break;
        }

        let header = current_line.split_once(":").unwrap_or((&current_line, ""));

        headers.insert(String::from(header.0), String::from(header.1));
    }

    let data: Vec<u8> = reader.fill_buf()?.to_vec();
    reader.consume(data.len());

    let data_string = String::from_utf8(data).expect("Could not parse data as utf8...");

    let response = HTTPResponse {
        status_code: status_parts[1].parse::<u16>().unwrap(),
        http_version: String::from(status_parts[0].trim_start_matches("HTTP/")),
        status_messaage: String::from(status_parts[2]),
        headers,
        data: data_string,
    };

    assert_eq!(response.headers.get("transfer-encoding"), None);
    assert_eq!(response.headers.get("content-encoding"), None);

    Ok(response)
}

fn show(source: &str) {
    let mut in_angle = false;

    for character in source.chars() {
        if character == '<' {
            in_angle = true
        } else if character == '>' {
            in_angle = false
        } else if !in_angle {
            print!("{character}")
        }
    }
}

fn load(full_url: &str) {
    let response = request(&full_url).expect("Couldn't parse response...");
    show(&response.data)
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let full_url = &args[1];

    load(&full_url)
}
