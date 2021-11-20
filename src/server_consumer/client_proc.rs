use std::{collections::HashMap, sync::Arc};
use tokio::{sync::RwLock, task::JoinHandle};

use crate::{
    decode_proc::{async_decode, decode},
    mem_cacher::MemCacher,
};

use super::{
    client_pool::ClientPool,
    communator::{Communator, CommunatorSender},
    interface::PtProcMsg,
    msg_adapter::PtProcMsgAdapter,
};

pub struct ClientShareInfo {
    send_mem_req:
        Option<crossbeam::channel::Sender<(usize, usize, crossbeam::channel::Sender<Vec<u8>>)>>,
}
impl ClientShareInfo {
    fn new(
        send_mem_req_tx: crossbeam::channel::Sender<(
            usize,
            usize,
            crossbeam::channel::Sender<Vec<u8>>,
        )>,
    ) -> Self {
        ClientShareInfo {
            send_mem_req: Some(send_mem_req_tx),
        }
    }
    pub fn read_mem(&self, addr: usize, len: usize, output_buff: &mut [u8]) {
        let (tx, mut rx) = crossbeam::channel::bounded::<Vec<u8>>(1);
        self.send_mem_req
            .as_ref()
            .unwrap()
            .send((addr, len, tx))
            .unwrap();
        if let Ok(data) = rx.recv() {
            let mut tmp = unsafe { Vec::from_raw_parts(output_buff.as_mut_ptr(), 0, len) };
            tmp.extend_from_slice(&data);
        }
    }
}
pub struct ServerAgent {
    pub h: Option<JoinHandle<()>>,
}
enum ProcCtrl {
    Init,
    Proc(Vec<u8>),
}
async fn proc_data(
    share_data: Arc<ClientShareInfo>,
    proc_rx: crossbeam::channel::Receiver<ProcCtrl>,
    sender: Arc<CommunatorSender<PtProcMsgAdapter>>,
) {
    let mut cacher = MemCacher::new();
    while let Ok(ctrl) = proc_rx.recv() {
        match ctrl {
            ProcCtrl::Init => {
                cacher.clear_data();
            }
            ProcCtrl::Proc(mut data) => {
                let mut data_rst = HashMap::new();
                let mut data_rst_tmp = HashMap::new();
                async_decode(
                    share_data.clone(),
                    &mut data,
                    &mut cacher,
                    &mut data_rst_tmp,
                )
                .await;
                // 转换
                for (addr, x) in data_rst_tmp {
                    let mut tmp_page = HashMap::new();
                    for (idx, ins) in x.iter().enumerate() {
                        if ins.exec_cnt > 0 {
                            tmp_page.insert(idx as u16, *ins);
                        }
                    }
                    data_rst.insert(addr, tmp_page);
                }
                // proc_cnt += 1;
                // if proc_cnt % 50 == 0 {
                //     println!("proc data: {:?}", proc_cnt);
                // }
                //println!("proc success!");
                sender.send_data(PtProcMsg::DataRsp(data_rst)).await;
            }
        }
    }
}

impl ServerAgent {
    pub async fn connect(address: &str) -> Self {
        let (mem_req_tx, mem_req_rx) =
            crossbeam::channel::unbounded::<(usize, usize, crossbeam::channel::Sender<Vec<u8>>)>();
        let (proc_tx, proc_rx) = crossbeam::channel::unbounded::<ProcCtrl>();
        let share_data = Arc::new(ClientShareInfo::new(mem_req_tx));
        let s = address.to_owned();
        let h1 = tokio::spawn(async move {
            let socket = tokio::net::TcpStream::connect(&s).await.unwrap();
            let connect = Communator::new(socket, PtProcMsgAdapter::new());
            let (sender, mut receiver) = connect.split();
            let sender = Arc::new(sender);
            let sender_clone = sender.clone();

            if sender.send_data(PtProcMsg::RegReq(0)).await.is_err() {
                return;
            }

            let mem_req_lst = Arc::new(RwLock::new(HashMap::new()));
            let mem_req_lst_clone = mem_req_lst.clone();

            let h2 = tokio::spawn(async move {
                while let Ok((addr, len, rsp_ch)) = mem_req_rx.recv() {
                    {
                        let mut lock = mem_req_lst.write().await;
                        lock.insert(addr, rsp_ch);
                    }
                    println!("send mem req");
                    if sender_clone
                        .send_data(PtProcMsg::MemReq(addr, len))
                        .await
                        .is_err()
                    {
                        break;
                    }
                    // if sender_clone.send_data(PtProcMsg::RegReq(5)).await.is_err() {
                    //     break;
                    // }
                }
            });

            for i in 0..2 {
                tokio::spawn(proc_data(
                    share_data.clone(),
                    proc_rx.clone(),
                    sender.clone(),
                ));
            }
            println!("start listenning");
            while let Ok(msg) = receiver.recv_data().await {
                // 处理recv请求时不能阻塞，否则永远无法接收服务器的消息
                match msg {
                    PtProcMsg::Init() => {
                        println!("recv init");
                        proc_tx.send(ProcCtrl::Init);
                    }
                    PtProcMsg::RegRsp(rst) => {
                        // notiong to do now
                        println!("recv reg rsp {}", rst);
                    }
                    PtProcMsg::DataReq(data) => {
                        //println!("recv data req");
                        proc_tx.send(ProcCtrl::Proc(data));
                    }
                    PtProcMsg::MemRsp(addr, data) => {
                        println!("recv mem rsp");
                        let mut lock = mem_req_lst_clone.write().await;
                        if let Some(x) = lock.get_mut(&addr) {
                            if x.send(data).is_err() {
                                break;
                            }
                        }
                    }
                    _ => {
                        unimplemented!("msg: {:?}", msg);
                    }
                }
            }
            println!("disconnected");
        });

        ServerAgent { h: Some(h1) }
    }
}

#[test]
fn test_client_server() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .enable_io()
        .build()
        .unwrap();
    rt.block_on(async {
        let mut server_pool = ClientPool::start_server("127.0.0.1:6116", 0, 0).await;
        let client = ServerAgent::connect("127.0.0.1:6116").await;
        server_pool.decode(vec![1, 2, 3]).await;
        let r = server_pool.get_result().await;
        println!("r = {:?}", r);
    });
    loop {}
}
