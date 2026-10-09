// SPDX-License-Identifier: AGPL-3.0-or-later

use async_graphql::{Context, Object, SimpleObject};
use webauthn_rs::prelude::Uuid;

use super::auth::{SignedIn, sign_in};
use super::schema::Ctx;
use crate::auth::invites::Invite;
use crate::auth::{self, invites};
use crate::error::{ApiError, ApiResult};
use crate::events::Event;

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
    async fn create_invite(
        &self,
        ctx: &Context<'_>,
        label: String,
        max_uses: i64,
        expires_in_hours: i64,
    ) -> ApiResult<CreatedInvite> {
        let admin = ctx.admin()?;
        let (invite, token) = invites::create(&ctx.state().db, admin.id, &label, max_uses, expires_in_hours).await?;
        Ok(CreatedInvite { invite, link: format!("/invite#{token}") })
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
            return Err(ApiError::bad_request("this invite has expired, been revoked, or reached its use limit"));
        }
        let hash = auth::hash_password(password).await?;
        let id = invites::redeem(&state.db, &token, &username, &hash, &Uuid::new_v4().to_string()).await?;
        state.events.send(Event::UsersChanged);
        sign_in(ctx, id).await
    }
}
