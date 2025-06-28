//! # `listener_poll`1          
//! Adds polling functionality with timeout to `TcpListener` and `UnixListener`
//! ## Example
//! ```rust
//! use std::{io, thread};
//! use std::net::TcpListener;
//! use std::sync::Arc;
//!
//! use std::sync::atomic::AtomicBool;
//! use std::sync::atomic::Ordering::SeqCst;
//! use std::time::Duration;
//!
//! use listener_poll::PollEx;
//!
//! fn handle_accept(listener: TcpListener, active: Arc<AtomicBool>) -> io::Result<()> {
//!     loop {
//!         if !active.load(SeqCst) {
//!             return Ok(());
//!         }
//!         if !listener.poll(Some(Duration::from_secs(5)))? {
//!             continue;
//!         }
//!         let (_sock, _addr) = listener.accept()?;
//!         //... probably thread::spawn or mpsc Sender::send
//!     }
//! }
//!
//! ```
//!
#![deny(
    clippy::correctness,
    clippy::perf,
    clippy::complexity,
    clippy::style,
    clippy::nursery,
    clippy::pedantic,
    clippy::clone_on_ref_ptr,
    clippy::decimal_literal_representation,
    clippy::float_cmp_const,
    clippy::missing_docs_in_private_items,
    clippy::multiple_inherent_impl,
    clippy::unwrap_used,
    clippy::cargo_common_metadata,
    clippy::used_underscore_binding
)]

use std::io;
use std::time::Duration;

/// extension Trait for `TcpListener` and `UnixListener`
pub trait PollEx {
    /// This function returns Ok(true) if a later call to `accept` returns a stream or error without blocking.
    ///
    /// Note: If this function returns Ok(true) and another thread calls `accept` before this thread
    /// calls `accept`, then calling `accept` in this thread may still block.
    /// If this is not acceptable, then it is recommended to set the listener to be non-blocking
    /// to ensure that `accept` returns an Err instead of blocking.
    ///
    /// # Errors
    /// Operating system and implementation-specific errors.
    ///
    fn poll_non_blocking(&self) -> io::Result<bool> {
        self.poll(Some(Duration::new(0, 0)))
    }

    /// This function will block until a later call to `accept` returns a stream or error without blocking.
    ///
    /// This function ignores spurious any wakeup.
    ///
    /// Note: If this function returns and another thread calls `accept` before this thread
    /// calls `accept`, then calling `accept` in this thread may still block.
    /// If this is not acceptable, then it is recommended to set the listener to be non-blocking
    /// to ensure that `accept` returns an Err instead of blocking.
    ///
    /// # Errors
    /// Operating system and implementation-specific errors.
    ///
    fn poll_until_ready(&self) -> io::Result<()> {
        loop {
            //Windows has a spurious wakeup sometimes.
            if self.poll(None)? {
                return Ok(());
            }
        }
    }

    /// This function returns Ok(true) if a later call to `accept` returns a stream or error without blocking.
    ///
    /// This function will return Ok(false) if the timeout elapses
    /// or an operating system dependent spurious wakeup occurs.
    /// This function does not guarantee that the full timeout has elapsed when it returns Ok(false).
    ///
    /// Note: If this function returns Ok(true) and another thread calls `accept` before this thread
    /// calls `accept`, then calling `accept` in this thread may still block.
    /// If this is not acceptable, then it is recommended to set the listener to be non-blocking
    /// to ensure that `accept` returns an Err instead of blocking.
    ///
    /// # Errors
    /// Operating system and implementation-specific errors.
    ///
    fn poll(&self, timeout: Option<Duration>) -> io::Result<bool>;
}

/// Unix libc specific impl using poll.
/// Apple and openbsd do not have the "ppoll" function and must therefore use this impl.
#[cfg(any(target_vendor = "apple", target_os = "openbsd"))]
mod unix_poll {
    use crate::PollEx;
    use libc::{c_int, poll, pollfd, POLLIN};
    use std::io;
    use std::net::TcpListener;
    use std::os::fd::AsRawFd;
    use std::os::fd::RawFd;
    use std::time::Duration;

    /// apple poll impl is the same for tcp and unix sockets.
    fn poll_impl_apple(fd: RawFd, timeout: Option<Duration>) -> io::Result<bool> {
        const MAX_TIMEOUT_PER_CALL: u128 = c_int::MAX as u128;

        let mut fd = Box::pin(pollfd {
            fd,
            events: POLLIN,
            revents: 0,
        });

        let Some(mut ms) = timeout.map(|a| a.as_millis()) else {
            let count = unsafe { poll(fd.as_mut().get_mut(), 1, -1) };
            if count < 0 {
                return Err(io::Error::last_os_error());
            }

            return Ok(count != 0);
        };

        while ms > MAX_TIMEOUT_PER_CALL {
            ms -= MAX_TIMEOUT_PER_CALL;
            let count = unsafe { poll(fd.as_mut().get_mut(), 1, c_int::MAX) };

            if count < 0 {
                return Err(io::Error::last_os_error());
            }

            if count != 0 {
                return Ok(true);
            }
        }


        let count = unsafe { poll(fd.as_mut().get_mut(), 1, c_int::try_from(ms).expect("Unreachable: a conversion from u128 to c_int failed even tho the u128 is less than c_int::MAX")) };

        if count < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(count != 0)
    }

    #[cfg(unix)]
    impl PollEx for TcpListener {
        fn poll(&self, timeout: Option<Duration>) -> io::Result<bool> {
            poll_impl_apple(self.as_raw_fd(), timeout)
        }
    }

    #[cfg(unix)]
    impl PollEx for std::os::unix::net::UnixListener {
        fn poll(&self, timeout: Option<Duration>) -> io::Result<bool> {
            poll_impl_apple(self.as_raw_fd(), timeout)
        }
    }
}

/// Unix libc specific impl using ppoll.
/// Apple and openbsd do not have ppoll.
#[cfg(all(unix, not(target_vendor = "apple"), not(target_os = "openbsd")))]
mod unix_ppoll {
    use crate::PollEx;
    use libc::{pollfd, ppoll, timespec, POLLIN};
    use std::io;
    use std::net::TcpListener;
    use std::os::fd::AsRawFd;
    use std::os::fd::RawFd;
    use std::ptr::null;
    use std::time::Duration;

    /// unix poll impl is the same for tcp and unix sockets.
    fn poll_impl_unix(fd: RawFd, timeout: Option<Duration>) -> io::Result<bool> {
        let mut fd = Box::pin(pollfd {
            fd,
            events: POLLIN,
            revents: 0,
        });

        let Some(timeout) = timeout else {
            let count = unsafe { ppoll(fd.as_mut().get_mut(), 1, null(), null()) };
            if count < 0 {
                return Err(io::Error::last_os_error());
            }

            return Ok(count != 0);
        };

        //This depends on the target and libc that is used!
        #[allow(clippy::unnecessary_fallible_conversions)]
        let time = Box::pin(timespec {
            tv_sec: timeout.as_secs().try_into().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "timeout duration is too large to fit into libc::timespec.tv_sec",
                )
            })?,
            tv_nsec: timeout.subsec_nanos().try_into().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "timeout subsec_nanos is too large to fit into libc::timespec.tv_nsec",
                )
            })?,
        });

        let count = unsafe { ppoll(fd.as_mut().get_mut(), 1, time.as_ref().get_ref(), null()) };
        if count < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(count != 0)
    }

    #[cfg(unix)]
    impl PollEx for TcpListener {
        fn poll(&self, timeout: Option<Duration>) -> io::Result<bool> {
            poll_impl_unix(self.as_raw_fd(), timeout)
        }
    }

    #[cfg(unix)]
    impl PollEx for std::os::unix::net::UnixListener {
        fn poll(&self, timeout: Option<Duration>) -> io::Result<bool> {
            poll_impl_unix(self.as_raw_fd(), timeout)
        }
    }
}

/// Windows-specific impl
#[cfg(windows)]
mod windows {
    use crate::PollEx;
    use std::io;
    use std::net::TcpListener;
    use std::os::windows::io::AsRawSocket;
    use std::time::Duration;
    use windows_sys::Win32::Networking::WinSock::{
        WSAGetLastError, WSAPoll, POLLRDNORM, SOCKET_ERROR, WSAPOLLFD,
    };

    impl PollEx for TcpListener {
        fn poll(&self, timeout: Option<Duration>) -> io::Result<bool> {
            const MAX_TIMEOUT_PER_CALL: u128 = i32::MAX as u128;

            let windows_sock_handle = windows_sys::Win32::Networking::WinSock::SOCKET::try_from(self.as_raw_socket())
                //Unreachable unless the stdlib or windows-sys or both fucked up!
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "as_raw_socket handle does not fit into windows_sys::Win32::Networking::WinSock::SOCKET"))?;

            let mut pollfd = Box::pin(WSAPOLLFD {
                fd: windows_sock_handle,
                events: POLLRDNORM,
                revents: 0,
            });

            let Some(mut ms) = timeout.map(|a| a.as_millis()) else {
                let result = unsafe {
                    //https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-wsapoll
                    WSAPoll(pollfd.as_mut().get_mut(), 1, -1)
                };

                if result == SOCKET_ERROR {
                    unsafe {
                        return Err(io::Error::from_raw_os_error(WSAGetLastError()));
                    }
                }

                return Ok(result != 0);
            };

            while ms > MAX_TIMEOUT_PER_CALL {
                ms -= MAX_TIMEOUT_PER_CALL;
                let result = unsafe {
                    //https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-wsapoll
                    WSAPoll(pollfd.as_mut().get_mut(), 1, i32::MAX)
                };

                if result == SOCKET_ERROR {
                    unsafe {
                        return Err(io::Error::from_raw_os_error(WSAGetLastError()));
                    }
                }

                if result != 0 {
                    return Ok(true);
                }
            }

            let result = unsafe {
                //https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-wsapoll
                WSAPoll(pollfd.as_mut().get_mut(), 1, i32::try_from(ms).expect("Unreachable: a conversion from u128 to i32 failed even tho the u128 is less than i32::MAX"))
            };

            if result == SOCKET_ERROR {
                unsafe {
                    return Err(io::Error::from_raw_os_error(WSAGetLastError()));
                }
            }

            Ok(result != 0)
        }
    }
}
