use openssl::ssl::{SslConnector, SslMethod};
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, Cursor, Read, Write};
use std::net::TcpStream;

#[derive(Debug)]
enum HttpScheme {
    Data,
    File,
    HTTP,
    HTTPS,
}

impl HttpScheme {
    fn from_str(value: &str) -> HttpScheme {
        match value {
            "data" => HttpScheme::Data,
            "file" => HttpScheme::File,
            "http" => HttpScheme::HTTP,
            "https" => HttpScheme::HTTPS,
            _other => HttpScheme::HTTP,
        }
    }
}

#[derive(Debug)]
struct URL {
    scheme: HttpScheme,
    hostname: String,
    path: String,
    port: String,
}

enum Header {
    Connection,
    UserAgent,
    Host,
}

impl Header {
    fn as_str(&self) -> &'static str {
        match self {
            Header::Connection => "Connection",
            Header::Host => "Host",
            Header::UserAgent => "User-Agent",
        }
    }
}

struct HTTPRequest {
    http_version: String,
    method: String,
    path: String,
    port: String,
    headers: HashMap<String, String>,
    data: String,
}

impl HTTPRequest {
    fn build(&self) -> String {
        format!(
            "{} {} HTTP/{}\r\n{}\r\n\r\n",
            self.method,
            self.path,
            self.http_version,
            self.build_headers()
        )
    }

    fn build_headers(&self) -> String {
        let mut output = String::from("");

        for (key, value) in &self.headers {
            output.push_str(&key);
            output.push_str(": ");
            output.push_str(&value);
            output.push_str("\r\n");
        }

        String::from(output)
    }
}

struct HTTPResponse {
    http_version: String,
    status_code: u16,
    status_message: String,
    headers: HashMap<String, String>,
    data: String,
}

fn parse_url(full_url: &str) -> URL {
    let scheme_regexp = Regex::new(r"^(http|https|file|data):/?/?").unwrap();

    let mut url_copy = String::from(full_url);
    let mut scheme = String::from("http");

    if scheme_regexp.is_match(&url_copy) {
        let _schemes = ["http", "https", "file", "data"];

        let scheme_url: Vec<&str> = full_url.split(":").collect();

        assert!(_schemes.contains(&scheme_url[0]));

        scheme = String::from(scheme_url[0]);

        url_copy = String::from(scheme_url[1]);

        if ["http", "https", "file"].contains(&scheme.as_str()) && url_copy.starts_with("//") {
            url_copy = String::from(url_copy.get(2..).unwrap_or(""))
        }
    }

    if ["http", "https"].contains(&scheme.as_str()) {
        let (mut hostname, path) = url_copy.split_once('/').unwrap_or((&url_copy, ""));

        let mut port = if scheme == "http" { "80" } else { "443" };

        if hostname.contains(":") {
            let split_hostname_port: Vec<&str> = hostname.split(":").collect();

            hostname = split_hostname_port[0];
            port = split_hostname_port[1];
        }

        URL {
            scheme: HttpScheme::from_str(&scheme),
            hostname: String::from(hostname),
            path: format!("/{}", path),
            port: String::from(port),
        }
    } else if scheme == "file" {
        URL {
            scheme: HttpScheme::from_str(&scheme),
            hostname: String::from(""),
            path: String::from(url_copy),
            port: String::from(""),
        }
    } else if scheme == "data" {
        URL {
            scheme: HttpScheme::from_str(&scheme),
            hostname: String::from(""),
            path: String::from(url_copy),
            port: String::from(""),
        }
    } else {
        URL {
            scheme: HttpScheme::from_str(&scheme),
            hostname: String::from(""),
            path: String::from(url_copy),
            port: String::from(""),
        }
    }
}

fn request(full_url: &str) -> io::Result<HTTPResponse> {
    let url = parse_url(full_url);

    let headers: HashMap<String, String> = HashMap::from([
        (
            String::from(Header::Host.as_str()),
            String::from(&url.hostname),
        ),
        (
            String::from(Header::Connection.as_str()),
            String::from("close"),
        ),
        (
            String::from(Header::UserAgent.as_str()),
            String::from("Bored Browser"),
        ),
    ]);

    let request = HTTPRequest {
        data: String::from(""),
        http_version: String::from("1.1"),
        headers,
        method: String::from("GET"),
        path: String::from(&url.path),
        port: String::from(&url.port),
    };

    let mut res = vec![];

    match url.scheme {
        HttpScheme::HTTPS => {
            let base_stream = TcpStream::connect(format!("{}:{}", &url.hostname, &url.port))
                .expect("Couldn't connect to the server...");
            let connector = SslConnector::builder(SslMethod::tls()).unwrap().build();

            let mut stream = connector.connect(&url.hostname, base_stream).unwrap();

            stream
                .write_all(request.build().as_bytes())
                .expect("Couldn't send data to server");

            stream.flush()?;

            stream.read_to_end(&mut res).unwrap();
        }
        HttpScheme::HTTP => {
            let mut stream = TcpStream::connect(format!("{}:{}", &url.hostname, &url.port))
                .expect("Couldn't connect to the server...");
            stream
                .write_all(request.build().as_bytes())
                .expect("Couldn't send data to server");

            stream.flush()?;

            stream.read_to_end(&mut res).unwrap();
        }
        HttpScheme::Data => {
            // _ is the content_type
            let (_, data) = url.path.split_once(',').unwrap_or((&url.path, ""));

            // Writing end-of-file.
            let new_data = String::new() + data + "\r\n";

            res = Vec::from(new_data.as_bytes())
        }
        HttpScheme::File => {
            let mut file = File::open(&url.path).expect("File not found...");

            file.read_to_end(&mut res).unwrap();
        }
    }

    let mut res_stream = Cursor::new(res);

    let mut reader = io::BufReader::new(&mut res_stream);

    let mut headers = HashMap::new();
    let mut status_code = 200;
    let mut http_version = String::from("");
    let mut status_message = String::from("");

    match url.scheme {
        HttpScheme::HTTP | HttpScheme::HTTPS => {
            let mut status_line: String = String::new();
            reader.read_line(&mut status_line)?;

            let status_parts: Vec<&str> = status_line.split(" ").collect();

            assert!(status_parts[0].contains("HTTP/"));

            http_version = String::from(status_parts[0].trim_start_matches("HTTP/"));

            status_code = status_parts[1].parse::<u16>().unwrap();

            status_message = String::from(status_parts[2]);

            loop {
                let mut current_line: String = String::new();
                reader.read_line(&mut current_line)?;

                if current_line == String::from("\r\n") {
                    break;
                }

                let (header, value) = current_line.split_once(":").unwrap_or((&current_line, ""));

                headers.insert(String::from(header), String::from(value));
            }
        }
        _ => (),
    }

    let data: Vec<u8> = reader.fill_buf()?.to_vec();
    reader.consume(data.len());

    let data_string = String::from_utf8(data).expect("Could not parse data as utf8...");

    let response = HTTPResponse {
        status_code,
        http_version,
        status_message,
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
