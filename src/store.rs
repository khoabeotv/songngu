//! Định dạng nhị phân của mô hình: mảng số và chuỗi nằm liền nhau, căn 8 byte, đọc thẳng qua mmap.
//! Trang file chỉ đọc không tính vào bộ nhớ riêng của tiến trình (footprint) và không phải parse lúc khởi động.
//! Đọc từng phần tử đều có kiểm tra biên: file hỏng thì tra không thấy, không đọc lố.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::io;
use std::path::Path;

/// Đổi khi định dạng đổi: file cũ bị từ chối thay vì đọc sai.
const MAGIC: &[u8; 8] = b"SNGUmd05";

/// Kiểu số đọc thẳng từ byte (little-endian, không padding).
pub trait Pod: Copy + 'static {}
impl Pod for u8 {}
impl Pod for u32 {}
impl Pod for f32 {}
impl Pod for f64 {}

/// Mảng của mô hình: mượn từ file đã map, hoặc tự giữ khi vừa học xong.
pub type Arr<T> = Cow<'static, [T]>;

fn bad() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "model file is damaged or from another version")
}

#[derive(Default)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Writer {
        Writer { buf: MAGIC.to_vec() }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    pub fn arr<T: Pod>(&mut self, a: &[T]) {
        self.buf.extend_from_slice(&(a.len() as u64).to_le_bytes());
        // SAFETY: T là kiểu số không padding; đọc byte của nó luôn hợp lệ.
        let bytes = unsafe { std::slice::from_raw_parts(a.as_ptr() as *const u8, std::mem::size_of_val(a)) };
        self.buf.extend_from_slice(bytes);
        while self.buf.len() % 8 != 0 {
            self.buf.push(0);
        }
    }

    pub fn f64(&mut self, v: f64) {
        self.arr(&[v]);
    }

    pub fn strs(&mut self, list: &StrList) {
        self.arr(&list.blob);
        self.arr(&list.off);
    }
}

pub struct Reader {
    data: &'static [u8],
    pos: usize,
}

impl Reader {
    pub fn new(data: &'static [u8]) -> io::Result<Reader> {
        if cfg!(target_endian = "big") || data.len() < 8 || &data[..8] != MAGIC || data.as_ptr() as usize % 8 != 0 {
            return Err(bad());
        }
        Ok(Reader { data, pos: 8 })
    }

    fn take(&mut self, n: usize) -> io::Result<&'static [u8]> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len()).ok_or_else(bad)?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    pub fn arr<T: Pod>(&mut self) -> io::Result<Arr<T>> {
        let n = u64::from_le_bytes(self.take(8)?.try_into().unwrap()) as usize;
        let bytes = self.take(n.checked_mul(std::mem::size_of::<T>()).ok_or_else(bad)?)?;
        self.pos = (self.pos + 7) & !7;
        if bytes.as_ptr() as usize % std::mem::align_of::<T>() != 0 {
            return Err(bad());
        }
        // SAFETY: đủ byte, đúng căn lề, T là kiểu số (mọi mẫu bit đều hợp lệ); dữ liệu sống tới hết tiến trình.
        Ok(Cow::Borrowed(unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const T, n) }))
    }

    pub fn f64(&mut self) -> io::Result<f64> {
        self.arr::<f64>()?.first().copied().ok_or_else(bad)
    }

    /// Kiểm UTF-8 và biên một lần lúc nạp (dưới 1 ms), để `StrList::get` trong đường nóng không phải kiểm lại.
    pub fn strs(&mut self) -> io::Result<StrList> {
        let blob = self.arr::<u8>()?;
        let off = self.arr::<u32>()?;
        let text = std::str::from_utf8(&blob).map_err(|_| bad())?;
        let ok = off.first() == Some(&0)
            && off.windows(2).all(|w| w[0] <= w[1])
            && off.iter().all(|&o| text.is_char_boundary(o as usize));
        if !ok {
            return Err(bad());
        }
        Ok(StrList { blob, off })
    }
}

/// Ánh xạ cả file vào bộ nhớ, chỉ đọc, sống tới hết tiến trình (bộ gõ nạp mô hình một lần).
/// File phải được thay bằng cách đổi tên (xem `write_atomic`), không ghi đè tại chỗ.
pub fn map_file(path: &Path) -> io::Result<&'static [u8]> {
    let f = std::fs::File::open(path)?;
    let len = f.metadata()?.len() as usize;
    if len == 0 {
        return Err(bad());
    }
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        use std::os::raw::{c_int, c_void};
        extern "C" {
            fn mmap(addr: *mut c_void, len: usize, prot: c_int, flags: c_int, fd: c_int, offset: i64) -> *mut c_void;
            fn madvise(addr: *mut c_void, len: usize, advice: c_int) -> c_int;
        }
        const PROT_READ: c_int = 1;
        const MAP_PRIVATE: c_int = 2;
        const MADV_WILLNEED: c_int = 3;
        // SAFETY: fd hợp lệ, len là kích thước file; kết quả kiểm tra MAP_FAILED trước khi dùng.
        let p = unsafe { mmap(std::ptr::null_mut(), len, PROT_READ, MAP_PRIVATE, f.as_raw_fd(), 0) };
        if p as isize == -1 {
            return Err(io::Error::last_os_error());
        }
        // Đọc trước cả file (bất đồng bộ): phím đầu tiên không phải chờ đĩa. Trang file sạch, không tính vào footprint.
        // SAFETY: vùng vừa map.
        unsafe { madvise(p, len, MADV_WILLNEED) };
        // SAFETY: vùng map dài `len`, không bao giờ unmap.
        Ok(unsafe { std::slice::from_raw_parts(p as *const u8, len) })
    }
    #[cfg(not(unix))]
    {
        drop(f);
        // Box<[u64]> để căn 8 byte.
        let bytes = std::fs::read(path)?;
        let mut words = vec![0u64; bytes.len().div_ceil(8)].into_boxed_slice();
        // SAFETY: words đủ chỗ cho bytes.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), words.as_mut_ptr() as *mut u8, bytes.len()) };
        let p = Box::leak(words).as_ptr() as *const u8;
        Ok(unsafe { std::slice::from_raw_parts(p, bytes.len()) })
    }
}

/// Ghi file mới rồi đổi tên: tiến trình đang map file cũ vẫn đọc bản cũ, không bị cắt ngang.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// Danh sách chuỗi liền nhau: một khối byte + vị trí bắt đầu của từng chuỗi. Khối là UTF-8 hợp lệ và mọi vị trí nằm
/// đúng ranh giới ký tự (kiểm lúc nạp hoặc tự dựng từ chuỗi).
pub struct StrList {
    blob: Arr<u8>,
    /// len + 1 phần tử.
    off: Arr<u32>,
}

impl StrList {
    pub fn from_strings<S: AsRef<str>>(items: &[S]) -> StrList {
        let mut blob = Vec::new();
        let mut off = vec![0u32];
        for s in items {
            blob.extend_from_slice(s.as_ref().as_bytes());
            off.push(blob.len() as u32);
        }
        StrList { blob: Cow::Owned(blob), off: Cow::Owned(off) }
    }

    pub fn len(&self) -> usize {
        self.off.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Chuỗi thứ `i` ("" nếu ngoài biên).
    pub fn get(&self, i: usize) -> &str {
        let (Some(&a), Some(&b)) = (self.off.get(i), self.off.get(i + 1)) else { return "" };
        match self.blob.get(a as usize..b as usize) {
            // SAFETY: khối đã kiểm UTF-8, a và b nằm trên ranh giới ký tự (xem `Reader::strs`).
            Some(s) => unsafe { std::str::from_utf8_unchecked(s) },
            None => "",
        }
    }

    /// Tìm nhị phân theo `cmp(chuỗi)` (Less = chuỗi đứng trước mục tiêu): vị trí đầu tiên không Less.
    pub fn partition(&self, lo: usize, hi: usize, mut less: impl FnMut(&str) -> bool) -> usize {
        let (mut lo, mut hi) = (lo, hi);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if less(self.get(mid)) {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// Danh sách xếp theo thứ tự chuỗi thường: vị trí của `s`.
    pub fn find(&self, s: &str) -> Option<usize> {
        let i = self.partition(0, self.len(), |x| x < s);
        (i < self.len() && self.get(i) == s).then_some(i)
    }

    /// Danh sách xếp theo thứ tự chuỗi thường: các chuỗi bắt đầu bằng `p`. So byte (thứ tự byte UTF-8 trùng thứ tự ký tự).
    pub fn prefix_range(&self, p: &str) -> (usize, usize) {
        self.prefix_range_in(0, self.len(), p)
    }

    /// Như `prefix_range` nhưng chỉ tìm trong [lo, hi) (khoảng của một tiền tố ngắn hơn của `p`).
    pub fn prefix_range_in(&self, lo: usize, hi: usize, p: &str) -> (usize, usize) {
        let lo = self.partition(lo, hi, |s| cmp_prefix_bytes(s, p).is_lt());
        let hi = self.partition(lo, hi, |s| cmp_prefix_bytes(s, p).is_le());
        (lo, hi)
    }
}

/// Như `cmp_prefix` cho chuỗi, so byte. Vòng lặp tự viết: chuỗi chỉ vài byte, gọi memcmp còn đắt hơn so.
#[inline]
pub fn cmp_prefix_bytes(s: &str, p: &str) -> Ordering {
    let (s, p) = (s.as_bytes(), p.as_bytes());
    for (i, &c) in p.iter().enumerate() {
        match s.get(i) {
            None => return Ordering::Less,
            Some(&x) if x != c => return x.cmp(&c),
            _ => {}
        }
    }
    Ordering::Equal
}

/// So chuỗi `s` với tiền tố `p`: Equal nếu `s` bắt đầu bằng `p`, không thì thứ tự từ điển.
pub fn cmp_prefix(mut s: impl Iterator<Item = char>, p: impl Iterator<Item = char>) -> Ordering {
    for c in p {
        match s.next() {
            None => return Ordering::Less,
            Some(x) if x != c => return x.cmp(&c),
            _ => {}
        }
    }
    Ordering::Equal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_search() {
        let words = ["an", "anh", "ba", "bao", "ca"];
        let list = StrList::from_strings(&words);
        let mut w = Writer::new();
        w.strs(&list);
        w.arr(&[1.5f64, 2.5]);
        let bytes: &'static [u8] = Box::leak(w.into_bytes().into_iter().collect::<Vec<u8>>().into_boxed_slice());
        // Vec<u8> không bảo đảm căn 8: chép sang vùng căn 8 như mmap.
        let aligned: &'static mut [u64] = Box::leak(vec![0u64; bytes.len().div_ceil(8)].into_boxed_slice());
        let p = aligned.as_mut_ptr() as *mut u8;
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len()) };
        let data: &'static [u8] = unsafe { std::slice::from_raw_parts(p, bytes.len()) };
        let mut r = Reader::new(data).unwrap();
        let l = r.strs().unwrap();
        assert_eq!(l.len(), 5);
        assert_eq!(l.find("bao"), Some(3));
        assert_eq!(l.find("b"), None);
        assert_eq!(l.prefix_range("an"), (0, 2));
        assert_eq!(l.prefix_range("b"), (2, 4));
        assert_eq!(&*r.arr::<f64>().unwrap(), &[1.5, 2.5]);
        assert!(Reader::new(&data[..4]).is_err());
    }
}
