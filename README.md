![](overkill_logo.png)

# Overkill Chat

An experimental peer to peer secure chat program written in rust. Uses tor hidden services and noise protocol. This is a learning project to get a better handle on rust and using cryptography libraries. The program has not been audited and I make no guarantees about it's security.

## Building 

```
cargo build --release 
```

## Setup

Overkill chat will only communicate over tor hidden services. To run the program you must have the tor service running and a hidden service configured to forward traffic. Default listening port is `7878`. To connect to a peer you must exchange hidden service addresses over another channel. 

Example torrc lines: 
```
HiddenServiceDir /home/user/my_hidden_service
HiddenServicePort 7878 127.0.0.1:7878
```

## Test Connection 

The program can be run in "echo" mode connecting to itself. Run the following command. If you are using a different hidden service port than the default `7878` set `--port` and `--listen` arguments to your new port.  

```
overkill_chat --host myhiddenservicename.onion
```

## Connect to Peer 

Once you have exchanged hidden service names each peer should run the following command. 
Both programs must be active at the same time to establish a connection. The program will
attempt to connect for n seconds before timing out.

```
overkill_chat --host peershiddenservicename.onion
```

## Pre-shared Secret 

The security of the inital key exchange can be improved by using a preshared 
secret. When connecting you may optionally specify a password which will be 
run through pbkdf2 to create a 32 byte key. This key is only used to protect 
the initial handshake. 

```
overkill_chat --host peershiddenservicename.onion --password secretsharedwithpeer
```



