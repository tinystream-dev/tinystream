// SPDX-License-Identifier: AGPL-3.0-or-later

use async_graphql::{Context, Enum, Object, SimpleObject};

use super::schema::Ctx;
use crate::auth::{self, User};
use crate::db::now;
use crate::error::ApiResult;
use crate::events::Event;
use crate::notifications::{self, Notification};

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationKind {
    Aired,

    Ready,

    Invite,

    Request,
    RequestApproved,
    RequestDeclined,

    Clip,

    ClipReady,
    Other,
}

impl NotificationKind {
    fn parse(s: &str) -> Self {
        match s {
            "aired" => Self::Aired,
            "ready" => Self::Ready,
            "invite" => Self::Invite,
            "request" => Self::Request,
            "requestApproved" => Self::RequestApproved,
            "requestDeclined" => Self::RequestDeclined,
            "clip" => Self::Clip,
            "clipReady" => Self::ClipReady,
            _ => Self::Other,
        }
    }
}

#[Object]
impl Notification {
    async fn id(&self) -> i64 {
        self.id
    }

    async fn kind(&self) -> NotificationKind {
        NotificationKind::parse(&self.kind)
    }

    async fn priority(&self) -> bool {
        self.priority
    }

    async fn title(&self) -> &str {
        &self.title
    }

    async fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    async fn image(&self) -> Option<&str> {
        self.image.as_deref()
    }

    async fn link(&self) -> Option<&str> {
        self.link.as_deref()
    }

    async fn actor(&self, ctx: &Context<'_>) -> ApiResult<Option<User>> {
        match &self.actor {
            Some(a) => auth::load_user(ctx.state(), a.id).await,
            None => Ok(None),
        }
    }

    async fn created_at(&self) -> i64 {
        self.created_at
    }

    async fn expires_at(&self) -> Option<i64> {
        self.expires_at
    }

    async fn read_at(&self) -> Option<i64> {
        self.read_at
    }
}

#[derive(SimpleObject)]
pub struct Inbox {
    items: Vec<Notification>,
    unread: i64,
}

async fn inbox(ctx: &Context<'_>, limit: i64) -> ApiResult<Inbox> {
    let (items, unread) = notifications::list(ctx.state(), ctx.user()?.id, limit).await?;
    Ok(Inbox { items, unread })
}

#[derive(Default)]
pub struct NotificationQuery;

#[Object]
impl NotificationQuery {
    async fn notifications(&self, ctx: &Context<'_>, #[graphql(default = 60)] limit: i64) -> ApiResult<Inbox> {
        inbox(ctx, limit.clamp(1, 200)).await
    }
}

#[derive(Default)]
pub struct NotificationMutation;

#[Object]
impl NotificationMutation {
    async fn mark_notifications_read(&self, ctx: &Context<'_>, ids: Option<Vec<i64>>) -> ApiResult<Inbox> {
        let (state, user) = (ctx.state(), ctx.user()?);

        match ids {
            Some(ids) => {
                for id in ids {
                    sqlx::query(
                        "UPDATE notifications SET read_at = ? WHERE id = ? AND user_id = ? AND read_at IS NULL",
                    )
                    .bind(now())
                    .bind(id)
                    .bind(user.id)
                    .execute(&state.db)
                    .await?;
                }
            },
            None => {
                sqlx::query("UPDATE notifications SET read_at = ? WHERE user_id = ? AND read_at IS NULL")
                    .bind(now())
                    .bind(user.id)
                    .execute(&state.db)
                    .await?;
            },
        }

        state.events.send(Event::NotificationsChanged { user_id: user.id });
        inbox(ctx, 60).await
    }

    async fn delete_notifications(&self, ctx: &Context<'_>, id: Option<i64>) -> ApiResult<Inbox> {
        let (state, user) = (ctx.state(), ctx.user()?);

        match id {
            Some(id) => {
                sqlx::query("DELETE FROM notifications WHERE id = ? AND user_id = ?")
                    .bind(id)
                    .bind(user.id)
                    .execute(&state.db)
                    .await?;
            },
            None => {
                sqlx::query("DELETE FROM notifications WHERE user_id = ?").bind(user.id).execute(&state.db).await?;
            },
        }

        state.events.send(Event::NotificationsChanged { user_id: user.id });
        inbox(ctx, 60).await
    }
}
