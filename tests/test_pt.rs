
extern crate ptkgenerator;
#[cfg(test)]
mod test {
    use std::sync::Once;
    use ptkgenerator::pt_ctrl::*;
    use winapi::um::winnt::HANDLE;
    use std::mem::size_of;
    use std::sync::Arc;
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;

    static mut g_handle: HANDLE = INVALID_HANDLE_VALUE;
    static H: Once = Once::new();
    fn test_get_handle() ->HANDLE
    {
        H.call_once(||{
            unsafe {
                g_handle =get_pt_handle("\\\\.\\PtCollector").expect("Open pt driver failed");
            }
         });
        unsafe {g_handle}
    }
    
    #[test]
    fn test_read_mem()
    {
        use winapi::um::processthreadsapi::GetCurrentProcessId;
        let handle = test_get_handle();
        let pid = unsafe {GetCurrentProcessId()};
        let tmp_val = 0x11223344u32;
        let size = size_of::<u32>();
        let mut out_buff = vec![0;size];
        let r = read_process_memory_drv(handle, pid, &tmp_val as *const _ as u64, size as u16, &mut out_buff);
        assert_eq!(r, Ok(()));
        assert_eq!(out_buff, [0x44, 0x33, 0x22, 0x11]);
    }
    
    #[test]
    #[should_panic]
    fn test_write_mem()
    {
        use winapi::um::processthreadsapi::GetCurrentProcessId;
        let handle = test_get_handle();
        let pid = unsafe {GetCurrentProcessId()};
        let mut tmp_val = 1u32;
        let out_buff = vec![0x11,0x22,0x33,0x44];
        let addr = &tmp_val as *const _ as u64;
        unsafe {println!("addr = {:X}, {},{},{:?}", addr, *(addr as *const u32), pid, (&out_buff).as_ptr());}
        let r = write_process_memory_drv(handle, pid, &mut tmp_val as *mut _ as u64, &out_buff);
        assert_eq!(r, Ok(()));
        assert_eq!(tmp_val, 0x44332211);
    }

    fn processor(i:usize, buff:&Vec<u8>)->bool {
        assert_eq!(i, 0);
        assert_ne!(buff.len(), 0);
        false
    }
    #[test]
    fn test_pt_setup()
    {
        use winapi::um::processthreadsapi::GetCurrentProcessId;
        let handle = test_get_handle();
        let pid = unsafe {GetCurrentProcessId()};
        let r = setup_host_pid(handle, pid);
        assert_eq!(r, Ok(()));
        let mut flags = [false; 16];
        let r = setup_pt_no_pmi(handle, pid, 256, 3,5,1,0,0,0, &mut processor);
        assert_eq!(r, Ok(()));
    }
}
