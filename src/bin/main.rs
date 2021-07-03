
use clap::{App, Arg};
use ptkgenerator::pt_ctrl::*;
use sysinfo::{ProcessExt, System, SystemExt, get_current_pid};
use ptkgenerator::file_writer::DataWriter;
use tokio::runtime::{self, Runtime};

static mut RT: Option<Box<Runtime>> = None;
static mut WT: Option<Box<Vec<Option<DataWriter>>>> = None;
static mut FILE_SIZE: usize = 1024*1024*1024;

fn processor(i:usize, buff:&Vec<u8>, size: usize)->bool {
    unsafe {
        if let Some(wt )= &mut WT {
            let mut new_file = false;
            if let Some(wt )= &mut wt[i] {
                wt.write(buff, size);
                if wt.write_size > FILE_SIZE {
                    new_file = true;
                }
            }
            if new_file {
                if let Some(rt) = &RT {
                    println!("new file for {}", i);
                    wt[i].replace(DataWriter::new(&rt, i as u32, "x:", "tmp"));
                }
            }
        }
    }
    true
}



fn create_env(out_dir:&str, file_size:usize)
{
    let s = System::new();
    let cpu_nums = s.get_processors().len();
    println!("cpu_nums = {:?}", cpu_nums);
    unsafe {
        RT = Some(Box::new(runtime::Builder::new_multi_thread().enable_all().build().unwrap()));
    }
    
    unsafe {
        FILE_SIZE = file_size;
    }

    unsafe {
        if let Some(rt) = &RT {
            WT = Some(Box::new(Vec::new()));
            if let Some(r_wt) = &mut WT {
                for i in 0..cpu_nums {
                    r_wt.push(Some(DataWriter::new(&rt, i as u32, "x:", "tmp")))
                }
            }
        }
    }
}
fn main() {
    let handle = get_pt_handle("\\\\.\\PtCollector").expect("Open pt driver failed");
    let matches = App::new("PtkGenerator")
                    .version("1.0")
                    .author("luny")
                    .arg(Arg::with_name("process").short("p").takes_value(true).required(true))
                    .arg(Arg::with_name("buff_size").default_value("256"))
                    .arg(Arg::with_name("mtc_freq").short("m").default_value("3")) 
                    .arg(Arg::with_name("psb_freq").default_value("5")) 
                    .arg(Arg::with_name("cyc_thld").default_value("1")) 
                    .arg(Arg::with_name("addr0_cfg").default_value("0"))
                    .arg(Arg::with_name("addr0_start").default_value("0"))
                    .arg(Arg::with_name("addr0_end").default_value("0"))
                    .arg(Arg::with_name("out_dir").default_value("x"))
                    .arg(Arg::with_name("file_size").default_value("1024*1024*1024"))
                    .get_matches();
    let proc_name = matches.value_of("process").unwrap();
    let buff_size = matches.value_of("buff_size").unwrap().parse().unwrap();
    let mtc = matches.value_of("mtc_freq").unwrap().parse().unwrap();
    let psb = matches.value_of("psb_freq").unwrap().parse().unwrap();
    let cyc = matches.value_of("cyc_thld").unwrap().parse().unwrap();
    let addr0_cfg = matches.value_of("addr0_cfg").unwrap().parse().unwrap();
    let addr0_start = matches.value_of("addr0_start").unwrap().parse().unwrap();
    let addr0_end = matches.value_of("addr0_end").unwrap().parse().unwrap();
    let out_dir = matches.value_of("out_dir").unwrap();
    let file_size = matches.value_of("out_dir").unwrap().parse().unwrap();
    println!("process {}", proc_name);
    let s = System::new_all();
    let p = s.get_process_by_name(proc_name)[0].pid();
    let cur_pid = get_current_pid().unwrap();
    setup_host_pid(handle, cur_pid as u32).expect("Set Host Pid Failed");
    create_env(out_dir, file_size);
    println!("start capturing ...");
    setup_pt_no_pmi(handle, p as u32, buff_size, mtc, psb, cyc, addr0_cfg, addr0_start, addr0_end, &mut processor).expect("Start pt failed");
    close_pt_handle(handle).expect("Close pt handle errord");
}
