#[cfg(test)]
mod test {
    use std::time::Duration;
    use tokio::fs;
    use tokio::runtime;
    use tokio::time::sleep;

    use ptkgenerator::file_writer::DataWriter;
    #[test]
    fn test_writer() {
        let rst_file:String;
        let suffix = "tmp".to_owned();
        let v = vec![65; 100];
        let threaded_rt = runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        {
            let mut d = DataWriter::new(&threaded_rt, 0, "", &suffix);
            d.write(&v, v.len());
            rst_file = d.file_name;
        }

        threaded_rt.block_on(async {
            sleep(Duration::from_millis(1000)).await;
            let complete_name = format!("{}data", &rst_file[..rst_file.len() - suffix.len()]);
            println!("complete name {}", complete_name);
            let r = fs::read(&complete_name).await.unwrap();
            assert_eq!(r[..], v[..r.len()]);
            fs::remove_file(&complete_name).await.unwrap();
        });
    }
}
