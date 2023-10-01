use std::io;

pub enum State {
    // Closed,
    // Listen,
    SynRcvd,
    Estab,
}

/// Transmission Control Block(TCB). stores all connection records.
/// - since tcp might have to resend packets it needs to keep track of what it sent
pub struct Connection {
    state: State,
    send: SendSequence,
    recv: RecvSequence,
    /// the ip header we use to send
    ip: etherparse::Ipv4Header,
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
            ip:  etherparse::Ipv4Header::new(
                0,
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
            )
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
        c.ip.set_payload_len(syn_ack.header_len() as usize + 0);

        // we construct the ip-header

        // kernel does this following checksum for us so no need to actually calculate it:
        // syn_ack.checksum = syn_ack.calc_checksum_ipv4(&c.ip, &[])
            // .expect("unable to compute checksum!");

        // we construct the the headers into buf
        // - we create a slice that points to the entire buf
        // - every time we write() the start of that buffer gets moved forward
        // - unwritten.len() -> returns how much is remaining of the buffer
        let unwritten = {
            let mut unwritten = &mut buf[..];
            c.ip.write(&mut unwritten);
            syn_ack.write(&mut unwritten);
            unwritten.len()
        };

        // dbg_print_incoming_packet(iph, tcph);
        // dbg_print_response_packet(&buf, unwritten);
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
        
        // check if ack we got is ok
        if !is_valid_ack(tcph.acknowledgment_number(), self.send.una, self.send.nxt) { 
            return Ok(());
        }
        // check if segment is in our range that we accept (window-size in bytes)


        self.dbg_print_packet(iph, tcph, data);
        match self.state {
            State::SynRcvd => {
                // we expect an ACK back from the SYN we just sent

                Ok(())
            },
            State::Estab => {
                todo!()
            },
        }
    }
    
    #[allow(dead_code)]
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


// acceptable ack? = SND.UNA < SEG.ACK =< SND.NXT THIS WRAPS ARROUND if hitting max!
fn is_valid_ack(ack:u32, una:u32, nxt:u32)-> bool{
    if nxt < una {
        // special case, nxt has wrapped arround after hitting 32bit.max
        return ack > una || ack <= nxt;
    }
    return una < ack && ack <= nxt;
}

#[allow(dead_code)]
fn dbg_print_incoming_packet(
    iph: etherparse::Ipv4HeaderSlice<'_>,
    tcph: etherparse::TcpHeaderSlice<'_>,
) {
    eprintln!("got iph {:02x?} and tcph {:02x?}\n", iph, tcph);
}

#[allow(dead_code)]
fn dbg_print_response_packet(buf: &[u8], unwritten: usize) {
    eprintln!("reponding with {:02x?}\n", &buf[..buf.len() - unwritten])
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;


    // og implementation, mine makes more sense for myself 
    fn _is_valid_ack_og(ack:u32, una:u32, nxt:u32)-> bool{
        if una < ack {
            if nxt > una && nxt < ack{
                return false;
    
            }
        } else {
            if nxt >= ack && nxt < una {
    
            } else {
                return false;
            }
        }
        return true;
    } 

    #[test]
    fn test_mine_og_comparison_valid_ack() {
       for n in 1..10000 {
        
        let ack = rand::random::<u32>();
        let una = rand::random::<u32>();
        let nxt = rand::random::<u32>();
        dbg!(ack, una, nxt);
        assert_eq!(is_valid_ack(ack, una, nxt), _is_valid_ack_og(ack, una, nxt))
       }
    }
}