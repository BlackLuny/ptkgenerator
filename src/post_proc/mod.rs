use std::{collections::HashMap, sync::mpsc};
use libipt::insn::Insn;
use std::thread;

#[derive(Default, Clone, Copy, Debug)]
pub struct InsInfo {
    pub exec_cnt: usize,
}

pub fn create_post_thread(data: &'static mut HashMap<usize, [InsInfo;4096]>)->mpsc::Sender<Vec<Insn>>
{
    let (tx, rx) = mpsc::channel::<Vec<Insn>>();
    thread::spawn(move || {
        while let Ok(d) = rx.recv() {
            for i in d{
                let addr = i.ip();
                let off = addr as usize & 0xfff;
                let page = addr as usize & (!0xfff);
                if let Some(it) = data.get_mut(&page) {
                    println!("got it: 0x{}", addr);
                    it[off].exec_cnt+=1;
                } else {
                    //println!("page: 0x{:x}", page);
                    let mut tmp = [InsInfo::default();4096];
                    tmp[off].exec_cnt += 1;
                    data.insert(page, tmp);
                }
            }
        }
    });
    tx
}