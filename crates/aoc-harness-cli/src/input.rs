use crate::credentials::{self, Session};
use reqwest::{blocking::Client, header, redirect::Policy};
use std::{
    io::{Read, Write},
    path::Path,
    time::Duration,
};

const BODY_LIMIT: u64 = 16 * 1024 * 1024;
const USER_AGENT: &str = concat!(
    "aoc-harness/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/pedantic79/aoc-harness)"
);

#[derive(Debug)]
struct DownloadError {
    message: String,
    stop: bool,
}

impl DownloadError {
    fn ordinary(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            stop: false,
        }
    }
    fn stop(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            stop: true,
        }
    }
}

trait Transport {
    fn fetch(&mut self, year: u16, day: u8, session: &Session) -> Result<Vec<u8>, DownloadError>;
}

#[derive(Default)]
struct HttpTransport {
    client: Option<Client>,
    #[cfg(test)]
    endpoint: Option<String>,
}

impl HttpTransport {
    fn client(&mut self) -> Result<&Client, DownloadError> {
        if self.client.is_none() {
            self.client = Some(
                Client::builder()
                    .redirect(Policy::none())
                    .connect_timeout(Duration::from_secs(10))
                    .timeout(Duration::from_secs(60))
                    .retry(reqwest::retry::never())
                    .user_agent(USER_AGENT)
                    .build()
                    .map_err(|_| DownloadError::ordinary("initialize HTTP transport failed"))?,
            );
        }
        Ok(self.client.as_ref().unwrap())
    }
}

impl Transport for HttpTransport {
    fn fetch(&mut self, year: u16, day: u8, session: &Session) -> Result<Vec<u8>, DownloadError> {
        let mut cookie = header::HeaderValue::from_str(&format!("session={}", session.token))
            .map_err(|_| DownloadError::stop("invalid session header"))?;
        cookie.set_sensitive(true);
        let endpoint = "https://adventofcode.com";
        #[cfg(test)]
        let endpoint = self.endpoint.as_deref().unwrap_or(endpoint);
        let url = format!("{endpoint}/{year}/day/{day}/input");
        let response = self
            .client()?
            .get(url)
            .header(header::COOKIE, cookie)
            .send()
            .map_err(|e| {
                DownloadError::ordinary(if e.is_timeout() {
                    "input request timed out"
                } else {
                    "input transport failed"
                })
            })?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let retry_after = response
            .headers()
            .get(header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok());
        check_status(status, retry_after)?;
        if response.content_length().is_some_and(|n| n > BODY_LIMIT) {
            return Err(DownloadError::ordinary("input exceeds the 16 MiB limit"));
        }
        let mut body = Vec::new();
        response
            .take(BODY_LIMIT + 1)
            .read_to_end(&mut body)
            .map_err(|_| DownloadError::ordinary("read input response failed or timed out"))?;
        validate_body(&body, &content_type)?;
        Ok(body)
    }
}

fn check_status(status: u16, retry_after: Option<&str>) -> Result<(), DownloadError> {
    match status {
        200 => Ok(()),
        300..=399 | 400 | 401 | 403 => Err(DownloadError::stop(
            "authentication failed or login redirect; check the selected session",
        )),
        404 => Err(DownloadError::ordinary("input unavailable (HTTP 404)")),
        429 => {
            let retry = retry_after
                .filter(|v| {
                    v.len() <= 100
                        && (v.bytes().all(|b| b.is_ascii_digit())
                            || chrono::DateTime::parse_from_rfc2822(v).is_ok())
                })
                .map(|v| format!("; Retry-After: {v}"))
                .unwrap_or_default();
            Err(DownloadError::stop(format!(
                "rate limited (HTTP 429){retry}"
            )))
        }
        _ => Err(DownloadError::ordinary(format!(
            "input HTTP failure ({status})"
        ))),
    }
}

fn validate_body(body: &[u8], content_type: &str) -> Result<(), DownloadError> {
    if body.len() as u64 > BODY_LIMIT {
        return Err(DownloadError::ordinary("input exceeds the 16 MiB limit"));
    }
    let text = std::str::from_utf8(body)
        .map_err(|_| DownloadError::ordinary("input response is not UTF-8"))?;
    let start = text.trim_start().to_ascii_lowercase();
    if content_type.to_ascii_lowercase().contains("html")
        || start.starts_with("<!doctype html")
        || start.starts_with("<html")
        || start.contains("please log in")
    {
        return Err(DownloadError::stop(
            "authentication failed: received HTML/login content",
        ));
    }
    if !content_type.is_empty() && !content_type.to_ascii_lowercase().starts_with("text/") {
        return Err(DownloadError::ordinary("input response is not text"));
    }
    Ok(())
}

fn cached(manifest: &Path, year: u16, day: u8) -> Result<Option<std::path::PathBuf>, String> {
    let path =
        aoc_harness::input_path(manifest, year, day).map_err(|e| format!("input path: {e}"))?;
    match std::fs::read_to_string(&path) {
        Ok(_) => Ok(Some(path)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            match std::fs::symlink_metadata(&path) {
                Err(missing) if missing.kind() == std::io::ErrorKind::NotFound => Ok(None),
                _ => Err(format!("read existing input {}: {e}", path.display())),
            }
        }
        Err(e) => Err(format!("read cached input {}: {e}", path.display())),
    }
}

fn publish(manifest: &Path, year: u16, day: u8, body: &[u8]) -> Result<bool, String> {
    let path = manifest.join(format!("input/{year}/day{day}.txt"));
    let directory = path.parent().unwrap();
    std::fs::create_dir_all(directory)
        .map_err(|e| format!("create {}: {e}", directory.display()))?;
    let mut temporary = tempfile::NamedTempFile::new_in(directory)
        .map_err(|e| format!("create temporary input: {e}"))?;
    temporary
        .write_all(body)
        .map_err(|e| format!("write temporary input: {e}"))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|e| format!("sync temporary input: {e}"))?;

    // Another process may have populated either supported spelling meanwhile.
    if cached(manifest, year, day)?.is_some() {
        return Ok(false);
    }
    match temporary.persist_noclobber(&path) {
        Ok(_) => Ok(true),
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
            cached(manifest, year, day)?.ok_or("concurrent input disappeared")?;
            Ok(false)
        }
        Err(e) => Err(format!("publish {}: {}", path.display(), e.error)),
    }
}

pub(crate) fn acquire(
    manifest: &Path,
    year: u16,
    days: &[u8],
    offline: bool,
    session_file: Option<&Path>,
) -> Result<(), String> {
    acquire_with(
        manifest,
        year,
        days,
        offline,
        || credentials::resolve(session_file),
        &mut HttpTransport::default(),
    )
}

fn acquire_with(
    manifest: &Path,
    year: u16,
    days: &[u8],
    offline: bool,
    mut resolve: impl FnMut() -> Result<Session, String>,
    transport: &mut impl Transport,
) -> Result<(), String> {
    let mut session = None;
    let (mut cached_count, mut downloaded, mut failed) = (0, 0, 0);
    let mut errors = Vec::new();
    for &day in days {
        let result = (|| {
            if let Some(path) = cached(manifest, year, day).map_err(|e| (e, false))? {
                println!("Cached: {}", path.display());
                return Ok(false);
            }
            if offline {
                return Err((
                    "input is missing and --offline prevents downloading".into(),
                    false,
                ));
            }
            if session.is_none() {
                session = Some(resolve().map_err(|e| (e, true))?);
            }
            let body = transport
                .fetch(year, day, session.as_ref().unwrap())
                .map_err(|e| (e.message, e.stop));
            let body = match body {
                Ok(body) => body,
                Err((message, stop)) => return Err((message, stop)),
            };
            validate_body(&body, "text/plain").map_err(|e| (e.message, e.stop))?;
            let created = publish(manifest, year, day, &body).map_err(|e| (e, false))?;
            println!(
                "{}: {}",
                if created { "Downloaded" } else { "Cached" },
                manifest
                    .join(format!("input/{year}/day{day}.txt"))
                    .display()
            );
            Ok(created)
        })();
        match result {
            Ok(true) => downloaded += 1,
            Ok(false) => cached_count += 1,
            Err((message, stop)) => {
                failed += 1;
                let message = format!("{year} day {day}: {message}");
                eprintln!("Failed: {message}");
                errors.push(message);
                if stop {
                    break;
                }
            }
        }
    }
    println!("Inputs: {cached_count} cached, {downloaded} downloaded, {failed} failed");
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

pub(crate) fn validate_description(
    bytes: &[u8],
    manifest: &Path,
    action: &str,
) -> Result<(u16, Vec<u8>), String> {
    let invalid = || {
        "invalid or unsupported input-plan protocol; upgrade the consumer's aoc-harness runtime"
            .to_owned()
    };
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if value["protocol"] != 1
        || !value["capabilities"]
            .as_array()
            .is_some_and(|c| c.iter().any(|v| v == "input-plan-v1"))
        || value["action"] != action
    {
        return Err(invalid());
    }
    let reported = value["manifest_dir"].as_str().ok_or_else(invalid)?;
    if std::fs::canonicalize(reported).map_err(|_| invalid())?
        != manifest.canonicalize().map_err(|_| invalid())?
    {
        return Err("input plan refers to a different consumer manifest directory".into());
    }
    let year = value["year"]
        .as_u64()
        .filter(|y| (2015..=u64::from(u16::MAX)).contains(y))
        .ok_or_else(invalid)? as u16;
    let mut selected_days = Vec::new();
    for field in ["selection", "defaults"] {
        let selection = value[field].as_array().ok_or_else(invalid)?;
        if field == "selection" && selection.is_empty() {
            return Err(invalid());
        }
        for item in selection {
            let day = item["day"]
                .as_u64()
                .filter(|d| (1..=25).contains(d))
                .ok_or_else(invalid)? as u8;
            if !item["part"].as_u64().is_some_and(|p| (1..=2).contains(&p))
                || item["name"].as_str().is_none()
            {
                return Err(invalid());
            }
            selected_days.push(day);
        }
    }
    selected_days.sort_unstable();
    selected_days.dedup();
    let days: Vec<_> = value["required_input_days"]
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .map(|v| {
            v.as_u64()
                .filter(|d| (1..=25).contains(d))
                .map(|d| d as u8)
                .ok_or_else(invalid)
        })
        .collect::<Result<_, _>>()?;
    if days != selected_days {
        return Err(invalid());
    }
    Ok((year, days))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeTransport {
        calls: Vec<u8>,
        responses: std::collections::VecDeque<Result<Vec<u8>, DownloadError>>,
    }

    impl Transport for FakeTransport {
        fn fetch(
            &mut self,
            year: u16,
            day: u8,
            session: &Session,
        ) -> Result<Vec<u8>, DownloadError> {
            assert_eq!(year, 2025);
            assert_eq!(session.token, "abcd");
            self.calls.push(day);
            self.responses.pop_front().expect("unexpected HTTP request")
        }
    }

    fn session() -> Result<Session, String> {
        Ok(Session {
            token: "abcd".into(),
            source: "synthetic".into(),
        })
    }

    #[test]
    fn cache_newlines_offline_and_concurrent_writer() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("input/2025");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("day01.txt"), "").unwrap();
        let mut transport = FakeTransport {
            calls: Vec::new(),
            responses: [Ok(b"personal-looking synthetic input\r\n\n".to_vec())].into(),
        };

        acquire_with(
            temp.path(),
            2025,
            &[1],
            true,
            || panic!("cached inputs need no credential"),
            &mut transport,
        )
        .unwrap();
        acquire_with(temp.path(), 2025, &[1, 2], false, session, &mut transport).unwrap();

        assert_eq!(transport.calls, [2]);
        assert_eq!(
            std::fs::read(directory.join("day2.txt")).unwrap(),
            b"personal-looking synthetic input\r\n\n"
        );
        assert!(!directory.join("day1.txt").exists());
        assert!(!publish(temp.path(), 2025, 2, b"losing writer").unwrap());
        assert!(acquire_with(
            temp.path(),
            2025,
            &[3],
            true,
            || panic!("offline"),
            &mut transport
        )
        .is_err());
        std::fs::write(directory.join("day3.txt"), [255]).unwrap();
        assert!(acquire_with(
            temp.path(),
            2025,
            &[3],
            false,
            || panic!("bad cache is not a miss"),
            &mut transport
        )
        .is_err());
    }

    #[test]
    fn failures_leave_no_file_and_stop_on_authentication_or_rate_limit() {
        let temp = tempfile::tempdir().unwrap();
        let mut transport = FakeTransport {
            calls: Vec::new(),
            responses: [
                Err(DownloadError::ordinary("unavailable HTTP 404")),
                Ok(b"synthetic\n".to_vec()),
                Err(DownloadError::stop("HTTP 429")),
            ]
            .into(),
        };

        let error = acquire_with(
            temp.path(),
            2025,
            &[1, 2, 3, 4],
            false,
            session,
            &mut transport,
        )
        .unwrap_err();

        assert_eq!(transport.calls, [1, 2, 3]);
        assert!(error.contains("404") && error.contains("429"));
        assert!(!temp.path().join("input/2025/day1.txt").exists());
        assert!(temp.path().join("input/2025/day2.txt").exists());
        assert!(!temp.path().join("input/2025/day3.txt").exists());
        for code in [301, 302, 307, 400, 401, 403, 429] {
            assert!(check_status(code, None).unwrap_err().stop);
        }
        for code in [404, 500] {
            assert!(!check_status(code, None).unwrap_err().stop);
        }
        assert!(check_status(429, Some("120"))
            .unwrap_err()
            .message
            .contains("Retry-After: 120"));
        assert!(!check_status(429, Some("secret-value\n"))
            .unwrap_err()
            .message
            .contains("secret-value"));
        assert!(validate_body(b"<html>login</html>", "text/plain").is_err());
        assert!(validate_body(b"anything", "text/html").is_err());
        assert!(validate_body(&[255], "text/plain").is_err());
        assert!(validate_body(&vec![b'a'; BODY_LIMIT as usize + 1], "text/plain").is_err());
        assert!(validate_body(b"ok\n", "text/plain").is_ok());
    }

    #[test]
    fn protocol_rejects_malformed_unsupported_and_wrong_consumer_data() {
        let temp = tempfile::tempdir().unwrap();
        let value = serde_json::json!({ "protocol":1,"capabilities":["input-plan-v1"],
            "manifest_dir":temp.path(),"year":2025,"action":"bench",
            "selection":[{"day":2,"part":1,"name":"variant"}],
            "defaults":[{"day":2,"part":1,"name":"default"}],"required_input_days":[2] });

        assert_eq!(
            validate_description(value.to_string().as_bytes(), temp.path(), "bench").unwrap(),
            (2025, vec![2])
        );
        for field in [
            "protocol",
            "capabilities",
            "selection",
            "defaults",
            "year",
            "manifest_dir",
            "action",
            "required_input_days",
        ] {
            let mut invalid = value.clone();
            invalid[field] = serde_json::Value::Null;

            assert!(
                validate_description(invalid.to_string().as_bytes(), temp.path(), "bench").is_err(),
                "{field}"
            );
        }
        assert!(validate_description(b"not JSON", temp.path(), "run").is_err());
    }
}

#[cfg(test)]
mod http_tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader},
        net::TcpListener,
        sync::mpsc,
        thread,
    };

    #[test]
    fn transport_sends_expected_headers_disables_redirects_and_bounds_reads() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (requests, received) = mpsc::channel();
        let server = thread::spawn(move || {
            for response in [
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 7\r\nConnection: close\r\n\r\nalpha\r\n",
                "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/credential-leak\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 16777217\r\nConnection: close\r\n\r\n",
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 120\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            ] {
                let (stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut reader = BufReader::new(stream);
                let mut request = String::new();
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" { break; }
                    request.push_str(&line);
                }
                requests.send(request).unwrap();
                reader.get_mut().write_all(response.as_bytes()).unwrap();
            }
        });
        let client = Client::builder()
            .redirect(Policy::none())
            .retry(reqwest::retry::never())
            .timeout(Duration::from_secs(2))
            .no_proxy()
            .user_agent(USER_AGENT)
            .build()
            .unwrap();
        let mut transport = HttpTransport {
            client: Some(client),
            endpoint: Some(endpoint),
        };
        let session = Session {
            token: "abcdef".into(),
            source: "synthetic".into(),
        };

        assert_eq!(transport.fetch(2025, 1, &session).unwrap(), b"alpha\r\n");
        assert!(transport.fetch(2025, 2, &session).unwrap_err().stop);
        assert!(transport
            .fetch(2025, 3, &session)
            .unwrap_err()
            .message
            .contains("16 MiB"));
        assert!(transport
            .fetch(2025, 4, &session)
            .unwrap_err()
            .message
            .contains("404"));
        assert!(transport
            .fetch(2025, 5, &session)
            .unwrap_err()
            .message
            .contains("Retry-After: 120"));
        server.join().unwrap();
        let requests: Vec<_> = received.try_iter().collect();

        assert_eq!(requests.len(), 5);
        assert!(requests[0].starts_with("GET /2025/day/1/input HTTP/1.1"));
        for request in requests {
            assert!(request.to_lowercase().contains("cookie: session=abcdef"));
            assert!(request.contains(USER_AGENT));
        }
    }

    #[test]
    fn transport_timeout_is_safe_and_does_not_publish_an_input() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(100));
        });
        let client = Client::builder()
            .redirect(Policy::none())
            .no_proxy()
            .timeout(Duration::from_millis(20))
            .build()
            .unwrap();
        let mut transport = HttpTransport {
            client: Some(client),
            endpoint: Some(endpoint),
        };
        let session = Session {
            token: "abcdef".into(),
            source: "synthetic".into(),
        };

        let error = transport.fetch(2025, 1, &session).unwrap_err();
        server.join().unwrap();

        assert!(error.message.contains("timed out"));
        assert!(!error.message.contains("abcdef"));
    }
}
