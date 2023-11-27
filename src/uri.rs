use regex::Regex;

#[derive(Debug, PartialEq, Clone)]
pub enum URIScheme {
    Data,
    File,
    HTTP,
    HTTPS,
    ViewSourceHTTP,
    ViewSourceHTTPS,
}

const DATA_SCHEME: &str = "data";
const FILE_SCHEME: &str = "file";
const HTTP_SCHEME: &str = "http";
const HTTPS_SCHEME: &str = "https";
const VIEWSOURCE_HTTP_SCHEME: &str = "view-source:http";
const VIEWSOURCE_HTTPS_SCHEME: &str = "view-source:https";

const HTTP_SCHEMES: [&str; 4] = [
    HTTPS_SCHEME,
    HTTP_SCHEME,
    VIEWSOURCE_HTTPS_SCHEME,
    VIEWSOURCE_HTTP_SCHEME,
];

const SCHEMES: [&str; 6] = [
    DATA_SCHEME,
    FILE_SCHEME,
    HTTP_SCHEME,
    HTTPS_SCHEME,
    VIEWSOURCE_HTTPS_SCHEME,
    VIEWSOURCE_HTTP_SCHEME,
];

impl URIScheme {
    pub fn from_str(value: &str) -> URIScheme {
        match value {
            DATA_SCHEME => URIScheme::Data,
            FILE_SCHEME => URIScheme::File,
            HTTPS_SCHEME => URIScheme::HTTPS,
            HTTP_SCHEME => URIScheme::HTTP,
            VIEWSOURCE_HTTPS_SCHEME => URIScheme::ViewSourceHTTPS,
            VIEWSOURCE_HTTP_SCHEME => URIScheme::ViewSourceHTTP,
            _other => URIScheme::HTTP,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            URIScheme::Data => DATA_SCHEME,
            URIScheme::File => FILE_SCHEME,
            URIScheme::HTTPS => HTTPS_SCHEME,
            URIScheme::HTTP => HTTP_SCHEME,
            URIScheme::ViewSourceHTTPS => VIEWSOURCE_HTTPS_SCHEME,
            URIScheme::ViewSourceHTTP => VIEWSOURCE_HTTP_SCHEME,
        }
    }
}

#[derive(Debug, Clone)]
pub struct URI {
    pub scheme: URIScheme,
    pub hostname: String,
    pub path: String,
    pub port: String,
}

impl URI {
    pub fn parse(url: &String) -> Self {
        let scheme_regexp = Regex::new(r"^(http|https|file|data|view-source):/?/?").unwrap();

        let mut url_copy = String::from(url.clone());
        let mut scheme = String::from(HTTP_SCHEME);

        if scheme_regexp.is_match(&url_copy) {
            let mut scheme_url = Vec::new();
            let mut scheme_found = false;
            let mut prev = String::new();

            let spliter = ":";

            for scheme_part in url_copy.split(spliter) {
                if prev.len() > 0 {
                    prev = prev + spliter + scheme_part;
                } else {
                    prev += scheme_part
                }
                if scheme_found == false {
                    if SCHEMES.contains(&prev.as_str()) {
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
                HTTP_SCHEME,
                HTTPS_SCHEME,
                FILE_SCHEME,
                VIEWSOURCE_HTTP_SCHEME,
                VIEWSOURCE_HTTPS_SCHEME,
            ]
            .contains(&scheme.as_str())
                && url_copy.starts_with("//")
            {
                url_copy = String::from(url_copy.get(2..).unwrap_or(""))
            }
        }

        if HTTP_SCHEMES.contains(&scheme.as_str()) {
            let (mut hostname, path) = url_copy.split_once('/').unwrap_or((&url_copy, ""));

            let mut port = if scheme.contains(HTTPS_SCHEME) {
                "443"
            } else {
                "80"
            };

            if hostname.contains(":") {
                let split_hostname_port: Vec<&str> = hostname.split(":").collect();

                hostname = split_hostname_port[0];
                port = split_hostname_port[1];
            }

            Self {
                scheme: URIScheme::from_str(&scheme),
                hostname: String::from(hostname),
                path: format!("/{}", path),
                port: String::from(port),
            }
        } else if scheme == FILE_SCHEME {
            Self {
                scheme: URIScheme::from_str(&scheme),
                hostname: String::from(""),
                path: String::from(url_copy),
                port: String::from(""),
            }
        } else if scheme == DATA_SCHEME {
            Self {
                scheme: URIScheme::from_str(&scheme),
                hostname: String::from(""),
                path: String::from(url_copy),
                port: String::from(""),
            }
        } else {
            Self {
                scheme: URIScheme::from_str(&scheme),
                hostname: String::from(""),
                path: String::from(url_copy),
                port: String::from(""),
            }
        }
    }
}

#[cfg(test)]
mod data_scheme_tests {
    use super::URIScheme;
    use super::URI;

    #[test]
    fn parses_data_scheme() {
        let url: String = String::from("data:text/html,Hellow world!");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "");
        assert_eq!(parse_url.path, "text/html,Hellow world!");
        assert_eq!(parse_url.port, "");
        assert_eq!(parse_url.scheme, URIScheme::Data);
    }
}

#[cfg(test)]
mod file_scheme_tests {
    use super::URIScheme;
    use super::URI;

    #[test]
    fn parses_file_absolute_scheme() {
        let url: String = String::from("file:///Users/test/main.rs");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "");
        assert_eq!(parse_url.path, "/Users/test/main.rs");
        assert_eq!(parse_url.port, "");
        assert_eq!(parse_url.scheme, URIScheme::File);
    }

    #[test]
    fn parses_file_relative_scheme() {
        let url: String = String::from("file://main.rs");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "");
        assert_eq!(parse_url.path, "main.rs");
        assert_eq!(parse_url.port, "");
        assert_eq!(parse_url.scheme, URIScheme::File);
    }
}

#[cfg(test)]
mod http_scheme_tests {
    use super::URIScheme;
    use super::URI;

    #[test]
    fn parses_http_scheme() {
        let url: String = String::from("http://www.example.org");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "www.example.org");
        assert_eq!(parse_url.path, "/");
        assert_eq!(parse_url.port, "80");
        assert_eq!(parse_url.scheme, URIScheme::HTTP);
    }

    #[test]
    fn parses_http_scheme_with_path() {
        let url: String = String::from("http://www.example.org/one");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "www.example.org");
        assert_eq!(parse_url.path, "/one");
        assert_eq!(parse_url.port, "80");
        assert_eq!(parse_url.scheme, URIScheme::HTTP);
    }

    #[test]
    fn parses_http_scheme_with_port() {
        let url: String = String::from("http://www.example.org:9090");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "www.example.org");
        assert_eq!(parse_url.path, "/");
        assert_eq!(parse_url.port, "9090");
        assert_eq!(parse_url.scheme, URIScheme::HTTP);
    }
}

#[cfg(test)]
mod https_scheme_tests {
    use super::URIScheme;
    use super::URI;

    #[test]
    fn parses_https_scheme() {
        let url: String = String::from("https://www.example.org");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "www.example.org");
        assert_eq!(parse_url.path, "/");
        assert_eq!(parse_url.port, "443");
        assert_eq!(parse_url.scheme, URIScheme::HTTPS);
    }

    #[test]
    fn parses_https_scheme_with_path() {
        let url: String = String::from("https://www.example.org/one");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "www.example.org");
        assert_eq!(parse_url.path, "/one");
        assert_eq!(parse_url.port, "443");
        assert_eq!(parse_url.scheme, URIScheme::HTTPS);
    }

    #[test]
    fn parses_https_scheme_with_port() {
        let url: String = String::from("https://www.example.org:9090");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "www.example.org");
        assert_eq!(parse_url.path, "/");
        assert_eq!(parse_url.port, "9090");
        assert_eq!(parse_url.scheme, URIScheme::HTTPS);
    }
}

#[cfg(test)]
mod view_source_http_scheme_tests {
    use super::URIScheme;
    use super::URI;

    #[test]
    fn parses_http_view_source_scheme() {
        let url: String = String::from("view-source:http://www.example.org");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "www.example.org");
        assert_eq!(parse_url.path, "/");
        assert_eq!(parse_url.port, "80");
        assert_eq!(parse_url.scheme, URIScheme::ViewSourceHTTP);
    }

    #[test]
    fn parses_http_view_source_scheme_with_path() {
        let url: String = String::from("view-source:http://www.example.org/one");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "www.example.org");
        assert_eq!(parse_url.path, "/one");
        assert_eq!(parse_url.port, "80");
        assert_eq!(parse_url.scheme, URIScheme::ViewSourceHTTP);
    }

    #[test]
    fn parses_http_view_source_scheme_with_port() {
        let url: String = String::from("view-source:http://www.example.org:9090");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "www.example.org");
        assert_eq!(parse_url.path, "/");
        assert_eq!(parse_url.port, "9090");
        assert_eq!(parse_url.scheme, URIScheme::ViewSourceHTTP);
    }
}

#[cfg(test)]
mod view_source_https_scheme_tests {
    use super::URIScheme;
    use super::URI;

    #[test]
    fn parses_https_view_source_scheme() {
        let url: String = String::from("view-source:https://www.example.org");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "www.example.org");
        assert_eq!(parse_url.path, "/");
        assert_eq!(parse_url.port, "443");
        assert_eq!(parse_url.scheme, URIScheme::ViewSourceHTTPS);
    }

    #[test]
    fn parses_https_view_source_scheme_with_path() {
        let url: String = String::from("view-source:https://www.example.org/one");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "www.example.org");
        assert_eq!(parse_url.path, "/one");
        assert_eq!(parse_url.port, "443");
        assert_eq!(parse_url.scheme, URIScheme::ViewSourceHTTPS);
    }

    #[test]
    fn parses_https_view_source_scheme_with_port() {
        let url: String = String::from("view-source:https://www.example.org:9090");
        let parse_url = URI::parse(&url);

        assert_eq!(parse_url.hostname, "www.example.org");
        assert_eq!(parse_url.path, "/");
        assert_eq!(parse_url.port, "9090");
        assert_eq!(parse_url.scheme, URIScheme::ViewSourceHTTPS);
    }
}
