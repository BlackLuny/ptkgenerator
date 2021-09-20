use tokio::{io::AsyncReadExt, net::{TcpListener, TcpStream, tcp::{OwnedReadHalf, OwnedWriteHalf, ReadHalf}}, runtime::Runtime, sync::mpsc::Sender, task::JoinHandle};
use crossbeam::channel::{self, Receiver};
use std::{fmt::Write, io::{self, Read}, sync::Arc};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use futures::{SinkExt, StreamExt, io::{AsyncWriteExt}};
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
pub struct CommunatorSender<D: MsgAdapter> {
    //frame_writer: FramedWrite<OwnedWriteHalf, LengthDelimitedCodec>,
    tx: tokio::sync::mpsc::Sender<(Bytes, Option<tokio::sync::oneshot::Sender<io::Result<()>>>)>,
    h: Option<JoinHandle<()>>, 
    adapter: D,
}

impl<D: MsgAdapter> CommunatorSender<D> {
    pub fn new(mut framer: FramedWrite<OwnedWriteHalf, LengthDelimitedCodec>, adapter: D)->Self {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<(Bytes, Option<tokio::sync::oneshot::Sender<io::Result<()>>>)>(10);
        let h = tokio::spawn(async move {
            while let Some((data, oh) )= rx.recv().await {
                let r = if framer.send(data).await.is_err() {
                    Err(io::Error::new(io::ErrorKind::Other, "Send data failed"))
                } else {
                    Ok(())
                };
                if let Some(oh) = oh {
                    oh.send(r).unwrap();
                }
            }
        });
        CommunatorSender{tx: tx, h: Some(h), adapter: adapter}
    }
    pub async fn send_data(&self, data: D::DataType)->io::Result<()> {
        let r = if let Ok(data) = self.adapter.encode(&data) {
            let (tx, rx) = tokio::sync::oneshot::channel::<io::Result<()>>();
            let r = self.tx.send((data.freeze(), Some(tx))).await;
            if r.is_ok() {
                match rx.await {
                    Ok(v) => {
                        match v {
                            Ok(_) => {
                                Ok(())
                            },
                            Err(e) => {
                                Err(io::Error::new(io::ErrorKind::Other, "oh error"))
                            }
                        }
                    },
                    Err(e) => {
                        Err(io::Error::new(io::ErrorKind::Other, "send error"))
                    }
                }
            } else {
                Err(io::Error::new(io::ErrorKind::Other, "send error"))
            }
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "encode error"))
        };
        r
    }


}

pub struct CommunatorReceiver<D: MsgAdapter> {
    frame_reader: FramedRead<OwnedReadHalf, LengthDelimitedCodec>,
    adapter: D
}

// 接收数据必须要
impl<D: MsgAdapter> CommunatorReceiver<D> {
    pub fn new(framer: FramedRead<OwnedReadHalf, LengthDelimitedCodec>, adapter: D)->Self {
        CommunatorReceiver{frame_reader: framer, adapter: adapter}
    }

    pub async fn recv_data(&mut self)->io::Result<D::DataType> {
        let read_rst = self.frame_reader.next().await;
        match read_rst {
            Some(s) => {
                match s {
                    Ok(r) => {
                        match self.adapter.decode(&r.freeze()) {
                            Ok(r) => {
                                Ok(r)
                            },
                            Err(_) => {
                                Err(io::Error::new(io::ErrorKind::Other, "decode error"))
                            }
                        }
                    },
                    Err(e) => {
                        Err(io::Error::new(io::ErrorKind::Other, e))
                    }
                }
            },
            None => {
                Err(io::Error::new(io::ErrorKind::Other, "recv None"))
            }
        }
    }
}
pub struct Communator<D: MsgAdapter> {
    //ctrl_tx: Option<Sender<(ConnectCtrlMsg, Option<tokio::sync::oneshot::Sender<bool>>)>>,
    //msg_rx: Receiver<Bytes>,
    frame_reader: FramedRead<OwnedReadHalf, LengthDelimitedCodec>,
    frame_writer: FramedWrite<OwnedWriteHalf, LengthDelimitedCodec>,
    adapter: D,
    //handle: Option<tokio::task::JoinHandle<()>>,
}

impl<D: MsgAdapter + Clone> Communator<D> {
    pub async fn wait_stop(&mut self) {
        // drop(self.ctrl_tx.take());
        // self.handle.take().unwrap().await.unwrap();
    }
    pub fn split(self)->(CommunatorSender<D>, CommunatorReceiver<D>) {
        (CommunatorSender::new(self.frame_writer, self.adapter.clone()), CommunatorReceiver::new(self.frame_reader, self.adapter.clone()))
    }
    pub fn new(mut socket: TcpStream, adapter: D)->Communator<D> {
        let (mut task_tx, mut task_rx) = tokio::sync::mpsc::channel::<(ConnectCtrlMsg, Option<tokio::sync::oneshot::Sender<bool>>)>(1);
        let (reply_tx, reply_rx) = channel::unbounded::<Bytes>();
        let replay_tx_clone = reply_tx.clone();

        let (mut recv_s, mut send_s) = socket.into_split ();
        let mut frame_reader = FramedRead::new(recv_s, LengthDelimitedCodec::new());
        let mut frame_writer = FramedWrite::new(send_s, LengthDelimitedCodec::new());

        // new client
        // let handle = tokio::spawn(async move {

        //     while let Some((task, oh)) = task_rx.recv().await {
        //         //replay_tx_clone.send(xxx)
        //         match task {
        //             ConnectCtrlMsg::SendData(data) => {
        //                 frame_writer.send(data).await.unwrap();
        //             },
        //             ConnectCtrlMsg::FetchData() => {
        //                 if let Some(r) = frame_reader.next().await {
        //                     if let Ok(r) = r {
        //                         replay_tx_clone.send(r.freeze()).unwrap();
        //                     }
        //                 }
        //             },
        //             ConnectCtrlMsg::DisConnect() => {
        //                 unimplemented!();
        //             }
        //         }
        //         if let Some(oh) = oh {
        //             oh.send(true);
        //         }
        //     }
        // });
        Communator {adapter: adapter, frame_reader: frame_reader, frame_writer:frame_writer}
    }

    pub async fn send_data(&mut self, data: &D::DataType)->io::Result<()> {
        if let Ok(data) = self.adapter.encode(data) {
            if self.frame_writer.send(data.freeze()).await.is_err() {
                Err(io::Error::new(io::ErrorKind::Other, "Send data failed"))
            } else {
                Ok(())
            }
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "encode error"))
        }
    }

    pub async fn recv_data(&mut self)->io::Result<D::DataType> {
        if let Some(s) = self.frame_reader.next().await {
            if let Ok(r) = s {
                if let Ok(r) = self.adapter.decode(&r.freeze()) {
                    return Ok(r);
                }
            }
        }
        Err(io::Error::new(io::ErrorKind::Other, "recv error"))
    }
}

#[test]
fn test_communator() {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_io().build().unwrap();

    struct SimpleMsg {
        data:String,
    }
    #[derive(Clone)]
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