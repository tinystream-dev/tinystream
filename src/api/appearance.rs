// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;

use async_graphql::{Context, Enum, InputObject, Object, SimpleObject};
use serde::{Deserialize, Serialize};

use super::schema::Ctx;
use crate::auth::User;
use crate::db::now;
use crate::error::{ApiError, ApiResult};
use crate::events::Event;
use crate::state::AppState;
use crate::theme::{self, Color, SEEDS, Scheme, TOKENS};

const DEFAULTS_KEY: &str = "appearance";
const FALLBACK: &str = "grey";

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SchemeMode {
    Single,
    /// One scheme for the system's light mode, one for its dark mode.
    #[default]
    System,
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ComponentStyle {
    #[default]
    Layered,
    Flat,
    Glass,
}

/// Which schemes to use, by id: a built-in's name or a saved scheme's number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SimpleObject, InputObject)]
#[graphql(input_name = "SchemeChoiceInput")]
#[serde(rename_all = "camelCase", default)]
pub struct SchemeChoice {
    pub mode: SchemeMode,
    pub single: String,
    pub light: String,
    pub dark: String,
}

impl Default for SchemeChoice {
    fn default() -> Self {
        Self { mode: SchemeMode::System, single: FALLBACK.into(), light: FALLBACK.into(), dark: FALLBACK.into() }
    }
}

impl SchemeChoice {
    fn ids(&self) -> [&str; 3] {
        [&self.single, &self.light, &self.dark]
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, SimpleObject, InputObject)]
#[graphql(name = "ServerAppearance", input_name = "ServerAppearanceInput")]
#[serde(rename_all = "camelCase", default)]
pub struct Defaults {
    pub colors: SchemeChoice,
    pub style: ComponentStyle,
}

/// What someone picked for themselves; anything left out follows the server.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, SimpleObject, InputObject)]
#[graphql(name = "AppearanceSettings", input_name = "AppearanceSettingsInput")]
#[serde(rename_all = "camelCase", default)]
pub struct Personal {
    pub colors: Option<SchemeChoice>,
    pub style: Option<ComponentStyle>,
    /// Let the scheme colour what sits over artwork and video, too.
    pub media_tint: bool,
}

/// How tinystream looks for whoever's asking.
#[derive(SimpleObject)]
pub struct Appearance {
    mode: SchemeMode,
    /// The scheme for the system's light mode; the only one in single mode.
    light: ColorScheme,
    dark: ColorScheme,
    style: ComponentStyle,
    media_tint: bool,
}

#[derive(SimpleObject, Clone)]
pub struct Token {
    name: String,
    value: String,
}

#[derive(InputObject)]
pub struct TokenInput {
    name: String,
    value: String,
}

#[derive(SimpleObject)]
#[graphql(name = "ContrastWarning")]
pub struct Warning {
    foreground: String,
    background: String,
    ratio: f64,
    minimum: f64,
}

#[derive(SimpleObject)]
pub struct Palette {
    seeds: Vec<Token>,
    overrides: Vec<Token>,
    /// Every colour, derived from the seeds with the overrides applied.
    tokens: Vec<Token>,
    warnings: Vec<Warning>,
}

impl Palette {
    fn of(scheme: &Scheme) -> Self {
        let token = |name: &str, c: &Color| Token { name: name.into(), value: c.to_string() };
        let tokens = scheme.tokens();

        Self {
            seeds: SEEDS.iter().zip(&scheme.seeds).map(|(n, c)| token(n, c)).collect(),
            overrides: scheme.overrides.iter().map(|(&i, c)| token(TOKENS[i], c)).collect(),
            tokens: TOKENS.iter().zip(&tokens).map(|(n, c)| token(n, c)).collect(),
            warnings: theme::contrast_warnings(&tokens)
                .into_iter()
                .map(|w| Warning {
                    foreground: w.foreground.into(),
                    background: w.background.into(),
                    ratio: (w.ratio * 100.0).round() / 100.0,
                    minimum: w.minimum,
                })
                .collect(),
        }
    }
}

#[derive(SimpleObject)]
pub struct SchemeOrigin {
    /// Gone when the parent was deleted or you can't see it any more.
    id: Option<String>,
    name: String,
}

#[derive(Debug, Clone)]
pub struct ColorScheme {
    id: String,
    name: String,
    scheme: Scheme,
    built_in: bool,
    published: bool,
    owner: Option<i64>,
    forked_from: Option<(String, String)>,
}

impl ColorScheme {
    fn builtin(b: theme::Builtin) -> Self {
        Self {
            id: b.id.into(),
            name: b.name.into(),
            scheme: b.scheme,
            built_in: true,
            published: true,
            owner: None,
            forked_from: None,
        }
    }

    fn editable_by(&self, user: &User) -> bool {
        !self.built_in && (if self.published { user.is_admin } else { self.owner == Some(user.id) })
    }
}

#[Object]
impl ColorScheme {
    async fn id(&self) -> &str {
        &self.id
    }

    async fn name(&self) -> &str {
        &self.name
    }

    async fn built_in(&self) -> bool {
        self.built_in
    }

    /// Everyone on this server can pick it.
    async fn published(&self) -> bool {
        self.published && !self.built_in
    }

    async fn editable(&self, ctx: &Context<'_>) -> bool {
        ctx.maybe_user().is_some_and(|u| self.editable_by(u))
    }

    async fn forked_from(&self, ctx: &Context<'_>) -> ApiResult<Option<SchemeOrigin>> {
        let Some((id, name)) = &self.forked_from else { return Ok(None) };
        let found = find(ctx.state(), ctx.maybe_user(), id).await?;
        Ok(Some(SchemeOrigin { id: found.map(|s| s.id), name: name.clone() }))
    }

    /// `ts1.<payload>`: the scheme itself. Two schemes with the same code are the same scheme.
    async fn code(&self) -> String {
        self.scheme.code()
    }

    /// The code with the name attached, for sharing.
    async fn share_code(&self) -> String {
        self.scheme.share_code(&self.name)
    }

    async fn palette(&self) -> Palette {
        Palette::of(&self.scheme)
    }
}

#[derive(SimpleObject)]
pub struct DecodedScheme {
    name: Option<String>,
    code: String,
    palette: Palette,
}

type Row = (i64, Option<i64>, String, String, bool, Option<String>, Option<String>);
const COLUMNS: &str = "id, owner_id, name, code, published, forked_from, forked_from_name";

fn from_row((id, owner, name, code, published, forked_from, forked_from_name): Row) -> Option<ColorScheme> {
    let (scheme, _) = Scheme::decode(&code).ok()?;

    Some(ColorScheme {
        id: id.to_string(),
        name,
        scheme,
        built_in: false,
        published,
        owner,
        forked_from: forked_from.map(|f| (f, forked_from_name.unwrap_or_default())),
    })
}

/// A scheme `user` may use: a built-in, a published one, or their own.
async fn find(state: &AppState, user: Option<&User>, id: &str) -> ApiResult<Option<ColorScheme>> {
    if let Some(b) = theme::find_builtin(id) {
        return Ok(Some(ColorScheme::builtin(b)));
    }

    let Ok(id) = id.parse::<i64>() else { return Ok(None) };

    let row: Option<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM schemes WHERE id = ? AND (published = 1 OR owner_id = ?)"
    )))
    .bind(id)
    .bind(user.map(|u| u.id))
    .fetch_optional(&state.db)
    .await?;

    Ok(row.and_then(from_row))
}

async fn get(state: &AppState, user: &User, id: &str) -> ApiResult<ColorScheme> {
    find(state, Some(user), id).await?.ok_or_else(|| ApiError::not_found("colour scheme"))
}

async fn editable(state: &AppState, user: &User, id: &str) -> ApiResult<ColorScheme> {
    let s = get(state, user, id).await?;

    if s.built_in {
        return Err(ApiError::bad_request(format!("{} is built in; fork it to make changes", s.name)));
    }

    if !s.editable_by(user) {
        return Err(ApiError::forbidden());
    }

    Ok(s)
}

pub async fn defaults(state: &AppState) -> ApiResult<Defaults> {
    let stored: Option<String> = sqlx::query_scalar("SELECT value FROM server_settings WHERE key = ?")
        .bind(DEFAULTS_KEY)
        .fetch_optional(&state.db)
        .await?;

    Ok(stored.and_then(|json| serde_json::from_str(&json).ok()).unwrap_or_default())
}

async fn personal(state: &AppState, user: &User) -> ApiResult<Personal> {
    let json: String =
        sqlx::query_scalar("SELECT appearance FROM users WHERE id = ?").bind(user.id).fetch_one(&state.db).await?;

    Ok(serde_json::from_str(&json).unwrap_or_default())
}

/// What `user` (or someone signed out) sees. Schemes that are gone fall back
/// to the server's choice, then to Grey.
async fn resolve(state: &AppState, user: Option<&User>) -> ApiResult<Appearance> {
    let defaults = defaults(state).await?;

    let personal = match user {
        Some(u) => personal(state, u).await?,
        None => Personal::default(),
    };

    let colors = personal.colors.clone().unwrap_or_else(|| defaults.colors.clone());

    let pick = async |id: &str, fallback: &str| -> ApiResult<ColorScheme> {
        for id in [id, fallback] {
            if let Some(s) = find(state, user, id).await? {
                return Ok(s);
            }
        }
        Ok(ColorScheme::builtin(theme::find_builtin(FALLBACK).expect("Grey is built in")))
    };

    let (light, dark) = match colors.mode {
        SchemeMode::Single => {
            let s = pick(&colors.single, &defaults.colors.single).await?;
            (s.clone(), s)
        },
        SchemeMode::System => {
            (pick(&colors.light, &defaults.colors.light).await?, pick(&colors.dark, &defaults.colors.dark).await?)
        },
    };

    Ok(Appearance {
        mode: colors.mode,
        light,
        dark,
        style: personal.style.unwrap_or(defaults.style),
        media_tint: personal.media_tint,
    })
}

/// The server's default look, as `<head>` tags with CSS custom properties, for pages outside the
/// app.
pub async fn public_head(state: &AppState) -> ApiResult<String> {
    let a = resolve(state, None).await?;

    let vars = |s: &ColorScheme| {
        let scheme = if s.scheme.dark() { "dark" } else { "light" };
        let vars: String = TOKENS.iter().zip(s.scheme.tokens()).map(|(n, c)| format!("--color-{n}:{c};")).collect();
        format!(":root{{color-scheme:{scheme};{vars}}}")
    };

    let canvas = |s: &ColorScheme| s.scheme.tokens()[0];

    Ok(match a.mode {
        SchemeMode::Single => {
            format!("<style>{}</style>\n<meta name=\"theme-color\" content=\"{}\">", vars(&a.light), canvas(&a.light))
        },
        SchemeMode::System => format!(
            "<style>{}@media (prefers-color-scheme: dark){{{}}}</style>\n\
             <meta name=\"theme-color\" content=\"{}\" media=\"(prefers-color-scheme: light)\">\n\
             <meta name=\"theme-color\" content=\"{}\" media=\"(prefers-color-scheme: dark)\">",
            vars(&a.light),
            vars(&a.dark),
            canvas(&a.light),
            canvas(&a.dark)
        ),
    })
}

fn parse_colors(input: Vec<TokenInput>, names: &[&str], what: &str) -> ApiResult<BTreeMap<usize, Color>> {
    let mut out = BTreeMap::new();

    for t in input {
        let i = names
            .iter()
            .position(|n| *n == t.name)
            .ok_or_else(|| ApiError::bad_request(format!("{:?} isn't a {what}", t.name)))?;

        let c = Color::parse(&t.value)
            .ok_or_else(|| ApiError::bad_request(format!("{:?} isn't a colour like #1a2b3c", t.value)))?;

        if out.insert(i, c).is_some() {
            return Err(ApiError::bad_request(format!("{} is set twice", t.name)));
        }
    }

    Ok(out)
}

#[derive(InputObject)]
pub struct SchemeInput {
    name: String,
    /// Every seed, opaque.
    seeds: Vec<TokenInput>,
    overrides: Vec<TokenInput>,
}

impl SchemeInput {
    fn parse(self) -> ApiResult<(String, Scheme)> {
        let name = theme::clamp_name(&self.name);

        if name.is_empty() {
            return Err(ApiError::bad_request("give the scheme a name"));
        }

        let seeds = parse_colors(self.seeds, &SEEDS, "seed")?;

        if seeds.len() != SEEDS.len() {
            return Err(ApiError::bad_request(format!("a scheme needs all {} seeds", SEEDS.len())));
        }

        if seeds.values().any(|c| !c.opaque()) {
            return Err(ApiError::bad_request("seeds can't be see-through"));
        }

        let seeds = std::array::from_fn(|i| seeds[&i]);
        let overrides = parse_colors(self.overrides, &TOKENS, "colour")?;
        Ok((name, Scheme { seeds, overrides }))
    }
}

async fn insert(
    state: &AppState,
    owner: &User,
    name: &str,
    scheme: &Scheme,
    forked_from: Option<&ColorScheme>,
) -> ApiResult<ColorScheme> {
    let t = now();

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO schemes (owner_id, name, code, forked_from, forked_from_name, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(owner.id)
    .bind(name)
    .bind(scheme.code())
    .bind(forked_from.map(|f| &f.id))
    .bind(forked_from.map(|f| &f.name))
    .bind(t)
    .bind(t)
    .fetch_one(&state.db)
    .await?;

    get(state, owner, &id.to_string()).await
}

/// Ids someone may choose: built-ins, published schemes, and (unless it's for everyone) their own.
async fn check_choice(state: &AppState, user: Option<&User>, choice: &SchemeChoice) -> ApiResult<()> {
    for id in choice.ids() {
        if find(state, user, id).await?.is_none() {
            return Err(ApiError::bad_request(format!("there's no colour scheme {id:?} to pick")));
        }
    }
    Ok(())
}

fn changed(state: &AppState, user_id: Option<i64>) {
    state.events.send(Event::AppearanceChanged { user_id });
}

#[derive(Default)]
pub struct AppearanceQuery;

#[Object]
impl AppearanceQuery {
    /// How tinystream should look right now. Works signed out too, with the server's defaults.
    async fn appearance(&self, ctx: &Context<'_>) -> ApiResult<Appearance> {
        resolve(ctx.state(), ctx.maybe_user()).await
    }

    async fn appearance_settings(&self, ctx: &Context<'_>) -> ApiResult<Personal> {
        personal(ctx.state(), ctx.user()?).await
    }

    async fn server_appearance(&self, ctx: &Context<'_>) -> ApiResult<Defaults> {
        ctx.user()?;
        defaults(ctx.state()).await
    }

    /// Built-ins, then published schemes, then your own.
    async fn color_schemes(&self, ctx: &Context<'_>) -> ApiResult<Vec<ColorScheme>> {
        let (state, user) = (ctx.state(), ctx.user()?);

        let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {COLUMNS} FROM schemes WHERE published = 1 OR owner_id = ?
             ORDER BY published DESC, name COLLATE NOCASE, id"
        )))
        .bind(user.id)
        .fetch_all(&state.db)
        .await?;

        Ok(theme::builtins()
            .into_iter()
            .map(ColorScheme::builtin)
            .chain(rows.into_iter().filter_map(from_row))
            .collect())
    }

    async fn color_scheme(&self, ctx: &Context<'_>, id: String) -> ApiResult<Option<ColorScheme>> {
        find(ctx.state(), Some(ctx.user()?), &id).await
    }

    /// What a code holds, to look at before importing it.
    async fn decode_scheme(&self, ctx: &Context<'_>, code: String) -> ApiResult<DecodedScheme> {
        ctx.user()?;
        let (scheme, name) = Scheme::decode(&code).map_err(|e| ApiError::bad_request(e.to_string()))?;
        Ok(DecodedScheme { name, code: scheme.code(), palette: Palette::of(&scheme) })
    }
}

#[derive(Default)]
pub struct AppearanceMutation;

#[Object]
impl AppearanceMutation {
    /// Creates a scheme, or changes one you may edit.
    async fn save_scheme(&self, ctx: &Context<'_>, id: Option<String>, input: SchemeInput) -> ApiResult<ColorScheme> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let (name, scheme) = input.parse()?;

        let Some(id) = id else {
            return insert(state, user, &name, &scheme, None).await;
        };

        let old = editable(state, user, &id).await?;

        sqlx::query("UPDATE schemes SET name = ?, code = ?, updated_at = ? WHERE id = ?")
            .bind(&name)
            .bind(scheme.code())
            .bind(now())
            .bind(&old.id)
            .execute(&state.db)
            .await?;

        changed(state, (!old.published).then_some(user.id));
        get(state, user, &id).await
    }

    /// An editable copy of any scheme you can see.
    async fn fork_scheme(&self, ctx: &Context<'_>, id: String) -> ApiResult<ColorScheme> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let parent = get(state, user, &id).await?;
        let name = theme::clamp_name(&format!("{} copy", parent.name));
        insert(state, user, &name, &parent.scheme, Some(&parent)).await
    }

    async fn import_scheme(&self, ctx: &Context<'_>, code: String, name: Option<String>) -> ApiResult<ColorScheme> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let (scheme, carried) = Scheme::decode(&code).map_err(|e| ApiError::bad_request(e.to_string()))?;

        let name = name
            .map(|n| theme::clamp_name(&n))
            .filter(|n| !n.is_empty())
            .or(carried)
            .unwrap_or_else(|| "Imported scheme".into());

        insert(state, user, &name, &scheme, None).await
    }

    async fn delete_scheme(&self, ctx: &Context<'_>, id: String) -> ApiResult<String> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let s = editable(state, user, &id).await?;
        sqlx::query("DELETE FROM schemes WHERE id = ?").bind(&s.id).execute(&state.db).await?;
        changed(state, (!s.published).then_some(user.id));
        Ok(id)
    }

    /// Shares one of your schemes with everyone here, or takes it back.
    async fn publish_scheme(&self, ctx: &Context<'_>, id: String, published: bool) -> ApiResult<ColorScheme> {
        let (state, admin) = (ctx.state(), ctx.admin()?);
        let s = editable(state, admin, &id).await?;

        sqlx::query("UPDATE schemes SET published = ?, owner_id = COALESCE(owner_id, ?), updated_at = ? WHERE id = ?")
            .bind(published)
            .bind(admin.id)
            .bind(now())
            .bind(&s.id)
            .execute(&state.db)
            .await?;

        tracing::info!(
            "{} {} the colour scheme {:?}",
            admin.username,
            if published { "published" } else { "unpublished" },
            s.name
        );

        changed(state, None);
        get(state, admin, &id).await
    }

    async fn set_appearance(&self, ctx: &Context<'_>, input: Personal) -> ApiResult<Appearance> {
        let (state, user) = (ctx.state(), ctx.user()?);

        if let Some(colors) = &input.colors {
            check_choice(state, Some(user), colors).await?;
        }

        sqlx::query("UPDATE users SET appearance = ? WHERE id = ?")
            .bind(serde_json::to_string(&input).map_err(anyhow::Error::from)?)
            .bind(user.id)
            .execute(&state.db)
            .await?;

        changed(state, Some(user.id));
        resolve(state, Some(user)).await
    }

    /// What everyone sees unless they pick something else, signed-out pages included.
    async fn set_server_appearance(&self, ctx: &Context<'_>, input: Defaults) -> ApiResult<Defaults> {
        let (state, admin) = (ctx.state(), ctx.admin()?);
        check_choice(state, None, &input.colors).await?;

        sqlx::query(
            "INSERT INTO server_settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(DEFAULTS_KEY)
        .bind(serde_json::to_string(&input).map_err(anyhow::Error::from)?)
        .execute(&state.db)
        .await?;

        tracing::info!("{} changed the server's appearance", admin.username);
        changed(state, None);
        Ok(input)
    }
}
