use flate2::read::GzDecoder;
use openssl::ssl::{SslConnector, SslMethod};
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::fmt;
use std::fs;
use std::io::{self, BufRead, Cursor, Read, Write};
use std::net::TcpStream;

#[derive(Debug)]
enum HttpScheme {
    Data,
    File,
    HTTP,
    HTTPS,
    ViewSourceHTTP,
    ViewSourceHTTPS,
}

impl HttpScheme {
    fn from_str(value: &str) -> HttpScheme {
        match value {
            "data" => HttpScheme::Data,
            "file" => HttpScheme::File,
            "https" => HttpScheme::HTTPS,
            "http" => HttpScheme::HTTP,
            "view-source:https" => HttpScheme::ViewSourceHTTPS,
            "view-source:http" => HttpScheme::ViewSourceHTTP,
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
    AcceptEncoding,
    ContentEncoding,
    TransferEncoding,
}

impl Header {
    fn as_str(&self) -> &'static str {
        match self {
            Header::Connection => "Connection",
            Header::Host => "Host",
            Header::UserAgent => "User-Agent",
            Header::AcceptEncoding => "Accept-Encoding",
            Header::ContentEncoding => "Content-Encoding",
            Header::TransferEncoding => "Transfer-Encoding",
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

impl HTTPResponse {
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

impl fmt::Display for HTTPResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HTTP/{} {} {}\r\n{}\r\n{}\r\n",
            self.http_version,
            self.status_code,
            self.status_message,
            self.build_headers(),
            self.data
        )
    }
}

fn parse_url(full_url: &str) -> URL {
    let scheme_regexp = Regex::new(r"^(http|https|file|data|view-source):/?/?").unwrap();

    let mut url_copy = String::from(full_url);
    let mut scheme = String::from("http");

    if scheme_regexp.is_match(&url_copy) {
        let _schemes = vec![
            "http",
            "https",
            "file",
            "data",
            "view-source:http",
            "view-source:https",
        ];

        let mut scheme_url = Vec::new();
        let mut scheme_found = false;
        let mut prev = String::new();

        let spliter = ":";

        for scheme_part in full_url.split(spliter) {
            if prev.len() > 0 {
                prev = prev + spliter + scheme_part;
            } else {
                prev += scheme_part
            }
            if scheme_found == false {
                if _schemes.contains(&prev.as_str()) {
                    scheme_url.push(prev);
                    scheme_found = true;
                    prev = "".to_string();
                }
            }
        }

        scheme_url.push(prev);

        assert!(scheme_found);
        scheme = String::from(scheme_url[0].as_str());

        url_copy = String::from(scheme_url[1].as_str());

        if [
            "http",
            "https",
            "file",
            "view-source:http",
            "view-source:https",
        ]
        .contains(&scheme.as_str())
            && url_copy.starts_with("//")
        {
            url_copy = String::from(url_copy.get(2..).unwrap_or(""))
        }
    }

    if ["http", "https", "view-source:http", "view-source:https"].contains(&scheme.as_str()) {
        let (mut hostname, path) = url_copy.split_once('/').unwrap_or((&url_copy, ""));

        let mut port = if scheme.contains("https") {
            "443"
        } else {
            "80"
        };

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

fn transform(data: &str) -> String {
    let lt_re = Regex::new(r"<").unwrap();
    let gt_re = Regex::new(r">").unwrap();

    let no_lt = String::from(lt_re.replace_all(data, "&lt;"));

    String::from(gt_re.replace_all(&no_lt.as_str(), "&gt;"))
}

fn request(url: &URL) -> io::Result<HTTPResponse> {
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
        (
            String::from(Header::AcceptEncoding.as_str()),
            String::from("gzip"),
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

    // Make request
    match url.scheme {
        HttpScheme::HTTPS | HttpScheme::ViewSourceHTTPS => {
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
        HttpScheme::HTTP | HttpScheme::ViewSourceHTTP => {
            let mut stream = TcpStream::connect(format!("{}:{}", &url.hostname, &url.port))
                .expect("Couldn't connect to the server...");
            stream
                .write_all(request.build().as_bytes())
                .expect("Couldn't send data to server");

            stream.flush()?;

            stream.read_to_end(&mut res).unwrap();
        }
        _ => {}
    }

    let mut res_stream = Cursor::new(res);

    let mut reader = io::BufReader::new(&mut res_stream);

    let mut headers = HashMap::new();
    let mut status_code = 200;
    let mut http_version = String::from("");
    let mut status_message = String::from("");

    // Extract HTTP information
    match url.scheme {
        HttpScheme::HTTP
        | HttpScheme::HTTPS
        | HttpScheme::ViewSourceHTTP
        | HttpScheme::ViewSourceHTTPS => {
            let mut status_line: String = String::new();
            reader.read_line(&mut status_line)?;

            let status_parts: Vec<&str> = status_line.split(" ").collect();

            assert!(status_parts[0].contains("HTTP/"));

            http_version = String::from(status_parts[0].trim_start_matches("HTTP/"));

            status_code = status_parts[1].parse::<u16>().unwrap();

            if status_code >= 400 && status_code < 600 {
                println!("Could not complete request. Dumping...");
                println!("{}", request.build())
            }

            status_message = String::from(status_parts[2]);

            loop {
                let mut current_line: String = String::new();
                reader.read_line(&mut current_line)?;

                if current_line == String::from("\r\n") {
                    break;
                }

                let (header, value) = current_line.split_once(":").unwrap_or((&current_line, ""));

                headers.insert(String::from(header), String::from(value.trim()));
            }
        }
        _ => (),
    }

    let mut data = vec![];
    let mut data_length = 0;
    // Transfer encoding (chunked)
    if headers.contains_key(Header::TransferEncoding.as_str()) {
        assert!(headers.get(Header::TransferEncoding.as_str()) == Some(&String::from("chunked")));

        loop {
            let mut length_buffer = String::new();
            let bytes_read = reader
                .read_line(&mut length_buffer)
                .expect("reading line works");

            if bytes_read == 0 {
                break;
            }

            let bytes_to_read = u64::from_str_radix(length_buffer.trim_end(), 16).unwrap();

            let mut data_buffer = vec![];

            {
                let reader_reference = reader.by_ref();

                // read at most specified number of bytes
                let data_bytes_read = reader_reference
                    .take(bytes_to_read)
                    .read_to_end(&mut data_buffer)?;

                if bytes_read == 0 {
                    break;
                }

                data_length += data_bytes_read;
            } // drop our &mut reader_reference so we can use reader again

            data.append(&mut data_buffer);

            let mut spacing_buffer: [u8; 2] = [0; 2];

            reader.read(&mut spacing_buffer)?;
        }
    } else {
        data_length = reader.read_to_end(&mut data)?;
    }

    let mut data_string = String::new();

    // GZIP extraction if required
    if headers.contains_key(Header::ContentEncoding.as_str()) {
        assert!(headers.get(Header::ContentEncoding.as_str()) == Some(&String::from("gzip")));
        let mut deflater = GzDecoder::new(data.as_slice());
        deflater.read_to_string(&mut data_string)?;
    } else {
        data_string = String::from_utf8(data).expect("Could not parse data as utf8...");
    }

    reader.consume(data_length);

    let response = HTTPResponse {
        status_code,
        http_version,
        status_message,
        headers,
        data: data_string,
    };

    Ok(response)
}

fn show(source: &str, only_body: bool) {
    let mut in_angle = false;
    let mut in_body = false;

    let html_entities = HashMap::from([("&lt;", "<"), ("&gt;", ">")]);

    let mut current_tag = String::new();
    let mut possible_entity = String::new();

    for character in source.chars() {
        if character == '<' {
            in_angle = true
        } else if character == '>' {
            if current_tag == "body" {
                in_body = true
            } else if current_tag == "/body" {
                in_body = false
            }
            current_tag = String::new();
            in_angle = false
        } else if !in_angle {
            if only_body && !in_body {
                // way to show only inside the body element
                continue;
            }

            if character == '&' || possible_entity.len() > 0 {
                // HTML entity interpretation
                if character == '&' && possible_entity.len() == 0 {
                    possible_entity += &character.to_string();
                } else if possible_entity.len() > 0 {
                    if possible_entity.len() > 25 {
                        // No entity has an allowable name space large than 23 + 2, dump current buffer.
                        print!("{possible_entity}");
                        possible_entity = String::new();
                        continue;
                    }

                    possible_entity += &character.to_string();

                    if character == ';' {
                        if html_entities.contains_key(&possible_entity.as_str()) {
                            let string_value =
                                html_entities.get(&possible_entity.as_str()).unwrap_or(&"");
                            print!("{}", string_value)
                        } else {
                            print!("{possible_entity}")
                        }

                        possible_entity = String::new();
                    }
                }

                continue;
            }

            print!("{character}")
        } else if in_angle {
            current_tag += &character.to_string();
        }
    }

    if possible_entity.len() > 0 {
        // If buffer still full, dump its content
        print!("{possible_entity}");
        possible_entity = String::new();
    }
}

fn load(full_url: &str) {
    let url = parse_url(&full_url);

    match url.scheme {
        HttpScheme::HTTPS | HttpScheme::HTTP => {
            let response = request(&url).expect("Couldn't parse response...");
            show(&response.data, true)
        }
        HttpScheme::ViewSourceHTTPS | HttpScheme::ViewSourceHTTP => {
            let response = request(&url).expect("Couldn't parse response...");

            show(&transform(&response.data), false)
        }
        HttpScheme::Data => {
            // _ is the content_type
            let (_, path_data) = url.path.split_once(',').unwrap_or((&url.path, ""));

            // Writing end-of-file.
            let data = String::new() + path_data + "\r\n";
            show(&data, false)
        }
        HttpScheme::File => {
            let data = fs::read_to_string(&url.path).expect("File not found...");
            show(&data, false)
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut url = &String::new();

    for argument in &args[1..] {
        if !argument.starts_with("-") {
            url = argument
        }
    }

    if url.len() > 0 {
        load(&url)
    };
}
