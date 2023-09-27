use std::io;


fn main() ->io::Result<()> {
    println!("starting");

    let nic = tun_tap::Iface::new("tun0", tun_tap::Mode::Tun)?;
    let mut buf = [0u8; 1504];

    loop {
        // bytes we received
        let nbytes = nic.recv(&mut buf[..])?;
        // info on the ETHERNET FRAME we got:
        let _eth_flags = u16::from_be_bytes([buf[0], buf[1]]);
        let eth_proto = u16::from_be_bytes([buf[2], buf[3]]);

        // proto=0x0800->Ipv4-packet | proto=0x86dd->Ipv6-packet
        if eth_proto != 0x0800 { continue; }    // we ignore all but ipv4
        // eprint!("tun-flags: {:X}, tun-proto: {:x}, || ", _eth_flags, eth_proto);
 
        match  etherparse::Ipv4HeaderSlice::from_slice(&buf[4..nbytes]) {
            Ok(p) => {
                // info on IP-PACKET we got:
                let src = p.source_addr();
                let dst = p.destination_addr();
                let plen = p.payload_len();
                let proto = p.protocol();   // ex 1=ping | 6=TCP
                if proto != 0x06 { continue; }  // we ignore all but TCP


                eprintln!("{src}->{dst} proto:{proto}, bytes-payload:{plen}");
                eprintln!("got {} bytes of ipv4: {:x?}", nbytes - 4, p.payload_len());
            },
            Err(err) => {
                eprintln!("ignoring packet. err:{err:?}");
            },
        }       
    }
    //Ok(())
}

