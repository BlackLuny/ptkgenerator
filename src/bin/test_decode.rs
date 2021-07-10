use ptkgenerator::decode_proc::decode;
use ptkgenerator::mem_cacher::MemCacher;
use ptkgenerator::post_proc::*;
use ptkgenerator::pt_ctrl::*;
use std::collections::HashMap;
use sysinfo::{ProcessExt, System, SystemExt};

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

fn main() {
    let handle = get_pt_handle("\\\\.\\PtCollector").expect("Open pt driver failed");
    let p = get_process_id("gameapp.exe").unwrap();
    let mut cacher = MemCacher::new();
    let mut rst = HashMap::<usize, [InsInfo; 4096]>::new();
    for i in 0..16 {
        let path = format!("x:\\{}.dat",i);
        let mut data = std::fs::read(path).unwrap();
        println!("Processing {}", i);
        decode(handle as usize, p, &mut data, &mut cacher, &mut rst);
    }

    let mut cnt = 0;
    for (_, insn) in rst {
        for (_, ins) in insn.iter().enumerate() {
            if ins.exec_cnt > 0 {
                cnt+=1;
            }
        }
    }
    println!("cnt = {}", cnt);
    close_pt_handle(handle).expect("Close pt handle errord");
}