use crate::{
    binary::pcap::debug_block, ByteSource, Error as SourceError, ReloadInfo, SourceFilter,
    TransportProtocol,
};
use buf_redux::Buffer;
use log::{debug, error, trace};
use pcap_parser::{traits::PcapReaderIterator, PcapBlockOwned, PcapError, PcapNGReader};
use std::io::Read;

pub struct PcapngByteSource<R: Read> {
    pcapng_reader: PcapNGReader<R>,
    buffer: Buffer,
    last_know_timestamp: Option<u64>,
    total: usize,
}

impl<R: Read> PcapngByteSource<R> {
    pub fn new(reader: R) -> Result<Self, SourceError> {
        Ok(Self {
            pcapng_reader: PcapNGReader::new(65536, reader)
                .map_err(|e| SourceError::Setup(format!("{e}")))?,
            buffer: Buffer::new(),
            last_know_timestamp: None,
            total: 0,
        })
    }
}

impl<R: Read + Send + Sync> ByteSource for PcapngByteSource<R> {
    async fn reload(
        &mut self,
        filter: Option<&SourceFilter>,
    ) -> Result<Option<ReloadInfo>, SourceError> {
        let raw_data;
        let mut consumed;
        let mut skipped = 0usize;
        loop {
            match self.pcapng_reader.next() {
                Ok((bytes_read, block)) => {
                    self.total += bytes_read;
                    trace!(
                        "PcapngByteSource::reload, bytes_read: {} (total: {})",
                        bytes_read,
                        self.total
                    );
                    consumed = bytes_read;
                    match block {
                        PcapBlockOwned::NG(pcap_parser::Block::EnhancedPacket(ref epb)) => {
                            trace!("Enhanced package");
                            let ts_us: u64 = (epb.ts_high as u64) << 32 | epb.ts_low as u64;
                            self.last_know_timestamp = Some(ts_us / 1000);
                            raw_data = &epb.data[..epb.caplen as usize];
                            break;
                        }
                        PcapBlockOwned::NG(pcap_parser::Block::SimplePacket(ref spb)) => {
                            trace!("SimplePacket");
                            raw_data = &spb.data[..spb.origlen as usize];
                            break;
                        }
                        other_type => {
                            debug_block(other_type);
                            skipped += consumed;
                            debug!("skipped in total {} bytes", skipped);
                            self.pcapng_reader.consume(consumed);
                            continue;
                        }
                    }
                }
                Err(PcapError::Eof) => {
                    debug!("reloading from pcap file, EOF");
                    return Ok(None);
                }
                Err(PcapError::Incomplete) => {
                    trace!("reloading from pcap file, Incomplete");
                    self.pcapng_reader
                        .refill()
                        .expect("refill pcapng reader failed");
                    // continue;
                }
                Err(e) => {
                    let m = format!("{e}");
                    error!("reloading from pcap file, {}", m);
                    return Err(SourceError::Unrecoverable(m));
                }
            }
        }
        let res = match etherparse::SlicedPacket::from_ethernet(raw_data) {
            Ok(value) => {
                skipped += consumed - value.payload.len();
                match (value.transport, filter) {
                    (
                        Some(actual),
                        Some(SourceFilter {
                            transport: Some(wanted),
                        }),
                    ) => {
                        let actual_tp: TransportProtocol = actual.into();
                        let received_bytes = self.buffer.copy_from_slice(value.payload);
                        let availabe_bytes = self.buffer.len();
                        if actual_tp == *wanted {
                            Ok(Some(ReloadInfo::new(
                                received_bytes,
                                availabe_bytes,
                                skipped,
                                self.last_know_timestamp,
                            )))
                        } else {
                            Ok(Some(ReloadInfo::new(
                                0,
                                0,
                                value.payload.len() + skipped,
                                self.last_know_timestamp,
                            )))
                        }
                    }
                    _ => {
                        let copied = self.buffer.copy_from_slice(value.payload);
                        let availabe_bytes = self.buffer.len();
                        Ok(Some(ReloadInfo::new(
                            copied,
                            availabe_bytes,
                            skipped,
                            self.last_know_timestamp,
                        )))
                    }
                }
            }
            Err(e) => Err(SourceError::Unrecoverable(format!(
                "error trying to extract data from ethernet frame: {e}"
            ))),
        };
        // bytes are copied into buffer and can be dropped by pcap reader
        trace!("consume {} processed bytes", consumed);
        self.pcapng_reader.consume(consumed);
        res
    }

    fn current_slice(&self) -> &[u8] {
        self.buffer.buf()
    }

    fn consume(&mut self, offset: usize) {
        self.buffer.consume(offset);
    }

    fn len(&self) -> usize {
        self.buffer.len()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        binary::pcap::ng::PcapngByteSource,
        tests::{general_source_reload_test, mock_read::MockRepeatRead},
        ByteSource,
    };
    use env_logger;

    const SAMPLE_PCAPNG_DATA: &[u8] = &[
        /*0*/
        // section header block
        /* blocktype */ 0x0a, 0x0d, 0x0d, 0x0a,
        /* len */ 0x1c, 0x00, 0x00, 0x00, //
        /* magic number */ 0x4d, 0x3c, 0x2b, 0x1a, //
        /* version major/minor */ 0x01, 0x00, 0x00, 0x00, //
        /* timezone/accuracy */ 0xff, 0xff, 0xff, 0xff, //
        /* section le */ 0xff, 0xff, 0xff, 0xff, 0x1c, 0x00, 0x00, 0x00, // ---
        /*28*/
        // interface description block
        0x01, 0x00, 0x00, 0x00, /* blocktype */
        0x14, 0x00, 0x00, 0x00, /* len */
        0x01, 0x00, /* LINKTYPE_ETHERNET */
        0x00, 0x00, /* reserved */
        0x00, 0x00, 0x04, 0x00, /* snap-len */
        0x14, 0x00, 0x00, 0x00, // ---
        /*48 */
        // enhanced packet block
        0x06, 0x00, 0x00, 0x00, /* blocktype */
        0x84, 0x00, 0x00, 0x00, /* blocklen */
        0x00, 0x00, 0x00, 0x00, /* interface-id */
        0xf4, 0xc0, 0x05, 0x00, 0xa6, 0x90, 0x75, 0x80, /*timestamp */
        0x62, 0x00, 0x00, 0x00, /* captured packet len */
        0x62, 0x00, 0x00, 0x00, /* orig. packet len */
        // start of ethernet packet -------------------
        /*82*/
        0xb8, 0x27, 0xeb, 0x1d, 0x24, 0xc9, 0xb8, 0x27, 0xeb, 0x98, 0x94, 0xfa, 0x08, 0x00, 0x45,
        0x00, 0x00, 0x54, 0xa0, 0x48, 0x40, 0x00, 0x40, 0x11, 0x29, 0x85, 0xac, 0x16, 0x0c, 0x4f,
        0xac, 0x16, 0x0c, 0x50, // start of udp frame  -------------------------
        0xc3, 0x50, 0xc3, 0x50, 0x00, 0x40, 0x8e, 0xe3,
        // start of udp payload 56 bytes ---------------------------
        0xff, 0xff, 0x81, 0x00, 0x00, 0x00, 0x00, 0x30, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x02,
        0x00, 0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x01, 0x00, 0x00, 0x10, 0x01, 0x03,
        0x00, 0x01, 0x01, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x00,
        0x09, 0x04, 0x00, 0xac, 0x16, 0x0c, 0x4f, 0x00, 0x11, 0x75, 0x30,
        // --- end of ethernet packet ---------------------------
        // --- start of pcapng end ---------------------------
        0x00, 0x00, 0x84, 0x00, 0x00, 0x00,
    ];

    #[tokio::test]
    async fn test_read_one_message_from_pcapng() {
        let _ = env_logger::try_init();
        let udp_payload = &SAMPLE_PCAPNG_DATA[118..=173];
        let pcapng_file = std::io::Cursor::new(SAMPLE_PCAPNG_DATA);

        let mut source = PcapngByteSource::new(pcapng_file).expect("cannot create source");
        let reload_info = source.reload(None).await.expect("reload should work");
        println!("reload_info: {:?}", reload_info);
        let slice = source.current_slice();
        println!("slice: {:x?}", slice);
        assert_eq!(slice.len(), 56);
        assert_eq!(slice, udp_payload);
    }

    #[tokio::test]
    async fn test_general_source_reload() {
        let reader = MockRepeatRead::new(SAMPLE_PCAPNG_DATA.to_vec());
        let mut source = PcapngByteSource::new(reader).unwrap();

        general_source_reload_test(&mut source).await;
    }
}
