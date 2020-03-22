use libc;
use nix::pty::forkpty;
use nix::sys::socket::SockAddr;
use nix::unistd::ForkResult;
use nix::sys::termios::{tcgetattr,LocalFlags, OutputFlags, InputFlags};
use nix::unistd::execv;
use std::fs::File;
use std::io;
use std::os::unix::io::FromRawFd;
use std::thread;
use std::ffi::CString;
use vsock::{VsockListener, VsockStream};

use  std::process::Command;
use  std::os::unix::process::CommandExt;
use std::process::exit;
use nix::sys::wait::waitpid;
use std::net::Shutdown;

fn main() {
    println!("Hello, world!");

    let vsock_addr = &SockAddr::new_vsock(libc::VMADDR_CID_ANY, 1235);

    let listener = VsockListener::bind(vsock_addr).expect("Unable to bind to socket");

    println!("{:?}", listener);
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                println!("New connection: {}", stream.peer_addr().unwrap());
                thread::spawn(move || handle_stream(stream));
            }
            Err(e) => println!("accept error = {:?}", e),
        }
    }
}

fn handle_stream(mut stream: VsockStream) {
    // io::copy(reader, writer);
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
                    // let shell_path = &CString::new("/bin/sh").expect("wrong shell path");
                    // // io::copy(&mut io::stdin(), &mut io::stdout());
                    // execv(shell_path, &[]);
                    Command::new("/bin/sh").exec();
                    // exit(0)
                }
                ForkResult::Parent { child } => {
                    println!("forkpty OK = {:?}", child);
                    // io::copy(reader, &mut master);
                    // let (master_reader, master_writer) = &mut (&master, &master);
                    // let (reader, writer) = &mut (&stream, &stream);
                    // thread::spawn(move || {io::copy(master_reader, writer)});
                    let mut stream_clone = stream.try_clone().expect("stream clone failed...");
                    // let reader = &mut stream;
                    thread::spawn(move || {
                        let mut master = unsafe { File::from_raw_fd(result.master) };
                        io::copy(&mut stream, &mut master);
                        println!("{:?} copy stream to master end ", child);
                        stream.shutdown(Shutdown::Both);
                    });
                    thread::spawn(move || {
                        let mut master = unsafe { File::from_raw_fd(result.master) };
                        io::copy(&mut master, &mut stream_clone);
                        println!("{:?} copy master to stream end ", child);
                        stream_clone.shutdown(Shutdown::Both);
                    });
                    waitpid(child, None);
                }
            }
        }
    }
}
