use std::mem::size_of;
use std::ptr::{null, null_mut};
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::fileapi::OPEN_EXISTING;
use winapi::um::fileapi::CreateFileW;
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::winnt::{LPCSTR,LPCWSTR};
use winapi::shared::minwindef::DWORD;
use winapi::um::winnls::CP_ACP;
use winapi::um::stringapiset::MultiByteToWideChar;
use winapi::um::handleapi::CloseHandle;
use winapi::um::winnt::HANDLE;
use winapi::um::winnt::{GENERIC_READ, GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_ATTRIBUTE_NORMAL};
use winapi::um::winioctl::{CTL_CODE, FILE_DEVICE_UNKNOWN, METHOD_BUFFERED, FILE_ANY_ACCESS};
use winapi::um::ioapiset::DeviceIoControl;
use winapi::shared::minwindef::LPVOID;
use std::default::Default;
use winapi::shared::minwindef::LPDWORD;

const MAX_CPU_NUM: usize = 32;
#[repr(C)]
struct PtCopyMemReq {
	localbuf: u64,         // Buffer address
	target_ptr: u64,        // Target address
	size: u64,             // Buffer size
	pid: u64,             // Target process id
	write: u64,            // TRUE if write operation, FALSE if read
}

#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
struct EnventInfo {
    idx: u64,
    event_handle: u64
}
#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
struct AddrRange {
    addr_n_a: u64,
    addr_n_b: u64,
    cfg_mode: u64
}
impl AddrRange {
    fn new()->AddrRange {
        AddrRange{addr_n_a: 0, addr_n_b: 0, cfg_mode: 0}
    }
}

#[derive(Default, Debug)]
#[repr(C)]
struct PtSetupInfo
{
    pid: u32, buff_size: u32, mtc_freq: u32, ret_compress: u32,
    cyc_thld: u32, psb_freq: u32, event_num: u32, no_pmi: u32,
    event_info: [EnventInfo; MAX_CPU_NUM], addrs_cfg: [AddrRange; 4],
}

#[derive(Default)]
#[repr(C)]
struct PtResultInfo {
	output_addr: u64,
	data_len: u64
}

type PresultBuff = *const PtResultInfo;

#[derive(Debug)]
#[repr(C)]
struct PtRecorInfo {
	rst_info: PresultBuff
}

impl Default for PtRecorInfo {
    fn default() -> Self {
        PtRecorInfo{rst_info:null()}
    }
}
#[derive(Default, Debug)]
#[repr(C)]
struct PtSetupRst {
	rst: u32,
	record_num: u32,
	record_info: [PtRecorInfo; MAX_CPU_NUM],
	out_buff_num: u32,
	out_buffer_info: [u64; MAX_CPU_NUM],
	out_buffer_len: u32,
}

struct PtSetupServerPid {
	pid: u32
}
#[derive(Default)]
struct PtSetupServerPidRsp {
	rst: u32
}

pub fn read_process_memory_drv(h: HANDLE, pid:u32, address: u64, len:u16, ouput_buff:&mut Vec<u8>) ->Result<(),u32>
{
    let code = CTL_CODE(FILE_DEVICE_UNKNOWN, 0x1002, METHOD_BUFFERED, FILE_ANY_ACCESS);
    let mut read_req = PtCopyMemReq {pid: pid as u64, target_ptr: address, localbuf:ouput_buff.as_mut_ptr() as u64, size: len as u64, write: 0};
    let read_req_ptr = &mut read_req as *mut _ as LPVOID;
    let mut rst_len = 0;
    let stru_size = size_of::<PtCopyMemReq>();
    let r = unsafe {
        DeviceIoControl(h, code, read_req_ptr,stru_size as u32, null_mut(), 0,  (&mut rst_len) as *mut u32, null_mut())
    };
    if r != 0 {
        Ok(())
    } else {
        Err(rst_len)
    }
}

pub fn write_process_memory_drv(h: HANDLE, pid:u32, address: u64, buff:&Vec<u8>) ->Result<(),u32>
{
    let code = CTL_CODE(FILE_DEVICE_UNKNOWN, 0x1002, METHOD_BUFFERED, FILE_ANY_ACCESS);
    let mut read_req = PtCopyMemReq {pid: pid as u64, target_ptr: address, localbuf:buff.as_ptr() as u64, size: buff.len() as u64, write: 1};
    let read_req_ptr = &mut read_req as *mut _ as LPVOID;
    let mut rst_len = 0;
    let stru_size = size_of::<PtCopyMemReq>();
    let r = unsafe {
        DeviceIoControl(h, code, read_req_ptr,stru_size as u32, null_mut(), 0,  (&mut rst_len) as *mut u32, null_mut())
    };

    if r != 0 {
        Ok(())
    } else {
        Err(rst_len)
    }
}

pub fn setup_host_pid(h: HANDLE, pid: u32)->Result<(), u32>
{
    let code = CTL_CODE(FILE_DEVICE_UNKNOWN, 0x1000, METHOD_BUFFERED, FILE_ANY_ACCESS);
    let mut req = PtSetupServerPid{pid: pid};
    let req_ptr = &mut req as *mut _ as LPVOID;
    let mut rsp_len = 0;
    let req_size = size_of::<PtSetupServerPid>();
    let mut rsp: PtSetupServerPidRsp = Default::default();
    let rsp_size = size_of::<PtSetupServerPidRsp>();
    let r = unsafe {
        DeviceIoControl(h, code, req_ptr,req_size as u32, &mut rsp as *mut _ as LPVOID, rsp_size as DWORD,  (&mut rsp_len) as *mut u32, null_mut())
    };
    if r != 0 && rsp_size as u32 == rsp_len {
        Ok(())
    } else {
        Err(rsp_len)
    }
}

fn setup_pt_no_pmi_start(h: HANDLE, pid: u32, buff_size: u32, mtc_freq: u32, psb_freq: u32, cyc_thld:u32, addr_cfg: u32, addr_start: u32, addr_end: u32, rsp: &mut PtSetupRst) ->Result<(),u32>
{
    let mut setup_info = PtSetupInfo {
        pid: pid, buff_size: buff_size, ret_compress: 0, mtc_freq: mtc_freq,
        psb_freq: psb_freq, cyc_thld: cyc_thld, event_num: 16, no_pmi: 1,
        addrs_cfg: [AddrRange{cfg_mode: addr_cfg as u64, addr_n_a: addr_start as u64, addr_n_b: addr_end as u64}, AddrRange::new(), AddrRange::new(), AddrRange::new()],
        event_info: Default::default()
    };
    let code = CTL_CODE(FILE_DEVICE_UNKNOWN, 0x999, METHOD_BUFFERED, FILE_ANY_ACCESS);

    let setup_info_ptr = &mut setup_info as *mut _ as LPVOID;
    let mut rst_len = 0u32;
    let stru_size = size_of::<PtSetupInfo>();
    let rsp_size = size_of::<PtSetupRst>();
    let r = unsafe {
        DeviceIoControl(h, code, setup_info_ptr,stru_size as DWORD, rsp as *mut _ as LPVOID, rsp_size as DWORD,  (&mut rst_len) as *mut _ as LPDWORD, null_mut())
    };
    if r != 0 && rst_len as usize  == rsp_size {
        Ok(())
    } else {
        Err(rst_len)
    }
}

fn fetch_data(buff: *mut u8, buff_len: usize, pos: usize, out_buff:&mut Vec<u8>) ->usize
{
    let mut rst_size = 0;
    for i in 0..out_buff.len() {
        // check data valid
        let mut is_valid = false;
        for j in 0..10 {
            let cur_offset = (pos + i + j) % buff_len;
            let cur = unsafe { *(buff.add(cur_offset))};
            if cur != 0xFF {
                is_valid = true;
                break;
            }
        }
        if !is_valid {
            break;
        }
        let offset = (i + pos) % buff_len;
        out_buff[i] = unsafe {
            *(buff.add(offset))
        };
        unsafe {
            *(buff.add(offset)) = 0xFF;
        };
        rst_size+=1;
    }
    rst_size
}

pub fn setup_pt_no_pmi<F>(h: HANDLE, pid: u32, buff_size: u32, mtc_freq: u32, psb_freq: u32, cyc_thld:u32, addr_cfg: u32, addr_start: u32, addr_end: u32, processor:&mut F) ->Result<(),u32> where F: FnMut(usize, &Vec<u8>)->bool
{
    let mut rsp=Default::default();
    setup_pt_no_pmi_start(h, pid, buff_size, mtc_freq, psb_freq, cyc_thld, addr_cfg, addr_start, addr_end, &mut rsp).expect("Setup pt faile");
    // start capture data
    let mut read_pos = vec![0 as usize; rsp.out_buff_num as usize];
    let mut tmp_buff = vec![0; 16 * 1024 * 1024];
    let never = false;
    loop {
        for i in 0..rsp.out_buff_num as usize {
            let pos = read_pos[i];
            let buff = rsp.out_buffer_info[i];
            let len = rsp.out_buffer_len as usize;
            let read_size = fetch_data(buff as *mut u8, len, pos, &mut tmp_buff);
            read_pos[i] = (pos + read_size) % len;
            if !processor(i, &tmp_buff) {
                return Ok(());
            }
        }
        if never {
            break;
        }
    };
    Ok(())
}

pub fn get_pt_handle(pt_device:&str)->Result<HANDLE, DWORD>
{
    let len = unsafe {MultiByteToWideChar(CP_ACP, 0 as DWORD, pt_device.as_ptr() as LPCSTR, pt_device.len() as i32, null_mut(), 0)};

    let mut tmp = vec![0;len as usize + 1];

    unsafe {MultiByteToWideChar(CP_ACP, 0 as DWORD, pt_device.as_ptr() as LPCSTR, pt_device.len() as i32, tmp.as_mut_ptr(), len);};
    
    let r = unsafe {
        CreateFileW(tmp.as_mut_ptr() as LPCWSTR, GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, null_mut(), OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, null_mut())
    };
    if r == INVALID_HANDLE_VALUE {
        Err(unsafe {
            GetLastError()
        })
    } else {
        Ok(r)
    }
}

pub fn close_pt_handle(handle:HANDLE)->Result<(), u32>
{
    let r = unsafe {CloseHandle(handle)};

    if r != 0 {
        Ok(())
    } else {
        Err(unsafe {
            GetLastError()
        })
    }
}