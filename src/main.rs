use libc;
use nix::pty::forkpty;
use nix::sys::socket::SockAddr;
use nix::unistd::ForkResult;
use std::fs::File;
use std::io;
use std::os::unix::io::FromRawFd;
use std::thread;
use vsock::{VsockListener, VsockStream};

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
    match forkpty(None, None) {
        Err(e) => println!("forkpty error = {:?}", e),
        Ok(result) => {
            match result.fork_result {
                ForkResult::Child => {
                    io::copy(&mut io::stdin(), &mut io::stdout());
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
                        io::copy(&mut stream, &mut master)
                    });
                    thread::spawn(move || {
                        let mut master = unsafe { File::from_raw_fd(result.master) };
                        io::copy(&mut master, &mut stream_clone)
                    });
                }
            }
        }
    }
}
