use::std::{io, thread, time, process, num::NonZeroU32};
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream, SocketAddr};

extern crate clap;
extern crate socks;
extern crate snow;
use clap::{Arg, App};
use socks::{Socks5Stream};
use ring::pbkdf2;

const DEFAULTPORT: &str = "7878";
const DEFAULTALIAS: &str = "Alice";
const CONNECTIONTIMEOUT: u32 = 10;
static NOISEPATTERN: &'static str = "Noise_NN_25519_ChaChaPoly_BLAKE2s";
static NOISEPATTERNPSK: &'static str = "Noise_NNpsk0_25519_ChaChaPoly_BLAKE2s";

fn accept_connection(port: u16) -> TcpStream {
    let address: String = format!("{}:{}", "127.0.0.1", port);
    let listener = TcpListener::bind(address).unwrap();
    match listener.accept() {
        Ok((_socket, addr)) => { 
            println!("new client: {:?}", addr);
            _socket 
        }
        Err(e) => {
            eprintln!("couldn't get client: {:?}", e);
            panic!("Listener connection failed!");
        }
    }
}

fn connect(host: &str, port: u16, proxy_port: u16) -> TcpStream {
    let mut n: u32 = 0;
    let addr = SocketAddr::from(([127, 0, 0, 1], proxy_port));
    
    loop {
        if n > CONNECTIONTIMEOUT {
            panic!("Outgoing connection timed out")
        }

        match Socks5Stream::connect(addr, (host, port)) {
            Ok(socks_stream) => {
                return socks_stream.into_inner();
            }
            Err(e) => {
                eprintln!("outgoing tcp connection failed: {}", e);
                thread::sleep(time::Duration::from_secs(1));
            }
        }
        n += 1;
    }
        
}

#[inline(always)]
fn crypto_handshake(mut in_stream: &TcpStream, mut out_stream: &TcpStream, password_used: bool, password: &str) -> 
    (snow::TransportState, snow::TransportState) {

    // initiators and responders for snow noise protocol 
    let mut initiator: snow::HandshakeState;
    let mut responder: snow::HandshakeState;
    // buffers for handshake
    let (mut read_buf, mut first_msg, mut second_msg) =
    ([0u8; 1024], [0u8; 1024], [0u8; 1024]);
        
    // use psk pattern and set_psk if password provided
    if password_used {
        initiator = snow::Builder::new(NOISEPATTERNPSK.parse().unwrap())
            .build_initiator().unwrap();
        responder = snow::Builder::new(NOISEPATTERNPSK.parse().unwrap())
            .build_responder().unwrap();

        // adds pbkdf based psk to initial handshake
        let mut psk = [0u8; 32];
        let salt = [0u8; 16];
        let itterations = NonZeroU32::new(100).unwrap();
        static PBKDF2_ALG: pbkdf2::Algorithm = pbkdf2::PBKDF2_HMAC_SHA256;
        pbkdf2::derive(PBKDF2_ALG, itterations, &salt,
                        password.as_bytes(), &mut psk);
    
        // set our preshared key for the first key exchange 
        initiator.set_psk(0, &psk).unwrap();
        responder.set_psk(0, &psk).unwrap();     
    } 
    // fall back to nn key exchange if no password provided
    else {
        initiator = snow::Builder::new(NOISEPATTERN.parse().unwrap())
            .build_initiator().unwrap();
        responder = snow::Builder::new(NOISEPATTERN.parse().unwrap())
            .build_responder().unwrap();
    }

    // send first part of handshake over our output socket
    let mut len = initiator.write_message(&[], &mut first_msg).unwrap();
    out_stream.write(&first_msg[..len]).unwrap();

    // get first part of handshake from peer over input socket
    len = in_stream.read(&mut first_msg).unwrap();
    responder.read_message(&first_msg[..len], &mut read_buf).unwrap();

    // send response to first message over input socket 
    len = responder.write_message(&[], &mut second_msg).unwrap();
    in_stream.write(&second_msg[..len]).unwrap();

    // listen for response to initial handshake message over output socket 
    len = out_stream.read(&mut second_msg).unwrap();
    initiator.read_message(&second_msg[..len], &mut read_buf).unwrap();

    // NN handshake complete, transition into transport mode.
    let initiator = initiator.into_transport_mode().unwrap();
    let responder = responder.into_transport_mode().unwrap();

    return (initiator, responder);
}

fn process_incoming(mut socket: TcpStream, mut responder: snow::TransportState) {
    let mut net_buffer = [0u8; 1024];
    let mut out_buffer = [0u8; 1024];
    loop {
        match socket.read(&mut net_buffer) {
            Ok(n) => {
                // check for socket exception states here!
                if n < 1 {
                    eprintln!("Socket closed exiting...");
                    process::exit(0);
                }

                let n = responder.read_message(&net_buffer[..n], &mut out_buffer).unwrap();
                
                // make a string and trim null padding
                let output = String::from_utf8_lossy(&out_buffer[..n]);
                let output = output.trim_matches(char::from(0));
                println!("{}: {}", DEFAULTALIAS, output);
                
                // cipher state is updated after successful message receipt to implement 
                // hash ratchet 
                responder.rekey_incoming();

                // zero the buffers after every message
                net_buffer.fill(0);
                out_buffer.fill(0);

            },
            _ => {
                // clear buffers if any error occurs 
                net_buffer.fill(0);
                out_buffer.fill(0);
            },
        }
    }
}

#[inline(always)]
fn get_input(mut socket: TcpStream, mut initiator: snow::TransportState){
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut eof = false;
    let mut line = String::new();
    let mut line_buffer = [0u8; 256];
    let mut buf = [0u8; 512];
    
    // default lines() method reads all lines into heap so we manually 
    // buffer line by line into one heap allocated string
    while !eof {
        match handle.read_line(&mut line){
            Ok(0) => {
                eof = true;
            }
            Ok(_) => {
                line.pop(); // remove newline
                let msg_bytes = line.as_bytes();
 
               if msg_bytes.len() > 256 {
                    println!("Message length: {} bytes is over 256 byte limit",
                        msg_bytes.len());
                }
                else {
                    for i in 0..msg_bytes.len() {
                        line_buffer[i] = msg_bytes[i];
                    }
                    let len = initiator.write_message(&line_buffer, &mut buf).unwrap();
                    socket.write(&buf[..len]).unwrap();

                    // cipher state is updated after successful message send to implement 
                    // hash ratchet 
                    initiator.rekey_outgoing();
                }
                line.clear(); // reset buffer
                line_buffer.fill(0);
            }
            Err(e) => { panic!("read from stdin failed: {}", e); }
        }
    }
}

fn main() {
    // clap command line argument format
    let matches = App::new("Overkill Chat")
                        .version("1.0")
                        .author("Graham <grahamwt42@gmail.com>")
                        .about("experimental p2p chat client")
                        .arg(Arg::with_name("alias")
                            .short("A")
                            .long("alias")
                            .value_name("ALIAS")
                            .help("Alias for remote (default Alice)")
                            .takes_value(true))
                        .arg(Arg::with_name("listen")
                            .short("l")
                            .long("listen")
                            .value_name("LISTENPORT")
                            .help("Listening port (default 7878)")
                            .takes_value(true))
                        .arg(Arg::with_name("host")
                            .short("h")
                            .long("host")
                            .value_name("HOST")
                            .help("Tor service address of peer")
                            .takes_value(true)
                            .required(true))
                        .arg(Arg::with_name("port")
                            .short("p")
                            .long("port")
                            .value_name("PORT")
                            .help("Remote port (default 7878)")
                            .takes_value(true))
                        .arg(Arg::with_name("password")
                            .short("P")
                            .long("password")
                            .value_name("PASSWORD")
                            .help("Password (optional)")
                            .takes_value(true))
                        .get_matches();
    
    // validate argument types for command line input  
    let host = matches.value_of("host").unwrap();
    //let alias = matches.value_of("alias").unwrap_or(DEFAULTALIAS);

    let password_used: bool;
    let password: &str;
    if matches.is_present("password") {
        password_used = true;
        password = matches.value_of("password").unwrap();
    }
    else {
        password_used = false;
        password = "";
    }
    let listen_arg = matches.value_of("listen").unwrap_or(DEFAULTPORT);
    let listen: u16 = match listen_arg.parse::<u16>() {
        Ok(val) => val,
        Err(_e) => {
            eprintln!("Invalid value for listen: {}", listen_arg);
            return;
        }
    };
    let port_arg = matches.value_of("port").unwrap_or(DEFAULTPORT);
    let port: u16 = match port_arg.parse::<u16>() {
        Ok(val) => val,
        Err(_e) => {
            eprintln!("Invalid value for port: {}", port_arg);
            return;
        }
    };

    println!("listen {} port {} address {}", listen, port, host);
    // start listener for incoming connection
    let t_join_handle = thread::spawn(move || accept_connection(listen)); 

    // // attempt outgoing connection in main thread
    let out_stream = connect(host, port, 9050);

    // join listener thread
    let in_stream: TcpStream = t_join_handle.join().unwrap();
    println!("TCP connection established...");

    // Attempt noise protocol handshake to establish encrypted connection
    // we are going to need one for incoming connection and one for outgoing
    // so lets establish a double connection here
    let (initiator, responder) = crypto_handshake(&in_stream, &out_stream, password_used, &password);
    println!("Encrypted channel established...");

    // get any input from our incoming connection and do stuff with it
    // in a thread of course 
    let t_input_listener = thread::spawn( || process_incoming(in_stream, responder));

    // get any new messages from stdin until eof
    get_input(out_stream, initiator);    

    // join listener on close
    t_input_listener.join().unwrap();
}