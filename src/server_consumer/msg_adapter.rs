use std::iter::FromIterator;

use bytes::BytesMut;

use super::communator::MsgAdapter;
use super::interface::PtProcMsg;
#[derive(Clone)]
pub struct PtProcMsgAdapter {}
unsafe impl Send for PtProcMsgAdapter{}
impl MsgAdapter for PtProcMsgAdapter {
    type DataType = PtProcMsg;
    fn decode(&self, data: &bytes::Bytes) ->Result<Self::DataType, ()> {
        let r: PtProcMsg = postcard::from_bytes(&data).unwrap();
        Ok(r)
    }
    fn encode(&self, data: &Self::DataType) ->Result<bytes::BytesMut, ()> {
        let r: Vec<u8> = postcard::to_stdvec(data).unwrap();
        if r.len() > 7*1024*1024 {
            println!("{} too long", r.len());
        }
        Ok(BytesMut::from_iter(r.into_iter()))
    }
}

impl PtProcMsgAdapter {
    pub fn new()->Self {
        PtProcMsgAdapter{}
    }
}

#[test]
fn test_decoder() {
    let adapter = PtProcMsgAdapter::new();
    let msg = PtProcMsg::DataReq(vec![0,1,2]);
    let r = adapter.encode(&msg).unwrap();
    let r2 = adapter.decode(&r.freeze()).unwrap();
    assert_eq!(r2, msg);
}

#[test]
fn test_decoder_mem_rsp() {
    let adapter = PtProcMsgAdapter::new();
    let msg = PtProcMsg::MemReq(1, 10);
    let r = adapter.encode(&msg).unwrap();
    let r2 = adapter.decode(&r.freeze()).unwrap();
    assert_eq!(r2, msg);
}