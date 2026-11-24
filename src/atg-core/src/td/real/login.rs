//! 真实后端的登录：各步提交先做本地检查，再经 `auth_request` 交给 TDLib。
//! 从 `td/real.rs` 拆出（#60 R7）；根上 `Backend` 的登录方法一行转给这里的 `do_*`，函数体照原样。

use super::TdBackend;
use crate::auth;
use crate::error::CoreError;
use crate::secret::Secret;
use tdlib_rs::{enums, functions, types};

impl TdBackend {
    // 交给 TDLib 的值要转成 String（tdlib-rs 的接口如此）：那份副本清不掉，`secret.rs` 头部已说明
    pub(super) async fn do_auth_submit_phone(&self, phone: &Secret) -> Result<(), CoreError> {
        let kind = self.kind().ok_or_else(Self::not_started)?;
        let id = self.client_id().ok_or_else(Self::not_started)?;
        let phone = auth::prepare_phone(self.step(), kind, phone)?;
        let req =
            functions::set_authentication_phone_number(phone.expose_str().to_owned(), None, id);
        self.auth_request("phone", req).await
    }

    pub(super) async fn do_auth_submit_code(&self, code: &Secret) -> Result<(), CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        auth::AuthAction::Code.check(self.step())?;
        let req = functions::check_authentication_code(code.expose_str().to_owned(), id);
        self.auth_request("code", req).await
    }

    pub(super) async fn do_auth_resend_code(&self) -> Result<(), CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        auth::AuthAction::Resend.check(self.step())?;
        let req =
            functions::resend_authentication_code(Some(enums::ResendCodeReason::UserRequest), id);
        self.auth_request("resend", req).await
    }

    pub(super) async fn do_auth_submit_email(&self, address: &Secret) -> Result<(), CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        auth::AuthAction::Email.check(self.step())?;
        let req = functions::set_authentication_email_address(address.expose_str().to_owned(), id);
        self.auth_request("email", req).await
    }

    pub(super) async fn do_auth_submit_email_code(&self, code: &Secret) -> Result<(), CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        auth::AuthAction::EmailCode.check(self.step())?;
        let req = functions::check_authentication_email_code(
            enums::EmailAddressAuthentication::Code(types::EmailAddressAuthenticationCode {
                code: code.expose_str().to_owned(),
            }),
            id,
        );
        self.auth_request("email_code", req).await
    }

    pub(super) async fn do_auth_submit_password(&self, password: &Secret) -> Result<(), CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        auth::AuthAction::Password.check(self.step())?;
        let req = functions::check_authentication_password(password.expose_str().to_owned(), id);
        self.auth_request("password", req).await
    }

    pub(super) async fn do_auth_register(&self) -> Result<(), CoreError> {
        let kind = self.kind().ok_or_else(Self::not_started)?;
        let id = self.client_id().ok_or_else(Self::not_started)?;
        auth::prepare_register(self.step(), kind)?;
        let req = functions::register_user(
            auth::TEST_REGISTER_FIRST_NAME.to_owned(),
            auth::TEST_REGISTER_LAST_NAME.to_owned(),
            true,
            id,
        );
        self.auth_request("register", req).await
    }

    pub(super) async fn do_logout(&self) -> Result<(), CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        auth::prepare_logout(self.step())?;
        self.auth_request("logout", functions::log_out(id)).await
    }
}
