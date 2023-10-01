use std::io;

pub enum State {
    Closed,
    Listen,
    SynRcvd,
    // Estab,
}

/// Transmission Control Block(TCB). stores all connection records.
/// - since tcp might have to resend packets it needs to keep track of what it sent
pub struct Connection {
    state: State,
    send: SendSequence,
    recv: RecvSequence,
}

/// state of the Send Sequence - https://www.rfc-editor.org/rfc/rfc793.html#page-19
struct SendSequence {
    /// send unacknowledged:
    una: u32,
    /// send next:
    nxt: u32,
    /// send window:
    wnd: u16,
    /// send urgent pointer
    up: bool,
    /// sement sequence number used for last window update:
    wl1: usize,
    /// segment acknowledgment number used for last window update:
    wl2: usize,
    /// initial send sequance number
    iss: u32,
}

/// state of the Send Sequence https://www.rfc-editor.org/rfc/rfc793.html#page-19
struct RecvSequence {
    /// receive next:
    nxt: u32,
    /// receive window:
    wnd: u16,
    /// receive urgent pointer:
    up: bool,
    //// initial receive sequence number:
    irs: u32,
}

impl Connection {
    /// someone tries to start a tcp handshake with us, so we send our info back
    ///  and create the Connection struct for that Connection so we can track it.
    pub fn accept<'a>(
        nic: &mut tun_tap::Iface,
        iph: etherparse::Ipv4HeaderSlice<'a>,
        tcph: etherparse::TcpHeaderSlice<'a>,
        data: &'a [u8],
    ) -> io::Result<Option<Self>> {
        let mut buf = [0u8; 1500];

        if !tcph.syn() {
            return Ok(None); // we only expect/allow SYN packet from unknown.
        }
        // rcv SYN -> snd SYN,ACK (gets sent back) -> connection gets created
        let iss = 0; // default 1460?
        let mut c = Connection {
            state: State::SynRcvd,
            send: SendSequence {
                // set stuff for our answer back:
                iss,
                una: iss, // TODO: randomize this as per spec
                nxt: iss + 1,
                wnd: 10,
                up: false,
                wl1: 0,
                wl2: 0,
            },
            recv: RecvSequence {
                // we keep track of sender-info / aka. client-info
                nxt: tcph.sequence_number() + 1,
                wnd: tcph.window_size(),
                up: false,
                irs: tcph.sequence_number(),
            },
        };

        // we construct the tcp-header and set its proper flags
        let mut syn_ack = etherparse::TcpHeader::new(
            tcph.destination_port(),
            tcph.source_port(),
            c.send.iss,
            c.send.wnd,
        );
        syn_ack.acknowledgment_number = c.recv.nxt;
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
        
        // manually calculate the checksum for our outgoing packet
        syn_ack.checksum = syn_ack.calc_checksum_ipv4(&ip, &[]).expect("unable to compute checksum!");

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
        print!("got ");
        dbg_print_incoming_packet(iph, tcph);
        dbg_print_response_packet(&buf, unwritten);

        nic.send(&buf[..buf.len() - unwritten])?;
        return Ok(Some(c));
    }

    pub fn on_packet<'a>(
        &mut self,
        nic: &mut tun_tap::Iface,
        iph: etherparse::Ipv4HeaderSlice<'a>,
        tcph: etherparse::TcpHeaderSlice<'a>,
        data: &'a [u8],
    ) -> io::Result<()> {
        self.dbg_print_packet(iph, tcph, data);
        unimplemented!()
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

fn dbg_print_incoming_packet(iph: etherparse::Ipv4HeaderSlice<'_>, tcph: etherparse::TcpHeaderSlice<'_>)  {
    eprintln!("got iph {:02x?} and tcph {:02x?}\n", iph, tcph);
}

fn dbg_print_response_packet(buf: &[u8], unwritten: usize) {
    eprintln!("reponding with {:02x?}\n", &buf[.. buf.len() - unwritten])
}