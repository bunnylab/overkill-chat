![](overkill_logo.png)

# Overkill Chat

An experimental peer to peer secure chat program written in rust. Uses tor hidden services and noise protocol. Repository includes both a simple cli interface to the encrypted client program and a gtk gui. Chat program is comparable to [ricochet](https://github.com/blueprint-freespeech/ricochet-refresh). 

## Design 
- anonymous and metadata resistant communications (tor network)
- strong end to end message encryption+integrity with [noise protocol](https://github.com/mcginty/snow) 
- fully p2p architecture, no servers 
- simple open-source client code in a memory safe language (rust)

## Building 

```
make
```

Build with make, this will generate binaries for the gui and chat client. Chat client 
requires a rust compiler and gui requires gcc and gtk3 development libraries to build 
from source. To build either binary individually you can specify the name of the program
as follows. 

```
make overkill-chat
make overkill-gui
```

## Setup

Overkill chat will only communicate over tor hidden services. To run the program you must have the tor service running and a hidden service configured to forward traffic. Default listening port is `7878`. To connect to a peer you must exchange hidden service addresses over another channel. 

Example torrc lines: 
```
HiddenServiceDir /home/user/my_hidden_service
HiddenServicePort 7878 127.0.0.1:7878
```

### GUI Setup

To run the gui and connect to a peer you must add their hidden service address to 
the `overkill-start.sh` script. Change the following line

```
hservice=peer-hidden-service-address
```

### Installing

Running make install or the installation script will place the binaries and various
configuration files in the appropriate directories for most linux distributions. 

```
make install
``` 

or 

```
./install.sh
```

## Connect to Peer 

Once you have exchanged hidden service names each peer should run the following command. 
Both programs must be active at the same time to establish a connection. The program will
attempt to connect for n seconds before timing out.

```
overkill_chat --host peershiddenservicename.onion
```

For the GUI, simply find 'overkill' in your application menu and run it.

## Pre-shared Secret 

The security of the inital key exchange can be improved by using a preshared 
secret. When connecting you may optionally specify a password which will be 
run through pbkdf2 to create a 32 byte key. This key is only used to protect 
the initial handshake. 

```
overkill_chat --host peershiddenservicename.onion --password secretsharedwithpeer
```



