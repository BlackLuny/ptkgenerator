use std::collections::HashMap;

use serde::{Serialize, Deserialize};

use crate::post_proc::InsInfo;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum PtProcMsg {
    RegReq(u32),
    RegRsp(u32),
    UnReg(u32),

    Init(),

    DataReq(Vec<u8>), // data, data_len
    DataRsp(HashMap<usize, HashMap<u16, InsInfo>>),

    MemReq(usize, usize), // addr,len
    MemRsp(usize, Vec<u8>), // addr, len, data

}
