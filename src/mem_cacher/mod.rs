use std::collections::{HashMap};
pub struct MemCacher {
    cache_lst: HashMap<usize, Box<[u8; 4096]>>,
    last_page: usize,
    last_content: usize,
    pub f: Option<Box<dyn FnMut(&mut [u8], u64) -> usize>>,
}

unsafe impl Send for MemCacher {}

impl MemCacher {
    pub fn new() -> MemCacher {
        MemCacher {
            cache_lst: HashMap::new(),
            last_page: 0,
            last_content: 0,
            f:None,
        }
    }

    fn get_content_in_cache_item(&self, content: &[u8; 4096], addr: u64, rst: &mut [u8]) -> usize {
        let begin = (addr & 0xfff) as usize;
        let end = begin + rst.len();
        rst.copy_from_slice(&content[begin..end]);
        rst.len()
    }

    fn get_content_in_one_page(&mut self, rst: &mut [u8], addr: u64) -> usize {
        let p = (addr & (!0xfff)) as usize;

        if self.last_page == p {
            return self.get_content_in_cache_item(unsafe { &*(self.last_content as *const [u8;4096]) }, addr, rst);
        }

        let r = match self.cache_lst.get(&p) {
            Some(r) => self.get_content_in_cache_item(r, addr, rst),
            None => {
                if let Some(f) = &mut self.f {
                    let mut tmp_buff = Box::new([0 as u8; 4096]);
                    let r = f(&mut *tmp_buff, p as u64);
                    if r != 4096 {
                        let r = f(rst, addr);
                        if r != rst.len() {
                            println!("Get content failed at 0x{:x} size:{}", addr, rst.len());
                            0 as usize
                        } else {
                            r
                        }
                    } else {
                        let r = self.get_content_in_cache_item(&tmp_buff, addr, rst);
                        self.last_content = &*tmp_buff as *const[u8;4096] as usize;
                        self.cache_lst.insert(p, tmp_buff);
                        r
                    }
                } else {
                    0
                }
            }
        };
        r
    }

    pub fn get_content(&mut self, rst: &mut [u8], addr: u64)->usize {
        let start_addr = (addr & (!0xfff)) as usize;
        let end_addr = (addr as usize + rst.len()) & (!0xfff);
        if start_addr == end_addr {
            // one page
            return self.get_content_in_one_page(rst, addr);
        } else {
            let first_page_size = end_addr - addr as usize;
            let mut read_size = self.get_content_in_one_page(&mut rst[..first_page_size], addr);
            read_size += self.get_content_in_one_page(&mut rst[first_page_size..], addr + first_page_size as u64);
            read_size
        }
    }
}
