# Implementing basic TCP in rust

A Rust implementatio of the Transmission control Protocol. Just for the purpose of learning.

Along the Video https://www.youtube.com/watch?v=bzja9fQWzdA&list=PLqbS7AVVErFivDY3iKAQk3_VAm8SXwt1X&index=11 by Jon Gjengset on Youtube

## currently on timestamp

https://www.youtube.com/watch?v=bzja9fQWzdA&t=8970

## Notes on setting up tun tap
- a simple main.rs, creating the run interface, listening on it and just printing out received bytes:
```rs
use std::io;
fn main() ->io::Result<()> {
    println!("starting and listening");
    let nic = tun_tap::Iface::new("tun0", tun_tap::Mode::Tun)?;
    let mut buf = [0u8; 1504];
    let nbytes = nic.recv(&mut buf[..])?;
    eprintln!("read {} bytes: {:x?}", nbytes, &buf[..nbytes]);
    Ok(())
}
```

When implementing (your own) tcp, one problem is, that the kernel already implements it's own tcp-stack. That can interfere with our packets etc.
- the solution here, TUN/TAP - https://www.gabriel.urdhr.fr/2021/05/08/tuntap/
- this way the kernel will basically create a virtual networkcard/networkinterface (the TUN) we can use for our tcp-implementation.
- `cargo add tun_tap`
- so first we build our binary `cargo build --release` that we want to allow network access
- to enable networking capability without always having to run it as sudo we can: `sudo setcap cap_net_admin=eip ./target/release/rs_tcp`
- `ip addr` returns us our created interface-info: 
```
7: tun0: <POINTOPOINT,MULTICAST,NOARP> mtu 1500 qdisc noop state DOWN group default qlen 500    
link/none
```
- then while `./target/repease/rs_tcp` is running `sudo up addr add 192.168.0.1/24 dev tun0` and we apponted this ip addr to our interface
```
7: tun0: <POINTOPOINT,MULTICAST,NOARP> mtu 1500 qdisc noop state DOWN group default qlen 500
    link/none 
    inet 192.168.0.1/24 scope global tun0
       valid_lft forever preferred_lft forever
```
- `ip link set up dev tun0` will send some packet over to our watcher rust programm listening, so we know we set everything up

### more tools
```sh
# to trace packets
tshark -i tun0

# to quickly find our process if we need to kill it manually
pgrep -af target

# manually ping our interface. these get filtered out atm (only tcp-packets continue to get parsed)
ping -I tun0 192.168.0.1

# manually trying to connect via tcp
nc 192.168.0.2 80

```


## Ressources
A TCP/IP Tutorial/Intro:
- https://www.rfc-editor.org/rfc/rfc1180

A list of functionality-enhancements (like timeouts, loss recovery...):
- https://www.rfc-editor.org/rfc/rfc7414
    - rfc-793(obsoleted by 9293 these days):
        - https://www.rfc-editor.org/rfc/rfc793.html
    - rfc-1122(some more settings)
        - https://www.rfc-editor.org/rfc/rfc1122
    - rfc-5681 - TCP Congestion Control
        - https://www.rfc-editor.org/rfc/rfc5681
    - rfc-2398 - Tools for testing TCP Implementations
        - https://www.rfc-editor.org/rfc/rfc2398

For sending/receiving packets we also need basic ip
- rfc-791 the ip protocol:
    - https://www.rfc-editor.org/rfc/rfc791.html

### State Diagram
- Different states a connection can have. We encode these with a rust enum.
```
taken from: https://www.rfc-editor.org/rfc/rfc793.html#page-22

                              +---------+ ---------\      active OPEN
                              |  CLOSED |            \    -----------
                              +---------+<---------\   \   create TCB
                                |     ^              \   \  snd SYN
                   passive OPEN |     |   CLOSE        \   \
                   ------------ |     | ----------       \   \
                    create TCB  |     | delete TCB         \   \
                                V     |                      \   \
                              +---------+            CLOSE    |    \
                              |  LISTEN |          ---------- |     |
                              +---------+          delete TCB |     |
                   rcv SYN      |     |     SEND              |     |
                  -----------   |     |    -------            |     V
 +---------+      snd SYN,ACK  /       \   snd SYN          +---------+
 |         |<-----------------           ------------------>|         |
 |   SYN   |                    rcv SYN                     |   SYN   |
 |   RCVD  |<-----------------------------------------------|   SENT  |
 |         |                    snd ACK                     |         |
 |         |------------------           -------------------|         |
 +---------+   rcv ACK of SYN  \       /  rcv SYN,ACK       +---------+
   |           --------------   |     |   -----------
   |                  x         |     |     snd ACK
   |                            V     V
   |  CLOSE                   +---------+
   | -------                  |  ESTAB  |
   | snd FIN                  +---------+
   |                   CLOSE    |     |    rcv FIN
   V                  -------   |     |    -------
 +---------+          snd FIN  /       \   snd ACK          +---------+
 |  FIN    |<-----------------           ------------------>|  CLOSE  |
 | WAIT-1  |------------------                              |   WAIT  |
 +---------+          rcv FIN  \                            +---------+
   | rcv ACK of FIN   -------   |                            CLOSE  |
   | --------------   snd ACK   |                           ------- |
   V        x                   V                           snd FIN V
 +---------+                  +---------+                   +---------+
 |FINWAIT-2|                  | CLOSING |                   | LAST-ACK|
 +---------+                  +---------+                   +---------+
   |                rcv ACK of FIN |                 rcv ACK of FIN |
   |  rcv FIN       -------------- |    Timeout=2MSL -------------- |
   |  -------              x       V    ------------        x       V
    \ snd ACK                 +---------+delete TCB         +---------+
     ------------------------>|TIME WAIT|------------------>| CLOSED  |
                              +---------+                   +---------+
```

### the datastream beeing transmitted

#### Sequence Numbers
- every byte of data send over a TCP connection has a sequence number. So every byte can/must be acknowledged.
- if we acknowledge say byte 1111, we signal we received every previous byte before 1111 aswell.
- sequence space is finite (`2^32 byte = 4.3 GB`) so it will wrap arround. (need to modulo it etc.)

**The send-sequence needs the following info (check rfc for all values):**

- SND.UNA - whate we have sent, but what has not been acknowledged.
- SND.NXT - where we are going to send from the next time we send.
- SND.WND - how much we are allowed to send. A receiver can limit how much data it can handle (so it doesnt get overwhelmed).
    - so the last point that was acknowledged + size of that window is how much the `server` can send.
- ISS - the number we choose when we start the connection.

```
from: https://www.rfc-editor.org/rfc/rfc793.html#page-19

            1         2          3          4
        ----------|----------|----------|----------
                SND.UNA    SND.NXT    SND.UNA
                                    +SND.WND

1 - old sequence numbers which have been acknowledged
2 - sequence numbers of unacknowledged data
3 - sequence numbers allowed for new data transmission
4 - future sequence numbers which are not yet allowed
```

**The receive-sequence needs the following info (check rfc for all values):**

- RCV.NXT - what we expect the next sequence-byte we receive to be
- RCV.WND - how big our window is
- IRS - the number the other side started counting with.

```
from: https://www.rfc-editor.org/rfc/rfc793.html#page-19
                1          2          3
            ----------|----------|----------
                    RCV.NXT    RCV.NXT
                            +RCV.WND

1 - old sequence numbers which have been acknowledged
2 - sequence numbers allowed for new reception
3 - future sequence numbers which are not yet allowed
```

### 3way Handshake 
- https://www.rfc-editor.org/rfc/rfc793.html#page-26

- the Sequence-Number we start a connection gets picked randomly. This will avoid overlap if a one partner reconnects while the other still has a sequence. The likelyhood of both fitting together is really really slim.
    - in reality its not just random but randomly increasing. So old possible connections get timed out before the cycle repeats.

1. `A --> B` SYN my sequence number is X
2. `A <-- B` ACK your sequence number is X
3. `A <-- B` SYN my sequence number is Y
4. `A --> B` ACK your sequence number is X
5. After this handshake the connection is established (State::ESTAB)
