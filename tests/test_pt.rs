extern crate ptkgenerator;
#[cfg(test)]
mod test {
    use ptkgenerator::pt_ctrl::*;
    use std::mem::size_of;
    use std::sync::Once;
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;
    use winapi::um::winnt::HANDLE;
    use libipt::ConfigBuilder;
    use libipt::insn::InsnDecoder;
    use libipt::Image;
    use libipt::Asid;

    static mut HWD: HANDLE = INVALID_HANDLE_VALUE;
    static H: Once = Once::new();
    fn test_get_handle() -> HANDLE {
        H.call_once(|| unsafe {
            HWD = get_pt_handle("\\\\.\\PtCollector").expect("Open pt driver failed");
        });
        unsafe { HWD }
    }

    #[test]
    fn test_read_mem() {
        use winapi::um::processthreadsapi::GetCurrentProcessId;
        let handle = test_get_handle();
        let pid = unsafe { GetCurrentProcessId() };
        let tmp_val = 0x11223344u32;
        let size = size_of::<u32>();
        let mut out_buff = vec![0; size];
        let r = read_process_memory_drv(
            handle,
            pid,
            &tmp_val as *const _ as u64,
            size as u16,
            &mut out_buff,
        );
        assert_eq!(r, Ok(()));
        assert_eq!(out_buff, [0x44, 0x33, 0x22, 0x11]);
    }

    #[test]
    #[should_panic]
    fn test_write_mem() {
        use winapi::um::processthreadsapi::GetCurrentProcessId;
        let handle = test_get_handle();
        let pid = unsafe { GetCurrentProcessId() };
        let mut tmp_val = 1u32;
        let out_buff = vec![0x11, 0x22, 0x33, 0x44];
        let addr = &tmp_val as *const _ as u64;
        unsafe {
            println!(
                "addr = {:X}, {},{},{:?}",
                addr,
                *(addr as *const u32),
                pid,
                (&out_buff).as_ptr()
            );
        }
        let r = write_process_memory_drv(handle, pid, &mut tmp_val as *mut _ as u64, &out_buff);
        assert_eq!(r, Ok(()));
        assert_eq!(tmp_val, 0x44332211);
    }

    #[test]
    fn test_pt_setup() {
        use winapi::um::processthreadsapi::GetCurrentProcessId;
        let handle = test_get_handle();
        let pid = unsafe { GetCurrentProcessId() };
        let r = setup_host_pid(handle, pid);
        assert_eq!(r, Ok(()));
        let mut flags = [false; 16];
        let r = setup_pt_no_pmi(
            handle,
            pid,
            256,
            3,
            5,
            1,
            0,
            0,
            0,
            &mut (|i, _, _| {
                flags[i] = true;
                flags.iter().any(|d| *d == false)
            }),
        );
        assert_eq!(r, Ok(()));
        assert_eq!(true, flags.iter().all(|d| *d == true));
    }

    #[test]
    fn test_decode()
    {
        use winapi::um::processthreadsapi::GetCurrentProcessId;
        let mut cnt = 0;
        let mut buff = Vec::new();
        let handle = test_get_handle();
        let pid = unsafe { GetCurrentProcessId() };
        let r = setup_host_pid(handle, pid);
        assert_eq!(r, Ok(()));
        setup_pt_no_pmi(
            handle,
            pid,
            256,
            3,
            5,
            1,
            0,
            0,
            0,
            &mut (|i, d, len| {
                if i == 0 {
                    for j in 0..len {
                        buff.push(d[j])
                    }
                    if buff.len() > 5 * 1024 * 1024 {
                        println!("complete collect");
                        let cfg = ConfigBuilder::new(&mut buff).unwrap().finish();
                        
                        let mut decoder = InsnDecoder::new(&cfg).unwrap();
                
                        let cb = |rst:&mut [u8], addr: u64, _: Asid| {
                            for i in 0..rst.len() {
                                rst[i] = unsafe {
                                    *((addr + i as u64) as *const u8)
                                };
                            }
                            rst.len() as i32
                        };

                        let mut img = Image::new(None).unwrap();
                        img.set_callback(Some(cb)).unwrap();
                
                        decoder.set_image(Some(&mut img)).unwrap();

                        assert_eq!(decoder.sync_forward().unwrap().eos(), false);

                        
                        // fetch event
                        while let Ok((_, s)) = decoder.event() {
                            if !s.event_pending() {
                                break;
                            }
                        }

                        // loop {
                        while let Ok((_,_)) = decoder.next() {
                            cnt+=1;

                            // fetch event
                            while let Ok((_, s)) = decoder.event() {
                                if !s.event_pending() {
                                    break;
                                }
                            }
                        }

                        println!("complete decode {}", cnt);
                        return false;
                    }
                }
                true
            }),
        ).unwrap();
        
        assert_ne!(cnt, 0);
    }
}
