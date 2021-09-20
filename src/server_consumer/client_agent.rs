use lazy_static::__Deref;
use tokio::net::TcpStream;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use winapi::shared::basetsd::SIZE_T;
use winapi::shared::minwindef::{DWORD, LPCVOID, LPVOID};
use winapi::um::processthreadsapi::OpenProcess;

use crate::post_proc::InsInfo;
use crate::pt_ctrl::read_process_memory_drv;
use crate::server_consumer::interface::PtProcMsg;
use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc};
use tokio::sync::{RwLock,RwLockWriteGuard};
use std::cell::RefCell;
use super::communator::Communator;
use super::msg_adapter::PtProcMsgAdapter;
// enum ClientCtrlMsg {
//     Dummy,
// }
#[derive(PartialEq, Eq, Debug)]
pub enum ClientState {
    Connected,
    Activate,
    Working,
    InActivate,
    DisConnect,
}
unsafe impl Send for ClientState{}
pub struct ClientAgent {
    state: Arc<RwLock<ClientState>>,
    //con: Arc<Communator<PtProcMsgAdapter>>,
    h: Option<JoinHandle<()>>,
    h2:Option<JoinHandle<()>>,
    tx_proc_data: Option<crossbeam::channel::Sender<Vec<u8>>>,
    rst_data: Option<Arc<RwLock<HashMap<usize, [InsInfo; 4096]>>>>,
}

async fn set_state(state:&Arc<RwLock<ClientState>>, new_state: ClientState) {
    let mut state = state.as_ref().write().await;
    *state = new_state;
}
fn read_memory(pid: u32, addr: u64, len: usize, ouput_buff: &mut [u8]) ->usize {
    use winapi::um::memoryapi::ReadProcessMemory;
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::winnt::PROCESS_ALL_ACCESS;
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;
    let h = unsafe {OpenProcess(PROCESS_ALL_ACCESS, 0, pid as DWORD)};
    if h == INVALID_HANDLE_VALUE {
        return 0;
    }
    let mut rst_len = 0;
    let r = unsafe { ReadProcessMemory(h, addr as *const u64 as LPCVOID, ouput_buff.as_mut_ptr() as *const _ as LPVOID, len as SIZE_T, &mut rst_len as *mut SIZE_T)};
    return rst_len;
}

#[test]
fn test_read_mem() {
    use std::mem::size_of;
    use winapi::um::processthreadsapi::GetCurrentProcessId;
    let pid = unsafe { GetCurrentProcessId() };
    let tmp_val = 0x11223344u32;
    let size = size_of::<u32>();
    let mut out_buff = vec![0; size];
    let r = read_memory(
        pid,
        &tmp_val as *const _ as u64,
        size,
        &mut out_buff,
    );
    assert_ne!(r, 0);
    assert_eq!(out_buff, [0x44, 0x33, 0x22, 0x11]);
}

impl ClientAgent {
    pub async fn wait_stop(&mut self) {
        drop(self.tx_proc_data.take());
        drop(self.rst_data.take());
        self.h.take().unwrap().await.unwrap();
        self.h2.take().unwrap().await.unwrap();
    }
    pub async fn disconnect(&mut self) {
        self.h.take().unwrap().abort();
        self.h2.take().unwrap().abort();
        set_state(&self.state, ClientState::DisConnect);
    }


    pub async fn is_disconnected(&self)->bool{
        let lock = self.state.read().await;
        println!("is_disconnected = {:?}", *lock);
        *lock == ClientState::DisConnect
    }
    pub fn new(socket: TcpStream, pid: usize, drv_h: usize) ->Self {
        use winapi::um::winnt::HANDLE;
        let state = Arc::new(RwLock::new(ClientState::Connected));
        let state_clone = state.clone();
        let state_clone2 = state.clone();
        let con = Communator::new(socket, PtProcMsgAdapter::new());
        let (sender, mut receiver) = con.split();
        let sender = Arc::new(sender);
        let sender_clone = sender.clone();

        //let (tx, rx) = crossbeam::channel::unbounded::<PtProcMsg>();

        let (tx_proc_data, rx_proc_data) = crossbeam::channel::bounded::<Vec<u8>>(1024);

        let rst_data = Arc::new(RwLock::new(HashMap::new()));

        let rst_data_clone = rst_data.clone();

        let h = tokio::spawn(async move {
            while let Ok(recv_msg) = receiver.recv_data().await {
                // proc msg
                match recv_msg {
                    PtProcMsg::RegReq(id) => {
                        {
                            println!("proc reg req: {:?}", id);
                            let mut state = state_clone.as_ref().write().await;
                            *state = ClientState::Activate;
                        }
                        sender.send_data(PtProcMsg::RegRsp(id)).await;
                    },
                    PtProcMsg::MemReq(addr, size) => {
                        let mut output_buff= vec![0;8192];
                        if read_memory(pid as u32, addr as u64, size,  &mut output_buff) == 0 {
                            println!("read mem error");
                            //continue;
                        };
                        //println!("recv memreq for drv_h: {}, PID: {} addr: {}, len: {} rst: {:?}", drv_h, pid, addr, size, output_buff);
                        if sender.send_data(PtProcMsg::MemRsp(addr, output_buff)).await.is_err() {
                            println!("send error memrsp");
                        } else {
                            println!("send memrsp complete");
                        }

                    },
                    PtProcMsg::DataRsp(data)=> {
                        println!("recv data rsp");
                        let mut rst_data:RwLockWriteGuard<HashMap<usize, [InsInfo; 4096]>> = rst_data_clone.as_ref().write().await;
                        for (page, insn) in &data {
                            if let Some(p) = rst_data.get_mut(page) {
                                for (i, cur) in insn.iter() {
                                    p[*i as usize].exec_cnt += cur.exec_cnt;
                                }
                            } else {
                                let mut tmp = [InsInfo::default();4096];
                                for (i, cur) in insn.iter() {
                                    tmp[*i as usize].exec_cnt += cur.exec_cnt;
                                }
                                rst_data.insert(*page, tmp);
                            }
                        }
                    },
                    _ => {
                        println!("unknow msg: {:?}", recv_msg);
                        break;
                    }
                }
            }
            
            set_state(&state_clone, ClientState::DisConnect);
        });

        let h2 = tokio::spawn(async move {
            while let Ok(task_data) = rx_proc_data.recv() {
                //println!("send msg datareq len:{}", task_data.len());
                let r = sender_clone.send_data(PtProcMsg::DataReq(task_data)).await;
                match r {
                    Ok(_)=>{
                        
                    },
                    Err(e) => {
                        println!("send data req error: {:?}", e);
                        break;
                    }
                }
            }
            println!("end send data");
            set_state(&state_clone2, ClientState::DisConnect);
        });
        ClientAgent {h: Some(h), h2:Some(h2), tx_proc_data: Some(tx_proc_data), rst_data: Some(rst_data), state: state}
    }

    pub async fn has_result(&self)->bool{
        let r = self.rst_data.as_ref().unwrap().read().await;
        r.is_empty()
    }

    pub async fn fetch_result(&self)->HashMap<usize, [InsInfo; 4096]>{
        let mut a = self.rst_data.as_ref().unwrap().write().await;
        let r = a.clone();
        a.clear();
        r
    }

    pub async fn is_full(&self)->bool {
        if *self.state.read().await != ClientState::Activate {
            return true;
        }
        self.tx_proc_data.as_ref().unwrap().is_full()
    }

    pub fn publish_task(&self, task_data: Vec<u8>) {
        self.tx_proc_data.as_ref().unwrap().send(task_data).unwrap();
    }
}


#[test]
fn test_agent_con() {
    use tokio::net::TcpListener;
    use super::msg_adapter::PtProcMsgAdapter;
    let rt = tokio::runtime::Builder::new_multi_thread().enable_io().build().unwrap();

    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    let h = rt.spawn(async move {
         // start server
         let listener = TcpListener::bind("127.0.0.1:6116").await.unwrap();
         tx.send(true).unwrap();
         loop {
            let (socket, _) = listener.accept().await.unwrap();
            // create communator
            let mut server_conn = Communator::new(socket, PtProcMsgAdapter::new());
            if let Ok(x) = server_conn.recv_data().await {
                assert_eq!(x, PtProcMsg::DataReq(vec![0,1,2]));
            }
            println!("server send");
            let mut rsp = HashMap::new();
            let mut rsp_data = HashMap::new();
            rsp_data.insert(8, InsInfo { exec_cnt: (10) });
            rsp.insert(0x400100, rsp_data);
            server_conn.send_data(&PtProcMsg::DataRsp(rsp)).await.unwrap();
            println!("wait server stop");
            server_conn.wait_stop().await;
            println!("server stoped");
            break;
         }
    });

    rt.block_on(async {
        match rx.await {
            Ok(v) => {
                println!("server established!");
            },
            Err(e) =>{

            }
        }
    });

    // start client
    rt.block_on(async {
        let socket = TcpStream::connect("127.0.0.1:6116").await.unwrap();
        let mut connect = Communator::new(socket, PtProcMsgAdapter::new());
        println!("start send data");
        connect.send_data(&PtProcMsg::DataReq(vec![0,1,2])).await.unwrap();
        let r = connect.recv_data().await;
        println!("recv data rsp");
        let mut rsp = HashMap::new();
        let mut rsp_data = HashMap::new();
        rsp_data.insert(8, InsInfo { exec_cnt: (10) });
        rsp.insert(0x400100, rsp_data);
        assert_eq!(r.is_ok(), true);
        assert_eq!(r.unwrap(), PtProcMsg::DataRsp(rsp));
        connect.wait_stop().await;
    });

    // 等待服务器结束
    rt.block_on(async {
        h.await
    }).unwrap();
}

#[test]
fn test_agent_reg() {
    use tokio::net::TcpListener;
    use super::msg_adapter::PtProcMsgAdapter;
    let rt = tokio::runtime::Builder::new_multi_thread().enable_io().build().unwrap();

    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    let h = rt.spawn(async move {
         // start server
         let listener = TcpListener::bind("127.0.0.1:6116").await.unwrap();
         tx.send(true).unwrap();
         let mut all_clients = Vec::new();
         loop {
            let (socket, _) = listener.accept().await.unwrap();
            // create communator
            all_clients.push(ClientAgent::new(socket, 0, 0));
         }
    });

    rt.block_on(async {
        match rx.await {
            Ok(v) => {
                println!("server established!");
            },
            Err(e) =>{

            }
        }
    });

    // start client
    rt.block_on(async {
        let socket = TcpStream::connect("127.0.0.1:6116").await.unwrap();
        let mut connect = Communator::new(socket, PtProcMsgAdapter::new());
        println!("start send data");
        connect.send_data(&PtProcMsg::RegReq(1)).await.unwrap();
        let r = connect.recv_data().await;
        assert_eq!(r.is_ok(), true);
        assert_eq!(r.unwrap(), PtProcMsg::RegRsp(1));
        connect.wait_stop().await;
    });

    // 因为服务器会一直等待，所以强行结束
    h.abort();
}