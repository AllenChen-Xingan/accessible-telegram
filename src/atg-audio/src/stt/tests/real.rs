//! 要真辅助程序与系统语言模型的测试，全是 `#[ignore]`；运行条件见 `../tests.rs` 文件头。
//! 过滤词 `stt::tests::real` 正好选中这一组

use super::*;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// 真模型端到端：编的中文夹具出中文（运行条件见文件头）
#[test]
#[ignore = "要真辅助程序与系统语言模型，见文件头"]
fn real_helper_zh_fixture() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let h = real_helper();
    let pcm = crate::decode(&root.join("fixtures/zh.ogg")).unwrap();
    let r = transcribe(&h, &pcm, &both(), &AtomicBool::new(false));
    assert_eq!(locale_of(&r), Some("zh-CN"), "{r:?}");
}

/// 单边失败走真辅助程序（design.md 第三节第 1 条）：一个语种系统不支持，另一个照样转完、协议完整、退出码 0。
/// 直接读原始协议（`read_protocol` 本身要求每个语种恰好一行、done 计数对得上），逐语种断言三态，再看选择
#[test]
#[ignore = "要真辅助程序与系统语言模型，见文件头"]
fn real_helper_one_locale_fails() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let pcm = crate::decode(&root.join("fixtures/zh.ogg")).unwrap();
    let locales = vec!["zh-CN".to_owned(), "xx-XX".to_owned()];
    let mut h = Command::new(real_helper())
        .args(["--parent-pid", &std::process::id().to_string()])
        .args(["--rate", &SAMPLE_RATE.to_string(), "--deadline-s", "60"])
        .args(locales.iter().flat_map(|l| ["--locale", l.as_str()]))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    // 它先读完输入才出结果，输出只有几行：先写完再读不会互相堵住
    let bytes: Vec<u8> = pcm.samples.iter().flat_map(|x| x.to_le_bytes()).collect();
    h.stdin.take().unwrap().write_all(&bytes).unwrap();
    let out = h.wait_with_output().unwrap();
    assert!(out.status.success(), "退出码不是 0");
    let got = read_protocol(&out.stdout[..], &locales, || {})
        .unwrap_or_else(|e| panic!("协议不完整：{e:?}"));
    assert!(matches!(
        got.iter().find(|(l, _)| l == "zh-CN"),
        Some((_, Outcome::Text(..)))
    ));
    assert!(matches!(
        got.iter().find(|(l, _)| l == "xx-XX"),
        Some((_, Outcome::Failed { unavailable: true }))
    ));
    assert_eq!(locale_of(&choose(&locales, got)), Some("zh-CN"));
}

/// 兜底（design.md 第三节第 6 条）：第二次 Ctrl+C 时本进程来不及收尾，辅助程序靠「父进程没了」自己退出。
/// 中间进程 sh 起辅助程序（`--parent-pid` 是 sh），读到辅助程序的 pid 和它的 hello（hello 在兜底线程起来之后才出），
/// 再杀掉 sh，断言辅助程序 1 秒内消失、且一行结果都没出（它是被兜底杀的，不是读到输入结束自己收完的）。
/// 输入先 `exec 3<&0` 存下再给 `<&3`：非交互 sh 的后台命令在显式重定向之前标准输入先接 /dev/null，`<&0` 复制的就是它
#[test]
#[ignore = "要真辅助程序，见文件头"]
fn real_helper_exits_when_parent_dies() {
    use std::io::BufRead;
    use std::process::{Command, Stdio};
    let h = real_helper();
    let mut sh = Command::new("sh")
        .arg("-c")
        .arg(r#"exec 3<&0; "$1" --parent-pid $$ --rate "$2" --deadline-s 60 --locale zh-CN <&3 2>/dev/null & echo $!; exec sleep 60 3<&-"#)
        .arg("sh")
        .arg(&h)
        .arg(SAMPLE_RATE.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let _keep_open = sh.stdin.take();
    let mut out = std::io::BufReader::new(sh.stdout.take().unwrap());
    let (mut pid, mut hello) = (None, false);
    while pid.is_none() || !hello {
        let mut line = String::new();
        assert!(out.read_line(&mut line).unwrap() > 0, "没读到 pid 与 hello");
        if line.starts_with('{') {
            hello = line.contains(r#""kind":"hello""#);
        } else {
            pid = Some(line.trim().to_owned());
        }
    }
    let pid = pid.unwrap();
    sh.kill().unwrap();
    sh.wait().unwrap();
    assert!(
        gone_within(&pid, Duration::from_secs(1)),
        "父进程没了，辅助程序 1 秒内没退出"
    );
    // 写端只剩辅助程序，它没了就读到结尾
    let mut rest = String::new();
    std::io::Read::read_to_string(&mut out, &mut rest).unwrap();
    assert!(
        !rest.contains(r#""kind":"locale""#),
        "辅助程序读到了输入结束、自己收完了"
    );
}

/// 父进程在辅助程序起来之前就没了（这时 getppid 已是收养者）：给它一个不是父进程的 pid 模拟，
/// 它应当场退出（退出码 3）、连 hello 都不出
#[test]
#[ignore = "要真辅助程序，见文件头"]
fn real_helper_exits_when_parent_gone_before_start() {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut other = Command::new("sleep").arg("5").spawn().unwrap();
    let mut h = Command::new(real_helper())
        .args([
            "--parent-pid",
            &other.id().to_string(),
            "--rate",
            &SAMPLE_RATE.to_string(),
        ])
        .args(["--deadline-s", "60", "--locale", "zh-CN"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _keep_open = h.stdin.take();
    let t = Instant::now();
    let status = loop {
        if let Some(st) = h.try_wait().unwrap() {
            break Some(st);
        }
        if t.elapsed() > Duration::from_secs(1) {
            let _ = h.kill();
            let _ = h.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let _ = other.kill();
    let _ = other.wait();
    assert_eq!(status.and_then(|s| s.code()), Some(3), "1 秒内没以 3 退出");
    let mut printed = String::new();
    h.stdout
        .take()
        .unwrap()
        .read_to_string(&mut printed)
        .unwrap();
    assert!(printed.is_empty(), "核对父进程之前不该出任何行");
}

fn real_helper() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    locate(root.parent().and_then(|p| p.parent())).expect("先跑 build.sh")
}

/// 用 `kill -0` 看进程还在不在，最多等 `within`；到点还在就强杀并回 false
fn gone_within(pid: &str, within: Duration) -> bool {
    use std::process::{Command, Stdio};
    let alive = || {
        Command::new("kill")
            .args(["-0", pid])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    };
    let t = Instant::now();
    while alive() && t.elapsed() < within {
        std::thread::sleep(Duration::from_millis(50));
    }
    let gone = !alive();
    if !gone {
        let _ = Command::new("kill").args(["-9", pid]).status();
    }
    gone
}
