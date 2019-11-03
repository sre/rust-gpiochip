// © 2018 Sebastian Reichel
// SPDX-License-Identifier: ISC

#![crate_type = "lib"]
#![crate_name = "gpiochip"]

//! The `gpiochip` crate provides access to Linux gpiochip devices
//! from rust. The interface is wrapped, so that rust types are being
//! used instead of C types.
//!
//! # Examples
//!
//! ```
//! extern crate gpiochip as gpio;
//!
//! /// Print information about first gpiochip
//! fn main() {
//!     let chip = gpio::GpioChip::new("/dev/gpiochip0").unwrap();
//!
//!     println!("GPIOChip 0:");
//!     println!(" Name:  {:?}", chip.name);
//!     println!(" Label: {:?}", chip.label);
//!     println!(" Lines: {:?}", chip.lines);
//!     println!("");
//!
//!     for i in 0..chip.lines {
//!         let info = chip.info(i).unwrap();
//!
//!         println!(" GPIO {:?}: {:?}", info.gpio, info.name);
//!         println!("     Consumer: {:?}", info.consumer);
//!         println!("     Flags: {:?}", info.flags);
//!     }
//! }
//! ```
//!
//! ```
//! extern crate gpiochip as gpio;
//!
//! /// Simple get/set example
//! fn main() {
//!     let chip = gpio::GpioChip::new("/dev/gpiochip0").unwrap();
//!     let gpio_a = chip.request("gpioA", gpio::RequestFlags::INPUT, 0, 0).unwrap();
//!     let gpio_b = chip.request("gpioA", gpio::RequestFlags::OUTPUT | gpio::RequestFlags::ACTIVE_LOW, 1, 0).unwrap();
//!
//!     let val = gpio_a.get().unwrap();
//!     gpio_b.set(val).unwrap();
//! }
//! ```
//!
//! ```
//! extern crate gpiochip as gpio;
//!
//! /// GPIO events
//! fn main() {
//!     let chip = gpio::GpioChip::new("/dev/gpiochip0").unwrap();
//!
//!     let gpio_a = chip.request_event("gpioA", 0, gpio::RequestFlags::INPUT, gpio::EventRequestFlags::BOTH_EDGES).unwrap();
//!     let gpio_b = chip.request_event("gpioB", 1, gpio::RequestFlags::INPUT, gpio::EventRequestFlags::BOTH_EDGES).unwrap();
//!
//!     let bitmap = gpio::wait_for_event(&[&gpio_a, &gpio_b], 1000).unwrap();
//!
//!     if bitmap & 0b01 == 0b01 {
//!         let event = gpio_a.read().unwrap();
//!         println!("gpioA: event @ {:?} - {:?}", event.timestamp, event.id);
//!     }
//!
//!     if bitmap & 0b10 == 0b10 {
//!         let event = gpio_b.read().unwrap();
//!         println!("gpioB: event @ {:?} - {:?}", event.timestamp, event.id);
//!     }
//! }
//! ```

#[macro_use] extern crate nix;
#[macro_use] extern crate bitflags;
extern crate libc;

use std::io;
use std::os::unix::io::RawFd;
use std::os::unix::io::IntoRawFd;
use std::os::unix::io::FromRawFd;
use std::os::unix::io::AsRawFd;
use std::ffi::CStr;

bitflags! {
    /// bitflag describing the current gpio mode
    pub struct Flags: u32 {
        /// The GPIO is used by the kernel
        const KERNEL      = 0b00000001;
        /// The GPIO is in output mode (unset means it is in input mode)
        const OUTPUT      = 0b00000010;
        /// The GPIO is active-low
        const ACTIVE_LOW  = 0b00000100;
        /// The GPIO is open-drain
        const OPEN_DRAIN  = 0b00001000;
        /// The GPIO is open-source
        const OPEN_SOURCE = 0b00010000;
    }
}

bitflags! {
    /// bitflag describing the gpio mode, that should be requested
    pub struct RequestFlags: u32 {
        /// Request input mode
        const INPUT       = 0b00000001;
        /// Request output mode
        const OUTPUT      = 0b00000010;
        /// Request active-low
        const ACTIVE_LOW  = 0b00000100;
        /// Requst open-drain mode
        const OPEN_DRAIN  = 0b00001000;
        /// Request open-source mode
        const OPEN_SOURCE = 0b00010000;
    }
}

bitflags! {
    /// bitflag describing the events, that should generate a `GpioEvent` the `GpioEventHandle`
    pub struct EventRequestFlags: u32 {
        /// Generate event on rising edge
        const RISING_EDGE  = 0b00000001;
        /// Generate event on falling edge
        const FALLING_EDGE = 0b00000010;
        /// Generate event on rising and falling edges
        const BOTH_EDGES   = 0b00000011;
    }
}

/// Data returned by `GpioChip::info()`
#[derive(Clone)]
pub struct LineInfo {
    /// The GPIO number
    pub gpio: u32,
    /// The GPIO name
    pub name: String,
    /// The GPIO consumer name
    pub consumer: String,
    /// The GPIO flags
    pub flags: Flags,
}

#[allow(non_camel_case_types)]
#[repr(u32)]
#[derive(PartialEq)]
pub enum EventId {
    /// GPIO changed from low to high
    RISING_EDGE = 1,
    /// GPIO changed from high to low
    FALLING_EDGE = 2,
}

/// A GPIO event received from a `GpioEventHandle`
#[allow(non_camel_case_types)]
#[repr(C)]
pub struct GpioEvent {
    /// timestamp in ns
    pub timestamp: u64,
    /// event type
    pub id: EventId,
}

/* internal low-level API */
mod ioctl {
    use std::os::raw::c_char;
    use std::os::unix::io::RawFd;

    #[allow(non_camel_case_types)]
    #[repr(C)]
    pub struct gpiochip_info {
        pub name: [c_char; 32],
        pub label: [c_char; 32],
        pub lines: u32,
    }

    #[allow(non_camel_case_types)]
    #[repr(C)]
    pub struct gpioline_info {
        pub line_offset: u32,
        pub flags: u32,
        pub name: [c_char; 32],
        pub consumer: [c_char; 32],
    }

    #[allow(non_camel_case_types)]
    #[repr(C)]
    pub struct gpiohandle_request {
        pub lineoffsets: [u32; 64],
        pub flags: u32,
        pub default_values: [u8; 64],
        pub consumer_label: [c_char; 32],
        pub lines: u32,
        pub fd: RawFd,
    }

    #[allow(non_camel_case_types)]
    #[repr(C)]
    pub struct gpioevent_request {
        pub lineoffset: u32,
        pub handleflags: u32,
        pub eventflags: u32,
        pub consumer_label: [c_char; 32],
        pub fd: RawFd,
    }

    #[allow(non_camel_case_types)]
    #[repr(C)]
    pub struct gpiohandle_data {
        pub values: [u8; 64],
    }

    const GPIO_IOC_MAGIC: u8 = 0xB4;

    ioctl_read!(get_chipinfo, GPIO_IOC_MAGIC, 0x01, gpiochip_info );
    ioctl_readwrite!(get_lineinfo, GPIO_IOC_MAGIC, 0x02, gpioline_info );
    ioctl_readwrite!(get_linehandle, GPIO_IOC_MAGIC, 0x03, gpiohandle_request );
    ioctl_readwrite!(get_lineevent, GPIO_IOC_MAGIC, 0x04, gpioevent_request );
    ioctl_readwrite!(get_line_values, GPIO_IOC_MAGIC, 0x08, gpiohandle_data );
    ioctl_readwrite!(set_line_values, GPIO_IOC_MAGIC, 0x09, gpiohandle_data );
}

fn from_nix_error(err: ::nix::Error) -> io::Error {
    match err {
        nix::Error::Sys(err_no) => io::Error::from(err_no),
        _ => io::Error::new(io::ErrorKind::InvalidData, err)
    }
}

fn from_nix_result<T>(res: ::nix::Result<T>) -> io::Result<T> {
    match res {
        Ok(r) => Ok(r),
        Err(err) => Err(from_nix_error(err)),
    }
}

/// Provide high-level access to Linux gpiochip Driver
pub struct GpioChip {
    file: std::fs::File,

    /// name for the chip
    pub name: String,
    /// label for the chip
    pub label: String,
    /// amount of gpios provided by the chip
    pub lines: u32,
}

/// A GPIO handle acquired from the gpiochip
pub struct GpioHandle {
    file: std::fs::File,
    pub gpio: u32,
    pub consumer: String,
    pub flags: RequestFlags,
}

/// A GPIO array handle acquired from the gpiochip
pub struct GpioArrayHandle {
    file: std::fs::File,
    pub gpios: Box<[u32]>,
    pub consumer: String,
    pub flags: RequestFlags,
}

/// A GPIO event handle acquired from the gpiochip
pub struct GpioEventHandle {
    file: std::fs::File,
    pub gpio: u32,
    pub eventflags: EventRequestFlags,
    pub handleflags: RequestFlags,
}

impl GpioEventHandle {
    /// Read GpioEvent
    pub fn read(&self) -> io::Result<GpioEvent> {
        let mut buf = [0 as u8; std::mem::size_of::<GpioEvent>()];
        let size = try!(from_nix_result(nix::unistd::read(self.file.as_raw_fd(), &mut buf)));
        if size < std::mem::size_of::<GpioEvent>() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "not enough data received"));
        }
        let s: GpioEvent = unsafe { std::ptr::read(buf.as_ptr() as *const _) };

        Ok(s)
    }

    /// Flush event buffer
    pub fn flush(&self) -> io::Result<()> {
        let mut bitmap = try!(wait_for_event(&[&self], 0));

        while bitmap != 0 {
            try!(self.read());
            bitmap = try!(wait_for_event(&[&self], 0));
        }

        Ok(())
    }

    /// Get GPIO value
    pub fn get(&self) -> io::Result<u8> {
        let mut data = ioctl::gpiohandle_data { values: [0; 64] };

        try!(from_nix_result(unsafe {
            ioctl::get_line_values(self.file.as_raw_fd(), &mut data)
        }));

        Ok(data.values[0])
    }
}

impl GpioHandle {
    /// Get GPIO value
    pub fn get(&self) -> io::Result<u8> {
        let mut data = ioctl::gpiohandle_data { values: [0; 64] };

        try!(from_nix_result(unsafe {
            ioctl::get_line_values(self.file.as_raw_fd(), &mut data)
        }));

        Ok(data.values[0])
    }

    /// Set GPIO value
    pub fn set(&self, value: u8) -> io::Result<()> {
        let mut data = ioctl::gpiohandle_data { values: [0; 64] };
        data.values[0] = value;

        try!(from_nix_result(unsafe {
            ioctl::set_line_values(self.file.as_raw_fd(), &mut data)
        }));

        Ok(())
    }
}

impl GpioArrayHandle {
    /// Get GPIO values
    pub fn get(&self) -> io::Result<([u8; 64])> {
        let mut data = ioctl::gpiohandle_data { values: [0; 64] };

        try!(from_nix_result(unsafe {
            ioctl::get_line_values(self.file.as_raw_fd(), &mut data)
        }));

        Ok(data.values)
    }

    /// Set GPIO values
    pub fn set(&self, values: &[u8]) -> io::Result<()> {
        let mut data = ioctl::gpiohandle_data { values: [0; 64] };

        if values.len() != self.gpios.len() || values.len() > 64 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid amount of values"));
        }

        for i in 0..values.len() {
            data.values[i] = values[i];
        }

        try!(from_nix_result(unsafe {
            ioctl::set_line_values(self.file.as_raw_fd(), &mut data)
        }));

        Ok(())
    }
}

impl GpioChip {
    /// Acquire information about the gpiochip
    ///
    /// Returns an Error or (name, label, number_of_gpios)
    fn chipinfo(fd: RawFd) -> io::Result<(String, String, u32)> {
        let mut info = ioctl::gpiochip_info { name: [0; 32], label: [0; 32], lines: 0 };

        try!(from_nix_result(unsafe {
            ioctl::get_chipinfo(fd, &mut info)
        }));

        let name = unsafe {CStr::from_ptr(info.name.as_ptr())}.to_string_lossy().into_owned();
        let label = unsafe {CStr::from_ptr(info.label.as_ptr())}.to_string_lossy().into_owned();

        Ok((name, label, info.lines))
    }

    /// Open the gpiochip with the provided path
    ///
    /// Typically, the path will be something like `"/dev/gpiochip0"`.
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> io::Result<GpioChip> {
        let file = try!(std::fs::File::open(path));
        let (name, label, lines) = try!(GpioChip::chipinfo(file.as_raw_fd()));

        Ok(GpioChip {file: file, name: name, label: label, lines: lines})
    }

    /// Acquire information about a gpio
    pub fn info(&self, gpio: u32) -> io::Result<(LineInfo)> {
        let mut info = ioctl::gpioline_info { line_offset: 0, flags: 0, name: [0; 32], consumer: [0; 32] };
        info.line_offset = gpio;

        try!(from_nix_result(unsafe {
            ioctl::get_lineinfo(self.file.as_raw_fd(), &mut info)
        }));

        let name = unsafe {CStr::from_ptr(info.name.as_ptr())}.to_string_lossy().into_owned();
        let consumer = unsafe {CStr::from_ptr(info.consumer.as_ptr())}.to_string_lossy().into_owned();
        let flags = Flags { bits: info.flags, };
        Ok(LineInfo {gpio: gpio, name: name, consumer: consumer, flags: flags})
    }

    /// Request a `GpioHandle` for a single gpio
    pub fn request(&self, consumer: &str, flags: RequestFlags, gpio: u32, default: u8) -> io::Result<(GpioHandle)> {
        let mut request = ioctl::gpiohandle_request { lineoffsets: [0; 64], flags: 0, default_values: [0; 64], consumer_label: [0; 32], lines: 0, fd: 0 };

        request.lineoffsets[0] = gpio;
        request.flags = flags.bits;
        request.default_values[0] = default;
        request.lines = 1;

        for i in 0..request.consumer_label.len() {
            if i >= consumer.len() {
                break;
            }
            request.consumer_label[i] = consumer.as_bytes()[i] as std::os::raw::c_char;
        }

        try!(from_nix_result(unsafe {
            ioctl::get_linehandle(self.file.as_raw_fd(), &mut request)
        }));

        Ok(GpioHandle {file: unsafe {std::fs::File::from_raw_fd(request.fd)}, consumer: consumer.to_string(), flags: flags, gpio: gpio})
    }

    /// Request a `GpioArrayHandle` for multiple gpios, that should be get/set simultaneously
    pub fn request_array(&self, consumer: &str, flags: RequestFlags, gpios: &[u32], default_values: &[u8]) -> io::Result<(GpioArrayHandle)> {
        let mut request = ioctl::gpiohandle_request { lineoffsets: [0; 64], flags: 0, default_values: [0; 64], consumer_label: [0; 32], lines: 0, fd: 0 };
        let mut vec: std::vec::Vec<u32> = std::vec::Vec::with_capacity(gpios.len());

        if gpios.len() > request.lineoffsets.len() {
            io::Error::new(io::ErrorKind::InvalidInput, "array to big");
        }

        if gpios.len() != default_values.len() {
            io::Error::new(io::ErrorKind::InvalidInput, "number of default values does not match number of gpios");
        }

        request.flags = flags.bits;
        request.lines = gpios.len() as u32;
        for i in 0..request.consumer_label.len() {
            if i >= consumer.len() {
                break;
            }
            request.consumer_label[i] = consumer.as_bytes()[i] as std::os::raw::c_char;
        }

        for x in 0..gpios.len() {
            request.lineoffsets[x] = gpios[x];
            request.default_values[x] = default_values[x];
            vec.push(gpios[x]);
        }

        try!(from_nix_result(unsafe {
            ioctl::get_linehandle(self.file.as_raw_fd(), &mut request)
        }));

        Ok(GpioArrayHandle {file: unsafe {std::fs::File::from_raw_fd(request.fd)}, consumer: consumer.to_string(), flags: flags, gpios: vec.into_boxed_slice()})
    }

    /// Request a `GpioEventHandle` for a single gpio
    pub fn request_event(&self, consumer: &str, gpio: u32, handleflags: RequestFlags, eventflags: EventRequestFlags) -> io::Result<(GpioEventHandle)> {
        let mut request = ioctl::gpioevent_request { lineoffset: 0, handleflags: 0, eventflags: 0, consumer_label: [0; 32], fd: 0 };

        for i in 0..request.consumer_label.len() {
            if i >= consumer.len() {
                break;
            }
            request.consumer_label[i] = consumer.as_bytes()[i] as std::os::raw::c_char;
        }

        request.lineoffset = gpio;
        request.handleflags = handleflags.bits;
        request.eventflags = eventflags.bits;

        try!(from_nix_result(unsafe {
            ioctl::get_lineevent(self.file.as_raw_fd(), &mut request)
        }));

        Ok(GpioEventHandle {file: unsafe {std::fs::File::from_raw_fd(request.fd)}, gpio: gpio, handleflags: handleflags, eventflags: eventflags})
    }
}

/// Wait until at least one gpio event has been received or timeout occured.
///
/// The return value is a bitmap, which marks the GpioEventHandles with data available
pub fn wait_for_event(events: &[&GpioEventHandle], timeout_ms: i32) -> io::Result<(u64)> {
    let mut fds: std::vec::Vec<libc::pollfd> = Vec::with_capacity(events.len());
    let mut result: u64 = 0;

    if events.len() > 64 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Function does not support more than 64 events"))
    }

    for event in events {
        fds.push( libc::pollfd { fd: event.file.as_raw_fd(), events: libc::POLLIN | libc::POLLPRI, revents: 0 } );
    }

    let ret = unsafe { libc::poll(&mut fds[0], fds.len() as libc::nfds_t, timeout_ms) };
    if ret < 0 {
        return Err(io::Error::last_os_error())
    } else if ret == 0 {
        return Ok(0);
    }

    for i in 0..fds.len() {
        if fds[i].revents != 0 {
            result |= 1 << i;
        }
    }

    Ok(result)
}

impl FromRawFd for GpioChip {
    unsafe fn from_raw_fd(fd: RawFd) -> GpioChip {
        let file = std::fs::File::from_raw_fd(fd);
        let (name, label, lines) = GpioChip::chipinfo(fd).unwrap();
        GpioChip { file: file, name: name, label: label, lines: lines }
    }
}

impl IntoRawFd for GpioChip {
    fn into_raw_fd(self) -> RawFd {
        self.file.into_raw_fd()
    }
}

impl AsRawFd for GpioChip {
    fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}

impl IntoRawFd for GpioHandle {
    fn into_raw_fd(self) -> RawFd {
        self.file.into_raw_fd()
    }
}

impl AsRawFd for GpioHandle {
    fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}

impl IntoRawFd for GpioArrayHandle {
    fn into_raw_fd(self) -> RawFd {
        self.file.into_raw_fd()
    }
}

impl AsRawFd for GpioArrayHandle {
    fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}

impl IntoRawFd for GpioEventHandle {
    fn into_raw_fd(self) -> RawFd {
        self.file.into_raw_fd()
    }
}

impl AsRawFd for GpioEventHandle {
    fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}
