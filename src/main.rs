use openssl::ssl::{SslConnector, SslMethod};
use std::collections::HashMap;
use std::env;
use std::io::{self, BufRead, Cursor, Read, Write};
use std::net::TcpStream;

#[derive(Debug)]
struct URL {
    scheme: String,
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
    status_messaage: String,
    headers: HashMap<String, String>,
    data: String,
}

fn parse_url(full_url: &str) -> URL {
    let mut url_copy = String::from(full_url);
    let mut scheme = String::from("http");

    if url_copy.contains("://") {
        let _schemes = ["http", "https", "file", "data"];

        let scheme_url: Vec<&str> = full_url.split("://").collect();

        assert!(_schemes.contains(&scheme_url[0]));

        scheme = String::from(scheme_url[0]);
        url_copy = String::from(scheme_url[1]);
    }

    let hostname_and_path = url_copy.split_once('/').unwrap_or((&url_copy, ""));

    let mut hostname = hostname_and_path.0;

    let mut port = if scheme == "http" { "80" } else { "443" };

    if hostname.contains(":") {
        let split_hostname_port: Vec<&str> = hostname.split(":").collect();

        hostname = split_hostname_port[0];
        port = split_hostname_port[1];
    }

    URL {
        scheme: String::from(scheme),
        hostname: String::from(hostname),
        path: format!("/{}", hostname_and_path.1),
        port: String::from(port),
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

    if url.scheme == "https" || request.port == "443" {
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
