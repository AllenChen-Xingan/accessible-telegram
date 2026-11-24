//! 执行方写进本机接口连接的两种出口：一次性命令的 [`ConnSink`]、`watch` 订阅者的 [`ConnOut`]

use crate::out::Sink;
use crate::watch::LineOut;
use atg_core::ipc::{Conn, Frame};

/// 订阅者的出口：本机接口的连接
pub(super) struct ConnOut {
    pub(super) conn: Conn,
}

impl LineOut for ConnOut {
    fn line(&mut self, line: &str) -> bool {
        self.conn
            .send(&Frame::Out {
                line: line.to_owned(),
            })
            .is_ok()
    }

    fn gone(&self) -> bool {
        atg_core::ipc::peer_closed(self.conn.stream())
    }

    fn finish(&mut self, code: u8) {
        let _ = self.conn.send(&Frame::Exit { code });
    }
}

/// 写进连接的出口。对方走了（写失败）就不再写，命令照常做完
pub(super) struct ConnSink<'a> {
    pub(super) conn: &'a mut Conn,
    pub(super) broken: bool,
}

impl ConnSink<'_> {
    fn send(&mut self, f: &Frame) {
        if !self.broken && self.conn.send(f).is_err() {
            self.broken = true;
        }
    }
}

impl Sink for ConnSink<'_> {
    fn out(&mut self, line: &str) {
        self.send(&Frame::Out {
            line: line.to_owned(),
        });
    }

    fn err(&mut self, line: &str) {
        self.send(&Frame::Err {
            line: line.to_owned(),
        });
    }
}
