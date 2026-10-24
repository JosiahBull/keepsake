#![allow(dead_code)] // Shared process helpers are used by several integration-test binaries.

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub struct Server {
    child: Child,
    output: Arc<Mutex<String>>,
    readers: Vec<std::thread::JoinHandle<()>>,
    pub address: SocketAddr,
    pub directory: tempfile::TempDir,
}

impl Server {
    pub fn spawn(settings: &[(&str, &str)]) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let web = directory.path().join("web");
        std::fs::create_dir(&web).unwrap();
        std::fs::write(web.join("index.html"), "<h1>Keepsake</h1>").unwrap();
        let reserved = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reserved.local_addr().unwrap();
        drop(reserved);

        let mut command = Command::new(env!("CARGO_BIN_EXE_keepsake-server"));
        command
            .current_dir(directory.path())
            .env(
                "DATABASE_URL",
                format!("sqlite:{}", directory.path().join("library.db").display()),
            )
            .env("BIND_ADDR", address.to_string())
            .env("WEB_DIR", &web)
            .env("RUST_LOG", "info")
            .env("SHUTDOWN_GRACE_SECS", "2")
            .envs(settings.iter().copied())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().unwrap();
        let output = Arc::new(Mutex::new(String::new()));
        let mut readers = Vec::new();
        for reader in [
            Box::new(child.stdout.take().unwrap()) as Box<dyn Read + Send>,
            Box::new(child.stderr.take().unwrap()) as Box<dyn Read + Send>,
        ] {
            let output = output.clone();
            readers.push(std::thread::spawn(move || {
                for line in BufReader::new(reader).lines().map_while(Result::ok) {
                    let mut output = output.lock().unwrap();
                    output.push_str(&line);
                    output.push('\n');
                }
            }));
        }
        Self {
            child,
            output,
            readers,
            address,
            directory,
        }
    }

    pub fn start(settings: &[(&str, &str)]) -> Self {
        let mut server = Self::spawn(settings);
        server.wait_for_log("keepsake is listening");
        server
    }

    pub fn output(&self) -> String {
        self.output.lock().unwrap().clone()
    }

    pub fn wait_for_log(&mut self, expected: &str) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if self.output().contains(expected) {
                return;
            }
            assert!(
                self.child.try_wait().unwrap().is_none(),
                "server exited: {}",
                self.output()
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("missing log {expected}: {}", self.output());
    }

    pub fn signal(&self, name: &str) {
        assert!(
            Command::new("kill")
                .args([name, &self.child.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
    }

    pub fn wait(&mut self, limit: Duration) -> ExitStatus {
        let deadline = Instant::now() + limit;
        while Instant::now() < deadline {
            if let Some(status) = self.child.try_wait().unwrap() {
                for reader in self.readers.drain(..) {
                    reader.join().unwrap();
                }
                return status;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("server did not exit: {}", self.output());
    }

    pub fn request(&self, method: &str, path: &str, body: &str) -> String {
        let mut stream = TcpStream::connect(self.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        write!(stream,
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
