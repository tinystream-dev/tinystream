// SPDX-License-Identifier: AGPL-3.0-or-later

use async_graphql::{Context, Object, SimpleObject};
use webauthn_rs::prelude::Uuid;

use super::auth::{SignedIn, sign_in};
use super::schema::Ctx;
use crate::auth::invites::Invite;
use crate::auth::{self, invites};
use crate::error::{ApiError, ApiResult};
use crate::events::Event;
use crate::mail;

fn lifetime(hours: i64) -> String {
    match (hours / 24, hours % 24) {
        (1, 0) => "a day".into(),
        (7, 0) => "a week".into(),
        (365, 0) => "a year".into(),
        (days, 0) => format!("{days} days"),
        (0, 1) => "an hour".into(),
        _ => format!("{hours} hours"),
    }
}

#[derive(SimpleObject)]
pub struct CreatedInvite {
    invite: Invite,
    link: String,
}

#[derive(SimpleObject)]
pub struct InvitePreview {
    expires_at: i64,
    remaining_uses: i64,
}

#[derive(Default)]
pub struct InviteQuery;

#[Object]
impl InviteQuery {
    async fn invites(&self, ctx: &Context<'_>) -> ApiResult<Vec<Invite>> {
        ctx.admin()?;
        invites::list(&ctx.state().db).await
    }

    async fn invite(&self, ctx: &Context<'_>, token: String) -> ApiResult<Option<InvitePreview>> {
        Ok(invites::available(&ctx.state().db, &token)
            .await?
            .map(|(expires_at, remaining_uses)| InvitePreview { expires_at, remaining_uses }))
    }
}

#[derive(Default)]
pub struct InviteMutation;

#[Object]
impl InviteMutation {
    /// With `email`, the link is also sent there; the address isn't kept.
    async fn create_invite(
        &self,
        ctx: &Context<'_>,
        label: String,
        max_uses: i64,
        expires_in_hours: i64,
        email: Option<String>,
    ) -> ApiResult<CreatedInvite> {
        let (state, admin) = (ctx.state(), ctx.admin()?);
        let config = state.config.current();
        let email = email.map(|e| e.trim().to_string()).filter(|e| !e.is_empty());

        if email.is_some() && !mail::available(&config.smtp) {
            return Err(ApiError::bad_request("this server can't send email"));
        }

        if email.as_deref().is_some_and(|e| !mail::plausible(e)) {
            return Err(ApiError::bad_request("that isn't an email address"));
        }

        let (invite, token) = invites::create(&state.db, admin.id, &label, max_uses, expires_in_hours).await?;
        let link = format!("invite#{token}");

        if let Some(to) = &email {
            let body = format!(
                "{} invited you to tinystream.\n\nMake your account here:\n{}\n\nThe link works for {}.\n",
                admin.username,
                config.smtp.link(&link).unwrap_or_default(),
                lifetime(expires_in_hours),
            );

            if let Err(e) =
                mail::send(&config.smtp, to, &format!("{} invited you to tinystream", admin.username), body).await
            {
                invites::delete(&state.db, invite.id).await?;
                return Err(e.into());
            }

            tracing::info!("{} emailed an invite ({})", admin.username, invite.label);
        }

        Ok(CreatedInvite { invite, link: format!("/{link}") })
    }

    async fn revoke_invite(&self, ctx: &Context<'_>, id: i64) -> ApiResult<bool> {
        ctx.admin()?;
        invites::revoke(&ctx.state().db, id).await
    }

    async fn accept_invite(
        &self,
        ctx: &Context<'_>,
        token: String,
        username: String,
        password: String,
    ) -> ApiResult<SignedIn> {
        if ctx.maybe_user().is_some() {
            return Err(ApiError::bad_request("sign out before accepting an invite"));
        }
        let state = ctx.state();
        auth::validate_credentials(&username, &password)?;
        if invites::available(&state.db, &token).await?.is_none() {
            return Err(ApiError::bad_request("this invite doesn't work anymore"));
        }
        let hash = auth::hash_password(password).await?;
        let id = invites::redeem(&state.db, &token, &username, &hash, &Uuid::new_v4().to_string()).await?;
        state.events.send(Event::UsersChanged);
        sign_in(ctx, id).await
    }
}
