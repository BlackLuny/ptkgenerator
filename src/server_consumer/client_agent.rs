use tokio::net::TcpStream;

use crate::server_consumer::interface::PtProcMsg;

use super::communator::Communator;
use super::msg_adapter::PtProcMsgAdapter;
pub struct ClientAgent {
    con: Communator<PtProcMsgAdapter>,
}


impl ClientAgent {
    pub fn new(socket: TcpStream) ->Self {
        ClientAgent {con: Communator::new(socket, PtProcMsgAdapter::new())}
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
            server_conn.send_data(&PtProcMsg::DataRsp(1)).await.unwrap();
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
        assert_eq!(r.is_ok(), true);
        assert_eq!(r.unwrap(), PtProcMsg::DataRsp(1));
        connect.wait_stop().await;
    });

    // 等待服务器结束
    rt.block_on(async {
        h.await
    }).unwrap();
}