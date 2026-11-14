//! `localauth`：发送闸的人类确认——macOS 本地认证（LocalAuthentication 的 `LAContext`），Touch ID 或系统密码
//! （策略 `LAPolicyDeviceOwnerAuthentication`）。001 拍板 9；`actions.md` 第一节第 4 小节第 1、4 条。
//!
//! 为什么它拦得住 AI：弹框由系统进程画，`tmux send-keys`、伪终端、管道都按不了；人不在场就过不去。
//! 每次发送新建一个 `LAContext`，不复用上一次的认证结果。
//!
//! 弹不出框（经 SSH 运行、没有图形会话、本机没设密码）→ `not_interactive`；人按了取消或没过 → `auth_failed`。

use crate::error::SendRefusedReason;
use crate::gate::Authenticator;
use block2::RcBlock;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool};
use objc2_foundation::NSString;
use std::sync::mpsc;
use std::time::Duration;

#[link(name = "LocalAuthentication", kind = "framework")]
unsafe extern "C" {}

/// `LAPolicyDeviceOwnerAuthentication`：Touch ID，不行就系统密码
const POLICY_DEVICE_OWNER: isize = 2;
/// 人最多想多久。受一个上限约束：命令经本机接口转给常驻的 `atg-cli serve` 执行时，转发的那个进程每收到一行
/// 输出后最多再等 150 秒（`atg-cli` 的 `deadlines::FRAME_TIMEOUT`）；`send` 出完计划行后要等这里（≤ 60 秒）
/// 再等发送结果（≤ 30 秒）才有下一行，所以这里不能超过 150 − 30 秒，留足余量取 60
const WAIT: Duration = Duration::from_secs(60);

/// macOS 的 Touch ID / 系统密码
pub struct LocalAuth;

/// 经 SSH 来的会话：弹框会出在本机屏幕上，远端的人看不到，按不了——当作弹不出
fn over_ssh() -> bool {
    ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
        .iter()
        .any(|k| std::env::var_os(k).is_some())
}

impl Authenticator for LocalAuth {
    fn authenticate(&self, reason: &str) -> Result<(), SendRefusedReason> {
        if over_ssh() {
            return Err(SendRefusedReason::NotInteractive);
        }
        let Some(cls) = AnyClass::get(c"LAContext") else {
            return Err(SendRefusedReason::NotInteractive);
        };
        // SAFETY：`+[LAContext new]` 返回一个已持有的新对象
        let ctx: Option<Retained<AnyObject>> = unsafe { msg_send![cls, new] };
        let Some(ctx) = ctx else {
            return Err(SendRefusedReason::NotInteractive);
        };
        let mut err: *mut AnyObject = std::ptr::null_mut();
        // SAFETY：`-canEvaluatePolicy:error:` 的签名是 (NSInteger, NSError **) -> BOOL
        let can: Bool =
            unsafe { msg_send![&*ctx, canEvaluatePolicy: POLICY_DEVICE_OWNER, error: &mut err] };
        if !can.as_bool() {
            return Err(SendRefusedReason::NotInteractive);
        }
        let (tx, rx) = mpsc::channel::<bool>();
        let reply = RcBlock::new(move |ok: Bool, _e: *mut AnyObject| {
            let _ = tx.send(ok.as_bool());
        });
        let text = NSString::from_str(reason);
        // SAFETY：`-evaluatePolicy:localizedReason:reply:` 的签名是 (NSInteger, NSString *, void (^)(BOOL, NSError *))；
        // 回调在系统的私有队列上调，块由 RcBlock 持有到调用完
        unsafe {
            let _: () = msg_send![
                &*ctx,
                evaluatePolicy: POLICY_DEVICE_OWNER,
                localizedReason: &*text,
                reply: &*reply
            ];
        }
        match rx.recv_timeout(WAIT) {
            Ok(true) => Ok(()),
            Ok(false) => Err(SendRefusedReason::AuthFailed),
            // 超时：把框收掉（`-invalidate`），免得人过后按了以为发出去了。之后来的回调写进已没人收的信道，丢掉
            Err(_) => {
                // SAFETY：`-[LAContext invalidate]` 无参数无返回
                unsafe {
                    let _: () = msg_send![&*ctx, invalidate];
                }
                Err(SendRefusedReason::AuthFailed)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 刻度尺（交接记录里的「冒烟 B」）：手动量 Touch ID 框能完整显示多少，是 `gate::CONFIRM_MAX_CHARS` /
    /// `CONFIRM_MAX_LINES` / `CONFIRM_LINE_COLS` 的依据。会真弹框，默认不跑；人在场时
    /// `ATG_RULER=1 cargo test -p atg-core --lib localauth -- --ignored --nocapture`。
    /// 8 行：A–F 行是汉字，G、H 行是大写拉丁字母 M（验「拉丁字母也按 2 列算」够不够保守）。每行 6 段，每段 10 个字符：
    /// 8 个字 + 段标（行字母 + 段号），如 `C4` 是 C 行第 4 段的末尾。整行没有空格，框只能按字断。
    /// 人读出：① 每行第一个视觉行末尾看到的段标（框会把一行折开）；② 框里最后完整看到的段标、有没有省略号。然后按取消——不发任何东西。
    /// 换算：`gate` 一个字符算 2 列，一段 = 20 列。第一个视觉行里完整看到 k 段又 j 个字 → 列宽 = (10k + j) × 2；
    /// 最后完整看到的段标前一共折了几行 → 行数上限；G/H 行一行放下的段数不少于汉字行，就说明 M 按 2 列算是保守的。
    /// 口径：j 只数汉字或 M，段标里的字母数字不单算（落在段尾，量到段标就算整段）；macOS 自己加在说明前面的那句
    /// 「“atg-cli”想要……」不算进行数（它在说明区外另占位置，行数上限只管我们的说明）。
    /// 量不到的：按词折行（每行没有空格）。`gate::wrapped` 按「放不下的词整个挪到下一行」估，偏保守，暂不实测
    #[test]
    #[ignore]
    fn ruler_popup() {
        if std::env::var_os("ATG_RULER").is_none() {
            return;
        }
        let ruler: Vec<String> = "ABCDEFGH"
            .chars()
            .map(|row| {
                let fill = if row < 'G' {
                    "字字字字字字字字"
                } else {
                    "MMMMMMMM"
                };
                (1..=6).map(|k| format!("{fill}{row}{k}")).collect()
            })
            .collect();
        let got = LocalAuth.authenticate(&ruler.join("\n"));
        println!(
            "刻度尺弹框结果：{}",
            if got.is_ok() {
                "按了通过"
            } else {
                "取消或未过"
            }
        );
    }
}
