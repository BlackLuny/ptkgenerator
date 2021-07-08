use clap::{App, Arg};
use ptkgenerator::data_spliter::split;
use ptkgenerator::decode_proc::decode;
use ptkgenerator::mem_cacher::MemCacher;
use ptkgenerator::pt_ctrl::*;
use sysinfo::{get_current_pid, ProcessExt, System, SystemExt};
use tokio::runtime::{self, Runtime};
use tokio::sync::mpsc;

static mut RT: Option<Box<Runtime>> = None;
static mut PD: Option<Box<Vec<Option<ProcessorData>>>> = None;
static mut FILE_SIZE: usize = 1 * GB;
static GB: usize = 1024 * 1024 * 1024;
static MB: usize = 1024 * 1024;

static mut DEV_HANDLE: usize = 0;
static mut PID:usize = 0;

struct ProcessorData {
    tx: mpsc::Sender<Vec<u8>>,  // sernder for process data
}

fn create_process_thread(rt: &'static Runtime, idx: usize) ->mpsc::Sender<Vec<u8>>
{
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(1000000);
    rt.spawn(async move {
        let mut assemed_data = Vec::<u8>::new();
        let mut cacher = MemCacher::new();
        while let Some(mut d) = rx.recv().await {
            // copy to the end
            assemed_data.append(&mut d);

            let offsets = split(&mut assemed_data);
            if offsets.len() == 0 {
                continue;
            }
            let mut last_offset = offsets[0];
            for &offset in offsets.iter().skip(1) {
                let mut a = assemed_data[last_offset..offset].to_vec();
                //rt.spawn(async move{
                    let cnt = decode(unsafe {DEV_HANDLE}, unsafe {PID}, &mut a, &mut cacher);
                    println!("decode cnt = {}", cnt);
                //});
                last_offset = offset;
            }
            
            let end_offset = offsets.last().unwrap();
            if *end_offset > 0 {
                assemed_data = assemed_data[(*end_offset as usize)..].to_vec();
            }
        }
    });
    tx
}
impl ProcessorData {
    fn new(rt: &'static Runtime, idx: usize)->ProcessorData {
        ProcessorData {tx: create_process_thread(&rt, idx)}
    }
}


fn processor(i: usize, buff: &Vec<u8>, size: usize) -> bool {
    unsafe {
        if let Some(pd) = &mut PD {
            if let Some(pd) = &pd[i] {
                if size > 0 {
                    let d = buff[0..size].to_vec();
                    pd.tx.try_send(d).unwrap();
                }
            }
        }
    }
    true
}

fn create_env(_: &str, file_size: &str, dev_handle: usize, pid: usize) {
    let s = System::new();
    let cpu_nums = s.get_processors().len();
    println!("cpu_nums = {:?}", cpu_nums);
    unsafe {
        RT = Some(Box::new(
            runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap(),
        ));
    }

    unsafe {
        if let Some(rt) = &RT {
            PD = Some(Box::new(Vec::new()));
            if let Some(r_wt) = &mut PD {
                for i in 0..cpu_nums {
                    r_wt.push(Some(ProcessorData::new(&rt, i)));
                }
            }
        }
    }

    set_file_size(file_size);

    unsafe {
        DEV_HANDLE = dev_handle;
        PID = pid;
    }
}

fn get_process_id(name: &str) -> Option<usize> {
    let s = System::new_all();
    let all_proc = s.get_processes();
    for (pid, proc) in all_proc {
        let cur = proc.name();
        if name.to_uppercase() == cur.to_uppercase() {
            return Some(*pid);
        }
    }
    None
}

fn set_file_size(file_size: &str) {
    let file_size = file_size.to_uppercase();
    let mut unit = None;
    if file_size.ends_with("GB") {
        unit = Some(1 * GB);
    } else if file_size.ends_with("MB") {
        unit = Some(1 * MB);
    }

    if let Some(unit) = unit {
        let num: usize = file_size
            .trim_end_matches(char::is_alphabetic)
            .parse()
            .unwrap();
        let file_size = num * unit;
        unsafe {
            FILE_SIZE = file_size;
        }
    }
}

fn main() {
    let handle = get_pt_handle("\\\\.\\PtCollector").expect("Open pt driver failed");
    let matches = App::new("PtkGenerator")
        .version("1.0")
        .author("luny")
        .arg(
            Arg::with_name("process")
                .short("p")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("buff_size")
                .long("buff_size")
                .default_value("256"),
        )
        .arg(Arg::with_name("mtc_freq").short("m").default_value("3"))
        .arg(Arg::with_name("psb_freq").long("psb").default_value("5"))
        .arg(Arg::with_name("cyc_thld").long("cyc").default_value("1"))
        .arg(
            Arg::with_name("addr0_cfg")
                .long("addr0_cfg")
                .default_value("0"),
        )
        .arg(
            Arg::with_name("addr0_start")
                .long("addr0_start")
                .default_value("0"),
        )
        .arg(
            Arg::with_name("addr0_end")
                .long("addr0_end")
                .default_value("0"),
        )
        .arg(Arg::with_name("out_dir").short("o").default_value("x:\\"))
        .arg(
            Arg::with_name("file_size")
                .long("file_size")
                .default_value("1GB"),
        )
        .get_matches();

    let proc_name = matches.value_of("process").unwrap();
    let buff_size = matches.value_of("buff_size").unwrap().parse().unwrap();
    let mtc = matches.value_of("mtc_freq").unwrap().parse().unwrap();
    let psb = matches.value_of("psb_freq").unwrap().parse().unwrap();
    let cyc = matches.value_of("cyc_thld").unwrap().parse().unwrap();
    let addr0_cfg = matches.value_of("addr0_cfg").unwrap().parse().unwrap();
    let addr0_start = u32::from_str_radix(matches.value_of("addr0_start").unwrap(), 16).unwrap();
    let addr0_end = u32::from_str_radix(matches.value_of("addr0_end").unwrap(), 16).unwrap();
    let out_dir = matches.value_of("out_dir").unwrap();
    let file_size = matches.value_of("file_size").unwrap();

    println!("process {}", proc_name);
    let p = get_process_id(proc_name).unwrap();

    create_env(out_dir, file_size, handle as usize, p);
    setup_host_pid(handle, get_current_pid().unwrap() as u32).expect("Set Host Pid Failed");

    println!("start capturing ...");
    setup_pt_no_pmi(
        handle,
        p as u32,
        buff_size,
        mtc,
        psb,
        cyc,
        addr0_cfg,
        addr0_start,
        addr0_end,
        &mut processor,
    )
    .expect("Start pt failed");
    close_pt_handle(handle).expect("Close pt handle errord");
}
