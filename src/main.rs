use std::io;


fn main() ->io::Result<()> {
    println!("starting");

    let nic = tun_tap::Iface::new("tun0", tun_tap::Mode::Tun)?;
    let mut buf = [0u8; 1504];

    loop {
        // bytes we received
        let nbytes = nic.recv(&mut buf[..])?;
        // strip off tun-headers
        let flags = u16::from_be_bytes([buf[0], buf[1]]);
        let proto = u16::from_be_bytes([buf[2], buf[3]]);

        // proto=0x0800->Ipv4-packet | proto=0x86dd->Ipv6-packet
        if proto != 0x0800 { continue; }    // we ignore all but ipv4

        eprint!("tun-flags: {:X}, tun-proto: {:x}, ", flags, proto);


        eprintln!("read {} bytes: {:x?}", nbytes - 4, &buf[..nbytes]);
    }
    //Ok(())
}

