use std::io;

#[derive(Debug)]
pub enum State {
    // Closed,
    // Listen,
    SynRcvd,
    Estab,
    /// we sent fin to signal we want to close down connection
    FinWait1,
    FinWait2,
    /// other party sent us signal to fin => were closing down connection
    // CloseWait,
    TimeWait,
}

impl State {
    fn is_synchronized(&self) -> bool {
        match *self {
            State::SynRcvd => false,
            State::Estab => true,
            State::FinWait1 => true,
            State::FinWait2 => true,
            State::TimeWait => true,
        }
    }
}

#[derive(Debug)]
/// Transmission Control Block(TCB). stores all connection records.
/// - since tcp might have to resend packets it needs to keep track of what it sent
pub struct Connection {
    state: State,
    send: SendSequence,
    recv: RecvSequence,
    /// the ip header we use to send
    ip: etherparse::Ipv4Header,
    tcp: etherparse::TcpHeader,
}

#[derive(Debug)]
/// state of the Send Sequence - https://www.rfc-editor.org/rfc/rfc793.html#page-19
struct SendSequence {
    /// send unacknowledged. this is how far we know the other person received since he acked it.
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

#[derive(Debug)]
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
        let iss = 0;
        let wnd = 1024; // default 1460?
        let mut c = Connection {
            state: State::SynRcvd,
            send: SendSequence {
                // set stuff for our answer back:
                iss,
                una: iss, // TODO: randomize this as per spec
                nxt: iss,
                wnd,
                up: false,
                wl1: 0,
                wl2: 0,
            },
            recv: RecvSequence {
                // we keep track of sender-info / aka. client-info
                irs: tcph.sequence_number(),
                nxt: tcph.sequence_number() + 1,
                wnd: tcph.window_size(),
                up: false,
            },
            ip: etherparse::Ipv4Header::new(
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
            ),
            // we construct the tcp-header and set its proper flags
            tcp: etherparse::TcpHeader::new(tcph.destination_port(), tcph.source_port(), iss, wnd),
        };
        c.tcp.syn = true;
        c.tcp.ack = true;
        c.write(nic, &[])?;
        Ok(Some(c))
    }

    // writes however much of the payload it can
    fn write(&mut self, nic: &mut tun_tap::Iface, payload: &[u8]) -> io::Result<usize> {
        let mut buf = [0u8; 1500];
        self.tcp.sequence_number = self.send.nxt;
        self.tcp.acknowledgment_number = self.recv.nxt;
        let size = std::cmp::min(
            buf.len(),
            self.tcp.header_len() as usize + self.ip.header_len() as usize + payload.len(),
        );
        self.ip.set_payload_len(size - self.ip.header_len());
        // kernel does this following checksum for us so no need to actually calculate it:
        self.tcp.checksum = self.tcp.calc_checksum_ipv4(&self.ip, &[])
        .expect("unable to compute checksum!");

        // - we create a slice-pointer unwritten, that points to the entire buf
        // - every time we write() the start of that buffer gets moved forward
        // - unwritten.len() -> returns how much is remaining of the buffer
        use std::io::Write;
        let mut unwritten = &mut buf[..];
        self.ip.write(&mut unwritten);
        self.tcp.write(&mut unwritten)?;
        let payload_bytes = unwritten.write(payload)?; // write as much as possible (might be too much data)
        let unwritten = unwritten.len();
        // dbg_print_response_packet(&buf, unwritten);
        // increment our sequence number:
        self.send.nxt = self.send.nxt.wrapping_add(payload_bytes as u32);
        if self.tcp.syn {
            self.send.nxt = self.send.nxt.wrapping_add(1);
            self.tcp.syn = false;
        }
        if self.tcp.fin {
            self.send.nxt = self.send.nxt.wrapping_add(1);
            self.tcp.fin = false;
        }
        // write out or packet:
        nic.send(&buf[..buf.len() - unwritten])?;
        Ok(payload_bytes)
    }

    // sends a reset packet
    fn send_reset(&mut self, nic: &mut tun_tap::Iface) -> io::Result<()> {
        self.tcp.rst = true;
        // TODO: fix sequence numbers here
        // TODO: handle synchroniyed reset
        self.tcp.sequence_number = 0;
        self.tcp.acknowledgment_number = 0;
        self.write(nic, &[])?;
        Ok(())
    }

    // receive a packet on a conn with previous state (ex doing handshake or open connection)
    pub fn on_packet<'a>(
        &mut self,
        nic: &mut tun_tap::Iface,
        iph: etherparse::Ipv4HeaderSlice<'a>,
        tcph: etherparse::TcpHeaderSlice<'a>,
        data: &'a [u8],
    ) -> io::Result<()> {
        //
        // SEQUENCE acceptable CHECK
        // check if segment is in our range that we accept (window-size in bytes)
        // wrong lengths or we receive outside our window we quit out with Ok(()):
        let seqn = tcph.sequence_number(); // first byte of segment
        let mut seg_len = data.len() as u32;
        if tcph.fin() {
            seg_len += 1;
        }
        if tcph.syn() {
            seg_len += 1;
        }
        let seq_end = seqn.wrapping_add(seg_len).wrapping_sub(1);
        // match (seg_len, self.recv.wnd) {
        //     (0, 0) => {
        //         if seqn != self.recv.nxt {
        //             return Ok(());
        //         }
        //     }
        //     (0, 1..) => {
        //         if !is_valid_segment(self.recv.nxt, seqn, self.recv.wnd) {
        //             return Ok(());
        //         }
        //     }
        //     (1.., 0) => {
        //         return Ok(());
        //     }
        //     (1.., 1..) => {
        //         if !is_valid_segment(self.recv.nxt, seqn, self.recv.wnd)
        //             && !is_valid_segment(self.recv.nxt, seq_end, self.recv.wnd)
        //         {
        //             return Ok(());
        //         }
        //     }
        // }
        // og implementation
        let wend = self.recv.nxt.wrapping_add(self.recv.wnd as u32);
        let okay = if seg_len == 0 {
            // zero length segment has separate rules
            if self.recv.wnd == 0 {
                if seqn != self.recv.nxt {
                   false
                } else {
                    true
                }
            } else if !is_between_wrapped(self.recv.nxt.wrapping_sub(1), seqn, wend) {
                false
            } else {
                true
            }
        } else {
            if self.recv.wnd == 0 {
                false
            } else if !is_between_wrapped(self.recv.nxt.wrapping_sub(1), seqn, wend)
                && !is_between_wrapped(
                    self.recv.nxt.wrapping_sub(1),
                    seqn.wrapping_add(seg_len-1),
                    wend,
                )
            {
                false
            } else {
                true
            }
        };
        if !okay {
            self.write(nic, &[]);
            return Ok(());
        }
        self.recv.nxt = seqn.wrapping_add(seg_len);
        // TODO: we should ack that are moving our nxt so other party can move up their una
        // TODO: if seq-check not acceptable send ACK

        if !tcph.ack() {
            return Ok(());
        }

        let ackn = tcph.acknowledgment_number();
        if let State::SynRcvd = self.state {
            if is_between_wrapped(
                self.send.una.wrapping_sub(1),
                ackn,
                self.send.nxt.wrapping_add(1),
            ) {
                self.state = State::Estab;
            } else {
                // TODO: Reset <SEQ=SEG.ACK><CTL=RST>
            }
        }

        if let State::Estab | State::FinWait1 | State::FinWait2 = self.state {
            //
            // ACK-CHECK check if ack we got is ok
            if !is_valid_ack(self.send.una, ackn, self.send.nxt) {
                return Ok(());
            }
            // we take note of how far we know the other party acks (we know he got)
            self.send.una = ackn; // ack is the next byte they are expecting == first unacknowledged byte
                                  // TODO
            assert!(data.is_empty());

            if let State::Estab = self.state {
                // // for now we try to close the connection here:
                // // TODO: needs to be stored in the retransmission queue. (retransmissions might still have to be sent with fin=false)
                self.tcp.fin = true;
                self.write(nic, &[]);
                self.state = State::FinWait1;
            }
        }

        if let State::FinWait1 = self.state {
            // We already sent fin to signal we want to close down connection.
            if self.send.una == self.send.iss + 2 {
                dbg!("they acked our fin!");
                self.state = State::FinWait2;
            }
        }

        if tcph.fin() {
            match self.state {
                State::FinWait2 => {
                    // were done with the connection
                    self.write(nic, &[]);
                    dbg!("they fined!");
                    self.state = State::TimeWait;
                }
                _ => unimplemented!(),
            }
        }
        // self.dbg_print_packet(&iph, &tcph, data);
        Ok(())
    }

    #[allow(dead_code)]
    fn dbg_print_packet<'a>(
        &mut self,
        iph: &etherparse::Ipv4HeaderSlice<'a>,
        tcph: &etherparse::TcpHeaderSlice<'a>,
        data: &'a [u8],
    ) {
        eprintln!(
            "dbg_print_packet() {}:{}->{}:{} || {}b",
            iph.source_addr(),
            tcph.source_port(),
            iph.destination_addr(),
            tcph.destination_port(),
            data.len(),
        );
    }
}

fn is_between_wrapped(start: u32, x: u32, end: u32) -> bool {
    use std::cmp::{Ord, Ordering};
    match start.cmp(&x) {
        Ordering::Equal => return false,
        Ordering::Less => {
            if end >= start && end <= x {
                return false;
            }
        }
        Ordering::Greater => {
            if end < start && end > x {
            } else {
                return false;
            }
        }
    }
    return true;
}

// acceptable ack? = SND.UNA < SEG.ACK =< SND.NXT THIS WRAPS ARROUND if hitting max!
fn is_valid_ack(una: u32, ack: u32, nxt: u32) -> bool {
    if nxt < una {
        // special case, nxt has wrapped arround after hitting 32bit.max
        return ack > una || ack <= nxt;
    }
    return una < ack && ack <= nxt;
}

// in range of window?
// 1) RCV.NXT =< SEG.SEQ           < RCV.NXT + RCV.WND
// 2) RCV.NXT =< SEG.SEQ+SEG.LEN-1 < RCV.NXT + RCV.WND
// 1) checks if beginning is in window, 2) if ending of packet is in window
fn is_valid_segment(nxt: u32, seq: u32, wnd: u16) -> bool {
    let max = nxt.wrapping_add(wnd as u32);
    if max < nxt {
        // special case nxt+wnd has wrapped arround after hitting 32bit.max
        return seq <= max || seq > nxt;
    }
    return (nxt <= seq && seq < max);
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
    fn _is_valid_ack_og(una: u32, ack: u32, nxt: u32) -> bool {
        if una < ack {
            if nxt > una && nxt < ack {
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
        for n in 1..1000000 {
            let una = rand::random::<u32>();
            let ack = rand::random::<u32>();
            let nxt = rand::random::<u32>();
            dbg!(ack, una, nxt);
            assert_eq!(is_valid_ack(ack, una, nxt), _is_valid_ack_og(ack, una, nxt));
            assert_eq!(
                is_valid_ack(una, ack, nxt),
                is_between_wrapped(una, ack, nxt.wrapping_add(1))
            );
        }
    }

    #[test]
    fn test_mine_og_comparison_valid_window() {
        for n in 1..1000000 {
            let nxt = rand::random::<u32>();
            let seq = rand::random::<u32>();
            let wnd = rand::random::<u16>();
            dbg!(nxt, seq, wnd);
            assert_eq!(
                is_valid_segment(nxt, seq, wnd),
                is_between_wrapped(nxt.wrapping_sub(1), seq, nxt.wrapping_add(wnd as u32))
            );
        }
    }
}
