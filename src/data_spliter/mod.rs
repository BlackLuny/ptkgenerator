
use std::ptr::{slice_from_raw_parts, slice_from_raw_parts_mut};
use libipt::{ConfigBuilder, Config};
use libipt::insn::InsnDecoder;

pub fn split(data: &mut Vec<u8>)->Vec<usize> {
    let cfg = ConfigBuilder::new(data).unwrap().finish();
    let mut decoder = InsnDecoder::new(&cfg).unwrap();
    let mut tasks = Vec::new();
    while let Ok(s) = decoder.sync_forward() {
        if let Ok(offset) = decoder.sync_offset() {
            tasks.push(offset as usize);
        }
    }
    tasks
}

