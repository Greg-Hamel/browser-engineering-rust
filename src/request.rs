use crate::uri::Scheme;
use crate::uri::URI;

use flate2::read::GzDecoder;
use openssl::ssl::{SslConnector, SslMethod};
use std::collections::HashMap;
use std::fmt;
use std::io::{self, BufRead, Cursor, Read, Write};
use std::net::TcpStream;

pub enum Header {
    AcceptEncoding,
    Connection,
    ContentEncoding,
    Host,
    Location,
    TransferEncoding,
    UserAgent,
}

impl Header {
    fn as_str(&self) -> &'static str {
        match self {
            Header::AcceptEncoding => "Accept-Encoding",
            Header::Connection => "Connection",
            Header::ContentEncoding => "Content-Encoding",
            Header::Host => "Host",
            Header::Location => "Location",
            Header::TransferEncoding => "Transfer-Encoding",
            Header::UserAgent => "User-Agent",
        }
    }
}

#[derive(Clone, Debug)]
pub enum HTTPMethod {
    CONNECT,
    DELETE,
    GET,
    HEAD,
    OPTIONS,
    POST,
    PUT,
    TRACE,
}

impl HTTPMethod {
    fn as_str(&self) -> &'static str {
        match self {
            HTTPMethod::CONNECT => "CONNECT",
            HTTPMethod::DELETE => "DELETE",
            HTTPMethod::GET => "GET",
            HTTPMethod::HEAD => "HEAD",
            HTTPMethod::OPTIONS => "OPTIONS",
            HTTPMethod::POST => "POST",
            HTTPMethod::PUT => "PUT",
            HTTPMethod::TRACE => "TRACE",
        }
    }
}
#[derive(Clone, Debug)]
pub struct HTTPRequest {
    pub url: URI,
    pub http_version: String,
    pub method: HTTPMethod,
    pub headers: HashMap<String, String>,
    pub data: String,
}

impl HTTPRequest {
    fn build(&self) -> String {
        format!(
            "{} {} HTTP/{}\r\n{}\r\n\r\n",
            self.method.as_str(),
            self.url.path,
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
    fn build_default_headers(url: &URI) -> HashMap<String, String> {
        let output = HashMap::from([
            (
                String::from(Header::Host.as_str()),
                String::from(&url.authority.as_ref().expect("No authority").host),
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

        output
    }

    fn make_request(request: &HTTPRequest) -> io::Result<Vec<u8>> {
        let mut res = vec![];

        let request_authority = request
            .url
            .authority
            .as_ref()
            .expect("No authority")
            .clone();

        let host = request_authority.host;
        let port = request_authority.port.to_string();

        // Make request
        match request.url.scheme {
            Scheme::HTTPS => {
                let base_stream = TcpStream::connect(format!("{}:{}", host, port))
                    .expect("Couldn't connect to the server...");
                let connector = SslConnector::builder(SslMethod::tls()).unwrap().build();

                let mut stream = connector.connect(host.as_str(), base_stream).unwrap();

                stream
                    .write_all(request.build().as_bytes())
                    .expect("Couldn't send data to server");

                stream.flush()?;

                stream.read_to_end(&mut res).unwrap();
            }
            Scheme::HTTP => {
                let mut stream = TcpStream::connect(format!("{}:{}", host, port))
                    .expect("Couldn't connect to the server...");
                stream
                    .write_all(request.build().as_bytes())
                    .expect("Couldn't send data to server");

                stream.flush()?;

                stream.read_to_end(&mut res).unwrap();
            }
            _ => panic!("Unexpected scheme provided to Request"),
        }

        Ok(res)
    }

    fn parse_http_response(data_buffer: &Vec<u8>) -> io::Result<HTTPResponse> {
        let mut res_stream = Cursor::new(data_buffer);

        let mut reader = io::BufReader::new(&mut res_stream);

        // Extract HTTP information
        let mut status_line: String = String::new();
        reader.read_line(&mut status_line)?;

        let status_parts: Vec<&str> = status_line.split(" ").collect();

        assert!(status_parts[0].contains("HTTP/"));

        let mut headers = HashMap::new();

        let http_version = String::from(status_parts[0].trim_start_matches("HTTP/"));

        let status_code = status_parts[1].parse::<u16>().unwrap();

        let status_message = String::from(status_parts[2]);

        loop {
            let mut current_line: String = String::new();
            reader.read_line(&mut current_line)?;

            if current_line == String::from("\r\n") {
                break;
            }

            let (header, value) = current_line.split_once(":").unwrap_or((&current_line, ""));

            headers.insert(String::from(header), String::from(value.trim()));
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

        Ok(HTTPResponse {
            http_version,
            status_code,
            status_message,
            headers,
            data: data_string,
        })
    }

    fn build_request_from_redirect_response(
        request: HTTPRequest,
        response: &HTTPResponse,
    ) -> HTTPRequest {
        let mut new_request = request.clone();

        let location_header = response
            .headers
            .get(Header::Location.as_str())
            .expect("Redirect response without Location header");

        if location_header.starts_with(Scheme::HTTP.as_str()) {
            // Absolute
            println!("{location_header}");
            let new_url = URI::parse(&location_header);
            new_request.url = new_url;
        } else {
            // Relative path
            if new_request.url.path.is_empty() {
                let new_path = format!("{}", location_header);
                new_request.url.path = new_path;
            } else {
                let last_slash_index = new_request.url.path.rfind('/').expect("No slash");

                let path_without_last_relative_portion =
                    new_request.url.path.split_at(last_slash_index).0;

                let new_path = format!("{path_without_last_relative_portion}{location_header}");
                new_request.url.path = new_path;
            }
        }

        new_request
    }

    pub fn send(url: &URI) -> io::Result<HTTPResponse> {
        let mut request = HTTPRequest {
            url: url.clone(),
            data: String::from(""),
            http_version: String::from("1.1"),
            headers: Self::build_default_headers(&url),
            method: HTTPMethod::GET,
        };

        let mut redirect_count = 0;
        let mut response;

        loop {
            if redirect_count < 5 {
                println!("{request:?}");
                let res = Self::make_request(&request)?;

                response = Self::parse_http_response(&res);

                match &response {
                    Ok(response) => {
                        println!("{response}");
                        if &response.status_code < &300 || &response.status_code > &399 {
                            break;
                        }

                        request = Self::build_request_from_redirect_response(request, &response)
                    }
                    Err(e) => (),
                }

                redirect_count += 1;
            } else {
                panic!("Exceeded maximum redirect count.");
            }
        }

        response
    }
}

#[cfg(test)]
mod redirect_response_to_request {
    use std::collections::HashMap;

    use crate::request::Header;
    use crate::uri::URI;

    #[test]
    fn absolute_url_get_redirected_correctly() {
        let request = super::HTTPRequest {
            url: URI::parse(&String::from("http://www.example.org/this_is_a_redirect")),
            http_version: String::from("1.1"),
            method: super::HTTPMethod::GET,
            headers: HashMap::new(),
            data: String::from(""),
        };

        let redirect_url_string = String::from("http://www.example.org/redirected");

        let redirect_url = URI::parse(&redirect_url_string);

        let response = super::HTTPResponse {
            http_version: String::from("1.1"),
            status_code: 301,
            status_message: String::new(),
            headers: HashMap::from([(
                String::from(Header::Location.as_str()),
                redirect_url_string.clone(),
            )]),
            data: String::from(""),
        };

        let new_request = super::Request::build_request_from_redirect_response(request, &response);

        assert_eq!(
            redirect_url.authority.as_ref().expect("No authority").host,
            new_request
                .url
                .authority
                .as_ref()
                .expect("No authority")
                .host
        );
        assert_eq!(redirect_url.path, new_request.url.path);
        assert_eq!(
            redirect_url.authority.as_ref().expect("No authority").port,
            new_request
                .url
                .authority
                .as_ref()
                .expect("No authority")
                .port
        );
        assert_eq!(redirect_url.scheme, new_request.url.scheme);
    }

    #[test]
    fn relative_url_get_redirected_correctly() {
        let request = super::HTTPRequest {
            url: URI::parse(&String::from("http://www.example.org/this_is_a_redirect")),
            http_version: String::from("1.1"),
            method: super::HTTPMethod::GET,
            headers: HashMap::new(),
            data: String::from(""),
        };

        let redirect_url_string = String::from("http://www.example.org/redirected");

        let redirect_url = URI::parse(&redirect_url_string);

        let response = super::HTTPResponse {
            http_version: String::from("1.1"),
            status_code: 301,
            status_message: String::new(),
            headers: HashMap::from([(
                String::from(Header::Location.as_str()),
                String::from("/redirected"),
            )]),
            data: String::from(""),
        };

        let new_request = super::Request::build_request_from_redirect_response(request, &response);

        let authority = redirect_url.authority.unwrap();
        let new_request_authority = new_request.url.authority.unwrap();

        assert_eq!(authority.host, new_request_authority.host);
        assert_eq!(redirect_url.path, new_request.url.path);
        assert_eq!(authority.port, new_request_authority.port);
        assert_eq!(redirect_url.scheme, new_request.url.scheme);
    }

    #[test]
    fn relative_url_get_redirected_correctly_with_deep_path() {
        let request = super::HTTPRequest {
            url: URI::parse(&String::from(
                "http://www.example.org/deep/path/this_is_a_redirect",
            )),
            http_version: String::from("1.1"),
            method: super::HTTPMethod::GET,
            headers: HashMap::new(),
            data: String::from(""),
        };

        let redirect_url_string = String::from("http://www.example.org/deep/path/redirected");

        let redirect_url = URI::parse(&redirect_url_string);

        let response = super::HTTPResponse {
            http_version: String::from("1.1"),
            status_code: 301,
            status_message: String::new(),
            headers: HashMap::from([(
                String::from(Header::Location.as_str()),
                String::from("/redirected"),
            )]),
            data: String::from(""),
        };

        let new_request = super::Request::build_request_from_redirect_response(request, &response);

        let authority = redirect_url.authority.unwrap();
        let new_request_authority = new_request.url.authority.unwrap();

        assert_eq!(authority.host, new_request_authority.host);
        assert_eq!(redirect_url.path, new_request.url.path);
        assert_eq!(authority.port, new_request_authority.port);
        assert_eq!(redirect_url.scheme, new_request.url.scheme);
    }
}
