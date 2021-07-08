use std::collections::HashSet;

use winapi::um::winnt::HANDLE;

use libipt::{Config, ConfigBuilder};
use libipt::insn::InsnDecoder;
use libipt::Image;
use libipt::Asid;
use crate::mem_cacher::MemCacher;

use super::pt_ctrl::read_process_memory_drv;

pub fn decode(h: usize, pid: usize, data:&mut [u8], cacher: &mut MemCacher)->usize
{
    let mut cnt = 0;
    let cfg = ConfigBuilder::new(data).unwrap().finish();
    let mut decoder = InsnDecoder::new(&cfg).unwrap();

    if decoder.sync_forward().is_err() {
        println!("not async");
        return 0;
    }
    let f = move |rst:&mut [u8], addr: u64| {
        if read_process_memory_drv(h as HANDLE, pid as u32, addr, rst.len() as u16, rst).is_err() {
            println!("Read mem as addr: 0x{:X} err!", addr);
            return 0;
        }
        rst.len()
    };
    cacher.f = Some(Box::new(f));    

    let cb = |rst:&mut [u8], addr: u64, _: Asid| {
        // for i in 0..rst.len() {
        //     rst[i] = unsafe {
        //         *((addr + i as u64) as *const u8)
        //     };
        // }
        cacher.get_content(rst, addr) as i32
    };

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
            Ok((_,_)) => {
                cnt+=1;
    
                // fetch event
                while let Ok((_, s)) = decoder.event() {
                    if !s.event_pending() {
                        break;
                    }
                }
            }
            Err(e) => {
                match e.code() {
                    libipt::PtErrorCode::Nosync => {
                        if decoder.sync_forward().is_err() {
                            break;
                        }
                        continue;
                    }
                    _ => {
                        break;
                    }
                    
                }
            }
        }
    }

    cnt
}