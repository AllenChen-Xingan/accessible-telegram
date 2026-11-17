//! `transcribe` 外壳：节点一行 + 结尾行的形状、18+ 遮挡、语种选择。内容全是编的
use super::*;
use atg_core::model::MsgId;
use atg_core::tree::{Outcome, Transcript, TranscriptSegment, TranscriptSource};
use serde_json::Value;

#[derive(Default)]
struct Cap {
    out: Vec<String>,
    err: Vec<String>,
}

impl Sink for Cap {
    fn out(&mut self, line: &str) {
        self.out.push(line.to_owned());
    }
    fn err(&mut self, line: &str) {
        self.err.push(line.to_owned());
    }
}

fn cx(reveal: bool) -> SpeechCtx {
    history::speech_ctx(Some("+08:00"), reveal).unwrap_or_else(|_| panic!("时区"))
}

fn text() -> Outcome {
    let seg = |t: &str, a: f64| TranscriptSegment {
        text: t.into(),
        start_s: a,
        end_s: a + 1.0,
    };
    Outcome::Text(Transcript {
        lang: "zh-CN".into(),
        source: TranscriptSource::Local,
        segments: vec![seg("今天下午开会", 0.0), seg("记得带资料。", 1.0)],
    })
}

fn node(outcome: Outcome, sensitive: bool) -> Transcription {
    Transcription {
        id: MsgId {
            chat_id: -1001,
            message_id: 7,
        },
        sensitive,
        outcome,
    }
}

fn json_lines(t: Transcription, reveal: bool) -> (Vec<Value>, Vec<String>) {
    let mut c = Cap::default();
    assert_eq!(
        render(&mut c, Format::Json, &cx(reveal), t, Instant::now()),
        0
    );
    let ls = c
        .out
        .iter()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    (ls, c.err)
}

#[test]
fn text_is_one_node_line_and_a_result() {
    let (ls, err) = json_lines(node(text(), false), false);
    assert_eq!(ls.len(), 2);
    assert_eq!(ls[0]["kind"], "transcription");
    assert_eq!(ls[0]["data"]["type"], "transcription");
    assert_eq!(ls[0]["data"]["id"]["message_id"], 7);
    assert_eq!(ls[0]["data"]["outcome"]["type"], "text");
    assert_eq!(ls[0]["data"]["outcome"]["lang"], "zh-CN");
    assert_eq!(ls[0]["speech"]["brief"], "今天下午开会，记得带资料。");
    assert!(ls[0]["at"].is_string());
    assert_eq!(ls[1]["kind"], "result");
    assert_eq!(ls[1]["data"]["command"], "transcribe");
    assert_eq!(ls[1]["data"]["outcome"], "text");
    assert!(err.is_empty());
}

#[test]
fn no_speech_says_so() {
    let (ls, _) = json_lines(node(Outcome::NoSpeech, false), false);
    assert_eq!(ls[0]["data"]["outcome"]["type"], "no_speech");
    assert_eq!(ls[0]["speech"]["brief"], "没有听出内容");
    assert_eq!(ls[1]["data"]["outcome"], "no_speech");
}

/// 18+ 遮挡没展开：文字不出（JSON、读法、结尾行都不带），标准错误提示加 --reveal；展开了照出
#[test]
fn sensitive_hidden_unless_revealed() {
    let (ls, err) = json_lines(node(text(), true), false);
    assert_eq!(
        ls[0]["data"]["outcome"],
        serde_json::json!({"type": "hidden"})
    );
    assert_eq!(ls[0]["speech"]["brief"], "隐藏内容");
    assert_eq!(ls[1]["data"]["outcome"], "hidden");
    let all = format!("{ls:?}{err:?}");
    assert!(!all.contains("开会"), "{all}");
    assert!(err[0].contains("--reveal"));
    let (ls, err) = json_lines(node(text(), true), true);
    assert_eq!(ls[0]["data"]["outcome"]["type"], "text");
    assert!(ls[0]["speech"]["brief"].as_str().unwrap().contains("开会"));
    assert!(err.is_empty());
}

#[test]
fn text_format_is_just_the_sentence() {
    let mut c = Cap::default();
    render(
        &mut c,
        Format::Text,
        &cx(false),
        node(text(), false),
        Instant::now(),
    );
    assert_eq!(c.out, vec!["今天下午开会，记得带资料。".to_owned()]);
}

#[test]
fn lang_picks_locales() {
    assert_eq!(locales(None), vec!["zh-CN", "en-US"]);
    assert_eq!(locales(Some(Lang::Zh)), vec!["zh-CN"]);
    assert_eq!(locales(Some(Lang::En)), vec!["en-US"]);
}
