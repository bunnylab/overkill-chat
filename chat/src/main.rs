use::std::{io, thread, time, process, num::NonZeroU32};
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc;
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
static NOISEPATTERN: &'static str = "Noise_NN_25519_ChaChaPoly_BLAKE2s";
static NOISEPATTERNPSK: &'static str = "Noise_NNpsk0_25519_ChaChaPoly_BLAKE2s";


fn connect(host: &str, port: u16, proxy_port: u16, local_port: u16) -> (TcpStream, bool) {
    let proxy_addr = SocketAddr::from(([127, 0, 0, 1], proxy_port));
    let local_addr: String = format!("{}:{}", "127.0.0.1", local_port);
    let listener = TcpListener::bind(local_addr).unwrap();
    
    // make listener nonblocking
    listener.set_nonblocking(true).expect("Cannot set non-blocking");
    
    // loop forever alternating between listening and attempting 
    // outgoing connection until one succeeds
    loop {
        // listen for incoming
        for n in 0..100 {
            match listener.accept() {
                Ok((_socket, a)) => { 
                    println!("new client listener: {:?}", a);
                    return (_socket, false); 
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    ;
                }
                Err(e) => panic!("encountered IO error: {}", e)
            }
            thread::sleep(time::Duration::from_millis(25));
        }        
        
        // try an outgoing connection, blocks
        match Socks5Stream::connect(proxy_addr, (host, port)) {
            Ok(socks_stream) => {
                println!("new client outgoing: {:?}", host);
                return (socks_stream.into_inner(), true);
            }
            Err(e) => {
                eprintln!("outgoing tcp connection failed: {}", e);
            }
        }
        thread::sleep(time::Duration::from_millis(100));
    }
}

#[inline(always)]
fn crypto_handshake(mut stream: &TcpStream, is_initiator: bool, password_used: bool, password: &str) -> 
    snow::TransportState {

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


    if(is_initiator) {
        // send first part of handshake over our output socket
        let mut len = initiator.write_message(&[], &mut first_msg).unwrap();
        stream.write(&first_msg[..len]).unwrap();

        // listen for response to initial handshake message over output socket 
        len = stream.read(&mut second_msg).unwrap();
        initiator.read_message(&second_msg[..len], &mut read_buf).unwrap();

        // NN handshake complete, transition into transport mode.
        return initiator.into_transport_mode().unwrap();
    }
    else {
        // get first part of handshake from peer over socket
        let mut len = stream.read(&mut first_msg).unwrap();
        responder.read_message(&first_msg[..len], &mut read_buf).unwrap();

        // send response to first message over input socket 
        len = responder.write_message(&[], &mut second_msg).unwrap();
        stream.write(&second_msg[..len]).unwrap();

        // NN handshake complete, transition into transport mode.
        return responder.into_transport_mode().unwrap();
    }
}

fn get_incoming(mut socket: TcpStream, sender: mpsc::Sender<([u8; 512], usize)>){
    let mut net_buffer = [0u8; 512];
    loop {
        match socket.read(&mut net_buffer) {
            Ok(n) => {
                // check for socket exception states here!
                if n < 1 {
                    eprintln!("Socket closed exiting...");
                    process::exit(0);
                }

                sender.send( (net_buffer, n) ).unwrap();

                // zero the buffer after every message
                net_buffer.fill(0);
            },
            _ => {
                // clear buffers if any error occurs 
                net_buffer.fill(0);
            }
        }
    }
} 

fn get_lines_stdin(sender: mpsc::Sender<[u8; 256]>){
    let stdin = io::stdin();
    let mut line_buffer = [0u8; 256];
    
    for line in stdin.lock().lines() {
        let temp = line.unwrap();
        let msg_bytes = temp.as_bytes();
        if msg_bytes.len() > 256 {
            println!("Message length: {} bytes is over 256 byte limit",
                msg_bytes.len());
        }
        else {
            for i in 0..msg_bytes.len() {
                line_buffer[i] = msg_bytes[i];
            }
            sender.send(line_buffer).unwrap();
            line_buffer.fill(0);
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

    // attempt outgoing connection in main thread
    let (mut stream, is_initiator) = connect(host, port, 9050, listen);

    println!("local: {:?}", stream.local_addr().unwrap());
    println!("peer: {:?}", stream.peer_addr().unwrap());
    println!("TCP connection established...");

    // attempt crypt handshake in main thread
    let mut crypt = crypto_handshake(&stream, is_initiator, password_used, &password);
    println!("Encrypted connection established...");
    println!("Is Initator: {:?}", crypt.is_initiator());

    // establish channels
    let mut stream_clone = stream.try_clone().expect("clone failed...");
    let (tx_input, rx_input): (Sender<[u8; 256]>, Receiver<[u8; 256]>) = mpsc::channel();
    let (tx_net, rx_net): (Sender<([u8; 512], usize)>, Receiver<([u8; 512], usize)>) = mpsc::channel();

    // launch threads to get/receive messages
    let t_input_listener = thread::spawn(move || get_lines_stdin(tx_input));
    let t_net_listener = thread::spawn(move || get_incoming(stream_clone, tx_net));

    // Main loop for chat service 
    // - receive encrypted messages from socket 
    // - receive plaintext from stdin
    // - encrypt/decrypt with snow transport state
    // - rekey ciphers to give us a basic single ratchet
    let mut decrypt_buf = [0u8; 512];
    let mut encrypt_buf = [0u8; 512];
    loop{
        // try input from stdin
        match rx_input.try_recv() {
            Ok(val) => {
                let n = crypt.write_message(&val, &mut encrypt_buf).unwrap();
                stream.write(&encrypt_buf[..n]).unwrap();

                // update cipherstate after send
                crypt.rekey_outgoing();
                // zero buffer after every message
                encrypt_buf.fill(0);
            },
            Err(_e) => {
            }
        }
        // try input from socket 
        match rx_net.try_recv() {
            Ok(val) => {
                let (buf, n) = val;
                let n = crypt.read_message(&buf[..n], &mut decrypt_buf).unwrap();                                
                
                // make a string and trim null padding
                let output = String::from_utf8_lossy(&decrypt_buf[..n]);
                let output = output.trim_matches(char::from(0));
                println!("{}: {}", DEFAULTALIAS, output);

                // update cipherstate after receive
                crypt.rekey_incoming();
                // zero buffer after every message
                decrypt_buf.fill(0);
            
            },
            Err(_e) => {
            }
        }


    }
    
    // join listeners on close
    t_input_listener.join().unwrap();
    t_net_listener.join().unwrap();
}