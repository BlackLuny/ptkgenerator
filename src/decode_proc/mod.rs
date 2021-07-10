use winapi::um::winnt::HANDLE;

use crate::mem_cacher::MemCacher;
use libipt::insn::{Insn, InsnDecoder};
use libipt::Asid;
use libipt::ConfigBuilder;
use libipt::Image;

use super::post_proc::InsInfo;
use super::pt_ctrl::read_process_memory_drv;
use std::collections::HashMap;

pub fn decode(
    h: usize,
    pid: usize,
    data: &mut [u8],
    cacher: &mut MemCacher,
    data_rst: &mut HashMap<usize, [InsInfo; 4096]>,
) -> usize {
    let mut cnt = 0;
    let cfg = ConfigBuilder::new(data).unwrap().finish();
    let mut decoder = InsnDecoder::new(&cfg).unwrap();

    if decoder.sync_forward().is_err() {
        println!("not async");
        return 0;
    }
    let f = move |rst: &mut [u8], addr: u64| {
        if read_process_memory_drv(h as HANDLE, pid as u32, addr, rst.len() as u16, rst).is_err() {
            println!("Read mem as addr: 0x{:X} err!", addr);
            return 0;
        }
        rst.len()
    };
    cacher.f = Some(Box::new(f));

    let cb = |rst: &mut [u8], addr: u64, _: Asid| cacher.get_content(rst, addr) as i32;

    let mut img = Image::new(None).unwrap();
    img.set_callback(Some(cb)).unwrap();

    decoder.set_image(Some(&mut img)).unwrap();

    // fetch event
    while let Ok((_, s)) = decoder.event() {
        if !s.event_pending() {
            break;
        }
    }
    loop {
        match decoder.next() {
            Ok((i, _)) => {
                cnt += 1;
                let addr = i.ip();
                let off = addr as usize & 0xfff;
                let page = addr as usize & (!0xfff);
                if let Some(it) = data_rst.get_mut(&page) {
                    it[off].exec_cnt += 1;
                } else {
                    let mut tmp = [InsInfo::default(); 4096];
                    tmp[off].exec_cnt += 1;
                    data_rst.insert(page, tmp);
                }
                // fetch event
                while let Ok((_, s)) = decoder.event() {
                    if !s.event_pending() {
                        break;
                    }
                }
            }
            Err(e) => match e.code() {
                libipt::PtErrorCode::Nosync => {
                    if decoder.sync_forward().is_err() {
                        break;
                    }
                    continue;
                }
                libipt::PtErrorCode::BadQuery => {
                    if decoder.sync_forward().is_err() {
                        break;
                    }
                    continue;
                }
                _ => {
                    break;
                }
            },
        }
    }
    cnt
}
