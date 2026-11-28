use std::fs;
use std::io;
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

pub struct PreviewServer {
	server: Server,
	root: PathBuf,
	addr: SocketAddr,
}

enum Resolved {
	File(PathBuf),
	Redirect(String),
	NotFound,
}

impl PreviewServer {
	pub fn bind(root: PathBuf, host: &str, port: u16) -> Result<Self> {
		let root = root
			.canonicalize()
			.with_context(|| format!("failed to resolve {}", root.display()))?;
		if !root.is_dir() {
			bail!("preview root is not a directory: {}", root.display());
		}

		let address = format!("{host}:{port}");
		let server = Server::http(&address).map_err(|error| {
			anyhow::anyhow!("failed to start preview server on {address}: {error}")
		})?;
		let addr = server
			.server_addr()
			.to_ip()
			.context("preview server did not bind to a TCP address")?;

		Ok(Self { server, root, addr })
	}

	pub fn addr(&self) -> SocketAddr {
		self.addr
	}

	pub fn run(self) -> Result<()> {
		self.run_until(Arc::new(AtomicBool::new(false)))
	}

	pub fn run_until(self, stop: Arc<AtomicBool>) -> Result<()> {
		while !stop.load(Ordering::Relaxed) {
			if let Some(request) = self
				.server
				.recv_timeout(Duration::from_millis(200))
				.context("preview server failed")?
				&& let Err(error) = handle(&self.root, request)
			{
				eprintln!("preview: request failed: {error:#}");
			}
		}

		Ok(())
	}
}

fn handle(root: &Path, request: Request) -> Result<()> {
	if request.method() != &Method::Get && request.method() != &Method::Head {
		let response = Response::empty(StatusCode(405))
			.with_header(header("Allow", "GET, HEAD"))
			.with_header(header("Cache-Control", "no-store"));
		request
			.respond(response)
			.context("failed to send 405 response")?;
		return Ok(());
	}

	let url = request.url().to_string();
	let resolved = match resolve_request(root, &url) {
		Ok(resolved) => resolved,
		Err(error) => {
			let response = text_response(StatusCode(400), &format!("400 Bad Request\n\n{error}\n"));
			request
				.respond(response)
				.context("failed to send 400 response")?;
			return Ok(());
		}
	};

	match resolved {
		Resolved::File(path) => serve_file(request, &path),
		Resolved::Redirect(location) => {
			let response = Response::empty(StatusCode(301))
				.with_header(header("Location", &location))
				.with_header(header("Cache-Control", "no-store"));
			request
				.respond(response)
				.context("failed to send redirect")?;
			Ok(())
		}
		Resolved::NotFound => {
			let data = match fs::read(root.join("404.html")) {
				Ok(data) => data,
				Err(error) => {
					let (status, body) = match error.kind() {
						io::ErrorKind::NotFound => (StatusCode(404), "404 Not Found\n".to_string()),
						io::ErrorKind::PermissionDenied => {
							(StatusCode(403), "403 Forbidden\n".to_string())
						}
						_ => (
							StatusCode(500),
							format!("500 Internal Server Error\n\n{error}\n"),
						),
					};
					let response = text_response(status, &body);
					request
						.respond(response)
						.context("failed to send error response")?;
					return Ok(());
				}
			};
			let response = Response::from_data(data)
				.with_status_code(404)
				.with_header(header("Content-Type", get_content_type("html")))
				.with_header(header("Cache-Control", "no-store"));
			request
				.respond(response)
				.context("failed to send response")?;
			Ok(())
		}
	}
}

fn serve_file(request: Request, path: &Path) -> Result<()> {
	let data = match fs::read(path) {
		Ok(data) => data,
		Err(error) => {
			let (status, body) = match error.kind() {
				io::ErrorKind::NotFound => (StatusCode(404), "404 Not Found\n".to_string()),
				io::ErrorKind::PermissionDenied => (StatusCode(403), "403 Forbidden\n".to_string()),
				_ => (
					StatusCode(500),
					format!("500 Internal Server Error\n\n{error}\n"),
				),
			};
			let response = text_response(status, &body);
			request
				.respond(response)
				.context("failed to send error response")?;
			return Ok(());
		}
	};

	let response = Response::from_data(data)
		.with_header(header("Content-Type", content_type(path)))
		.with_header(header("Cache-Control", "no-store"));
	request
		.respond(response)
		.context("failed to send response")?;
	Ok(())
}

fn resolve_request(root: &Path, url: &str) -> Result<Resolved> {
	let (raw_path, query) = match url.split_once('?') {
		Some((path, query)) => (path, Some(query)),
		None => (url, None),
	};

	if !raw_path.starts_with('/') {
		bail!("request path must start with '/'");
	}

	let decoded = percent_decode(raw_path)?;
	if decoded.contains('\0') {
		bail!("request path contains a NUL byte");
	}

	let mut relative = PathBuf::new();
	for component in Path::new(&decoded).components() {
		match component {
			Component::RootDir | Component::CurDir => {}
			Component::Normal(name) => relative.push(name),
			_ => bail!("request path contains an invalid component"),
		}
	}

	let mut path = root.join(&relative);
	if path.is_dir() {
		if !raw_path.ends_with('/') {
			let mut location = raw_path.to_string();
			if let Some(query) = query {
				location.push('?');
				location.push_str(query);
			}
			location.push('/');
			return Ok(Resolved::Redirect(location));
		}
		path = path.join("index.html");
	}

	if !path.is_file() {
		return Ok(Resolved::NotFound);
	}

	let canonical = path
		.canonicalize()
		.with_context(|| format!("failed to resolve {}", path.display()))?;
	if !canonical.starts_with(root) {
		bail!("request path resolves outside the preview root");
	}

	Ok(Resolved::File(canonical))
}

fn percent_decode(value: &str) -> Result<String> {
	let bytes = value.as_bytes();
	let mut decoded = Vec::with_capacity(bytes.len());
	let mut index = 0;

	while index < bytes.len() {
		if bytes[index] == b'%' {
			if index + 2 >= bytes.len() {
				bail!("invalid percent-encoding in request path");
			}
			let high = hex_value(bytes[index + 1])?;
			let low = hex_value(bytes[index + 2])?;
			decoded.push(high << 4 | low);
			index += 3;
		} else {
			decoded.push(bytes[index]);
			index += 1;
		}
	}

	String::from_utf8(decoded).context("request path is not valid UTF-8")
}

fn hex_value(byte: u8) -> Result<u8> {
	match byte {
		b'0'..=b'9' => Ok(byte - b'0'),
		b'a'..=b'f' => Ok(byte - b'a' + 10),
		b'A'..=b'F' => Ok(byte - b'A' + 10),
		_ => bail!("invalid percent-encoding in request path"),
	}
}

fn content_type(path: &Path) -> &'static str {
	let extension = path
		.extension()
		.and_then(|extension| extension.to_str())
		.unwrap_or_default()
		.to_ascii_lowercase();

	get_content_type(&extension)
}

fn get_content_type(ext: &str) -> &'static str {
	match ext {
		"html" | "htm" => "text/html; charset=utf-8",
		"css" => "text/css; charset=utf-8",
		"js" | "mjs" => "text/javascript; charset=utf-8",
		"json" | "map" => "application/json; charset=utf-8",
		"xml" => "application/xml; charset=utf-8",
		"txt" | "md" => "text/plain; charset=utf-8",
		"svg" => "image/svg+xml",
		"png" => "image/png",
		"jpg" | "jpeg" => "image/jpeg",
		"gif" => "image/gif",
		"webp" => "image/webp",
		"avif" => "image/avif",
		"ico" => "image/x-icon",
		"pdf" => "application/pdf",
		"wasm" => "application/wasm",
		"woff" => "font/woff",
		"woff2" => "font/woff2",
		"ttf" => "font/ttf",
		"otf" => "font/otf",
		"mp4" => "video/mp4",
		"webm" => "video/webm",
		"mp3" => "audio/mpeg",
		"ogg" => "audio/ogg",
		_ => "application/octet-stream",
	}
}

fn text_response(status: StatusCode, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
	Response::from_data(body.as_bytes().to_vec())
		.with_status_code(status)
		.with_header(header("Content-Type", "text/plain; charset=utf-8"))
		.with_header(header("Cache-Control", "no-store"))
}

fn header(name: &str, value: &str) -> Header {
	Header::from_bytes(name.as_bytes(), value.as_bytes())
		.unwrap_or_else(|()| panic!("invalid HTTP header: {name}: {value}"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn percent_decode_handles_escapes_and_utf8() {
		assert_eq!(percent_decode("/hello%20world").unwrap(), "/hello world");
		assert_eq!(percent_decode("/caf%C3%A9").unwrap(), "/café");
		assert!(percent_decode("/%").is_err());
		assert!(percent_decode("/%zz").is_err());
	}

	#[test]
	fn resolve_request_serves_index_and_redirects_directories() {
		let temp = tempfile::tempdir().unwrap();
		let root = temp.path().canonicalize().unwrap();
		fs::create_dir(root.join("post")).unwrap();
		fs::write(root.join("index.html"), "home").unwrap();
		fs::write(root.join("post/index.html"), "post").unwrap();

		match resolve_request(&root, "/").unwrap() {
			Resolved::File(path) => assert_eq!(path, root.join("index.html")),
			_ => panic!("expected file"),
		}

		match resolve_request(&root, "/post").unwrap() {
			Resolved::Redirect(location) => assert_eq!(location, "/post/"),
			_ => panic!("expected redirect"),
		}

		match resolve_request(&root, "/post/").unwrap() {
			Resolved::File(path) => assert_eq!(path, root.join("post/index.html")),
			_ => panic!("expected file"),
		}
	}

	#[test]
	fn resolve_request_rejects_parent_components() {
		let temp = tempfile::tempdir().unwrap();
		let root = temp.path().canonicalize().unwrap();

		assert!(resolve_request(&root, "/../etc/passwd").is_err());
		assert!(resolve_request(&root, "/%2e%2e/etc/passwd").is_err());
	}

	#[test]
	fn content_type_maps_common_extensions() {
		assert_eq!(
			content_type(Path::new("index.html")),
			"text/html; charset=utf-8"
		);
		assert_eq!(
			content_type(Path::new("style.css")),
			"text/css; charset=utf-8"
		);
		assert_eq!(content_type(Path::new("image.png")), "image/png");
		assert_eq!(
			content_type(Path::new("file.unknown")),
			"application/octet-stream"
		);
	}
}
