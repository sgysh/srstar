use std::fmt;
use std::fs;
use std::io;
use std::io::prelude::*;
use std::iter;
use std::iter::repeat;
use std::mem;
use std::os::unix::prelude::*;
use std::path::Path;

#[repr(C)]
pub struct OldHeader {
    pub name: [u8; 100],
    pub mode: [u8; 8],
    pub uid: [u8; 8],
    pub gid: [u8; 8],
    pub size: [u8; 12],
    pub mtime: [u8; 12],
    pub chksum: [u8; 8],
    pub linkflag: [u8; 1],
    pub linkname: [u8; 100],
    pub pad: [u8; 255],
}

#[repr(C)]
struct GnuHeader {
    name: [u8; 100],
    mode: [u8; 8],
    uid: [u8; 8],
    gid: [u8; 8],
    size: [u8; 12],
    mtime: [u8; 12],
    chksum: [u8; 8],
    typeflag: [u8; 1],
    linkname: [u8; 100],
    magic: [u8; 6],
    version: [u8; 2],
    uname: [u8; 32],
    gname: [u8; 32],
    devmajor: [u8; 8],
    devminor: [u8; 8],
    atime: [u8; 12],
    ctime: [u8; 12],
    offset: [u8; 12],
    longnames: [u8; 4],
    unused: [u8; 1],
    sparse: [GnuSparseHeader; 4],
    isextended: [u8; 1],
    realsize: [u8; 12],
    pad: [u8; 17],
}

#[repr(C)]
struct GnuSparseHeader {
    offset: [u8; 12],
    numbytes: [u8; 12],
}

#[repr(C)]
struct Header {
    bytes: [u8; 512],
}

impl Header {
    pub fn new() -> Header {
        let mut header = Header { bytes: [0; 512] };
        unsafe {
            let gnu = cast_mut::<_, GnuHeader>(&mut header);
            gnu.magic = *b"ustar ";
            gnu.version = *b" \0";
        }
        header
    }

    pub fn set_name(&mut self, path: &Path) -> io::Result<()> {
        let slot: &mut [u8] = &mut self.as_mut_gnu().name;
        let bytes = path.as_os_str().as_bytes();
        copy_into(slot, bytes)?;
        return Ok(());
    }

    fn calculate_chksum(&self) -> u32 {
        let old = self.as_old();
        let start = old as *const _ as usize;
        let chksum_start = old.chksum.as_ptr() as *const _ as usize;
        let offset = chksum_start - start;
        let len = old.chksum.len();
        self.bytes[0..offset]
            .iter()
            .chain(iter::repeat(&b' ').take(len))
            .chain(&self.bytes[offset + len..])
            .fold(0, |a, b| a + (*b as u32))
    }

    pub fn as_mut_gnu(&mut self) -> &mut GnuHeader {
        unsafe { cast_mut(self) }
    }

    fn as_old(&self) -> &OldHeader {
        unsafe { cast(self) }
    }
}

pub struct Archiver<W: Write> {
    obj: Option<W>,
}

impl<W: Write> Archiver<W> {
    pub fn new(obj: W) -> Archiver<W> {
        Archiver { obj: Some(obj) }
    }

    fn inner(&mut self) -> &mut W {
        self.obj.as_mut().unwrap()
    }

    pub fn add_file<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let meta = fs::metadata(&path)?;
        let mut header = Header::new();

        header.set_name(path.as_ref())?;
        octal_into(&mut header.as_mut_gnu().mode, meta.mode());
        octal_into(&mut header.as_mut_gnu().uid, meta.uid());
        octal_into(&mut header.as_mut_gnu().gid, meta.gid());
        octal_into(
            &mut header.as_mut_gnu().size,
            if meta.is_file() { meta.len() } else { 0 },
        );
        octal_into(&mut header.as_mut_gnu().mtime, meta.mtime());

        let chksum = header.calculate_chksum();
        octal_into(&mut header.as_mut_gnu().chksum, chksum);

        self.inner().write_all(&header.bytes)?;

        let len = if meta.is_file() {
            let mut contents = fs::File::open(&path)?;
            io::copy(&mut contents, &mut self.inner())?
        } else {
            0
        };

        let buf = [0; 512];
        let remaining = 512 - (len % 512);
        if remaining < 512 {
            self.inner().write_all(&buf[..remaining as usize])?;
        }

        Ok(())
    }
}

impl<W: Write> Drop for Archiver<W> {
    fn drop(&mut self) {
        let _ = self.inner().write_all(&[0; 1024]);
    }
}

unsafe fn cast<T, U>(a: &T) -> &U {
    assert_eq!(mem::size_of_val(a), mem::size_of::<U>());
    assert_eq!(mem::align_of_val(a), mem::align_of::<U>());
    &*(a as *const T as *const U)
}

unsafe fn cast_mut<T, U>(a: &mut T) -> &mut U {
    assert_eq!(mem::size_of_val(a), mem::size_of::<U>());
    assert_eq!(mem::align_of_val(a), mem::align_of::<U>());
    &mut *(a as *mut T as *mut U)
}

fn octal_into<T: fmt::Octal>(dst: &mut [u8], val: T) {
    let o = format!("{:o}", val);
    let value = o.bytes().rev().chain(repeat(b'0'));
    for (slot, value) in dst.iter_mut().rev().skip(1).zip(value) {
        *slot = value;
    }
}

fn copy_into(slot: &mut [u8], bytes: &[u8]) -> io::Result<()> {
    for (slot, val) in slot.iter_mut().zip(bytes.iter().chain(Some(&0))) {
        *slot = *val;
    }
    Ok(())
}
