use bytes::BytesMut;
use lazy_static::__Deref;
use tokio::{net::{TcpListener, TcpStream}, runtime::Runtime, task::JoinHandle};
use crossbeam::channel;
use std::{collections::HashMap, fmt::{Result, Write}, io, str::Bytes, sync::{Arc}, time::Duration};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use futures::io::{AsyncReadExt, AsyncWriteExt};
use crate::{mem_cacher::MemCacher, post_proc::InsInfo, server_consumer::communator::Communator};
use tokio::sync::RwLock;
use super::client_agent::ClientAgent;
use std::sync::atomic::{AtomicBool, Ordering};

struct ClientsInfo {
    clients: Vec<Option<ClientAgent>>,
}

pub struct ClientPool {
    h: Option<JoinHandle<()>>,
    client_info: Arc::<RwLock<ClientsInfo>>,
    stop_flag : Arc<AtomicBool>,
    data_rst: Option<HashMap<usize, [InsInfo; 4096]>>
}

impl ClientPool {
    async fn wait_stop(&mut self) {
        // 停止所有client
        self.h.take().unwrap().await.unwrap();
    }
    pub async fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        self.h.take().unwrap().abort();
        let mut lock = self.client_info.write().await;
        for client in lock.clients.iter_mut(){
            if let Some(mut client) = client.take() {
                client.disconnect().await;
            }
        }
    }
    pub async fn start_server(addr: &str, pid: usize, drv_h: usize) ->ClientPool
    {
        let s = addr.to_owned();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();
        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
        let client_info = Arc::new(RwLock::new(ClientsInfo{clients: Vec::new()}));
        let client_info_clone = client_info.clone();
        let h = tokio::spawn( async move {
                let listener = TcpListener::bind(&s).await.unwrap();
                tx.send(true).unwrap();
                loop {
                    let (socket, _) = listener.accept().await.unwrap();
                    // add client agent
                    let mut lock = client_info_clone.write().await;
                    lock.clients.push(Some(ClientAgent::new(socket, pid, drv_h)));
                    if lock.clients.len() > 32 || stop_flag.load(Ordering::SeqCst) == true {
                        println!("stop server");
                        break;
                    }
                }
        });
        tokio::spawn(async {
            rx.await.unwrap()
        }).await.unwrap();
        ClientPool{h:Some(h), data_rst: Some(HashMap::new()), stop_flag: stop_flag_clone, client_info: client_info}
    }

    pub async fn decode(&self,
        data: Vec<u8>,
        )
    {
        let rlock = self.client_info.read().await;
        let mut select = rlock.clients.iter().nth(0);
        for i in rlock.clients.iter() {
            if let Some(client) = i {
                if !client.is_full().await {
                    select = Some(i);
                    break;
                }
            }
        }
        if let Some(Some(clt)) = select {
            //println!("publish");
            clt.publish_task(data);
        }
    }

    pub async fn get_result(&mut self)->HashMap<usize, [InsInfo; 4096]> {
        let mut rst:HashMap<usize, [InsInfo; 4096]> = HashMap::new();
        let mut rlock = self.client_info.write().await;
        let mut dis_connected = vec![];
        for (idx, client ) in rlock.clients.iter().enumerate() {
            if let Some(i) = client {
                println!("test for {}", idx);
                if i.has_result().await {
                    let r = i.fetch_result().await;
                    for (page, insn) in &r {
                        if let Some(p) = rst.get_mut(page) {
                            for (i, cur) in insn.iter().enumerate() {
                                p[i].exec_cnt += cur.exec_cnt;
                            }
                        } else {
                            rst.insert(*page, *insn);
                        }
                    }
                }
                if i.is_disconnected().await {
                    println!("disconnected");
                    dis_connected.push(idx);
                }
            }
        }
        // 干掉disconnected的
        for idx in dis_connected {
            let mut c = rlock.clients[idx].take().unwrap();
            c.wait_stop().await;
        }
        rst
    }
}

#[test]
fn test_client_pool() {
    use super::msg_adapter::PtProcMsgAdapter;
    use super::interface::PtProcMsg;
    use tokio::time::sleep;
    let rt = tokio::runtime::Builder::new_multi_thread().enable_time().enable_io().build().unwrap();

    let h = rt.block_on(async {
        let mut client_pool = ClientPool::start_server("127.0.0.1:6117", 0, 0).await;
        {
            let socket = TcpStream::connect("127.0.0.1:6117").await.unwrap();
            let mut connect = Communator::new(socket, PtProcMsgAdapter::new());
            println!("start send data");
            connect.send_data(&PtProcMsg::RegReq(1)).await.unwrap();
            let r = connect.recv_data().await;
            assert_eq!(r.is_ok(), true);
            assert_eq!(r.unwrap(), PtProcMsg::RegRsp(1));
            connect.wait_stop().await;
        }
        sleep(Duration::from_secs(2)).await;
        client_pool.stop();
        // for i in 0..10000 {
        //     client_pool.get_result().await;
        // }
    });
}