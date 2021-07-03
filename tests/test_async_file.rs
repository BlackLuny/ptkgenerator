#[cfg(test)]
mod test {
    use std::sync::mpsc::Sender;

    use tokio::runtime;
    use tokio::fs;
    use tokio::sync::{mpsc, oneshot};
    use tokio::io::AsyncWriteExt;
    use std::time::Duration;
    use tokio::time::sleep;
    use std::sync::Arc;
    use std::cell::RefCell;

    use ptkgenerator::process_util::DataWriter;
    #[test]
    fn test_writer()
    {
        let rst_file;
        let v = vec![65;100];
        let threaded_rt = runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        {
            let d = DataWriter::new(&threaded_rt, 0, "", "tmp");
            d.write(&v, v.len());
            rst_file = d.file_name;
        }

        threaded_rt.block_on(async {
            sleep(Duration::from_millis(1000)).await;
            let r = fs::read(&rst_file).await.unwrap();
            assert_eq!(r[..], v[..r.len()]);
            fs::remove_file(&rst_file).await.unwrap();
        });
        
    }

}