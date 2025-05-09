use listener_poll::PollEx;
use std::net::{TcpListener, TcpStream};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
use std::thread;
use std::time::{Duration, Instant};

#[test]
pub fn test_tcp_listen() {
    let bnd = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let time = Instant::now();
    assert_eq!(false, bnd.poll(Some(Duration::from_secs(2))).unwrap());
    assert!(time.elapsed().as_millis() >= 1800);
    let time = Instant::now();
    assert_eq!(false, bnd.poll_non_blocking().unwrap());
    assert!(time.elapsed().as_millis() < 500);

    let laddr = bnd.local_addr().unwrap();
    let jh = thread::spawn(move || {
        let _stream = TcpStream::connect(laddr).unwrap();
    });
    assert_eq!(true, bnd.poll(Some(Duration::from_secs(2))).unwrap());
    assert_eq!(true, bnd.poll_non_blocking().unwrap());

    bnd.accept().unwrap();

    let time = Instant::now();
    assert_eq!(false, bnd.poll(Some(Duration::from_secs(2))).unwrap());
    assert!(time.elapsed().as_millis() >= 1800);
    let time = Instant::now();
    assert_eq!(false, bnd.poll_non_blocking().unwrap());
    assert!(time.elapsed().as_millis() < 500);

    jh.join().unwrap();
    let jh = thread::spawn(move || {
        thread::sleep(Duration::from_secs(6));
        let _stream = TcpStream::connect(laddr).unwrap();
    });
    let time = Instant::now();
    assert_eq!(false, bnd.poll(Some(Duration::from_secs(2))).unwrap());
    assert!(time.elapsed().as_millis() >= 1800);
    let time = Instant::now();
    assert_eq!(false, bnd.poll_non_blocking().unwrap());
    assert!(time.elapsed().as_millis() < 500);
    let time = Instant::now();
    bnd.poll_until_ready().unwrap();
    assert!(time.elapsed().as_millis() >= 2800);
    assert_eq!(true, bnd.poll_non_blocking().unwrap());
    assert_eq!(true, bnd.poll(Some(Duration::from_secs(2))).unwrap());
    jh.join().unwrap();
}

#[test]
#[cfg(unix)]
pub fn test_unix_listen() {
    _ = std::fs::remove_file("/tmp/897987698779182378");
    let bnd = UnixListener::bind("/tmp/897987698779182378").unwrap();
    let time = Instant::now();
    assert_eq!(false, bnd.poll(Some(Duration::from_secs(2))).unwrap());
    assert!(time.elapsed().as_millis() >= 1800);
    let time = Instant::now();
    assert_eq!(false, bnd.poll_non_blocking().unwrap());
    assert!(time.elapsed().as_millis() < 500);

    let jh = thread::spawn(move || {
        let _stream = UnixStream::connect("/tmp/897987698779182378").unwrap();
    });
    assert_eq!(true, bnd.poll(Some(Duration::from_secs(2))).unwrap());
    assert_eq!(true, bnd.poll_non_blocking().unwrap());

    bnd.accept().unwrap();

    let time = Instant::now();
    assert_eq!(false, bnd.poll(Some(Duration::from_secs(2))).unwrap());
    assert!(time.elapsed().as_millis() >= 1800);
    let time = Instant::now();
    assert_eq!(false, bnd.poll_non_blocking().unwrap());
    assert!(time.elapsed().as_millis() < 500);

    jh.join().unwrap();
    _ = std::fs::remove_file("/tmp/897987698779182378");
}
