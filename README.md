# Implementing basic TCP in rust

Along the Video https://www.youtube.com/watch?v=bzja9fQWzdA&list=PLqbS7AVVErFivDY3iKAQk3_VAm8SXwt1X&index=11 by Jon Gjengset on Youtube

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

# manually ping our interface
ping -I tun0 192.168.0.1
# manually trying to connect via tcp
nc 192.168.0.2 80

```
## currently
- video 1 - 0:50:00 
- https://www.youtube.com/watch?v=bzja9fQWzdA&t=3000s