// NOTE/TODO: this is a work in progress to re-implement the TypeScript transmuxing logic into Rust,
// it is being done, from the beginning of the pipeline to its end.
//
// None of those files are ready, nor optimized, nor used for the moment. You're very welcome to
// improve it.

const MP2T_PACKET_LENGTH: usize = 188;
const SYNC_BYTE: u8 = 0x47;

/// Splits an incoming stream of binary data into single MPEG-2 Transport
/// Stream packets.
struct TransportStreamSplitter {
    input: Vec<u8>,
    incomplete_packet_buffer: Vec<u8>,
    bytes_in_incomplete_packet_buffer: usize,
    start_index: usize,
    end_index: usize,
}

impl TransportStreamSplitter {
    pub(super) fn new() -> Self {
        Self {
            input: vec![],
            incomplete_packet_buffer: Vec::with_capacity(MP2T_PACKET_LENGTH),
            bytes_in_incomplete_packet_buffer: 0,
            start_index: 0,
            end_index: MP2T_PACKET_LENGTH,
        }
    }

    pub(super) fn feed(&mut self, bytes: Vec<u8>) {
        // If there are bytes remaining from the last segment, prepend them to the
        // bytes that were pushed in
        if self.bytes_in_incomplete_packet_buffer > 0 {
            let mut new_input =
                Vec::with_capacity(bytes.len() + self.bytes_in_incomplete_packet_buffer);
            new_input.extend(
                self.incomplete_packet_buffer[0..self.bytes_in_incomplete_packet_buffer].iter(),
            );
            new_input.extend(bytes);
            self.input = new_input;
            self.bytes_in_incomplete_packet_buffer = 0;
        } else {
            self.input = bytes;
        }

        self.start_index = 0;
        self.end_index = MP2T_PACKET_LENGTH;
    }

    pub fn read_next_packet(&mut self) -> (Option<&[u8]>, bool) {
        if self.input.is_empty() {
            return (None, true);
        }

        // While we have enough data for a packet
        while self.end_index < self.input.len() {
            // Look for a pair of start and end sync bytes in the data..
            if self.input[self.start_index] == SYNC_BYTE && self.input[self.end_index] == SYNC_BYTE
            {
                // We found a packet so emit it and jump one whole packet forward
                let data = &self.input[self.start_index..self.end_index];
                self.start_index += MP2T_PACKET_LENGTH;
                self.end_index += MP2T_PACKET_LENGTH;
                let is_ended = self.start_index >= self.input.len();
                return (Some(data), is_ended);
            }
            // If we get here, we have somehow become de-synchronized and we need to step
            // forward one byte at a time until we find a pair of sync bytes that denote
            // a packet
            self.start_index += 1;
            self.end_index += 1;
        }

        // If there was some data left over at the end of the segment that couldn't
        // possibly be a whole packet, keep it because it might be the start of a packet
        // that continues in the next segment
        if self.start_index < self.input.len() {
            // self.incompletePacketBuffer.set(
            //     self.input.subarray(self.start_index),
            //     0
            // );
            self.bytes_in_incomplete_packet_buffer = self.input.len() - self.start_index;
        } else {
            self.bytes_in_incomplete_packet_buffer = 0;
        }
        // If the buffer contains a whole packet when we are being flushed, emit it
        // and empty the buffer. Otherwise hold onto the data because it may be
        // important for decoding the next segment
        if self.bytes_in_incomplete_packet_buffer == MP2T_PACKET_LENGTH
            && self.input[0] == SYNC_BYTE
        {
            self.bytes_in_incomplete_packet_buffer = 0;
            (Some(&self.input[self.start_index..]), true)
        } else {
            (None, true)
        }
    }

    pub(super) fn reset(&mut self) {
        self.input.clear();
        self.bytes_in_incomplete_packet_buffer = 0;
        self.start_index = 0;
        self.end_index = MP2T_PACKET_LENGTH;
    }
}
