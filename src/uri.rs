use regex::Regex;
use std::{collections::HashMap, result::Result};

#[derive(Debug, PartialEq, Clone)]
pub enum Scheme {
    Data,
    File,
    HTTP,
    HTTPS,
}

const DATA_SCHEME: &str = "data";
const FILE_SCHEME: &str = "file";
const HTTP_SCHEME: &str = "http";
const HTTPS_SCHEME: &str = "https";

const SCHEME_REGEX: &str = r"\w[\w\d+-.]*";

impl Scheme {
    pub fn from_str(value: &str) -> Result<Scheme, &'static str> {
        match value {
            DATA_SCHEME => Ok(Scheme::Data),
            FILE_SCHEME => Ok(Scheme::File),
            HTTPS_SCHEME => Ok(Scheme::HTTPS),
            HTTP_SCHEME => Ok(Scheme::HTTP),
            _other => Err("Invalid Scheme."),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Scheme::Data => DATA_SCHEME,
            Scheme::File => FILE_SCHEME,
            Scheme::HTTPS => HTTPS_SCHEME,
            Scheme::HTTP => HTTP_SCHEME,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Authority {
    userinfo: Option<String>,
    host: String,
    port: Option<u16>,
}

#[derive(Debug, Clone)]
pub struct URI {
    pub scheme: Scheme,
    pub authority: Option<Authority>,
    pub hostname: String,
    pub path: String,
    pub port: String,
    pub flags: Option<HashMap<String, bool>>,
}

impl URI {
    fn extract_scheme_from(uri: &String) -> (Scheme, String) {
        let scheme_regexp_lookup = format!(r"^(?<scheme>{SCHEME_REGEX}):(?<remainder>.*)");
        let scheme_regexp = Regex::new(&scheme_regexp_lookup).unwrap();

        let scheme_capture = scheme_regexp.captures(uri).expect("url not parsable");

        let scheme = Scheme::from_str(&scheme_capture["scheme"]).expect("no scheme");
        let remainder = String::from(&scheme_capture["remainder"]);

        (scheme, remainder)
    }

    pub fn parse(url: &String) -> Self {
        let (scheme, mut remainder) = Self::extract_scheme_from(url);

        match scheme {
            Scheme::HTTP | Scheme::HTTPS => {
                if remainder.starts_with("//") {
                    remainder = String::from(remainder.get(2..).unwrap_or(""))
                }

                let (mut hostname, path) = remainder.split_once('/').unwrap_or((&remainder, ""));

                let mut port: u16 = match scheme {
                    Scheme::HTTPS => 443,
                    _ => 80,
                };

                if hostname.contains(":") {
                    let split_hostname_port: Vec<&str> = hostname.split(":").collect();

                    hostname = split_hostname_port[0];
                    port = split_hostname_port[1]
                        .parse()
                        .expect("No port provided after colon");
                }

                return Self {
                    scheme,
                    authority: Some(Authority {
                        userinfo: None,
                        host: String::from(hostname),
                        port: Some(port),
                    }),
                    hostname: String::from(hostname),
                    path: format!("/{}", path),
                    port: port.to_string(),
                    flags: None,
                };
            }
            Scheme::File => {
                if remainder.starts_with("//") {
                    remainder = String::from(remainder.get(2..).unwrap_or(""))
                }
                return Self {
                    scheme,
                    authority: Some(Authority {
                        userinfo: None,
                        host: String::from(""),
                        port: None,
                    }),
                    hostname: String::from(""),
                    path: String::from(remainder),
                    port: String::from(""),
                    flags: None,
                };
            }
            Scheme::Data => {
                return Self {
                    scheme,
                    authority: None,
                    hostname: String::from(""),
                    path: String::from(remainder),
                    port: String::from(""),
                    flags: None,
                }
            }
        }
    }
}

#[cfg(test)]
mod data_scheme_tests {
    use super::Scheme;
    use super::URI;

    #[test]
    fn parses_data_scheme() {
        let url: String = String::from("data:text/html,Hellow world!");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "");
        assert_eq!(parse_url.path, "text/html,Hellow world!");
        assert_eq!(parse_url.port, "");
        assert_eq!(parse_url.scheme, Scheme::Data);
    }
}

#[cfg(test)]
mod file_scheme_tests {
    use super::Scheme;
    use super::URI;

    #[test]
    fn parses_file_absolute_scheme() {
        let url: String = String::from("file:///Users/test/main.rs");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "");
        assert_eq!(parse_url.path, "/Users/test/main.rs");
        assert_eq!(parse_url.port, "");
        assert_eq!(parse_url.scheme, Scheme::File);
    }
}

#[cfg(test)]
mod http_scheme_tests {
    use super::Scheme;
    use super::URI;

    #[test]
    fn parses_http_scheme() {
        let url: String = String::from("http://www.example.org");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "www.example.org");
        assert_eq!(parse_url.path, "/");
        assert_eq!(parse_url.port, "80");
        assert_eq!(parse_url.scheme, Scheme::HTTP);
    }

    #[test]
    fn parses_http_scheme_with_path() {
        let url: String = String::from("http://www.example.org/one");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "www.example.org");
        assert_eq!(parse_url.path, "/one");
        assert_eq!(parse_url.port, "80");
        assert_eq!(parse_url.scheme, Scheme::HTTP);
    }

    #[test]
    fn parses_http_scheme_with_port() {
        let url: String = String::from("http://www.example.org:9090");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "www.example.org");
        assert_eq!(parse_url.path, "/");
        assert_eq!(parse_url.port, "9090");
        assert_eq!(parse_url.scheme, Scheme::HTTP);
    }
}

#[cfg(test)]
mod https_scheme_tests {
    use super::Scheme;
    use super::URI;

    #[test]
    fn parses_https_scheme() {
        let url: String = String::from("https://www.example.org");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "www.example.org");
        assert_eq!(parse_url.path, "/");
        assert_eq!(parse_url.port, "443");
        assert_eq!(parse_url.scheme, Scheme::HTTPS);
    }

    #[test]
    fn parses_https_scheme_with_path() {
        let url: String = String::from("https://www.example.org/one");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "www.example.org");
        assert_eq!(parse_url.path, "/one");
        assert_eq!(parse_url.port, "443");
        assert_eq!(parse_url.scheme, Scheme::HTTPS);
    }

    #[test]
    fn parses_https_scheme_with_port() {
        let url: String = String::from("https://www.example.org:9090");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "www.example.org");
        assert_eq!(parse_url.path, "/");
        assert_eq!(parse_url.port, "9090");
        assert_eq!(parse_url.scheme, Scheme::HTTPS);
    }
}
