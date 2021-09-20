use clap::{App, Arg};
use crossbeam::channel;
use ptkgenerator::data_spliter::split;
use ptkgenerator::decode_proc::decode;
use ptkgenerator::mem_cacher::MemCacher;
use ptkgenerator::post_proc::*;
use ptkgenerator::pt_ctrl::*;
use ptkgenerator::server_consumer::client_proc::ServerAgent;
use std::collections::HashMap;
use std::io::Write;
use std::sync::mpsc;
use std::thread;
use std::thread::JoinHandle;
use sysinfo::{get_current_pid, ProcessExt, System, SystemExt};
use std::time::SystemTime;
use ptkgenerator::file_writer::DataWriter;

use tokio::runtime::{self, Runtime};
static mut WT: Option<Box<Vec<Option<DataWriter>>>> = None;
static mut RT: Option<Box<Runtime>> = None;

static mut PD: Option<Box<Vec<Option<ProcessorData>>>> = None;
static mut FILE_SIZE: usize = 1 * GB;
static GB: usize = 1024 * 1024 * 1024;
static MB: usize = 1024 * 1024;

static mut DEV_HANDLE: usize = 0;
static mut PID: usize = 0;

static mut G_STOP: bool = false;

struct ProcessorData {
    tx: (Option<mpsc::Sender<Vec<u8>>>, Option<JoinHandle<()>>), // sernder for process data
    data: HashMap<usize, [InsInfo; 4096]>,                       // processor collected data
    worker: Option<JoinHandle<()>>,
}

fn create_worker_thread(
    _: usize,
    rx: channel::Receiver<Vec<u8>>,
    data: &'static mut HashMap<usize, [InsInfo; 4096]>,
) -> JoinHandle<()> {
    let h = thread::spawn(move || {
        let mut cacher = MemCacher::new();
        while let Ok(mut d) = rx.recv() {
            let _ = decode(
                unsafe { DEV_HANDLE },
                unsafe { PID },
                &mut d,
                &mut cacher,
                data,
            );
        }
    });
    h
}

fn create_spliter_thread(
    _: usize,
    worker_tx: channel::Sender<Vec<u8>>,
) -> (Option<mpsc::Sender<Vec<u8>>>, Option<JoinHandle<()>>) {
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let h = thread::spawn(move || {
        let mut assemed_data = Vec::<u8>::new();
        while let Ok(mut d) = rx.recv() {
            // copy to the end
            assemed_data.append(&mut d);

            let offsets = split(&mut assemed_data);
            if offsets.len() == 0 {
                continue;
            }
            let mut last_offset = offsets[0];
            for &offset in offsets.iter().skip(1) {
                worker_tx
                    .send(assemed_data[last_offset..offset].to_vec())
                    .unwrap();
                last_offset = offset;
            }
            let end_offset = offsets.last().unwrap();
            if *end_offset > 0 {
                assemed_data = assemed_data[(*end_offset as usize)..].to_vec();
            }
        }
    });
    (Some(tx), Some(h))
}

impl ProcessorData {
    fn new(idx: usize, tx: channel::Sender<Vec<u8>>) -> ProcessorData {
        ProcessorData {
            tx: create_spliter_thread(idx, tx),
            data: HashMap::new(),
            worker: None,
        }
    }
}

fn processor(i: usize, buff: &Vec<u8>, size: usize) -> bool {
    unsafe {
        if let Some(pd) = &mut PD {
            if let Some(pd) = &pd[i] {
                if size > 0 {
                    let d = buff[0..size].to_vec();
                    if let Some(t) = &pd.tx.0 {
                        t.send(d).unwrap();
                    }
                }
            }
        }
    }
    unsafe { G_STOP == false }
}

fn create_env(
    _: &str,
    file_size: &str,
    dev_handle: usize,
    pid: usize,
    (tx, rx): (channel::Sender<Vec<u8>>, channel::Receiver<Vec<u8>>),
) {
    let s = System::new();
    let cpu_nums = s.get_processors().len();
    println!("cpu_nums = {:?}", cpu_nums);

    unsafe {
        PD = Some(Box::new(Vec::new()));
        if let Some(pd) = &mut PD {
            for i in 0..cpu_nums {
                pd.push(Some(ProcessorData::new(i, tx.clone())));
            }
        }
    }

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
            WT = Some(Box::new(Vec::new()));
            if let Some(r_wt) = &mut WT {
                for i in 0..cpu_nums {
                    r_wt.push(Some(DataWriter::new(&rt, i as u32, "x:\\", "dat")))
                }
            }
        }
    }

    // create workers -1 cpu nums

    for i in 0..cpu_nums - 1 {
        unsafe {
            if let Some(pd) = &mut PD {
                if let Some(pd) = &mut pd[i] {
                    pd.worker = Some(create_worker_thread(i, rx.clone(), &mut pd.data));
                }
            }
        };
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
fn wait_for_complete() {
    let s = System::new();
    let cpu_nums = s.get_processors().len();

    // drop spliter tx
    println!("Wait spliter finish work");
    for i in 0..cpu_nums {
        unsafe {
            if let Some(pd) = &mut PD {
                if let Some(pd) = &mut pd[i] {
                    println!("Wait spliter[{}]", i);
                    let _ = pd.tx.0.take();
                    let h = pd.tx.1.take().unwrap();
                    h.join().unwrap();
                }
            }
        }
    }
    println!("All spliter finished!");

    println!("Wait worker finish work");
    for i in 0..cpu_nums {
        unsafe {
            if let Some(pd) = &mut PD {
                if let Some(pd) = &mut pd[i] {
                    if let Some(h) = pd.worker.take() {
                        println!("Wait worker[{}]", i);
                        h.join().unwrap();
                    }
                }
            }
        }
    }
    println!("All worker finished!");
}

fn write_to_file(data:& HashMap<usize, [InsInfo; 4096]>)
{
    use serde_json::*;
    let mut data_wt = HashMap::<String, usize>::new();
    let time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();
    use ptkgenerator::native_helper::{Process};
    let mut p = Process::new(unsafe {
        PID
    });

    let mut f = std::fs::File::create(format!("result-{}.txt", time)).expect("Create restult.txt failed!");
    for (page, insn) in data {
        for (i, ins) in insn.iter().enumerate() {
            if ins.exec_cnt > 0 {
                let addr = page + i;
                let m = p.query_module_info_by_addr(addr).or::<()>(Ok(("dummy".to_owned(), addr))).unwrap();
                data_wt.insert(format!("{} + {}", m.0, m.1), ins.exec_cnt);
            }
        }
    }
    let x = json!(data_wt);
    f.write_fmt(format_args!("{}", x.to_string())).unwrap();
    println!("total unique addr cnt: {}", data_wt.len());
}

fn collect_all_data() {
    let mut data: HashMap<usize, [InsInfo; 4096]> = HashMap::new();
    let s = System::new();
    let cpu_nums = s.get_processors().len();

    // 合并多核数据

    unsafe {
        if let Some(pd) = &mut PD {
            for i in 0..cpu_nums {
                if let Some(pd) = &mut pd[i] {
                    for (page, insn) in &pd.data {
                        if let Some(p) = data.get_mut(page) {
                            for (i, cur) in insn.iter().enumerate() {
                                p[i].exec_cnt += cur.exec_cnt;
                            }
                        } else {
                            data.insert(*page, *insn);
                        }
                    }
                }
            }
        }
        PD = None;
    };

    write_to_file(&data);
    println!("print complete!");
}

fn main() {
    // ctrlc::set_handler(move || {
    //     println!("Stop collection……");
    //     unsafe {
    //         G_STOP = true;
    //     };
    // })
    // .expect("Error setting Ctrl-C handler");

    let matches = App::new("Pt Decoder Client")
        .version("1.0")
        .author("luny")
        .arg(
            Arg::with_name("addr")
                .short("a")
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
    let addr = matches.value_of("addr").unwrap();
    let rt = tokio::runtime::Builder::new_multi_thread().enable_time().enable_io().build().unwrap();
    rt.block_on(async {
        let server_agent = ServerAgent::connect(addr).await;
        server_agent.h.unwrap().await.unwrap()
    });

    loop {}
}
