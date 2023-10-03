use std::collections::HashMap;
use std::io;
use std::net::Ipv4Addr;

mod tcp;

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
struct Quad {
    src: (Ipv4Addr, u16),
    dst: (Ipv4Addr, u16),
}

fn main() -> io::Result<()> {
    println!("starting");

    let mut connections: HashMap<Quad, tcp::Connection> = Default::default();

    let mut nic = tun_tap::Iface::without_packet_info("tun0", tun_tap::Mode::Tun)?;
    let mut buf = [0u8; 1504];

    loop {
        // bytes we received
        let nbytes = nic.recv(&mut buf[..])?;

        // we match against the expected Ipv4-Packet
        match etherparse::Ipv4HeaderSlice::from_slice(&buf[..nbytes]) {
            Ok(iph) => {
                // info on IP-PACKET we got:
                let src = iph.source_addr();
                let dst = iph.destination_addr();
                let _payloadlen = iph.payload_len();
                let proto = iph.protocol(); // ex 1=ping | 6=TCP
                if proto != 0x06 {
                    continue;
                } // we ignore all but TCP

                // we match against the expected TCP-Packet
                let iph_size = iph.slice().len();
                match etherparse::TcpHeaderSlice::from_slice(&buf[iph_size..nbytes]) {
                    Ok(tcph) => {
                        use std::collections::hash_map::Entry;
                        let tcph_size = tcph.slice().len();
                        let data_start_idx = iph_size + tcph_size;

                        match connections.entry(Quad {
                            src: (src, tcph.source_port()),
                            dst: (dst, tcph.destination_port()),
                        }) {
                            Entry::Occupied(mut c) => {
                                // existing connecion
                                c.get_mut().on_packet(
                                    &mut nic,
                                    iph,
                                    tcph,
                                    &buf[data_start_idx..nbytes],
                                )?;
                            }
                            Entry::Vacant(e) => {
                                //
                                if let Some(mut c) = tcp::Connection::accept(
                                    &mut nic,
                                    iph,
                                    tcph,
                                    &buf[data_start_idx..nbytes],
                                )? {
                                    e.insert(c);
                                }
                            }
                        }

                        //eprintln!("{src}->{dst} (proto:{proto}|{}bytes-payload) port:{}", tcph.slice().len(), tcph.destination_port());
                    }
                    Err(err) => eprintln!("Ignoring bad-TCP-packet. With err:{err:?}"),
                }
                //eprintln!("got {} bytes of ipv4: payload:{:x?}", nbytes - 4, p.payload_len());
            }
            Err(err) => {
                // eprintln!("Ignoring bad-IP-packet. With err:{err:?}")
            }
        }
    }
    //Ok(())
}
