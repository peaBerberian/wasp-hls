// NOTE/TODO: this is a work in progress to re-implement the TypeScript transmuxing logic into Rust,
// it is being done, from the beginning of the pipeline to its end.
//
// None of those files are ready, nor optimized, nor used for the moment. You're very welcome to
// improve it.

/// Parser for exponential Golomb codes, a variable-bitwidth number encoding
/// scheme used by h264.
///
/// It is a very simple number encoding:
///   1. Write down x+1 in binary
///   2. Count the bits written, subtract one, and write that number of starting zero bits preceding
///     the previous bit string.
pub(super) struct ExpGolomb<'a> {
    /// The current data that is being read
    working_data: &'a [u8],
    /// The number of bytes left to examine in `working_data`
    /// TODO needed? Why not just "sliding" the `working_data` slice?
    working_bytes_available: usize,
    /// The current word being examined. 32 bits maximum here.
    working_word: u32,
    /// The number of bits left to examine in the current word
    working_bits_available: u64,
}

impl<'a> ExpGolomb<'a> {
    pub(super) fn new(working_data: &'a [u8]) -> Self {
        Self {
            working_data,
            working_bytes_available: working_data.len(),
            working_word: 0,
            working_bits_available: 0,
        }
    }

    pub fn len(&self) -> usize {
        8 * self.working_bytes_available
    }

    pub fn bits_available(&self) -> u64 {
        8 * (self.working_bytes_available as u64) + self.working_bits_available
    }

    pub fn load_word(&mut self) {
        let position = self.working_data.len() - self.working_bytes_available;
        let available_bytes = usize::min(4, self.working_bytes_available);

        if available_bytes == 0 {
            self.working_word = 0;
            return;
        }

        self.working_word = match available_bytes {
            4 => {
                (self.working_data[position] as u32) << 24
                    | (self.working_data[position + 1] as u32) << 16
                    | (self.working_data[position + 2] as u32) << 8
                    | (self.working_data[position + 3] as u32)
            }
            3 => {
                (self.working_data[position] as u32) << 16
                    | (self.working_data[position + 1] as u32) << 8
                    | self.working_data[position + 2] as u32
            }
            2 => (self.working_data[position] as u32) << 8 | self.working_data[position + 1] as u32,
            _ => self.working_data[0] as u32,
        };

        // track the amount of workingData that has been processed
        self.working_bits_available = (available_bytes as u64) * 8;
        self.working_bytes_available -= available_bytes;
    }

    pub fn skip_bits(&mut self, count: u64) {
        if self.working_bits_available > count {
            self.working_word <<= count;
            self.working_bits_available -= count;
        } else {
            let mut used_count = count;
            used_count -= self.working_bits_available;
            let skip_bytes = used_count / 8;
            used_count -= skip_bytes * 8;
            self.working_bytes_available -= skip_bytes as usize;
            self.load_word();
            self.working_word <<= used_count;
            self.working_bits_available -= used_count;
        }
    }

    pub fn read_bits(&mut self, size: u64) -> u32 {
        let mut bits = u64::min(self.working_bits_available, size);
        let valu = self.working_word >> (32 - bits);
        self.working_bits_available -= bits;
        if self.working_bits_available > 0 {
            self.working_word <<= bits;
        } else if self.working_bytes_available > 0 {
            self.load_word();
        }

        bits = size - bits;
        if bits > 0 {
            (valu << bits) | self.read_bits(bits)
        } else {
            valu
        }
    }

    pub fn skip_leading_zeros(&mut self) -> u64 {
        for leading_zero_count in 0..self.working_bits_available {
            if (self.working_word & (0x80000000 >> leading_zero_count)) != 0 {
                // the first bit of working word is 1
                self.working_word <<= leading_zero_count;
                self.working_bits_available -= leading_zero_count;
                return leading_zero_count;
            }
        }

        // we exhausted workingWord and still have not found a 1
        let working_bits = self.working_bits_available;
        self.load_word();
        working_bits + self.skip_leading_zeros()
    }

    pub fn skip_unsigned(&mut self) {
        let skipped = self.skip_leading_zeros();
        self.skip_bits(1 + skipped);
    }

    pub fn skip_signed(&mut self) {
        self.skip_unsigned();
    }

    pub fn read_unsigned(&mut self) -> u32 {
        let clz = self.skip_leading_zeros();
        self.read_bits(clz + 1) - 1
    }

    pub fn read_signed(&mut self) -> i32 {
        let valu = self.read_unsigned() as i32; // :int
        if 0x01 & valu != 0 {
            // the number is odd if the low order bit is set
            (1 + valu) >> 1 // add 1 to make it even, and divide by 2
        } else {
            -(valu >> 1) // divide by two then make it negative
        }
    }

    pub fn read_boolean(&mut self) -> bool {
        self.read_bits(1) == 1
    }

    pub fn read_unsigned_byte(&mut self) -> u8 {
        self.read_bits(8) as u8
    }
}
