use std::{collections::HashMap, ffi::CStr, os::raw::c_char};

pub struct ModuleInfo {
    pub name: String,
    pub base: u64,
    pub size: u64,
}
impl ModuleInfo {
    pub fn new_dummy()->ModuleInfo {
        ModuleInfo {
            name: "Dummy".to_owned(),
            base: 0,
            size: 0,
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
struct ModuleInfoInner{
    name: [c_char;256],
    base: u64,
    size: u64
}

impl Default for ModuleInfoInner {
    fn default() -> Self {
        ModuleInfoInner {
            name: [0;256],
            base: 0,
            size: 0
        }
    }
}
impl Clone for ModuleInfo {
    fn clone(&self) -> Self {
        ModuleInfo {name: self.name.to_owned(), base: self.base, size: self.size}
    }
}

pub fn get_module_info(pid: usize) -> Result<Vec<ModuleInfo>, Box<dyn std::error::Error>> {
    unsafe {
        let lib = libloading::Library::new("LibNativeHelper.dll")?;
        let func: libloading::Symbol<unsafe extern fn(u32, *mut ModuleInfoInner, u32) -> u32> = lib.get(b"GetProcessModules")?;
        let mut r = vec![ModuleInfoInner::default(); 1024];
        let mut all_module = Vec::new();
        let cnt = func(pid as u32, r.as_mut_ptr(), r.len() as u32);
        for i in 0..cnt {
            let a = &mut r[i as usize];
            let c_str: &CStr = CStr::from_ptr(a.name.as_ptr());
            let s = c_str.to_owned().into_string()?;
            let m = ModuleInfo { name: s, base: a.base, size: a.size };
            all_module.push(m);
        }
        Ok(all_module)
    }
}

pub struct Process {
    pid: usize,
    module_info_cache: Option<ModuleInfo>,
    all_module_info: Vec<ModuleInfo>,
    modules_by_name: HashMap<String, ModuleInfo>,
}

impl Process {
    pub fn new(pid: usize)->Process {
        let all_module_info = get_module_info(pid).unwrap();
        let mut modules_by_name = HashMap::new();
        for x in &all_module_info {
            modules_by_name.insert(x.name.to_owned(), x.clone());
        }
        Process {pid: pid, module_info_cache: None, all_module_info: all_module_info, modules_by_name: modules_by_name}
    }

    pub fn query_module_info_by_addr(&mut self, addr: usize) ->Result<(String, usize), ()> {
        if let Some(m) = &self.module_info_cache {
            let base = m.base as usize;
            let size = m.size as usize;
            if addr >= base && addr <= base + size {
                return Ok((m.name.to_owned(), addr - m.base as usize));
            }
        }

        for m in &self.all_module_info {
            let base = m.base as usize;
            let size = m.size as usize;
            if addr >= base && addr <= base + size {
                self.module_info_cache = Some(ModuleInfo {name: m.name.to_owned(), base: m.base, size: m.size});
                return Ok((m.name.to_owned(), addr - m.base as usize));
            }
        }
        Err(())
    }

    pub fn query_module_info_by_module_name(&self, name: &str)->Result<(usize, usize),()> {
        if let Some(r) = self.modules_by_name.get(name) {
            Ok((r.base as usize, r.size as usize))
        } else {
            Err(())
        }
    }
}
#[test]
fn test_get_module()
{
    use winapi::um::processthreadsapi::GetCurrentProcessId;
    let pid = unsafe {
        GetCurrentProcessId()
    };
    let m = get_module_info(pid as usize).unwrap();
    assert_ne!(m.len(), 0);
    for m in &m {
        assert_ne!(m.base, 0);
        assert_ne!(m.size, 0);
    }
}

#[test]
fn test_process()
{
    use winapi::um::processthreadsapi::GetCurrentProcessId;
    let pid = unsafe {
        GetCurrentProcessId()
    };

    let m = get_module_info(pid as usize).unwrap();
    assert_ne!(m.len(), 0);
    let one_module = &m[0];
    let mut p = Process::new(pid as usize);
    let r = p.query_module_info_by_addr(one_module.base as usize + 1);
    assert_eq!(r.is_ok(), true);
}