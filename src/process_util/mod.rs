
    use tokio::io::AsyncWriteExt;
    use tokio::fs;
    use tokio::runtime::Runtime;
    use tokio::sync::{mpsc};
    use std::time::SystemTime;
    pub struct DataWriter {
        tx: mpsc::Sender<Vec<u8>>,
        pub file_name:String,
    }
    fn create_async_writer(rt: &Runtime, file_name: &str) -> mpsc::Sender<Vec<u8>>
    {
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(10000);
        let f_name = file_name.to_owned();
        rt.spawn(async move {
            let mut f = fs::File::create(&f_name).await.unwrap();
            while let Some(d) = rx.recv().await {
                f.write_all(&d).await.unwrap();
            }
            println!("write finished!");
            f.flush().await.unwrap();
        });
        tx
    }

    fn get_file_name(idx:u32, dir:&str, suffix:&str)->String {
        let time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs().to_string();
        format!("{}\\{}-{}.{}", dir, time, idx.to_string(), suffix)
    }

    impl DataWriter {
        pub fn new(rt: &Runtime, idx:u32, dir:&str, suffix:&'static str) ->DataWriter {
            let name = get_file_name(idx, dir, suffix);
            DataWriter {tx: create_async_writer(rt, &name), file_name: name}
        }

        pub fn write(&self, data:&Vec<u8>, len:usize) {
            let d = data[..len].to_vec();
            self.tx.try_send(d).unwrap();
        }
    }