//! `secret`：装密码、验证码、钥匙、`api_hash` 的盒子 [`Secret`]。
//!
//! `config-privacy.md` 第一节第 5 小节：用完清零，`Debug` 只打 `<secret>`；不实现 `Clone` / `Display` /
//! `Serialize`，要用原值只能显式调 [`Secret::expose`]。交给 TDLib 时它会以 JSON 字符串穿过 tdlib-rs 与
//! TDLib 的内存，那里的副本清不掉（规约已知）。

use std::fmt;

pub struct Secret(Vec<u8>);

impl Secret {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn from_string(s: String) -> Self {
        Self(s.into_bytes())
    }

    /// 原值。只在交给 TDLib / 钥匙串的那一刻用
    pub fn expose(&self) -> &[u8] {
        &self.0
    }

    /// 原值当 UTF-8 读（`api_hash` 这类）；不是 UTF-8 就给空
    pub fn expose_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("")
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        wipe_all(&mut self.0);
    }
}

/// 把整块容量都清零，不只 `0..len`：读输入时退格 / 截短删掉的字节还留在容量区里（#40）
pub fn wipe_all(v: &mut Vec<u8>) {
    let len = v.len();
    // SAFETY: 先把长度设成 0，再把 0..capacity 整块当未初始化的字节写 0：写的都在这块分配之内、对齐的 u8；
    // 随后长度恢复成原值，前 len 个字节是刚写好的 0，已初始化
    unsafe {
        v.set_len(0);
        let p = v.as_mut_ptr();
        for i in 0..v.capacity() {
            // volatile：别让编译器把「写完就丢」的清零优化掉
            std::ptr::write_volatile(p.add(i), 0);
        }
        v.set_len(len);
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<secret>")
    }
}

/// 标准 base64（带补齐）。TDLib JSON 里 `bytes` 类型的字段用它（如 `database_encryption_key`）
pub fn base64(bytes: &[u8]) -> Secret {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let idx = |shift: u32| T[((n >> shift) & 63) as usize];
        out.push(idx(18));
        out.push(idx(12));
        out.push(if chunk.len() > 1 { idx(6) } else { b'=' });
        out.push(if chunk.len() > 2 { idx(0) } else { b'=' });
    }
    Secret(out)
}

#[cfg(test)]
mod tests {
    use super::wipe_all;

    /// 截短后留在容量区的字节也清掉（#40）
    #[test]
    fn wipe_all_clears_the_spare_capacity_too() {
        let mut v = Vec::with_capacity(16);
        v.extend_from_slice(b"secret");
        v.truncate(2);
        wipe_all(&mut v);
        assert_eq!(v, [0, 0]);
        // SAFETY: 读的是本 Vec 自己分配的容量区，刚被 wipe_all 写过 0，已初始化
        let spare = unsafe { std::slice::from_raw_parts(v.as_ptr(), v.capacity()) };
        assert!(spare.iter().all(|&b| b == 0));
    }
}
