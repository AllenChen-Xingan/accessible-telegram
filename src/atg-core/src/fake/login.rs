//! 假后端的登录（第 4 块）：假账号 [`FakeAuth`]、按账号核对每步提交、推下一步的授权状态。
//! 从 `fake` 拆出（#60 R7）；`Backend` 的登录方法在根上一行转给这里的 `do_*`，函数体照原样。

use super::{Call, FakeBackend, lock};
use crate::auth;
use crate::config::ServerKind;
use crate::error::{CoreError, map_td_error};
use crate::secret::Secret;
use serde_json::{Value, json};

/// 假账号：登录各步该输什么。测试数据，值都是编的
#[derive(Clone, Default)]
pub struct FakeAuth {
    pub phone: String,
    pub code: String,
    /// 号码之后先要设登录邮箱：（邮箱，邮箱里的验证码）。之后不再要短信验证码
    pub email: Option<(String, String)>,
    /// 二步验证：（密码，提示语）
    pub password: Option<(String, String)>,
    /// 号码还没注册（测试服务器被清空后会这样）
    pub needs_registration: bool,
    /// 起来就已登录
    pub logged_in: bool,
}

impl FakeBackend {
    /// 给一个假账号（`start` 之前设）
    pub fn set_auth(&self, a: FakeAuth) {
        *lock(&self.auth) = Some(a);
    }

    fn kind(&self) -> ServerKind {
        lock(&self.run)
            .as_ref()
            .map_or(ServerKind::Main, |(p, _)| p.kind())
    }

    fn account(&self) -> Result<FakeAuth, CoreError> {
        lock(&self.auth).clone().ok_or(CoreError::Internal {
            what: "fake_no_account",
        })
    }

    /// 一个登录动作的公共部分：记账 → 注入的错误 → 内核故障。过了才轮到账号核对
    async fn auth_call(&self, action: &'static str) -> Result<FakeAuth, CoreError> {
        lock(&self.calls).push(Call::Auth(action));
        self.injected(action).await?;
        self.account()
    }

    fn push_state(&self, state: Value) {
        self.push(json!({ "@type": "updateAuthorizationState", "authorization_state": state }));
    }

    fn push_wait_code(&self, a: &FakeAuth, via: &str, can_resend: bool) {
        let next = if can_resend {
            json!({ "@type": "authenticationCodeTypeSms", "length": a.code.len() })
        } else {
            Value::Null
        };
        self.push_state(json!({
            "@type": "authorizationStateWaitCode",
            "code_info": {
                "@type": "authenticationCodeInfo",
                "phone_number": a.phone,
                "type": { "@type": via, "length": a.code.len() },
                "next_type": next,
                "timeout": 0,
            },
        }));
    }

    /// 验证码（或邮箱验证码）对了之后的下一步
    fn push_after_code(&self, a: &FakeAuth) {
        if a.needs_registration {
            self.push_state(json!({ "@type": "authorizationStateWaitRegistration" }));
        } else {
            self.push_after_registration(a);
        }
    }

    fn push_after_registration(&self, a: &FakeAuth) {
        match &a.password {
            Some((_, hint)) => self.push_state(json!({
                "@type": "authorizationStateWaitPassword",
                "password_hint": hint,
                "has_recovery_email_address": false,
                "has_passport_data": false,
                "recovery_email_address_pattern": "",
            })),
            None => self.push_auth("authorizationStateReady"),
        }
    }

    pub(super) async fn do_auth_submit_phone(&self, phone: &Secret) -> Result<(), CoreError> {
        self.started()?;
        let phone = auth::prepare_phone(self.step(), self.kind(), phone)?;
        let a = self.auth_call("phone").await?;
        let want = auth::normalize_phone(a.phone.as_bytes())?;
        if phone.expose() != want.expose() {
            return Err(map_td_error(400, "PHONE_NUMBER_INVALID"));
        }
        if a.email.is_some() {
            self.push_state(json!({
                "@type": "authorizationStateWaitEmailAddress",
                "allow_apple_id": false,
                "allow_google_id": false,
            }));
        } else {
            self.push_wait_code(&a, "authenticationCodeTypeTelegramMessage", true);
        }
        Ok(())
    }

    pub(super) async fn do_auth_submit_code(&self, code: &Secret) -> Result<(), CoreError> {
        self.started()?;
        auth::AuthAction::Code.check(self.step())?;
        let a = self.auth_call("code").await?;
        if !same(code, &a.code) {
            return Err(map_td_error(400, "PHONE_CODE_INVALID"));
        }
        self.push_after_code(&a);
        Ok(())
    }

    pub(super) async fn do_auth_resend_code(&self) -> Result<(), CoreError> {
        self.started()?;
        auth::AuthAction::Resend.check(self.step())?;
        let a = self.auth_call("resend").await?;
        if !self.shared.auth_prompt().can_resend {
            // TDLib：没有下一种方式时重发被拒
            return Err(map_td_error(400, "SEND_CODE_UNAVAILABLE"));
        }
        self.push_wait_code(&a, "authenticationCodeTypeSms", false);
        Ok(())
    }

    pub(super) async fn do_auth_submit_email(&self, address: &Secret) -> Result<(), CoreError> {
        self.started()?;
        auth::AuthAction::Email.check(self.step())?;
        let a = self.auth_call("email").await?;
        let Some((want, code)) = &a.email else {
            return Err(map_td_error(400, "EMAIL_NOT_ALLOWED"));
        };
        if !same(address, want) {
            return Err(map_td_error(400, "EMAIL_INVALID"));
        }
        self.push_state(json!({
            "@type": "authorizationStateWaitEmailCode",
            "allow_apple_id": false,
            "allow_google_id": false,
            "code_info": {
                "@type": "emailAddressAuthenticationCodeInfo",
                "email_address_pattern": "e***@example.com",
                "length": code.len(),
            },
            "email_address_reset_state": null,
        }));
        Ok(())
    }

    pub(super) async fn do_auth_submit_email_code(&self, code: &Secret) -> Result<(), CoreError> {
        self.started()?;
        auth::AuthAction::EmailCode.check(self.step())?;
        let a = self.auth_call("email_code").await?;
        let ok = a.email.as_ref().is_some_and(|(_, want)| same(code, want));
        if !ok {
            return Err(map_td_error(400, "EMAIL_CODE_INVALID"));
        }
        self.push_after_code(&a);
        Ok(())
    }

    pub(super) async fn do_auth_submit_password(&self, password: &Secret) -> Result<(), CoreError> {
        self.started()?;
        auth::AuthAction::Password.check(self.step())?;
        let a = self.auth_call("password").await?;
        let ok = a
            .password
            .as_ref()
            .is_some_and(|(want, _)| same(password, want));
        if !ok {
            return Err(map_td_error(400, "PASSWORD_HASH_INVALID"));
        }
        self.push_auth("authorizationStateReady");
        Ok(())
    }

    pub(super) async fn do_auth_register(&self) -> Result<(), CoreError> {
        self.started()?;
        auth::prepare_register(self.step(), self.kind())?;
        let a = self.auth_call("register").await?;
        self.push_after_registration(&a);
        Ok(())
    }

    pub(super) async fn do_logout(&self) -> Result<(), CoreError> {
        self.started()?;
        auth::prepare_logout(self.step())?;
        self.auth_call("logout").await?;
        self.push_auth("authorizationStateLoggingOut");
        self.push_auth("authorizationStateClosing");
        self.push_auth("authorizationStateClosed");
        Ok(())
    }
}

/// 输入与假账号里的值一样（逐字节）
fn same(given: &Secret, want: &str) -> bool {
    given.expose() == want.as_bytes()
}
