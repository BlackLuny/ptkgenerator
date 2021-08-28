use bytes::BytesMut;
use lazy_static::__Deref;
use tokio::{net::{TcpListener, TcpStream}, runtime::Runtime, task::JoinHandle};
use crossbeam::channel;
use std::{fmt::{Result, Write}, io, str::Bytes};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use futures::io::{AsyncReadExt, AsyncWriteExt};
use crate::server_consumer::communator::Communator;
use std::sync::RwLock;
use super::client_agent::ClientAgent;

struct ClientsInfo {
    clients: Vec<ClientAgent>,
}
static G_CLIENTS_INFO: state::Storage<RwLock<ClientsInfo>> = state::Storage::new();
struct ClientPool {
    h: Option<JoinHandle<()>>,
}

impl ClientPool {
    async fn wait_stop(&mut self) {
        // 停止所有client
        self.h.take().unwrap().await.unwrap();
    }
    fn start_server(addr: &str) ->ClientPool
    {
        let s = addr.to_owned();
        let h = tokio::spawn( async move {
                let listener = TcpListener::bind(&s).await.unwrap();
                loop {
                    let (socket, _) = listener.accept().await.unwrap();
                    // add client agent
                    let a = G_CLIENTS_INFO.get();
                    let mut lock = a.write().unwrap();
                    lock.clients.push(ClientAgent::new(socket));
                    if lock.clients.len() > 32 {
                        break;
                    }
                }
        });
        ClientPool{h:Some(h)}
    }

    fn push_task(&mut self) {
        unimplemented!()
    }

    fn get_result(&mut self) {
        unimplemented!()
    }
}