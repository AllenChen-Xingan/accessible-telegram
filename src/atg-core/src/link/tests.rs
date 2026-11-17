//! `link` 的单测：授权线与代次在同一把锁里换（#64）

use super::Shared;
use crate::auth::AuthPrompt;
use crate::status::AuthStep;
use std::thread;
use std::time::Duration;

/// 等待方看到新代次时，拿到的一定是这一次的步骤（修前代次先加、步骤后改，窄窗口里会拿到上一步）
#[test]
fn a_new_generation_always_comes_with_its_step() {
    let (shared, _stream) = Shared::new();
    let steps = [AuthStep::WaitCode, AuthStep::WaitPassword];
    for i in 0..1000 {
        let step = steps[i % 2];
        let seen = shared.auth_gen();
        let pusher = {
            let shared = shared.clone();
            thread::spawn(move || shared.set_auth(AuthPrompt::bare(step)))
        };
        let got = shared.wait_auth_after(Duration::from_secs(5), seen);
        pusher.join().unwrap();
        assert_eq!(got, Some(step), "第 {i} 次");
        assert_eq!(shared.auth_prompt().step, step);
    }
}
