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


## Notes

When implementing (your own) tcp, one problem is, that the kernel already implements it's own tcp-stack. That can interfere with our packets etc.
- the solution here, TUN/TAP - https://www.gabriel.urdhr.fr/2021/05/08/tuntap/
- this way the kernel will basically create a virtual networkcard/networkinterface (the TUN) we can use for our tcp-implementation.
- `cargo add tun_tap`
- to enable networking capability without always having to run it as sudo we can: `sudo setcap cap_net_admin=eip ./target/release/rs_tcp`

## currently
- video 1 - https://www.youtube.com/watch?v=bzja9fQWzdA&t=852s - 0:27:20 