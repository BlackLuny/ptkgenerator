use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum PtProcMsg {
    RegReq(u32),
    RegRsp(u32),
    UnReg(u32),

    DataReq(Vec<u8>), // data, data_len
    DataRsp(u32),

    MemReq(usize, usize), // addr,len
    MemRsp(usize, Vec<u8>), // addr, len, data

}
