use tokio::{net::{TcpListener, TcpStream}, runtime::Runtime, sync::mpsc::Sender};
use crossbeam::channel::{self, Receiver};
use std::{fmt::Write, io::{self, Read}};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use futures::{SinkExt, StreamExt, io::{AsyncReadExt, AsyncWriteExt}};
use bytes::{BytesMut, Bytes};

pub trait MsgAdapter {
    type DataType;
    fn decode(&self, data: &Bytes)->Result<Self::DataType, ()>;
    fn encode(&self, data: &Self::DataType)->Result<BytesMut, ()>;
}
#[derive(Debug)]
enum ConnectCtrlMsg {
    SendData(Bytes),
    FetchData(),
    DisConnect(),
}
pub struct Communator<D: MsgAdapter> {
    ctrl_tx: Option<Sender<(ConnectCtrlMsg, Option<tokio::sync::oneshot::Sender<bool>>)>>,
    msg_rx: Receiver<Bytes>,
    adapter: D,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl<D: MsgAdapter> Communator<D> {
    pub async fn wait_stop(&mut self) {
        drop(self.ctrl_tx.take());
        self.handle.take().unwrap().await.unwrap();
    }
    pub fn new(mut socket: TcpStream, adapter: D)->Communator<D> {
        let (mut task_tx, mut task_rx) = tokio::sync::mpsc::channel::<(ConnectCtrlMsg, Option<tokio::sync::oneshot::Sender<bool>>)>(1);
        let (reply_tx, reply_rx) = channel::unbounded::<Bytes>();
        let replay_tx_clone = reply_tx.clone();
        // new client
        let handle = tokio::spawn(async move {
            let (mut recv_s, mut send_s) = socket.split();
            let mut frame_reader = FramedRead::new(recv_s, LengthDelimitedCodec::new());
            let mut frame_writer = FramedWrite::new(send_s, LengthDelimitedCodec::new());
            while let Some((task, oh)) = task_rx.recv().await {
                //replay_tx_clone.send(xxx)
                match task {
                    ConnectCtrlMsg::SendData(data) => {
                        frame_writer.send(data).await.unwrap();
                    },
                    ConnectCtrlMsg::FetchData() => {
                        if let Some(r) = frame_reader.next().await {
                            if let Ok(r) = r {
                                replay_tx_clone.send(r.freeze()).unwrap();
                            }
                        }
                    },
                    ConnectCtrlMsg::DisConnect() => {
                        unimplemented!();
                    }
                }
                if let Some(oh) = oh {
                    oh.send(true);
                }
            }
        });
        Communator {ctrl_tx: Some(task_tx), adapter: adapter, msg_rx: reply_rx, handle: Some(handle)}
    }

    pub async fn send_data(&self, data: &D::DataType)->io::Result<()> {
        if let Ok(data) = self.adapter.encode(data) {
            let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
            if self.ctrl_tx.as_ref().unwrap().send((ConnectCtrlMsg::SendData(data.freeze()), Some(tx))).await.is_err() {
                Err(io::Error::new(io::ErrorKind::Other, "Send data failed"))
            } else {
                match rx.await {
                    Ok(_) => {
                        Ok(())
                    },
                    Err(e) => {
                        Err(io::Error::new(io::ErrorKind::Other, e))
                    }
                }
            }
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "encode error"))
        }
    }

    pub async fn recv_data(&self)->io::Result<D::DataType> {
        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
        if self.ctrl_tx.as_ref().unwrap().send((ConnectCtrlMsg::FetchData(), Some(tx))).await.is_err() {
            Err(io::Error::new(io::ErrorKind::Other, "Send data failed"))
        } else {
            match rx.await {
                Ok(_) => {
                    if let Ok(d) = self.msg_rx.recv() {
                        if let Ok(data) = self.adapter.decode(&d) {
                            Ok(data)
                        } else {
                            Err(io::Error::new(io::ErrorKind::Other, "decode error"))
                        }
                    } else {
                        Err(io::Error::new(io::ErrorKind::Other, "recv channel failed"))
                    }
                },
                Err(e) => {
                    Err(io::Error::new(io::ErrorKind::Other, e))
                }
            }
        }
    }
}

#[test]
fn test_communator() {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_io().build().unwrap();

    struct SimpleMsg {
        data:String,
    }
    struct SimpleAdapter {}
    impl MsgAdapter for SimpleAdapter {
        type DataType = SimpleMsg;
        fn decode(&self, data: &Bytes) ->Result<Self::DataType, ()> {
            Ok(SimpleMsg{data:String::from_utf8(data.to_vec()).unwrap()})
        }
        fn encode(&self, data: &Self::DataType) ->Result<BytesMut, ()> {
            Ok( BytesMut::from(data.data.as_bytes()))
        }
    }
    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    let h = rt.spawn(async move {
         // start server
         let listener = TcpListener::bind("127.0.0.1:6116").await.unwrap();
         tx.send(true).unwrap();
         loop {
            let (socket, _) = listener.accept().await.unwrap();
            // create communator
            let mut server_conn = Communator::new(socket, SimpleAdapter{});
            if let Ok(x) = server_conn.recv_data().await {
                assert_eq!(x.data, "hello from client");
            }
            println!("server send");
            server_conn.send_data(&SimpleMsg{data:"hello from server".to_owned()}).await.unwrap();
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
        let mut connect = Communator::new(socket, SimpleAdapter{});
        println!("start send data");
        connect.send_data(&SimpleMsg{data:"hello from client".to_owned()}).await.unwrap();
        let r = connect.recv_data().await;
        assert_eq!(r.is_ok(), true);
        assert_eq!(r.unwrap().data, "hello from server".to_owned());
        connect.wait_stop().await;
    });

    // 等待服务器结束
    rt.block_on(async {
        h.await
    }).unwrap();
}