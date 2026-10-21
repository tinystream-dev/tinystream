// SPDX-License-Identifier: AGPL-3.0-or-later

use async_graphql::{Context, Object};

use super::auth::{SignedIn, sign_in};
use super::schema::Ctx;
use crate::auth::{self, resets};
use crate::error::{ApiError, ApiResult};
use crate::mail;

fn no_email() -> ApiError {
    ApiError::bad_request("this server can't send email")
}

#[derive(Default)]
pub struct EmailQuery;

#[Object]
impl EmailQuery {
    /// Whose password a reset link is for, while it still works.
    async fn password_reset(&self, ctx: &Context<'_>, token: String) -> ApiResult<Option<String>> {
        resets::available(&ctx.state().db, &token).await
    }
}

#[derive(Default)]
pub struct EmailMutation;

#[Object]
impl EmailMutation {
    /// Emails a reset link to the account `who` (a username or an email address) names. Says the
    /// same whether or not there's such an account, and sends in the background so the wait
    /// doesn't tell either.
    async fn request_password_reset(&self, ctx: &Context<'_>, who: String) -> ApiResult<bool> {
        let state = ctx.state().clone();
        let config = state.config.current();

        if !mail::available(&config.smtp) {
            return Err(no_email());
        }

        tokio::spawn(async move {
            let recipients = match resets::recipients(&state.db, &who).await {
                Ok(r) => r,
                Err(e) => return tracing::warn!("can't look up who to send a password reset: {}", e.message),
            };

            for r in recipients {
                let token = match resets::create(&state.db, r.id).await {
                    Ok(Some(token)) => token,
                    Ok(None) => continue,
                    Err(e) => return tracing::warn!("can't make a password reset: {}", e.message),
                };

                let body = format!(
                    "Someone, hopefully you, asked to reset the password of {} on tinystream.\n\n\
                     Pick a new one here:\n{}\n\n\
                     The link works for an hour. If it wasn't you, ignore this email and your password stays as it is.\n",
                    r.username,
                    config.smtp.link(&format!("reset#{token}")).unwrap_or_default(),
                );

                match mail::send(&config.smtp, &r.email, "Reset your tinystream password", body).await {
                    Ok(()) => tracing::info!("sent {} a password reset", r.username),
                    Err(e) => tracing::warn!("can't send {} a password reset: {e:#}", r.username),
                }
            }
        });

        Ok(true)
    }

    async fn reset_password(&self, ctx: &Context<'_>, token: String, password: String) -> ApiResult<SignedIn> {
        let state = ctx.state();

        let username = resets::available(&state.db, &token)
            .await?
            .ok_or_else(|| ApiError::bad_request("this link doesn't work anymore; ask for a new one"))?;

        auth::validate_credentials(&username, &password)?;
        let hash = auth::hash_password(password).await?;
        let id = resets::redeem(&state.db, &token, &hash).await?;
        tracing::info!("{username} reset their password");
        sign_in(ctx, id).await
    }

    /// The address password resets go to; `None` takes it away.
    async fn set_email(&self, ctx: &Context<'_>, email: Option<String>, password: String) -> ApiResult<Option<String>> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let email = email.map(|e| e.trim().to_string()).filter(|e| !e.is_empty());

        if email.is_some() && !mail::available(&state.config.current().smtp) {
            return Err(no_email());
        }

        if email.as_deref().is_some_and(|e| !mail::plausible(e)) {
            return Err(ApiError::bad_request("that isn't an email address"));
        }

        let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?")
            .bind(user.id)
            .fetch_one(&state.db)
            .await?;

        if !auth::verify_password(password, hash).await {
            return Err(ApiError::bad_request("your password isn't right"));
        }

        sqlx::query("UPDATE users SET email = ? WHERE id = ?").bind(&email).bind(user.id).execute(&state.db).await?;
        Ok(email)
    }

    async fn send_test_email(&self, ctx: &Context<'_>, to: String) -> ApiResult<bool> {
        let state = ctx.state();
        let admin = ctx.admin()?;
        let config = state.config.current();

        if !mail::available(&config.smtp) {
            return Err(no_email());
        }

        let body = format!(
            "{} sent this from tinystream's settings to check that email works. It does.\n\nLinks in emails go to {}\n",
            admin.username,
            config.smtp.link("").unwrap_or_default(),
        );

        mail::send(&config.smtp, &to, "tinystream can send email", body).await?;
        Ok(true)
    }
}
