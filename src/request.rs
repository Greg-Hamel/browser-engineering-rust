use crate::url::URIScheme;
use crate::url::URL;

use flate2::read::GzDecoder;
use openssl::ssl::{SslConnector, SslMethod};
use std::collections::HashMap;
use std::fmt;
use std::io::{self, BufRead, Cursor, Read, Write};
use std::net::TcpStream;

pub enum Header {
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

pub enum HTTPMethod {
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
    CONNECT,
    OPTIONS,
    TRACE,
}

impl HTTPMethod {
    fn as_str(&self) -> &'static str {
        match self {
            HTTPMethod::GET => "GET",
            HTTPMethod::HEAD => "HEAD",
            HTTPMethod::POST => "POST",
            HTTPMethod::PUT => "PUT",
            HTTPMethod::DELETE => "DELETE",
            HTTPMethod::CONNECT => "CONNECT",
            HTTPMethod::OPTIONS => "OPTIONS",
            HTTPMethod::TRACE => "TRACE",
        }
    }
}

pub struct HTTPRequest {
    pub http_version: String,
    pub method: HTTPMethod,
    pub path: String,
    pub port: String,
    pub headers: HashMap<String, String>,
    pub data: String,
}

impl HTTPRequest {
    fn build(&self) -> String {
        format!(
            "{} {} HTTP/{}\r\n{}\r\n\r\n",
            self.method.as_str(),
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

pub struct HTTPResponse {
    pub http_version: String,
    pub status_code: u16,
    pub status_message: String,
    pub headers: HashMap<String, String>,
    pub data: String,
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

pub struct Request {}

impl Request {
    pub fn send(url: &URL) -> io::Result<HTTPResponse> {
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
            method: HTTPMethod::GET,
            path: String::from(&url.path),
            port: String::from(&url.port),
        };

        let mut res = vec![];

        // Make request
        match url.scheme {
            URIScheme::HTTPS | URIScheme::ViewSourceHTTPS => {
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
            URIScheme::HTTP | URIScheme::ViewSourceHTTP => {
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
            URIScheme::HTTP
            | URIScheme::HTTPS
            | URIScheme::ViewSourceHTTP
            | URIScheme::ViewSourceHTTPS => {
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

                    let (header, value) =
                        current_line.split_once(":").unwrap_or((&current_line, ""));

                    headers.insert(String::from(header), String::from(value.trim()));
                }
            }
            _ => (),
        }

        let mut data = vec![];
        let mut data_length = 0;
        // Transfer encoding (chunked)
        if headers.contains_key(Header::TransferEncoding.as_str()) {
            assert!(
                headers.get(Header::TransferEncoding.as_str()) == Some(&String::from("chunked"))
            );

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
}
