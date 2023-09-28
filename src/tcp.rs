use std::io;

pub enum State {
    Closed,
    Listen,
    // SynRcvd,
    // Estab,
}

/// Transmission Control Block(TCB). stores all connection records.
/// - since tcp might have to resend packets it needs to keep track of what it sent
pub struct Connection {
    state: State,
}

impl Default for Connection {
    fn default() -> Self {
        // Connection {state:: State::Closed} // the proper default a connection should start with
        Connection {
            state: State::Listen // for quick development our default.
        }
    }
}

impl Connection {
    pub fn on_packet<'a>(
        &mut self,
        nic: &mut tun_tap::Iface,
        iph: etherparse::Ipv4HeaderSlice<'a>,
        tcph: etherparse::TcpHeaderSlice<'a>,
        data: &'a [u8],
    ) -> io::Result<usize> {
        let mut buf = [0u8; 1500];
        match *self {
            Connection::Closed => {
                return Ok(0);
            }
            Connection::Listen => {
                if !tcph.syn() {
                    return Ok(0); // we only expect/allow SYN packet from unknown.
                }
                // rcv SYN -> snd SYN,ACK (gets sent back) -> connection gets established

                // we construct the tcp-header and set its proper flags
                let mut syn_ack = etherparse::TcpHeader::new(
                    tcph.destination_port(),
                    tcph.source_port(),
                    unimplemented!(),
                    unimplemented!(),
                );
                syn_ack.syn = true;
                syn_ack.ack = true;

                // we construct the ip-header
                let mut ip = etherparse::Ipv4Header::new(
                    syn_ack.header_len(),
                    64,
                    etherparse::IpTrafficClass::Tcp,
                    [
                        iph.destination()[0],
                        iph.destination()[1],
                        iph.destination()[2],
                        iph.destination()[3],
                    ],
                    [
                        iph.source()[0],
                        iph.source()[1],
                        iph.source()[2],
                        iph.source()[3],
                    ],
                );

                // we construct the the headers into buf
                // - we create a slice that points to the entire buf
                // - every time we write() the start of that buffer gets moved forward
                // - unwritten.len() -> returns how much is remaining of the buffer
                let unwritten = {
                    let mut unwritten = &mut buf[..];
                    ip.write(&mut unwritten);
                    syn_ack.write(&mut unwritten);
                    unwritten.len()
                };
                let test = nic.send(&buf[..unwritten]);
                return Ok(test.unwrap());
            }
        }

        self.dbg_print_packet(iph, tcph, data);
    }

    fn dbg_print_packet<'a>(
        &mut self,
        iph: etherparse::Ipv4HeaderSlice<'a>,
        tcph: etherparse::TcpHeaderSlice<'a>,
        data: &'a [u8],
    ) {
        eprintln!(
            "{}:{}->{}:{} || {}b",
            iph.source_addr(),
            tcph.source_port(),
            iph.destination_addr(),
            tcph.destination_port(),
            data.len(),
        );
    }
}
