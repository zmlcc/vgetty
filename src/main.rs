use libc;
use nix::pty::forkpty;
use nix::sys::socket::SockAddr;
use nix::sys::termios::{tcgetattr, InputFlags, LocalFlags, OutputFlags};
use nix::unistd::ForkResult;
use std::fs::File;
use std::io;
use std::os::unix::io::FromRawFd;
use std::thread;
use vsock::{VsockListener, VsockStream};

use nix::fcntl::{fcntl, FcntlArg, FdFlag};
use nix::sys::wait::waitpid;
use std::io::Write;
use std::net::Shutdown;
use std::os::unix::io::AsRawFd;
use std::os::unix::process::CommandExt;
use std::process::exit;
use std::process::Command;
use nix::unistd::close;

fn main() {
    println!("Hello, world!");

    let vsock_addr = &SockAddr::new_vsock(libc::VMADDR_CID_ANY, 1235);

    let listener = VsockListener::bind(vsock_addr).expect("Unable to bind to socket");

    fcntl(listener.as_raw_fd(), FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC));

    println!("{:?}", listener);
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                fcntl(stream.as_raw_fd(), FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC));
                println!("New connection: {} {}", stream.as_raw_fd(), stream.peer_addr().unwrap());
                thread::spawn(move || handle_stream(stream));
            }
            Err(e) => println!("accept error = {:?}", e),
        }
    }
}

fn handle_stream(stream: VsockStream) {
    // prepare termio
    let mut termio = tcgetattr(0).expect("cannot get tty attr");
    termio.local_flags.remove(LocalFlags::ECHO);
    termio.output_flags.insert(OutputFlags::ONLCR);
    termio.output_flags.insert(OutputFlags::XTABS);
    termio.input_flags.insert(InputFlags::ICRNL);
    termio.input_flags.remove(InputFlags::IXOFF);
 
    match forkpty(None, &termio) {
        Err(e) => println!("forkpty error = {:?}", e),
        Ok(result) => {
            match result.fork_result {
                ForkResult::Child => {
                    Command::new("/bin/sh").exec();
                    // exit(0)
                }
                ForkResult::Parent { child } => {
                    println!("forkpty OK = {:?}", child);
                    let mut writer = stream.try_clone().expect("stream clone failed...");
                    let mut reader = stream.try_clone().expect("stream clone failed...");
                    // let reader = &mut stream;
                    thread::spawn(move || {
                        let mut master = unsafe { File::from_raw_fd(result.master) };
                        io::copy(&mut reader, &mut master);
                        println!("{:?} copy stream to master end ", child);
                        // reader.shutdown(Shutdown::Both);
                        master.write(b"\n");
                    });
                    thread::spawn(move || {
                        let mut master = unsafe { File::from_raw_fd(result.master) };
                        io::copy(&mut master, &mut writer);
                        println!("{:?} copy master to stream end ", child);
                    });
                    waitpid(child, None);
                    stream.shutdown(Shutdown::Both);
                    close(stream.as_raw_fd());
                }
            }
        }
    }

}
