//! Tests for serving a Root and for diagnosing a navigation that did not
//! arrive: which file a request reaches, which requests are refused, the
//! content type a file is served with, and what Dry says about an address it
//! could not load.
//!
//! Everything here runs against a temporary directory on disk or against the
//! loopback interface. Nothing opens a window or runs an event loop.

use super::*;
use std::{
  net::TcpListener,
  sync::atomic::AtomicU32,
  time::{SystemTime, UNIX_EPOCH},
};

/// A throwaway directory tree, removed when the test drops it.
struct Fixture {
  directory: PathBuf,
}

impl Fixture {
  fn new() -> Self {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let unique = format!(
      "dry-root-{}-{}",
      SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock should be after the epoch")
        .as_nanos(),
      COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let directory = std::env::temp_dir().join(unique);
    fs::create_dir_all(&directory).expect("the fixture directory should be created");
    Fixture { directory }
  }

  fn write(&self, relative: &str, contents: &str) {
    let path = self.directory.join(relative);
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent).expect("the parent directory should be created");
    }
    fs::write(path, contents).expect("the fixture file should be written");
  }

  fn root(&self) -> Root {
    Root::new(self.directory.clone())
  }
}

impl Drop for Fixture {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.directory);
  }
}

/// The body a request came back with, as text.
fn body_of(response: &Response<Cow<'static, [u8]>>) -> String {
  String::from_utf8_lossy(response.body()).into_owned()
}

/// The content type a request came back with.
fn content_type_of(response: &Response<Cow<'static, [u8]>>) -> String {
  response
    .headers()
    .get(CONTENT_TYPE)
    .expect("every response should carry a content type")
    .to_str()
    .expect("the content type should be text")
    .to_owned()
}

#[test]
fn a_root_serves_a_file_beneath_it() {
  let fixture = Fixture::new();
  fixture.write("index.html", "<h1>Root</h1>");
  fixture.write("assets/app.js", "export const x = 1;");

  let root = fixture.root();

  let page = root.serve("/index.html");
  assert_eq!(page.status(), 200);
  assert_eq!(body_of(&page), "<h1>Root</h1>");

  let script = root.serve("/assets/app.js");
  assert_eq!(script.status(), 200);
  assert_eq!(body_of(&script), "export const x = 1;");
}

#[test]
fn a_request_for_a_directory_serves_its_index() {
  let fixture = Fixture::new();
  fixture.write("index.html", "<h1>Root</h1>");
  fixture.write("nested/index.html", "<h1>Nested</h1>");

  let root = fixture.root();

  assert_eq!(body_of(&root.serve("/")), "<h1>Root</h1>");
  assert_eq!(body_of(&root.serve("/nested")), "<h1>Nested</h1>");
  assert_eq!(body_of(&root.serve("/nested/")), "<h1>Nested</h1>");
}

#[test]
fn a_percent_escaped_name_reaches_its_file() {
  let fixture = Fixture::new();
  fixture.write("a file.css", "body { color: red; }");

  let response = fixture.root().serve("/a%20file.css");

  assert_eq!(response.status(), 200);
  assert_eq!(body_of(&response), "body { color: red; }");
}

#[test]
fn a_missing_file_is_a_404() {
  let fixture = Fixture::new();
  fixture.write("index.html", "<h1>Root</h1>");

  let response = fixture.root().serve("/nowhere.js");

  assert_eq!(response.status(), 404);
  assert!(body_of(&response).contains("nowhere.js"));
}

#[test]
fn a_path_climbing_out_of_the_root_is_refused() {
  let fixture = Fixture::new();
  fixture.write("index.html", "<h1>Root</h1>");

  let root = fixture.root();

  for request in [
    "/../secret.txt",
    "/%2e%2e/secret.txt",
    "/assets/../../out.txt",
  ] {
    let response = root.serve(request);
    assert_eq!(response.status(), 403, "{request} should be refused");
  }

  assert_eq!(root.resolve("/../secret.txt"), Err(Rejection::Outside));
}

#[test]
fn an_absolute_or_separator_bearing_path_is_refused() {
  let fixture = Fixture::new();
  fixture.write("index.html", "<h1>Root</h1>");

  let root = fixture.root();

  assert_eq!(root.resolve("/C:/Windows/win.ini"), Err(Rejection::Outside));
  assert_eq!(root.resolve("/..\\secret.txt"), Err(Rejection::Outside));
}

#[test]
fn content_types_follow_the_extension() {
  assert_eq!(
    content_type(Path::new("a/index.html")),
    "text/html; charset=utf-8"
  );
  assert_eq!(
    content_type(Path::new("a/app.js")),
    "text/javascript; charset=utf-8"
  );
  assert_eq!(
    content_type(Path::new("a/app.mjs")),
    "text/javascript; charset=utf-8"
  );
  assert_eq!(
    content_type(Path::new("a/site.css")),
    "text/css; charset=utf-8"
  );
  assert_eq!(content_type(Path::new("a/logo.png")), "image/png");
  assert_eq!(content_type(Path::new("a/logo.SVG")), "image/svg+xml");
  assert_eq!(content_type(Path::new("a/photo.jpeg")), "image/jpeg");
  assert_eq!(content_type(Path::new("a/font.woff2")), "font/woff2");
}

/// The old handler answered every image with `image/`, which is not a media
/// type at all. Nothing may answer with it again.
#[test]
fn no_extension_answers_with_a_bare_image_type() {
  for name in ["a.png", "a.jpg", "a.jpeg", "a.gif", "a.svg"] {
    assert_ne!(content_type(Path::new(name)), "image/");
  }
}

#[test]
fn an_unknown_extension_gets_a_safe_default() {
  assert_eq!(
    content_type(Path::new("a/thing.xyz")),
    "application/octet-stream"
  );
  assert_eq!(
    content_type(Path::new("a/LICENSE")),
    "application/octet-stream"
  );
}

#[test]
fn a_served_file_carries_its_content_type() {
  let fixture = Fixture::new();
  fixture.write("index.html", "<h1>Root</h1>");
  fixture.write("style.css", "body {}");

  let root = fixture.root();

  assert_eq!(
    content_type_of(&root.serve("/")),
    "text/html; charset=utf-8"
  );
  assert_eq!(
    content_type_of(&root.serve("/style.css")),
    "text/css; charset=utf-8"
  );
}

#[test]
fn percent_decoding_leaves_a_broken_escape_alone() {
  assert_eq!(percent_decode("/a%20b.css"), "/a b.css");
  assert_eq!(percent_decode("/100%.css"), "/100%.css");
  assert_eq!(percent_decode("/a%zz.css"), "/a%zz.css");
  assert_eq!(percent_decode("/a%2"), "/a%2");
}

/// A port nothing is listening on: bound, read back, and dropped, so the number
/// is real and free by the time it is handed out.
fn closed_port() -> u16 {
  let listener =
    TcpListener::bind("127.0.0.1:0").expect("a loopback port should be available");
  listener
    .local_addr()
    .expect("a bound listener should have an address")
    .port()
}

/// The three failures this exists for read differently. A developer who has
/// only the one line still knows which of them they are looking at.
#[test]
fn the_three_failures_read_differently() {
  let certificate = headline("certificate").expect("an untrusted certificate is named");
  let refused = headline("refused").expect("a refused connection is named");
  let host = headline("host").expect("an unresolvable host is named");

  assert!(certificate.contains("certificate"));
  assert!(refused.contains("refused"));
  assert!(host.contains("resolved"));

  assert_ne!(certificate, refused);
  assert_ne!(refused, host);
  assert_ne!(certificate, host);
}

/// An address Dry reached is an address Dry has nothing to say about. A blank
/// window whose server answered is not a navigation failure.
#[test]
fn a_reachable_address_is_not_accused() {
  assert_eq!(headline("reachable"), None);
  assert_eq!(headline("something a later probe learns to say"), None);
}

#[test]
fn a_refused_connection_is_named_as_one() {
  let url = format!("http://127.0.0.1:{}/", closed_port());

  let reason = Python::attach(|py| probe(py, &url)).expect("the probe should answer");

  assert_eq!(reason.0, "refused");
  assert!(reason.1.contains("ConnectionRefusedError"), "{}", reason.1);

  let reported = diagnose(&url).expect("a refused connection is reported");
  assert!(
    reported.contains("the connection was refused"),
    "{reported}"
  );
}

#[test]
fn a_missing_file_is_named_as_one() {
  let url = "file:///dry/no/such/file.html";

  let reason = Python::attach(|py| probe(py, url)).expect("the probe should answer");

  assert_eq!(reason.0, "missing");
}

/// Only the schemes Dry can say something useful about are watched. A Root
/// answers its own failures in the window, so watching it would be noise.
#[test]
fn only_addresses_dry_can_ask_about_are_watched() {
  let watched = |url: &str| PROBED_SCHEMES.iter().any(|scheme| url.starts_with(scheme));

  assert!(watched("https://example.invalid/"));
  assert!(watched("http://127.0.0.1:8081/"));
  assert!(watched("file:///tmp/index.html"));
  assert!(!watched(ROOT_ORIGIN));
}

/// A failed navigation still finishes, on the blank page. Reading that as an
/// arrival is what kept the failure invisible, so the blank page is named and
/// stays named.
#[test]
fn the_blank_page_is_not_an_arrival() {
  assert_eq!(BLANK_PAGE, "about:blank");
}
