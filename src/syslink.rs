
use embedded_hal::serial;

#[derive(Clone)]
pub struct SyslinkPacket {
    pub packet_type: u8,
    pub buffer: [u8; 32],
    pub length: usize,
    cksum: u16,
}

impl Default for SyslinkPacket {
    fn default() -> Self {
        SyslinkPacket {
            packet_type: 0,
            buffer: [0; 32],
            length: 0,
            cksum: 0,
        }
    }
}

impl SyslinkPacket {
    fn check_checksum(&self) -> bool {
        let mut a = self.packet_type;
        let mut b = a;

        a = a.wrapping_add(self.length as u8);
        b = b.wrapping_add(a);

        for (n, data) in self.buffer.iter().enumerate() {
            if n == self.length as usize {
                break;
            }

            a = a.wrapping_add(*data);
            b = b.wrapping_add(a);
        }

        let checksum: u16 = (a as u16) + ((b as u16) << 8);
        
        // defmt::info!("cksum: {:u16}, calculated: {:u16}, {:u8} {:u8}", self.cksum, checksum, b, a);

        checksum == self.cksum
    }
}

#[derive(defmt::Format)]
enum State {
    ReadBC,
    ReadCF,
    ReadType,
    ReadLen,
    ReadData,
    ReadCK0,
    ReadCK1,
}

pub struct Syslink<RX: serial::Read<u8>, TX: serial::Write<u8>> {
    state: State,
    rx: RX,
    tx: TX,
    received_packet: SyslinkPacket,
    received_bytes: usize,
}

impl <RX: serial::Read<u8>, TX: serial::Write<u8>> Syslink<RX, TX>  {

    pub fn new(rx: RX, tx: TX) -> Self {
        Syslink {
            state: State::ReadBC,
            rx, tx,
            received_packet: SyslinkPacket::default(),
            received_bytes: 0,
        }
    }

    pub fn receive(&mut self) -> nb::Result<SyslinkPacket, nb::Error<()>> {

        if let Ok(b) = self.rx.read() {

            // defmt::info!("{:?}: {:u8}", self.state, b);

            match self.state {
                State::ReadBC => {
                    self.received_bytes = 0;
                    if b == 0xBC { self.state = State::ReadCF; }
                },
                State::ReadCF => {
                    if b == 0xCF { self.state = State::ReadType; }
                    else { self.state = State::ReadBC; }
                },
                State::ReadType => {
                    self.received_packet.packet_type = b;
                    self.state = State::ReadLen;
                },
                State::ReadLen => {
                    self.received_packet.length = b as usize;
                    if self.received_packet.length > 0 {
                        self.state = State::ReadData;
                    } else {
                        self.state = State::ReadCK0;
                    }
                },
                State::ReadData => {
                    self.received_packet.buffer[self.received_bytes] = b;
                    self.received_bytes += 1;
                    
                    if self.received_bytes >= self.received_packet.length {
                        self.state = State::ReadCK0;
                    }
                },
                State::ReadCK0 => {
                    self.received_packet.cksum = b as u16;
                    self.state = State::ReadCK1;
                },
                State::ReadCK1 => {
                    self.received_packet.cksum |= (b as u16)<<8;
                    self.state = State::ReadBC;

                    if self.received_packet.check_checksum() {
                        return Ok(self.received_packet.clone());
                    } else {
                        defmt::warn!("Wrong checksum!");
                        self.state = State::ReadBC;
                    }
                    
                },
            }
        }

        Err(nb::Error::WouldBlock)
    }
}