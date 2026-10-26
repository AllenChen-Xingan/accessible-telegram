//! `auth`：登录（T01）一组动作的共用部分——两份后端（`td` / [`crate::fake`]）都用这里。
//!
//! - [`AuthPrompt`]：当前授权步骤 + **给人看的提示**（验证码发到哪、几位、密码提示语、邮箱样式）。
//!   提示语可能就是密码的变体、邮箱样式是个人信息：**只给终端 / 登录界面，不进 CLI 的 JSON**
//!   （`actions.md` `auth_state`）。所以密码提示语、邮箱样式装在 [`Secret`] 里，`Debug` 不打印它们；手机号
//!   （TDLib 的 `code_info.phone_number`）根本不取
//! - [`normalize_phone`]：本地先校验号码形状（不合格不发给 TDLib）
//! - [`test_code_for`]：测试号的验证码规则（`testing.md` 第一节第 5 小节）
//! - [`AuthAction`]：每个登录动作收哪几步（唯一一份表）· [`require_step`]：不收 → `auth_wrong_state`
//! - [`prepare_phone`] · [`prepare_register`] · [`prepare_logout`]：两份后端交给 TDLib 之前共用的检查
//! - [`strip_spaces`]：验证码、邮箱去空白
//!
//! 规约：`specs/contracts/actions.md` 第一节第 3 小节「登录」；`errors.md` 第一节第 1 小节 `auth_*`。

use crate::config::ServerKind;
use crate::error::CoreError;
use crate::secret::Secret;
use crate::status::AuthStep;
use serde::Serialize;
use serde_json::Value;
use std::fmt;

/// 测试服务器上自动注册用的名字（只在测试档案上注册，`testing.md` 第一节第 5 小节第 1 条；
/// `fixtures/names.txt` 还没建，先写死一个明显是测试的名字）
pub const TEST_REGISTER_FIRST_NAME: &str = "Atg";
pub const TEST_REGISTER_LAST_NAME: &str = "Test";

/// 验证码发到哪（`AuthenticationCodeType`，`td_api.tl:29-70`）。只要种类，不要附带的号码前缀、链接等
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeVia {
    /// 发到已登录的 Telegram 应用里
    TelegramApp,
    Sms,
    /// 短信里的一个词（输入首字母给出的那个词）
    SmsWord,
    /// 短信里的一句话
    SmsPhrase,
    /// 电话读出来
    Call,
    /// 来电号码就是验证码（要读号码的后几位）
    FlashCall,
    /// 未接来电：号码后几位是验证码
    MissedCall,
    /// fragment.com 上收
    Fragment,
    /// 走手机系统推送（本项目在 Mac 上收不到）
    Firebase,
    /// TDLib 升级后新增的
    Other,
}

impl CodeVia {
    fn from_td_type(t: &str) -> Self {
        match t {
            "authenticationCodeTypeTelegramMessage" => Self::TelegramApp,
            "authenticationCodeTypeSms" => Self::Sms,
            "authenticationCodeTypeSmsWord" => Self::SmsWord,
            "authenticationCodeTypeSmsPhrase" => Self::SmsPhrase,
            "authenticationCodeTypeCall" => Self::Call,
            "authenticationCodeTypeFlashCall" => Self::FlashCall,
            "authenticationCodeTypeMissedCall" => Self::MissedCall,
            "authenticationCodeTypeFragment" => Self::Fragment,
            "authenticationCodeTypeFirebaseAndroid" | "authenticationCodeTypeFirebaseIos" => {
                Self::Firebase
            }
            _ => Self::Other,
        }
    }

    /// 给人听的一句「验证码发到哪」（占位措辞，正本以后进 `specs/style/`）
    pub fn describe_zh(self) -> &'static str {
        match self {
            Self::TelegramApp => "验证码已发到你已登录的 Telegram 应用里",
            Self::Sms => "验证码已用短信发出",
            Self::SmsWord => "验证码是短信里的一个词",
            Self::SmsPhrase => "验证码是短信里的一句话",
            Self::Call => "验证码会用电话读给你",
            Self::FlashCall => "会有一个来电，来电号码就是验证码",
            Self::MissedCall => "会有一个未接来电，号码的后几位是验证码",
            Self::Fragment => "验证码发到了 fragment.com",
            Self::Firebase => "验证码走手机系统推送，本应用收不到，请输入 r 改用别的方式",
            Self::Other => "验证码已发出",
        }
    }
}

/// 当前授权步骤 + 这一步给人看的提示
pub struct AuthPrompt {
    pub step: AuthStep,
    /// `wait_code`：验证码发到哪
    pub code_via: Option<CodeVia>,
    /// `wait_code` / `wait_email_code`：验证码几位（TDLib 给 0 或没给就是 `None`）
    pub code_length: Option<u32>,
    /// `wait_code`：还能不能换一种方式重发（`next_type` 不为空）
    pub can_resend: bool,
    /// `wait_code`：多少秒后才可以重发（0 / 没给就是 `None`）
    pub resend_after_s: Option<u32>,
    /// `wait_password`：设过找回邮箱
    pub has_recovery_email: bool,
    password_hint: Option<Secret>,
    email_pattern: Option<Secret>,
}

impl AuthPrompt {
    /// 只有步骤、没有提示
    pub fn bare(step: AuthStep) -> Self {
        Self {
            step,
            code_via: None,
            code_length: None,
            can_resend: false,
            resend_after_s: None,
            has_recovery_email: false,
            password_hint: None,
            email_pattern: None,
        }
    }

    /// `updateAuthorizationState.authorization_state`（TDLib JSON）→ 提示。字段缺了就当没有，不 panic
    pub fn from_td(state: &Value) -> Self {
        let ty = state["@type"].as_str().unwrap_or("");
        let mut p = Self::bare(AuthStep::from_td_type(ty));
        let len = |v: &Value| {
            v.as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .filter(|n| *n > 0)
        };
        let secret = |v: &Value| {
            v.as_str()
                .filter(|s| !s.is_empty())
                .map(|s| Secret::new(s.as_bytes().to_vec()))
        };
        match p.step {
            AuthStep::WaitCode => {
                let info = &state["code_info"];
                let t = &info["type"];
                p.code_via = Some(CodeVia::from_td_type(t["@type"].as_str().unwrap_or("")));
                p.code_length = len(&t["length"]);
                p.can_resend = info["next_type"].is_object();
                p.resend_after_s = len(&info["timeout"]);
            }
            AuthStep::WaitEmailCode => {
                let info = &state["code_info"];
                p.code_length = len(&info["length"]);
                p.email_pattern = secret(&info["email_address_pattern"]);
            }
            AuthStep::WaitPassword => {
                p.password_hint = secret(&state["password_hint"]);
                p.has_recovery_email = state["has_recovery_email_address"].as_bool() == Some(true);
            }
            _ => {}
        }
        p
    }

    /// 密码提示语（人自己设的，可能就是密码的变体）：**只许写到终端 / 登录界面**
    pub fn password_hint(&self) -> Option<&str> {
        self.password_hint.as_ref().map(Secret::expose_str)
    }

    /// 打了码的邮箱样式（如 `a***@e***.com`）：**只许写到终端 / 登录界面**
    pub fn email_pattern(&self) -> Option<&str> {
        self.email_pattern.as_ref().map(Secret::expose_str)
    }
}

fn copy(s: &Option<Secret>) -> Option<Secret> {
    s.as_ref().map(|s| Secret::new(s.expose().to_vec()))
}

impl Clone for AuthPrompt {
    fn clone(&self) -> Self {
        Self {
            step: self.step,
            code_via: self.code_via,
            code_length: self.code_length,
            can_resend: self.can_resend,
            resend_after_s: self.resend_after_s,
            has_recovery_email: self.has_recovery_email,
            password_hint: copy(&self.password_hint),
            email_pattern: copy(&self.email_pattern),
        }
    }
}

impl fmt::Debug for AuthPrompt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 提示语与邮箱样式不打印，只说有没有
        f.debug_struct("AuthPrompt")
            .field("step", &self.step)
            .field("code_via", &self.code_via)
            .field("code_length", &self.code_length)
            .field("can_resend", &self.can_resend)
            .field("resend_after_s", &self.resend_after_s)
            .field("has_recovery_email", &self.has_recovery_email)
            .field("has_password_hint", &self.password_hint.is_some())
            .field("has_email_pattern", &self.email_pattern.is_some())
            .finish()
    }
}

/// 当前步骤在不在 `allowed` 里；不在 → `auth_wrong_state`（带当前步骤）
pub fn require_step(current: AuthStep, allowed: &[AuthStep]) -> Result<(), CoreError> {
    if allowed.contains(&current) {
        Ok(())
    } else {
        Err(CoreError::AuthWrongState { expected: current })
    }
}

/// 一个登录动作（两份后端与 CLI 共用这张「收哪几步」的表，免得各写一份）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthAction {
    Phone,
    Code,
    Resend,
    Email,
    EmailCode,
    Password,
    Register,
}

impl AuthAction {
    /// 这个动作在哪几步收
    pub fn allowed(self) -> &'static [AuthStep] {
        match self {
            Self::Phone => PHONE_STEPS,
            Self::Code | Self::Resend => &[AuthStep::WaitCode],
            Self::Email => &[AuthStep::WaitEmail],
            Self::EmailCode => &[AuthStep::WaitEmailCode],
            Self::Password => &[AuthStep::WaitPassword],
            Self::Register => &[AuthStep::WaitRegistration],
        }
    }

    /// 交「这一步」的值用哪个动作（`wait_code` → 交验证码……）；不收输入的步骤落 `Phone`（它收的步骤最多，
    /// 调用方接着会被 `require_step` 拦下）
    pub fn for_step(step: AuthStep) -> Self {
        match step {
            AuthStep::WaitCode => Self::Code,
            AuthStep::WaitEmail => Self::Email,
            AuthStep::WaitEmailCode => Self::EmailCode,
            AuthStep::WaitPassword => Self::Password,
            AuthStep::WaitRegistration => Self::Register,
            _ => Self::Phone,
        }
    }

    /// 当前步骤收不收这个动作；不收 → `auth_wrong_state`
    pub fn check(self, current: AuthStep) -> Result<(), CoreError> {
        require_step(current, self.allowed())
    }
}

/// `setAuthenticationPhoneNumber` 收的步骤（TDLib 允许在登录中途换号重来，`td_api.tl:12192` 的说明）
pub const PHONE_STEPS: &[AuthStep] = &[
    AuthStep::WaitPhone,
    AuthStep::WaitCode,
    AuthStep::WaitEmail,
    AuthStep::WaitEmailCode,
    AuthStep::WaitPassword,
    AuthStep::WaitRegistration,
];

/// 交号码之前两份后端都做的检查：步骤 → 形状 → 档案。过了给规范后的号码
pub fn prepare_phone(
    current: AuthStep,
    kind: ServerKind,
    raw: &Secret,
) -> Result<Secret, CoreError> {
    AuthAction::Phone.check(current)?;
    let phone = normalize_phone(raw.expose())?;
    if kind.phone_allowed(phone.expose_str()) {
        Ok(phone)
    } else {
        Err(phone_not_for_profile())
    }
}

/// 注册之前的检查：步骤对、而且是测试档案
pub fn prepare_register(current: AuthStep, kind: ServerKind) -> Result<(), CoreError> {
    AuthAction::Register.check(current)?;
    if kind == ServerKind::Test {
        Ok(())
    } else {
        Err(CoreError::Forbidden { td: None })
    }
}

/// 登出收的步骤：客户端还在、没在关（`logOut` 在 TDLib 里任何时候都能调，但已在退出 / 关闭时再调没有意义）
pub fn prepare_logout(current: AuthStep) -> Result<(), CoreError> {
    match current {
        AuthStep::WaitParameters | AuthStep::LoggingOut | AuthStep::Closing | AuthStep::Closed => {
            Err(CoreError::AuthWrongState { expected: current })
        }
        _ => Ok(()),
    }
}

/// 号码 → 规范形状：只留 `+` 与数字（空格、连字符、括号去掉），5–20 位数字。不合格 → `auth_phone_invalid`
pub fn normalize_phone(raw: &[u8]) -> Result<Secret, CoreError> {
    let s = std::str::from_utf8(raw).map_err(|_| invalid_phone())?;
    let s = s.trim();
    let mut out = Vec::with_capacity(s.len());
    for (i, c) in s.chars().enumerate() {
        match c {
            '0'..='9' => out.push(c as u8),
            '+' if i == 0 => out.push(b'+'),
            ' ' | '-' | '(' | ')' | '.' => {}
            _ => {
                let _ = Secret::new(out); // 清零
                return Err(invalid_phone());
            }
        }
    }
    let out = Secret::new(out);
    let digits = out.expose().iter().filter(|b| b.is_ascii_digit()).count();
    if (5..=20).contains(&digits) {
        Ok(out)
    } else {
        Err(invalid_phone())
    }
}

fn invalid_phone() -> CoreError {
    CoreError::AuthPhoneInvalid { td: None }
}

/// 号码与档案不配（测试档案只收 `99966` 开头，正式档案不收，`config-privacy.md` 第一节第 2 小节）
pub fn phone_not_for_profile() -> CoreError {
    CoreError::AuthPhoneNotForProfile
}

/// 验证码、邮箱 → 去掉全部空白（中间的空格也去掉：有人会照着「12 345」输）。密码不用它（空格是密码的一部分）
pub fn strip_spaces(raw: &[u8]) -> Secret {
    Secret::new(
        raw.iter()
            .copied()
            .filter(|b| !b.is_ascii_whitespace())
            .collect(),
    )
}

/// 测试号 `99966XYYYY`（X = 测试数据中心号 1–3）的验证码：X 连写五遍（core.telegram.org/api/auth
/// 「Test accounts」，`testing.md` 第一节第 5 小节）。不是这种号就 `None`
pub fn test_code_for(phone: &[u8]) -> Option<Secret> {
    let digits: Vec<u8> = phone.iter().copied().filter(u8::is_ascii_digit).collect();
    let digits = Secret::new(digits);
    let d = digits.expose();
    if d.len() == 10 && d.starts_with(b"99966") && (b'1'..=b'3').contains(&d[5]) {
        Some(Secret::new(vec![d[5]; 5]))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn prompt_from_wait_code() {
        let p = AuthPrompt::from_td(&json!({
            "@type": "authorizationStateWaitCode",
            "code_info": {
                "@type": "authenticationCodeInfo",
                "phone_number": "+99966 1 2345",
                "type": {"@type": "authenticationCodeTypeTelegramMessage", "length": 5},
                "next_type": {"@type": "authenticationCodeTypeSms", "length": 5},
                "timeout": 60,
            }
        }));
        assert_eq!(p.step, AuthStep::WaitCode);
        assert_eq!(p.code_via, Some(CodeVia::TelegramApp));
        assert_eq!(p.code_length, Some(5));
        assert!(p.can_resend);
        assert_eq!(p.resend_after_s, Some(60));
        assert!(!format!("{p:?}").contains("2345"));
    }

    #[test]
    fn prompt_from_wait_password_keeps_hint_out_of_debug() {
        let p = AuthPrompt::from_td(&json!({
            "@type": "authorizationStateWaitPassword",
            "password_hint": "暗号提示",
            "has_recovery_email_address": true,
            "has_passport_data": false,
            "recovery_email_address_pattern": "暗***@x.com",
        }));
        assert_eq!(p.step, AuthStep::WaitPassword);
        assert_eq!(p.password_hint(), Some("暗号提示"));
        assert!(p.has_recovery_email);
        let dbg = format!("{p:?} {:?}", p.clone());
        assert!(!dbg.contains("暗号"), "{dbg}");
    }

    #[test]
    fn prompt_from_wait_email_code() {
        let p = AuthPrompt::from_td(&json!({
            "@type": "authorizationStateWaitEmailCode",
            "allow_apple_id": false, "allow_google_id": false,
            "code_info": {"@type": "emailAddressAuthenticationCodeInfo",
                          "email_address_pattern": "暗***@x.com", "length": 6},
        }));
        assert_eq!(p.step, AuthStep::WaitEmailCode);
        assert_eq!(p.code_length, Some(6));
        assert_eq!(p.email_pattern(), Some("暗***@x.com"));
        assert!(!format!("{p:?}").contains("暗"));
    }

    #[test]
    fn prompt_tolerates_garbage() {
        let p = AuthPrompt::from_td(&json!({"@type": "authorizationStateWaitCode"}));
        assert_eq!(p.code_via, Some(CodeVia::Other));
        assert_eq!(p.code_length, None);
        let p = AuthPrompt::from_td(&json!(null));
        assert_eq!(p.step, AuthStep::Unsupported);
    }

    #[test]
    fn phone_normalization() {
        let ok = |s: &str| normalize_phone(s.as_bytes()).map(|p| p.expose_str().to_owned());
        assert_eq!(ok(" +99966 1-2345 ").unwrap(), "+9996612345");
        assert_eq!(ok("(999) 661 2345").unwrap(), "9996612345");
        assert!(ok("12").is_err());
        assert!(ok("99966a12345").is_err());
        assert!(ok("99+96612345").is_err());
        assert!(ok("").is_err());
        assert!(matches!(
            normalize_phone(b"x"),
            Err(CoreError::AuthPhoneInvalid { td: None })
        ));
    }

    #[test]
    fn test_codes() {
        let c = |s: &str| test_code_for(s.as_bytes()).map(|c| c.expose_str().to_owned());
        assert_eq!(c("+9996612345").as_deref(), Some("11111"));
        assert_eq!(c("99966 3 0000").as_deref(), Some("33333"));
        assert_eq!(c("9996642345"), None, "DC 只有 1–3");
        assert_eq!(c("999661234"), None, "位数不对");
        assert_eq!(c("8613800000000"), None);
    }

    #[test]
    fn wrong_state_reports_current() {
        assert_eq!(
            require_step(AuthStep::WaitPassword, &[AuthStep::WaitCode]),
            Err(CoreError::AuthWrongState {
                expected: AuthStep::WaitPassword
            })
        );
        assert!(require_step(AuthStep::WaitCode, PHONE_STEPS).is_ok());
    }

    #[test]
    fn phone_must_match_profile() {
        let s = |x: &str| Secret::new(x.as_bytes().to_vec());
        let test = ServerKind::Test;
        let main = ServerKind::Main;
        assert!(prepare_phone(AuthStep::WaitPhone, test, &s("+99966 2 1234")).is_ok());
        assert_eq!(
            prepare_phone(AuthStep::WaitPhone, test, &s("+86 138 0000 0000")).err(),
            Some(CoreError::AuthPhoneNotForProfile)
        );
        assert_eq!(
            prepare_phone(AuthStep::WaitPhone, main, &s("9996621234")).err(),
            Some(CoreError::AuthPhoneNotForProfile)
        );
        assert!(prepare_phone(AuthStep::WaitPhone, main, &s("+86 138 0000 0000")).is_ok());
        assert!(matches!(
            prepare_phone(AuthStep::Ready, test, &s("9996621234")),
            Err(CoreError::AuthWrongState { .. })
        ));
        assert!(prepare_register(AuthStep::WaitRegistration, test).is_ok());
        assert!(prepare_register(AuthStep::WaitRegistration, main).is_err());
        assert!(prepare_logout(AuthStep::Ready).is_ok());
        assert!(prepare_logout(AuthStep::Closed).is_err());
    }

    #[test]
    fn code_normalization_strips_spaces() {
        assert_eq!(strip_spaces(b" 12 345\n").expose_str(), "12345");
    }
}
