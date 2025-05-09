# listener_poll

Adds polling functionality with timeout to `TcpListener` and `UnixListener`
## Example
```rust
use std::{io, thread};
use std::net::TcpListener;
use std::sync::Arc;

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::time::Duration;

use listener_poll::PollEx;

fn handle_accept(listener: TcpListener, active: Arc<AtomicBool>) -> io::Result<()> {
    loop {
        if !active.load(SeqCst) {
            return Ok(());
        }
        if !listener.poll(Some(Duration::from_secs(5)))? {
            continue;
        }
        let (_sock, _addr) = listener.accept()?;
        //... probably thread::spawn or mpsc Sender::send
    }
}
```

### Tested targets
- |i686, x86_64, sparc64, powerpc, s390x|-unknown-linux-gnu
- |i686, x86_64|-unknown-linux-musl
- |i686, x86_64|-pc-windows-gnu

### Untested targets that probably work
Compilation is tested!
- |x86_64, aarch64|-apple-darwin
- x86_64-unknown-freebsd
- x86_64-unknown-netbsd