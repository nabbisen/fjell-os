//! Minimal FDT byte-slice parser (RFC v0.5-002 §7.2).
//!
//! Parses only the DTB sub-tree paths needed by Fjell OS.  Unknown nodes
//! are passed to the caller as `DeriveError::UnknownNode`.
//!
//! DTB header layout (big-endian 32-bit fields, offsets from base):
//!   0x00: magic       (0xd00dfeed)
//!   0x04: totalsize
//!   0x08: off_dt_struct
//!   0x0C: off_dt_strings
//!   0x10: off_mem_rsvmap
//!   0x14: version     (must be 17)
//!   0x18: last_comp_version
//!   0x1C: boot_cpuid_phys
//!   0x20: size_dt_strings
//!   0x24: size_dt_struct

pub const FDT_MAGIC:     u32 = 0xd00d_feed;
pub const FDT_VERSION:   u32 = 17;

/// FDT token tags.
pub const FDT_BEGIN_NODE: u32 = 0x0000_0001;
pub const FDT_END_NODE:   u32 = 0x0000_0002;
pub const FDT_PROP:       u32 = 0x0000_0003;
pub const FDT_NOP:        u32 = 0x0000_0004;
pub const FDT_END:        u32 = 0x0000_0009;

/// Errors from the DTB parser.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ParseError {
    /// Buffer too small to be a valid DTB.
    TooSmall         = 0x01,
    /// Magic number mismatch.
    BadMagic         = 0x02,
    /// Unsupported DTB version.
    BadVersion       = 0x03,
    /// Claimed total-size exceeds buffer length.
    SizeMismatch     = 0x04,
    /// Struct block offset is out of range.
    BadStructOffset  = 0x05,
    /// String block offset is out of range.
    BadStrOffset     = 0x06,
    /// An FDT token read was out of range.
    Truncated        = 0x07,
    /// Unexpected or unrecognised token.
    UnexpectedToken  = 0x08,
}

fn be32(b: &[u8], off: usize) -> Option<u32> {
    let s = b.get(off..off+4)?;
    Some(u32::from_be_bytes([s[0],s[1],s[2],s[3]]))
}

fn be64(b: &[u8], off: usize) -> Option<u64> {
    let s = b.get(off..off+8)?;
    Some(u64::from_be_bytes([s[0],s[1],s[2],s[3],s[4],s[5],s[6],s[7]]))
}

/// Parsed DTB header.
#[derive(Clone, Copy, Debug)]
pub struct DtbHeader {
    pub totalsize:       u32,
    pub off_dt_struct:   u32,
    pub off_dt_strings:  u32,
    pub size_dt_strings: u32,
}

/// Parse and validate the DTB header.
pub fn parse_header(dtb: &[u8]) -> Result<DtbHeader, ParseError> {
    if dtb.len() < 0x28 { return Err(ParseError::TooSmall); }
    let magic = be32(dtb, 0x00).ok_or(ParseError::TooSmall)?;
    if magic != FDT_MAGIC { return Err(ParseError::BadMagic); }
    let version = be32(dtb, 0x14).ok_or(ParseError::TooSmall)?;
    if version < FDT_VERSION { return Err(ParseError::BadVersion); }
    let totalsize       = be32(dtb, 0x04).ok_or(ParseError::TooSmall)?;
    let off_dt_struct   = be32(dtb, 0x08).ok_or(ParseError::TooSmall)?;
    let off_dt_strings  = be32(dtb, 0x0C).ok_or(ParseError::TooSmall)?;
    let size_dt_strings = be32(dtb, 0x20).ok_or(ParseError::TooSmall)?;
    if (totalsize as usize) > dtb.len() { return Err(ParseError::SizeMismatch); }
    if (off_dt_struct as usize) >= dtb.len() { return Err(ParseError::BadStructOffset); }
    if (off_dt_strings as usize) >= dtb.len() { return Err(ParseError::BadStrOffset); }
    Ok(DtbHeader { totalsize, off_dt_struct, off_dt_strings, size_dt_strings })
}

/// Read a NUL-terminated string from the strings block.
pub fn get_string<'a>(dtb: &'a [u8], str_off: u32, name_off: u32) -> Option<&'a [u8]> {
    let start = (str_off + name_off) as usize;
    let slice = dtb.get(start..)?;
    let end = slice.iter().position(|&b| b == 0)?;
    Some(&slice[..end])
}

/// An FDT property (parsed from `FDT_PROP` token).
#[derive(Clone, Copy, Debug)]
pub struct FdtProp<'a> {
    pub name:  &'a [u8],
    pub value: &'a [u8],
}

/// A parsed node entry yielded by the iterator.
#[derive(Clone, Copy, Debug)]
pub enum NodeEvent<'a> {
    BeginNode { name: &'a [u8] },
    EndNode,
    Prop(FdtProp<'a>),
}

/// Iterate over FDT struct tokens, yielding `NodeEvent`s.
pub struct FdtIter<'a> {
    dtb:      &'a [u8],
    str_off:  u32,
    pos:      usize,
    end:      usize,
    done:     bool,
}

impl<'a> FdtIter<'a> {
    pub fn new(dtb: &'a [u8], hdr: &DtbHeader) -> Self {
        let pos = hdr.off_dt_struct as usize;
        let end = dtb.len();
        Self { dtb, str_off: hdr.off_dt_strings, pos, end, done: false }
    }
}

impl<'a> Iterator for FdtIter<'a> {
    type Item = Result<NodeEvent<'a>, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done { return None; }
        loop {
            if self.pos + 4 > self.end { return Some(Err(ParseError::Truncated)); }
            let tok = be32(self.dtb, self.pos)?;
            self.pos += 4;
            match tok {
                FDT_NOP => continue,
                FDT_END => { self.done = true; return None; }
                FDT_BEGIN_NODE => {
                    // Node name: NUL-terminated, aligned to 4.
                    let _name_start = self.pos;
                    let slice = &self.dtb[self.pos..self.end];
                    let nul = slice.iter().position(|&b| b == 0)
                        .unwrap_or(slice.len());
                    let name = &slice[..nul];
                    self.pos += nul + 1;
                    self.pos = (self.pos + 3) & !3; // align
                    return Some(Ok(NodeEvent::BeginNode { name }));
                }
                FDT_END_NODE => return Some(Ok(NodeEvent::EndNode)),
                FDT_PROP => {
                    if self.pos + 8 > self.end { return Some(Err(ParseError::Truncated)); }
                    let val_size  = be32(self.dtb, self.pos)? as usize; self.pos += 4;
                    let name_off  = be32(self.dtb, self.pos)?; self.pos += 4;
                    let val_start = self.pos;
                    if val_start + val_size > self.end { return Some(Err(ParseError::Truncated)); }
                    let value = &self.dtb[val_start..val_start + val_size];
                    self.pos += val_size;
                    self.pos = (self.pos + 3) & !3;
                    let name = get_string(self.dtb, self.str_off, name_off)
                        .unwrap_or(b"");
                    return Some(Ok(NodeEvent::Prop(FdtProp { name, value })));
                }
                _ => return Some(Err(ParseError::UnexpectedToken)),
            }
        }
    }
}
