
use clap::{App, Arg};
//use crate::pt_ctrl::{get_pt_handle, close_pt_handle, setup_pt_no_pmi, setup_host_pid};
use winapi::um::processthreadsapi::GetCurrentProcessId;
use ptkgenerator::pt_ctrl::*;
fn processor(i:usize, buff:&Vec<u8>)->bool {
    println!("read processor {}, len = {}", i, buff.len());
    true
}
fn main() {
    let handle = get_pt_handle("\\\\.\\PtCollector").expect("Open pt driver failed");
    let matches = App::new("PtkGenerator")
                    .version("1.0")
                    .author("luny")
                    //.arg(Arg::with_name("process").short("p").required(true))
                    .arg(Arg::with_name("buff_size").default_value("256"))
                    .arg(Arg::with_name("mtc_freq").short("m").default_value("3")) 
                    .arg(Arg::with_name("psb_freq").default_value("5")) 
                    .arg(Arg::with_name("cyc_thld").default_value("1")) 
                    .arg(Arg::with_name("addr0_cfg").default_value("0"))
                    .arg(Arg::with_name("addr0_start").default_value("0"))
                    .arg(Arg::with_name("addr0_end").default_value("0"))
                    .get_matches();
    //let p = matches.value_of("process").unwrap().parse().unwrap();
    let p = unsafe {
        GetCurrentProcessId()
    };
    let buff_size = matches.value_of("buff_size").unwrap().parse().unwrap();
    let mtc = matches.value_of("mtc_freq").unwrap().parse().unwrap();
    let psb = matches.value_of("psb_freq").unwrap().parse().unwrap();
    let cyc = matches.value_of("cyc_thld").unwrap().parse().unwrap();
    let addr0_cfg = matches.value_of("addr0_cfg").unwrap().parse().unwrap();
    let addr0_start = matches.value_of("addr0_start").unwrap().parse().unwrap();
    let addr0_end = matches.value_of("addr0_end").unwrap().parse().unwrap();
    println!("process {}", p);
    setup_host_pid(handle, p).expect("Set Host Pid Failed");
    setup_pt_no_pmi(handle, p, buff_size, mtc, psb, cyc, addr0_cfg, addr0_start, addr0_end, &mut processor).expect("Start pt failed");
    close_pt_handle(handle).expect("Close pt handle errord");
}
