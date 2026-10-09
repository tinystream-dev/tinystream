/* eslint-disable */
/** Internal type. DO NOT USE DIRECTLY. */
type Exact<T extends { [key: string]: unknown }> = { [K in keyof T]: T[K] };
/** Internal type. DO NOT USE DIRECTLY. */
export type Incremental<T> = T | { [P in keyof T]?: P extends ' $fragmentName' | '__typename' ? T[P] : never };
import type { DocumentTypeDecoration } from '@graphql-typed-document-node/core';
export type AlbumSort =
  | 'ARTIST'
  | 'FREQUENT'
  | 'NAME'
  | 'NEWEST'
  | 'RANDOM'
  | 'RATED'
  | 'RECENT'
  | 'STARRED'
  | 'YEAR';

/** What someone picked for themselves; anything left out follows the server. */
export type AppearanceSettingsInput = {
  colors?: SchemeChoiceInput | null | undefined;
  /** Let the scheme colour what sits over artwork and video, too. */
  mediaTint: boolean;
  style?: ComponentStyle | null | undefined;
};

export type AutomationConfigInput = {
  defaultMonitor: Monitor;
  renameSuggestions: boolean;
  retry: Array<RetryStepInput>;
  rssInterval: string;
};

export type ChangedList =
  | 'APPEARANCE'
  | 'DOWNLOADS'
  | 'NOTIFICATIONS'
  | 'PLAYLISTS'
  | 'RENAME_SUGGESTIONS'
  | 'REQUESTS'
  | 'USERS';

export type ClipPatch = {
  name?: string | null | undefined;
  public?: boolean | null | undefined;
  recipe?: RecipeInput | null | undefined;
};

export type ClipScope =
  | 'MINE'
  | 'RECEIVED'
  | 'RENDERING'
  | 'SENT';

export type ClipState =
  | 'EVICTED'
  | 'FAILED'
  | 'QUEUED'
  | 'READY'
  | 'RENDERING';

export type ClipsConfigInput = {
  concurrency: number;
  defaultFont?: string | null | undefined;
  enabled: boolean;
  fontsDir?: string | null | undefined;
  maxStorage: number;
  path?: string | null | undefined;
  publicLinks: boolean;
};

export type ComponentStyle =
  | 'FLAT'
  | 'GLASS'
  | 'LAYERED';

export type Confidence =
  | 'HIGH'
  | 'LOW';

export type ConfigPatch = {
  automation?: AutomationConfigInput | null | undefined;
  clips?: ClipsConfigInput | null | undefined;
  downloads?: DownloadsConfigInput | null | undefined;
  log?: LogConfigInput | null | undefined;
  metadata?: MetadataConfigInput | null | undefined;
  music?: MusicConfigInput | null | undefined;
  network?: NetworkConfigInput | null | undefined;
  requests?: RequestsConfigInput | null | undefined;
  scan?: ScanConfigInput | null | undefined;
  signIn?: SignInConfigInput | null | undefined;
  transcode?: TranscodeConfigInput | null | undefined;
};

export type DownloadState =
  | 'DONE'
  | 'DOWNLOADING'
  | 'FAILED'
  | 'PAUSED'
  | 'REMOVED'
  | 'SEEDING';

export type DownloadsConfigInput = {
  bindInterface?: string | null | undefined;
  dht: boolean;
  downloadLimit: number;
  import: ImportMode;
  maxActive: number;
  path?: string | null | undefined;
  port: number;
  proxy?: string | null | undefined;
  seeding: SeedingInput;
  slowDownloadLimit: number;
  slowFrom?: string | null | undefined;
  slowTo?: string | null | undefined;
  slowUploadLimit: number;
  uploadLimit: number;
  upnp: boolean;
};

export type EpisodeNumberInput = {
  episode: number;
  season: number;
};

export type EpisodeState =
  | 'DONE'
  | 'GRABBED'
  | 'IDLE'
  | 'MISSING'
  | 'SKIPPED'
  | 'WANTED';

export type Hardware =
  | 'AUTO'
  | 'SOFTWARE'
  | 'VAAPI';

export type ImportMode =
  | 'AUTO'
  | 'COPY'
  | 'HARDLINK'
  | 'MOVE';

export type ImportState =
  | 'DONE'
  | 'FAILED'
  | 'PENDING'
  | 'SKIPPED';

export type LibraryInput = {
  downloadPath?: string | null | undefined;
  kind?: LibraryKind;
  managed?: boolean;
  metadataProvider?: Provider | null | undefined;
  name: string;
  path: string;
  profile?: string | null | undefined;
};

export type LibraryKind =
  | 'MUSIC'
  | 'VIDEO';

export type LogConfigInput = {
  level: string;
};

export type LyricsSource =
  | 'EMBEDDED'
  | 'FILE'
  | 'ONLINE';

export type MatchState =
  | 'MANUAL'
  | 'MATCHED'
  | 'PENDING'
  | 'UNMATCHED';

export type MediaCategory =
  | 'EPISODES'
  | 'MOVIES'
  | 'OTHER'
  | 'SPECIALS';

export type MetadataConfigInput = {
  language: string;
  tmdbApiKey?: string | null | undefined;
};

export type MetadataStatus =
  | 'FAILED'
  | 'FETCHING'
  | 'UPDATED';

export type Monitor =
  | 'FUTURE'
  | 'MISSING'
  | 'NONE';

export type MusicConfigInput = {
  analyzeLoudness: boolean;
  lyricsUrl: string;
  onlineLyrics: boolean;
};

export type MusicKind =
  | 'ALBUM'
  | 'ARTIST'
  | 'TRACK';

export type NetworkConfigInput = {
  cors: Array<string>;
  host: string;
  port: number;
};

export type NewClip = {
  name?: string;
  public?: boolean;
  recipe: RecipeInput;
  recipients?: Array<number>;
  room?: string | null | undefined;
  videoId: number;
};

export type NewListenRoom = {
  current?: number;
  paused?: boolean;
  position?: number;
  public?: boolean;
  tracks: Array<number>;
};

export type NewRequest = {
  library: string;
  name: string;
  note?: string | null | undefined;
  overview?: string | null | undefined;
  poster?: string | null | undefined;
  providerId: string;
  year?: number | null | undefined;
};

export type NewRoom = {
  paused?: boolean;
  position?: number;
  public?: boolean;
  tracks?: TracksInput;
  videoId: number;
};

export type NewScreenshot = {
  at: number;
  room?: string | null | undefined;
  subtitles?: string | null | undefined;
  videoId: number;
};

export type NewSeries = {
  library: string;
  monitor?: Monitor | null | undefined;
  name: string;
  overview?: string | null | undefined;
  poster?: string | null | undefined;
  profile?: string | null | undefined;
  provider: Provider;
  providerId: string;
  year?: number | null | undefined;
};

export type NewUser = {
  isAdmin?: boolean;
  password: string;
  permissions?: PermissionOverridesInput;
  username: string;
};

export type NotificationKind =
  | 'AIRED'
  | 'CLIP'
  | 'CLIP_READY'
  | 'INVITE'
  | 'OTHER'
  | 'READY'
  | 'REQUEST'
  | 'REQUEST_APPROVED'
  | 'REQUEST_DECLINED';

export type Numbering =
  | 'ABSOLUTE'
  | 'AUTO'
  | 'SEASONAL';

export type PermissionOverridesInput = {
  allLibraries?: boolean | null | undefined;
  autoApprove?: boolean | null | undefined;
  clip?: boolean | null | undefined;
  clipLimit?: number | null | undefined;
  clipLinks?: boolean | null | undefined;
  clipMaxLength?: number | null | undefined;
  clipStorage?: number | null | undefined;
  downloads?: boolean | null | undefined;
  editMetadata?: boolean | null | undefined;
  libraries?: Array<string> | null | undefined;
  manageRequests?: boolean | null | undefined;
  manageShows?: boolean | null | undefined;
  request?: boolean | null | undefined;
  requestLimit?: number | null | undefined;
  shareLinks?: boolean | null | undefined;
  watchTogether?: boolean | null | undefined;
};

export type PermissionsInput = {
  allLibraries: boolean;
  autoApprove: boolean;
  clip: boolean;
  clipLimit: number;
  clipLinks: boolean;
  clipMaxLength: number;
  clipStorage: number;
  downloads: boolean;
  editMetadata: boolean;
  libraries: Array<string>;
  manageRequests: boolean;
  manageShows: boolean;
  request: boolean;
  requestLimit: number;
  shareLinks: boolean;
  watchTogether: boolean;
};

export type PlaylistInput = {
  comment?: string | null | undefined;
  name?: string | null | undefined;
  public?: boolean | null | undefined;
  /** Replaces what's in it. */
  tracks?: Array<number> | null | undefined;
};

export type ProfileInput = {
  batches: boolean;
  codecs: Array<string>;
  groups: Array<string>;
  maxSize?: number | null | undefined;
  minSeeders: number;
  minSize?: number | null | undefined;
  name: string;
  preferDualAudio: boolean;
  reject: Array<string>;
  require: Array<string>;
  resolutions: Array<string>;
};

export type Provider =
  | 'ANILIST'
  | 'TMDB';

export type QueueInput = {
  current: number;
  position: number;
  repeat?: Repeat;
  shuffled?: boolean;
  tracks: Array<number>;
};

export type RecipeInput = {
  audio?: number | null | undefined;
  end: number;
  halfRate?: boolean;
  height: number;
  start: number;
  subtitles?: string | null | undefined;
};

export type ReleaseInput = {
  infoHash?: string | null | undefined;
  leechers?: number | null | undefined;
  link: string;
  page?: string | null | undefined;
  published?: number | null | undefined;
  seeders?: number | null | undefined;
  size?: number | null | undefined;
  source: string;
  title: string;
};

export type Repeat =
  | 'ALL'
  | 'OFF'
  | 'ONE';

export type RequestState =
  | 'APPROVED'
  | 'DECLINED'
  | 'PENDING';

export type RequestsConfigInput = {
  monitor: Monitor;
};

export type RetryStepInput = {
  every: string;
  until: string;
};

export type ScanConfigInput = {
  interval?: string | null | undefined;
  watch: boolean;
};

/** Which schemes to use, by id: a built-in's name or a saved scheme's number. */
export type SchemeChoiceInput = {
  dark: string;
  light: string;
  mode: SchemeMode;
  single: string;
};

export type SchemeInput = {
  name: string;
  overrides: Array<TokenInput>;
  /** Every seed, opaque. */
  seeds: Array<TokenInput>;
};

export type SchemeMode =
  | 'SINGLE'
  /** One scheme for the system's light mode, one for its dark mode. */
  | 'SYSTEM';

export type SeedAction =
  | 'PAUSE'
  | 'REMOVE';

export type SeedingInput = {
  idle?: string | null | undefined;
  ratio?: number | null | undefined;
  then: SeedAction;
  time?: string | null | undefined;
};

export type SeriesPatch = {
  aliases?: Array<string> | null | undefined;
  groups?: Array<string> | null | undefined;
  monitor?: Monitor | null | undefined;
  naming?: string | null | undefined;
  numbering?: Numbering | null | undefined;
  profile?: string | null | undefined;
  seeding?: SeedingInput | null | undefined;
  sources?: Array<string> | null | undefined;
};

export type ServerAppearanceInput = {
  colors: SchemeChoiceInput;
  style: ComponentStyle;
};

export type SignInConfigInput = {
  style: SignInStyle;
};

export type SignInStyle =
  | 'PROFILES'
  | 'USERNAME';

export type SourceInput = {
  apiKey?: string | null | undefined;
  categories: Array<number>;
  downloadPath?: string | null | undefined;
  enabled?: boolean;
  feed?: string | null | undefined;
  kind: SourceKind;
  name: string;
  seeding?: SeedingInput | null | undefined;
  url: string;
};

export type SourceKind =
  | 'RSS'
  | 'TORZNAB';

export type SourceStatus =
  | 'CHANGED'
  | 'GONE'
  | 'OK';

export type TitleArtwork =
  | 'BACKDROP'
  | 'POSTER';

export type TitleKind =
  | 'MOVIE'
  | 'SHOW';

export type TokenInput = {
  name: string;
  value: string;
};

export type TorrentStage =
  | 'CHECKING'
  | 'DOWNLOADING'
  | 'METADATA'
  | 'SEEDING';

export type TracksInput = {
  audio?: number | null | undefined;
  audioLanguage?: string | null | undefined;
  subtitle?: string | null | undefined;
  subtitleLanguage?: string | null | undefined;
};

export type TranscodeConfigInput = {
  hardware: Hardware;
  vaapiDevice: string;
};

export type UserPatch = {
  isAdmin?: boolean | null | undefined;
  password?: string | null | undefined;
  permissions?: PermissionOverridesInput | null | undefined;
  username?: string | null | undefined;
};

export type ColorSchemesQueryVariables = Exact<{ [key: string]: never; }>;


export type ColorSchemesQuery = { colorSchemes: Array<{ id: string, name: string, builtIn: boolean, published: boolean, editable: boolean, code: string, shareCode: string, forkedFrom: { id: string | null, name: string } | null, palette: { seeds: Array<{ name: string, value: string }>, overrides: Array<{ name: string, value: string }>, tokens: Array<{ name: string, value: string }>, warnings: Array<{ foreground: string, background: string, ratio: number, minimum: number }> } }> };

export type AppearanceSettingsQueryVariables = Exact<{ [key: string]: never; }>;


export type AppearanceSettingsQuery = { appearanceSettings: { style: ComponentStyle | null, mediaTint: boolean, colors: { mode: SchemeMode, single: string, light: string, dark: string } | null } };

export type ServerAppearanceQueryVariables = Exact<{ [key: string]: never; }>;


export type ServerAppearanceQuery = { serverAppearance: { style: ComponentStyle, colors: { mode: SchemeMode, single: string, light: string, dark: string } } };

export type SetAppearanceMutationVariables = Exact<{
  input: AppearanceSettingsInput;
}>;


export type SetAppearanceMutation = { setAppearance: { mode: SchemeMode } };

export type SetServerAppearanceMutationVariables = Exact<{
  input: ServerAppearanceInput;
}>;


export type SetServerAppearanceMutation = { setServerAppearance: { style: ComponentStyle } };

export type SaveSchemeMutationVariables = Exact<{
  id?: string | null | undefined;
  input: SchemeInput;
}>;


export type SaveSchemeMutation = { saveScheme: { id: string, name: string, builtIn: boolean, published: boolean, editable: boolean, code: string, shareCode: string, forkedFrom: { id: string | null, name: string } | null, palette: { seeds: Array<{ name: string, value: string }>, overrides: Array<{ name: string, value: string }>, tokens: Array<{ name: string, value: string }>, warnings: Array<{ foreground: string, background: string, ratio: number, minimum: number }> } } };

export type ForkSchemeMutationVariables = Exact<{
  id: string;
}>;


export type ForkSchemeMutation = { forkScheme: { id: string, name: string, builtIn: boolean, published: boolean, editable: boolean, code: string, shareCode: string, forkedFrom: { id: string | null, name: string } | null, palette: { seeds: Array<{ name: string, value: string }>, overrides: Array<{ name: string, value: string }>, tokens: Array<{ name: string, value: string }>, warnings: Array<{ foreground: string, background: string, ratio: number, minimum: number }> } } };

export type ImportSchemeMutationVariables = Exact<{
  code: string;
  name?: string | null | undefined;
}>;


export type ImportSchemeMutation = { importScheme: { id: string, name: string, builtIn: boolean, published: boolean, editable: boolean, code: string, shareCode: string, forkedFrom: { id: string | null, name: string } | null, palette: { seeds: Array<{ name: string, value: string }>, overrides: Array<{ name: string, value: string }>, tokens: Array<{ name: string, value: string }>, warnings: Array<{ foreground: string, background: string, ratio: number, minimum: number }> } } };

export type DeleteSchemeMutationVariables = Exact<{
  id: string;
}>;


export type DeleteSchemeMutation = { deleteScheme: string };

export type PublishSchemeMutationVariables = Exact<{
  id: string;
  published: boolean;
}>;


export type PublishSchemeMutation = { publishScheme: { id: string } };

export type DecodeSchemeQueryVariables = Exact<{
  code: string;
}>;


export type DecodeSchemeQuery = { decodeScheme: { name: string | null, code: string, palette: { tokens: Array<{ name: string, value: string }>, warnings: Array<{ foreground: string, background: string, ratio: number, minimum: number }> } } };

export type SetTitleArtworkMutationVariables = Exact<{
  id: number;
  kind: TitleArtwork;
  image?: Blob | null | undefined;
}>;


export type SetTitleArtworkMutation = { setTitleArtwork: { id: number } };

export type SetVideoArtworkMutationVariables = Exact<{
  videoId: number;
  image?: Blob | null | undefined;
}>;


export type SetVideoArtworkMutation = { setVideoArtwork: { id: number } };

export type SetAvatarMutationVariables = Exact<{
  image: Blob;
  userId?: number | null | undefined;
}>;


export type SetAvatarMutation = { setAvatar: { id: number, avatar: string | null } };

export type RemoveAvatarMutationVariables = Exact<{
  userId?: number | null | undefined;
}>;


export type RemoveAvatarMutation = { removeAvatar: { id: number, avatar: string | null } };

export type ClipStorageQueryVariables = Exact<{ [key: string]: never; }>;


export type ClipStorageQuery = { clipStorage: { bytes: number, dir: string | null, usage: Array<{ bytes: number, rendered: number, clips: number, storage: number, limit: number, user: { id: number, username: string, avatar: string | null } }>, publicClips: Array<{ id: number, screenshot: boolean, name: string, mine: boolean, canManage: boolean, start: number, end: number, audio: number | null, subtitles: string | null, state: ClipState, progress: number | null, error: string | null, bytes: number | null, width: number | null, height: number | null, fps: number | null, renderedAt: number | null, createdAt: number, sharedAt: number | null, public: boolean, link: string | null, linkLive: boolean, file: string, poster: string | null, owner: { id: number, username: string, avatar: string | null }, source: { name: string, kind: TitleKind, label: string | null, year: number | null, status: SourceStatus, video: { id: number } | null, title: { id: number } | null }, quality: { height: number, halfRate: boolean }, recipients: Array<{ sharedAt: number, hidden: boolean, user: { id: number, username: string, avatar: string | null } }> }> } };

export type SaveClipsConfigMutationVariables = Exact<{
  clips: ClipsConfigInput;
}>;


export type SaveClipsConfigMutation = { updateSettings: { clips: { enabled: boolean } } };

export type DropClipRendersMutationVariables = Exact<{ [key: string]: never; }>;


export type DropClipRendersMutation = { dropClipRenders: number };

export type UnpublishClipMutationVariables = Exact<{
  id: number;
}>;


export type UnpublishClipMutation = { updateClip: { id: number } };

export type AdminDeleteClipMutationVariables = Exact<{
  id: number;
}>;


export type AdminDeleteClipMutation = { deleteClip: number };

export type AddSeriesMutationVariables = Exact<{
  input: NewSeries;
}>;


export type AddSeriesMutation = { addSeries: { id: number } };

export type AiredEpisodesQueryVariables = Exact<{
  provider: Provider;
  id: string;
}>;


export type AiredEpisodesQuery = { airedEpisodes: number };

export type CreateRequestMutationVariables = Exact<{
  input: NewRequest;
}>;


export type CreateRequestMutation = { createRequest: { id: number } };

export type SaveSectionMutationVariables = Exact<{
  patch: ConfigPatch;
}>;


export type SaveSectionMutation = { updateSettings: { raw: string } };

export type SettingsEngineQueryVariables = Exact<{ [key: string]: never; }>;


export type SettingsEngineQuery = { downloadEngine: { downloadPath: string, killSwitch: string | null } };

export type AddSourceMutationVariables = Exact<{
  input: SourceInput;
}>;


export type AddSourceMutation = { addSource: { raw: string } };

export type UpdateSourceMutationVariables = Exact<{
  name: string;
  input: SourceInput;
}>;


export type UpdateSourceMutation = { updateSource: { raw: string } };

export type RemoveSourceMutationVariables = Exact<{
  name: string;
}>;


export type RemoveSourceMutation = { removeSource: { raw: string } };

export type DetectSourceQueryVariables = Exact<{
  url: string;
  apiKey?: string | null | undefined;
}>;


export type DetectSourceQuery = { detectSource: { kind: SourceKind, url: string, feed: string | null, name: string | null, searchable: boolean, sample: Array<{ link: string, title: string, size: number | null, seeders: number | null, published: number | null }> } };

export type AddProfileMutationVariables = Exact<{
  input: ProfileInput;
}>;


export type AddProfileMutation = { addProfile: { raw: string } };

export type UpdateProfileMutationVariables = Exact<{
  name: string;
  input: ProfileInput;
}>;


export type UpdateProfileMutation = { updateProfile: { raw: string } };

export type RemoveProfileMutationVariables = Exact<{
  name: string;
}>;


export type RemoveProfileMutation = { removeProfile: { raw: string } };

export type RenameSuggestionsQueryVariables = Exact<{ [key: string]: never; }>;


export type RenameSuggestionsQuery = { renameSuggestions: Array<{ id: number, library: string, managed: boolean, root: string | null, src: string, dst: string, reason: string, confidence: Confidence }> };

export type FileHistoryQueryVariables = Exact<{ [key: string]: never; }>;


export type FileHistoryQuery = { fileHistory: Array<{ batch: string, label: string, at: number, count: number, undone: boolean, operations: Array<{ kind: string, src: string | null, dst: string }> }> };

export type RefreshRenameSuggestionsMutationVariables = Exact<{ [key: string]: never; }>;


export type RefreshRenameSuggestionsMutation = { refreshRenameSuggestions: boolean };

export type ApplyRenamesMutationVariables = Exact<{
  ids: Array<number> | number;
}>;


export type ApplyRenamesMutation = { applyRenames: { renamed: number, problems: Array<string> } };

export type DismissRenamesMutationVariables = Exact<{
  ids: Array<number> | number;
}>;


export type DismissRenamesMutation = { dismissRenames: boolean };

export type UndoFileChangesMutationVariables = Exact<{
  batch: string;
}>;


export type UndoFileChangesMutation = { undoFileChanges: { undone: number, problems: Array<string> } };

export type InvitesQueryVariables = Exact<{ [key: string]: never; }>;


export type InvitesQuery = { invites: Array<{ id: number, label: string, createdAt: number, expiresAt: number, maxUses: number, uses: number, revoked: boolean }> };

export type CreateInviteMutationVariables = Exact<{
  label: string;
  maxUses: number;
  expiresInHours: number;
}>;


export type CreateInviteMutation = { createInvite: { link: string } };

export type RevokeInviteMutationVariables = Exact<{
  id: number;
}>;


export type RevokeInviteMutation = { revokeInvite: boolean };

export type SignInProfilesQueryVariables = Exact<{ [key: string]: never; }>;


export type SignInProfilesQuery = { signInProfiles: Array<{ key: string, avatar: string | null, passkey: boolean }> };

export type SetupMutationVariables = Exact<{
  username: string;
  password: string;
}>;


export type SetupMutation = { setup: { user: { id: number } } };

export type SignInMutationVariables = Exact<{
  username?: string | null | undefined;
  profile?: string | null | undefined;
  password: string;
}>;


export type SignInMutation = { signIn: { user: { id: number } } };

export type StartPasskeySignInMutationVariables = Exact<{
  username?: string | null | undefined;
  profile?: string | null | undefined;
}>;


export type StartPasskeySignInMutation = { startPasskeySignIn: { challenge: string, options: unknown } };

export type FinishPasskeySignInMutationVariables = Exact<{
  challenge: string;
  credential: unknown;
}>;


export type FinishPasskeySignInMutation = { finishPasskeySignIn: { user: { id: number } } };

export type SaveMusicConfigMutationVariables = Exact<{
  music: MusicConfigInput;
}>;


export type SaveMusicConfigMutation = { updateSettings: { music: { onlineLyrics: boolean } } };

export type AppPasswordsQueryVariables = Exact<{ [key: string]: never; }>;


export type AppPasswordsQuery = { appPasswords: Array<{ id: number, name: string, createdAt: number, lastUsed: number | null, client: string | null }> };

export type CreateAppPasswordMutationVariables = Exact<{
  name: string;
}>;


export type CreateAppPasswordMutation = { createAppPassword: { secret: string, password: { id: number, name: string } } };

export type DeleteAppPasswordMutationVariables = Exact<{
  id: number;
}>;


export type DeleteAppPasswordMutation = { deleteAppPassword: boolean };

export type UsersQueryVariables = Exact<{ [key: string]: never; }>;


export type UsersQuery = { users: Array<{ createdAt: number, lastSeen: number | null, isAdmin: boolean, id: number, username: string, avatar: string | null, overrides: { allLibraries: boolean | null, libraries: Array<string> | null, request: boolean | null, autoApprove: boolean | null, requestLimit: number | null, manageRequests: boolean | null, manageShows: boolean | null, downloads: boolean | null, editMetadata: boolean | null, watchTogether: boolean | null, shareLinks: boolean | null, clip: boolean | null, clipMaxLength: number | null, clipLimit: number | null, clipStorage: number | null, clipLinks: boolean | null }, permissions: { allLibraries: boolean, libraries: Array<string>, request: boolean, autoApprove: boolean, requestLimit: number, manageRequests: boolean, manageShows: boolean, downloads: boolean, editMetadata: boolean, watchTogether: boolean, shareLinks: boolean, clip: boolean, clipMaxLength: number, clipLimit: number, clipStorage: number, clipLinks: boolean } }> };

export type PermissionDefaultsQueryVariables = Exact<{ [key: string]: never; }>;


export type PermissionDefaultsQuery = { permissionDefaults: { allLibraries: boolean, libraries: Array<string>, request: boolean, autoApprove: boolean, requestLimit: number, manageRequests: boolean, manageShows: boolean, downloads: boolean, editMetadata: boolean, watchTogether: boolean, shareLinks: boolean, clip: boolean, clipMaxLength: number, clipLimit: number, clipStorage: number, clipLinks: boolean } };

export type SetPermissionDefaultsMutationVariables = Exact<{
  permissions: PermissionsInput;
}>;


export type SetPermissionDefaultsMutation = { setPermissionDefaults: { allLibraries: boolean, libraries: Array<string>, request: boolean, autoApprove: boolean, requestLimit: number, manageRequests: boolean, manageShows: boolean, downloads: boolean, editMetadata: boolean, watchTogether: boolean, shareLinks: boolean, clip: boolean, clipMaxLength: number, clipLimit: number, clipStorage: number, clipLinks: boolean } };

export type UpdateUserMutationVariables = Exact<{
  id: number;
  input: UserPatch;
}>;


export type UpdateUserMutation = { updateUser: { id: number } };

export type DeleteUserMutationVariables = Exact<{
  id: number;
}>;


export type DeleteUserMutation = { deleteUser: number };

export type CreateUserMutationVariables = Exact<{
  input: NewUser;
}>;


export type CreateUserMutation = { createUser: { id: number } };

export type TitleSeriesQueryVariables = Exact<{
  id: number;
}>;


export type TitleSeriesQuery = { title: { series: { id: number, monitor: Monitor, status: string | null, library: string, managed: boolean, path: string, name: string, year: number | null, poster: string | null, overview: string | null, provider: Provider | null, providerId: string | null, profile: string | null, effectiveProfile: string, sources: Array<string>, groups: Array<string>, aliases: Array<string>, knownAs: Array<string>, numbering: Numbering, naming: string | null, scheduleAt: number | null, addedAt: number, next: { season: number, episode: number, absolute: number | null, name: string | null, airAt: number | null, aired: boolean, state: EpisodeState, attempts: number, searchedAt: number | null, nextSearch: number | null, downloadId: number | null, video: { id: number } | null } | null, title: { id: number } | null, style: { file: string, folder: string, agreement: number | null, samples: number }, seeding: { ratio: number | null, time: string | null, idle: string | null, then: SeedAction } | null, counts: { have: number, wanted: number, missing: number, grabbed: number, total: number, upcoming: number, skipped: number }, episodes: Array<{ season: number, episode: number, absolute: number | null, name: string | null, airAt: number | null, aired: boolean, state: EpisodeState, attempts: number, searchedAt: number | null, nextSearch: number | null, downloadId: number | null, video: { id: number } | null }> } | null } | null };

export type TitleScheduleQueryVariables = Exact<{
  id: number;
}>;


export type TitleScheduleQuery = { title: { series: { id: number, monitor: Monitor, status: string | null, next: { season: number, episode: number, absolute: number | null, name: string | null, airAt: number | null, aired: boolean, state: EpisodeState, attempts: number, searchedAt: number | null, nextSearch: number | null, downloadId: number | null, video: { id: number } | null } | null } | null } | null };

export type ManageTitleMutationVariables = Exact<{
  titleId: number;
}>;


export type ManageTitleMutation = { manageTitle: { id: number } };

export type UpdateSeriesMutationVariables = Exact<{
  id: number;
  patch: SeriesPatch;
}>;


export type UpdateSeriesMutation = { updateSeries: { id: number, monitor: Monitor, status: string | null, library: string, managed: boolean, path: string, name: string, year: number | null, poster: string | null, overview: string | null, provider: Provider | null, providerId: string | null, profile: string | null, effectiveProfile: string, sources: Array<string>, groups: Array<string>, aliases: Array<string>, knownAs: Array<string>, numbering: Numbering, naming: string | null, scheduleAt: number | null, addedAt: number, next: { season: number, episode: number, absolute: number | null, name: string | null, airAt: number | null, aired: boolean, state: EpisodeState, attempts: number, searchedAt: number | null, nextSearch: number | null, downloadId: number | null, video: { id: number } | null } | null, title: { id: number } | null, style: { file: string, folder: string, agreement: number | null, samples: number }, seeding: { ratio: number | null, time: string | null, idle: string | null, then: SeedAction } | null, counts: { have: number, wanted: number, missing: number, grabbed: number, total: number, upcoming: number, skipped: number }, episodes: Array<{ season: number, episode: number, absolute: number | null, name: string | null, airAt: number | null, aired: boolean, state: EpisodeState, attempts: number, searchedAt: number | null, nextSearch: number | null, downloadId: number | null, video: { id: number } | null }> } };

export type RefreshSeriesScheduleMutationVariables = Exact<{
  id: number;
}>;


export type RefreshSeriesScheduleMutation = { refreshSeriesSchedule: { id: number } };

export type RemoveSeriesMutationVariables = Exact<{
  id: number;
}>;


export type RemoveSeriesMutation = { removeSeries: number };

export type NamingPreviewQueryVariables = Exact<{
  id: number;
  file: string;
}>;


export type NamingPreviewQuery = { series: { namingPreview: { samples: Array<string>, error: string | null } } | null };

export type TransfersQueryVariables = Exact<{ [key: string]: never; }>;


export type TransfersQuery = { downloadEngine: { version: string, downloadRate: number, uploadRate: number, active: number, killSwitch: string | null, listening: string | null, listenError: string | null, slowHours: boolean, downloadPath: string }, downloads: Array<{ category: MediaCategory, id: number, name: string, seriesId: number | null, seriesName: string | null, poster: string | null, source: string | null, size: number | null, savePath: string, state: DownloadState, importState: ImportState, importError: string | null, importMode: string | null, error: string | null, addedAt: number, finishedAt: number | null, importedAt: number | null, title: { id: number } | null, episodes: Array<{ season: number, episode: number }>, requestedBy: { username: string } | null, live: { stage: TorrentStage, paused: boolean, progress: number, downloadRate: number, uploadRate: number, done: number, uploaded: number, ratio: number, peers: number, seeds: number, seedingSeconds: number, eta: number | null, pieces: Array<number> } | null, seedGoal: { ratio: number | null, seconds: number | null } }> };

export type SignOutMutationVariables = Exact<{ [key: string]: never; }>;


export type SignOutMutation = { signOut: boolean };

export type SearchQueryVariables = Exact<{
  query: string;
}>;


export type SearchQuery = { musicSearch: { artists: Array<{ id: number, name: string, cover: string | null, albumCount: number }>, albums: Array<{ id: number, name: string, artist: string, cover: string | null, year: number | null }>, tracks: Array<{ id: number, title: string, artist: string, album: string, albumId: number | null, albumArtist: string | null, library: string, disc: number | null, number: number | null, year: number | null, duration: number, codec: string, suffix: string, lossless: boolean, bitrate: number | null, sampleRate: number | null, bitDepth: number | null, channels: number | null, size: number, file: string, flac: string, cover: string | null, starred: boolean, rating: number | null, playCount: number, artists: Array<{ id: number, name: string }>, gains: { trackGain: number | null, trackPeak: number | null, albumGain: number | null, albumPeak: number | null, pending: boolean } }> }, search: { titles: Array<{ id: number, kind: TitleKind, library: string, name: string, year: number | null, poster: string | null, backdrop: string | null, watchedCount: number, videoCount: number, progress: number | null, freshCount: number }>, videos: Array<{ id: number, label: string | null, name: string | null, title: { name: string } }> } };

export type RecentTitlesQueryVariables = Exact<{
  ids: Array<number> | number;
}>;


export type RecentTitlesQuery = { titles: Array<{ id: number }> };

export type DownloadStatesQueryVariables = Exact<{ [key: string]: never; }>;


export type DownloadStatesQuery = { downloads: Array<{ id: number, state: DownloadState }> };

export type PauseDownloadsMutationVariables = Exact<{
  ids: Array<number> | number;
}>;


export type PauseDownloadsMutation = { pauseDownloads: Array<{ id: number }> };

export type ResumeDownloadsMutationVariables = Exact<{
  ids: Array<number> | number;
}>;


export type ResumeDownloadsMutation = { resumeDownloads: Array<{ id: number }> };

export type ReleasesQueryVariables = Exact<{
  seriesId: number;
  season: number;
  episodes: Array<number> | number;
  query?: string | null | undefined;
}>;


export type ReleasesQuery = { series: { releases: Array<{ batch: boolean, release: { title: string, source: string, link: string, infoHash: string | null, size: number | null, seeders: number | null, leechers: number | null, published: number | null, page: string | null }, attributes: { group: string | null, resolution: number | null, codec: string | null, source: string | null, dualAudio: boolean, version: number, proper: boolean, tenBit: boolean }, episodes: Array<{ season: number, episode: number }>, verdict: { accepted: boolean, score: number, rejections: Array<string>, warnings: Array<string>, nonstandard: boolean } }> } | null };

export type GrabReleaseMutationVariables = Exact<{
  release: ReleaseInput;
  seriesId?: number | null | undefined;
  episodes: Array<EpisodeNumberInput> | EpisodeNumberInput;
}>;


export type GrabReleaseMutation = { grabRelease: { id: number } };

export type DeleteDownloadedMutationVariables = Exact<{
  seriesId: number;
  season?: number | null | undefined;
}>;


export type DeleteDownloadedMutation = { deleteDownloaded: { undone: number, problems: Array<string> } };

export type LookForAgainMutationVariables = Exact<{
  seriesId: number;
  season?: number | null | undefined;
  episode?: number | null | undefined;
}>;


export type LookForAgainMutation = { lookForAgain: boolean };

export type PersonFragment = { id: number, username: string, avatar: string | null };

export type PermissionsFieldsFragment = { allLibraries: boolean, libraries: Array<string>, request: boolean, autoApprove: boolean, requestLimit: number, manageRequests: boolean, manageShows: boolean, downloads: boolean, editMetadata: boolean, watchTogether: boolean, shareLinks: boolean, clip: boolean, clipMaxLength: number, clipLimit: number, clipStorage: number, clipLinks: boolean };

export type ViewerFragment = { isAdmin: boolean, id: number, username: string, avatar: string | null, permissions: { allLibraries: boolean, libraries: Array<string>, request: boolean, autoApprove: boolean, requestLimit: number, manageRequests: boolean, manageShows: boolean, downloads: boolean, editMetadata: boolean, watchTogether: boolean, shareLinks: boolean, clip: boolean, clipMaxLength: number, clipLimit: number, clipStorage: number, clipLinks: boolean } };

export type CardFragment = { id: number, kind: TitleKind, library: string, name: string, year: number | null, poster: string | null, backdrop: string | null, watchedCount: number, videoCount: number, progress: number | null, freshCount: number };

export type VideoRowFragment = { id: number, season: number | null, episode: number | null, episodeEnd: number | null, label: string | null, name: string | null, overview: string | null, still: string, customStill: boolean, airDate: string | null, duration: number | null, position: number | null, finished: boolean | null };

export type TitleDetailFragment = { customPoster: boolean, customBackdrop: boolean, overview: string | null, genres: Array<string>, rating: number | null, path: string | null, matchState: MatchState, provider: Provider | null, providerId: string | null, libraryProvider: Provider | null, id: number, kind: TitleKind, library: string, name: string, year: number | null, poster: string | null, backdrop: string | null, watchedCount: number, videoCount: number, progress: number | null, freshCount: number, seasons: Array<{ number: number, name: string, title: string | null, overview: string | null, poster: string | null, episodes: Array<{ id: number, season: number | null, episode: number | null, episodeEnd: number | null, label: string | null, name: string | null, overview: string | null, still: string, customStill: boolean, airDate: string | null, duration: number | null, position: number | null, finished: boolean | null }> }>, movie: { id: number, season: number | null, episode: number | null, episodeEnd: number | null, label: string | null, name: string | null, overview: string | null, still: string, customStill: boolean, airDate: string | null, duration: number | null, position: number | null, finished: boolean | null } | null, nextUp: { resuming: boolean, video: { id: number, season: number | null, episode: number | null, episodeEnd: number | null, label: string | null, name: string | null, overview: string | null, still: string, customStill: boolean, airDate: string | null, duration: number | null, position: number | null, finished: boolean | null } } | null };

export type PlaybackFragment = { id: number, still: string, label: string | null, name: string | null, position: number | null, finished: boolean | null, title: { id: number, kind: TitleKind, name: string, backdrop: string | null }, previous: { id: number, label: string | null, name: string | null } | null, next: { id: number, still: string, label: string | null, name: string | null } | null, media: { duration: number | null, video: { index: number, codec: string, codecString: string | null, width: number, height: number, fps: number, bitDepth: number, hdr: boolean } | null, audio: Array<{ index: number, codec: string, codecString: string | null, channels: number, language: string | null, title: string | null, default: boolean }>, subtitles: Array<{ id: string, codec: string, language: string | null, title: string | null, default: boolean, forced: boolean, supported: boolean }>, fonts: Array<{ index: number, filename: string }>, chapters: Array<{ start: number, end: number, title: string | null }> } };

export type TranscodingFieldsFragment = { vaapi: string | null, vaapiError: string | null, softwareH264: boolean };

export type DiscoverResultFieldsFragment = { category: MediaCategory, provider: Provider, id: string, name: string, romaji: string | null, year: number | null, poster: string | null, overview: string | null, library: string, titleId: number | null, seriesId: number | null, monitor: Monitor | null, requestState: RequestState | null, because: string | null };

export type ClipFieldsFragment = { id: number, screenshot: boolean, name: string, mine: boolean, canManage: boolean, start: number, end: number, audio: number | null, subtitles: string | null, state: ClipState, progress: number | null, error: string | null, bytes: number | null, width: number | null, height: number | null, fps: number | null, renderedAt: number | null, createdAt: number, sharedAt: number | null, public: boolean, link: string | null, linkLive: boolean, file: string, poster: string | null, owner: { id: number, username: string, avatar: string | null }, source: { name: string, kind: TitleKind, label: string | null, year: number | null, status: SourceStatus, video: { id: number } | null, title: { id: number } | null }, quality: { height: number, halfRate: boolean }, recipients: Array<{ sharedAt: number, hidden: boolean, user: { id: number, username: string, avatar: string | null } }> };

export type ClipAllowanceFieldsFragment = { canClip: boolean, canLink: boolean, maxLength: number, bytes: number, rendered: number, storage: number, limit: number, customDefaultFont: boolean };

export type NotificationFieldsFragment = { id: number, kind: NotificationKind, priority: boolean, title: string, body: string | null, image: string | null, link: string | null, createdAt: number, expiresAt: number | null, readAt: number | null, actor: { id: number, username: string, avatar: string | null } | null };

export type DownloadFieldsFragment = { category: MediaCategory, id: number, name: string, seriesId: number | null, seriesName: string | null, poster: string | null, source: string | null, size: number | null, savePath: string, state: DownloadState, importState: ImportState, importError: string | null, importMode: string | null, error: string | null, addedAt: number, finishedAt: number | null, importedAt: number | null, title: { id: number } | null, episodes: Array<{ season: number, episode: number }>, requestedBy: { username: string } | null, live: { stage: TorrentStage, paused: boolean, progress: number, downloadRate: number, uploadRate: number, done: number, uploaded: number, ratio: number, peers: number, seeds: number, seedingSeconds: number, eta: number | null, pieces: Array<number> } | null, seedGoal: { ratio: number | null, seconds: number | null } };

export type EngineFieldsFragment = { version: string, downloadRate: number, uploadRate: number, active: number, killSwitch: string | null, listening: string | null, listenError: string | null, slowHours: boolean, downloadPath: string };

export type SeriesEpisodeFieldsFragment = { season: number, episode: number, absolute: number | null, name: string | null, airAt: number | null, aired: boolean, state: EpisodeState, attempts: number, searchedAt: number | null, nextSearch: number | null, downloadId: number | null, video: { id: number } | null };

export type SeedingFieldsFragment = { ratio: number | null, time: string | null, idle: string | null, then: SeedAction };

export type SeriesFieldsFragment = { id: number, monitor: Monitor, status: string | null, library: string, managed: boolean, path: string, name: string, year: number | null, poster: string | null, overview: string | null, provider: Provider | null, providerId: string | null, profile: string | null, effectiveProfile: string, sources: Array<string>, groups: Array<string>, aliases: Array<string>, knownAs: Array<string>, numbering: Numbering, naming: string | null, scheduleAt: number | null, addedAt: number, next: { season: number, episode: number, absolute: number | null, name: string | null, airAt: number | null, aired: boolean, state: EpisodeState, attempts: number, searchedAt: number | null, nextSearch: number | null, downloadId: number | null, video: { id: number } | null } | null, title: { id: number } | null, style: { file: string, folder: string, agreement: number | null, samples: number }, seeding: { ratio: number | null, time: string | null, idle: string | null, then: SeedAction } | null, counts: { have: number, wanted: number, missing: number, grabbed: number, total: number, upcoming: number, skipped: number }, episodes: Array<{ season: number, episode: number, absolute: number | null, name: string | null, airAt: number | null, aired: boolean, state: EpisodeState, attempts: number, searchedAt: number | null, nextSearch: number | null, downloadId: number | null, video: { id: number } | null }> };

export type ReleaseCandidateFieldsFragment = { batch: boolean, release: { title: string, source: string, link: string, infoHash: string | null, size: number | null, seeders: number | null, leechers: number | null, published: number | null, page: string | null }, attributes: { group: string | null, resolution: number | null, codec: string | null, source: string | null, dualAudio: boolean, version: number, proper: boolean, tenBit: boolean }, episodes: Array<{ season: number, episode: number }>, verdict: { accepted: boolean, score: number, rejections: Array<string>, warnings: Array<string>, nonstandard: boolean } };

export type CalendarEntryFieldsFragment = { seriesId: number, library: string, show: string, poster: string | null, backdrop: string | null, monitor: Monitor, season: number, episode: number, absolute: number | null, name: string | null, airAt: number, state: EpisodeState, title: { id: number } | null, video: { id: number } | null, download: { stage: TorrentStage, progress: number, downloadRate: number, eta: number | null } | null };

export type SettingsFieldsFragment = { raw: string, error: string | null, network: { host: string, port: number, cors: Array<string> }, log: { level: string }, scan: { watch: boolean, interval: string | null }, metadata: { tmdbApiKey: string | null, language: string }, transcode: { hardware: Hardware, vaapiDevice: string }, clips: { enabled: boolean, path: string | null, publicLinks: boolean, concurrency: number, maxStorage: number, fontsDir: string | null, defaultFont: string | null }, music: { onlineLyrics: boolean, lyricsUrl: string, analyzeLoudness: boolean }, downloads: { path: string | null, import: ImportMode, port: number, upnp: boolean, dht: boolean, maxActive: number, downloadLimit: number, uploadLimit: number, slowDownloadLimit: number, slowUploadLimit: number, slowFrom: string | null, slowTo: string | null, bindInterface: string | null, proxy: string | null, seeding: { ratio: number | null, time: string | null, idle: string | null, then: SeedAction } }, automation: { defaultMonitor: Monitor, rssInterval: string, renameSuggestions: boolean, retry: Array<{ every: string, until: string }> }, requests: { monitor: Monitor }, signIn: { style: SignInStyle }, sources: Array<{ name: string, kind: SourceKind, url: string, feed: string | null, apiKey: string | null, categories: Array<number>, enabled: boolean, downloadPath: string | null, seeding: { ratio: number | null, time: string | null, idle: string | null, then: SeedAction } | null }>, profiles: Array<{ name: string, resolutions: Array<string>, groups: Array<string>, require: Array<string>, reject: Array<string>, minSize: number | null, maxSize: number | null, codecs: Array<string>, preferDualAudio: boolean, batches: boolean, minSeeders: number }>, libraries: Array<{ name: string, path: string, kind: LibraryKind, metadataProvider: Provider | null, managed: boolean, profile: string | null, downloadPath: string | null, resolvedPath: string | null, exists: boolean, error: string | null, titleCount: number, skippedCount: number }>, paths: { config: string, data: string, log: string } };

export type LibrariesQueryVariables = Exact<{ [key: string]: never; }>;


export type LibrariesQuery = { libraries: Array<{ name: string, kind: LibraryKind, showCount: number, movieCount: number, albumCount: number, trackCount: number }> };

export type SettingsQueryVariables = Exact<{ [key: string]: never; }>;


export type SettingsQuery = { settings: { raw: string, error: string | null, network: { host: string, port: number, cors: Array<string> }, log: { level: string }, scan: { watch: boolean, interval: string | null }, metadata: { tmdbApiKey: string | null, language: string }, transcode: { hardware: Hardware, vaapiDevice: string }, clips: { enabled: boolean, path: string | null, publicLinks: boolean, concurrency: number, maxStorage: number, fontsDir: string | null, defaultFont: string | null }, music: { onlineLyrics: boolean, lyricsUrl: string, analyzeLoudness: boolean }, downloads: { path: string | null, import: ImportMode, port: number, upnp: boolean, dht: boolean, maxActive: number, downloadLimit: number, uploadLimit: number, slowDownloadLimit: number, slowUploadLimit: number, slowFrom: string | null, slowTo: string | null, bindInterface: string | null, proxy: string | null, seeding: { ratio: number | null, time: string | null, idle: string | null, then: SeedAction } }, automation: { defaultMonitor: Monitor, rssInterval: string, renameSuggestions: boolean, retry: Array<{ every: string, until: string }> }, requests: { monitor: Monitor }, signIn: { style: SignInStyle }, sources: Array<{ name: string, kind: SourceKind, url: string, feed: string | null, apiKey: string | null, categories: Array<number>, enabled: boolean, downloadPath: string | null, seeding: { ratio: number | null, time: string | null, idle: string | null, then: SeedAction } | null }>, profiles: Array<{ name: string, resolutions: Array<string>, groups: Array<string>, require: Array<string>, reject: Array<string>, minSize: number | null, maxSize: number | null, codecs: Array<string>, preferDualAudio: boolean, batches: boolean, minSeeders: number }>, libraries: Array<{ name: string, path: string, kind: LibraryKind, metadataProvider: Provider | null, managed: boolean, profile: string | null, downloadPath: string | null, resolvedPath: string | null, exists: boolean, error: string | null, titleCount: number, skippedCount: number }>, paths: { config: string, data: string, log: string } }, server: { transcoding: { vaapi: string | null, vaapiError: string | null, softwareH264: boolean } } };

export type SchemeFieldsFragment = { id: string, name: string, builtIn: boolean, published: boolean, editable: boolean, code: string, shareCode: string, forkedFrom: { id: string | null, name: string } | null, palette: { seeds: Array<{ name: string, value: string }>, overrides: Array<{ name: string, value: string }>, tokens: Array<{ name: string, value: string }>, warnings: Array<{ foreground: string, background: string, ratio: number, minimum: number }> } };

export type AppearanceQueryVariables = Exact<{ [key: string]: never; }>;


export type AppearanceQuery = { appearance: { mode: SchemeMode, style: ComponentStyle, mediaTint: boolean, light: { id: string, palette: { tokens: Array<{ name: string, value: string }> } }, dark: { id: string, palette: { tokens: Array<{ name: string, value: string }> } } } };

export type ClipQueryVariables = Exact<{
  id: number;
}>;


export type ClipQuery = { clip: { id: number, screenshot: boolean, name: string, mine: boolean, canManage: boolean, start: number, end: number, audio: number | null, subtitles: string | null, state: ClipState, progress: number | null, error: string | null, bytes: number | null, width: number | null, height: number | null, fps: number | null, renderedAt: number | null, createdAt: number, sharedAt: number | null, public: boolean, link: string | null, linkLive: boolean, file: string, poster: string | null, owner: { id: number, username: string, avatar: string | null }, source: { name: string, kind: TitleKind, label: string | null, year: number | null, status: SourceStatus, video: { id: number } | null, title: { id: number } | null }, quality: { height: number, halfRate: boolean }, recipients: Array<{ sharedAt: number, hidden: boolean, user: { id: number, username: string, avatar: string | null } }> } | null };

export type RenderingClipsQueryVariables = Exact<{ [key: string]: never; }>;


export type RenderingClipsQuery = { clips: Array<{ id: number, screenshot: boolean, name: string, mine: boolean, canManage: boolean, start: number, end: number, audio: number | null, subtitles: string | null, state: ClipState, progress: number | null, error: string | null, bytes: number | null, width: number | null, height: number | null, fps: number | null, renderedAt: number | null, createdAt: number, sharedAt: number | null, public: boolean, link: string | null, linkLive: boolean, file: string, poster: string | null, owner: { id: number, username: string, avatar: string | null }, source: { name: string, kind: TitleKind, label: string | null, year: number | null, status: SourceStatus, video: { id: number } | null, title: { id: number } | null }, quality: { height: number, halfRate: boolean }, recipients: Array<{ sharedAt: number, hidden: boolean, user: { id: number, username: string, avatar: string | null } }> }> };

export type StatusQueryVariables = Exact<{ [key: string]: never; }>;


export type StatusQuery = { server: { setupRequired: boolean, clips: boolean, downloads: boolean, sources: number }, viewer: { isAdmin: boolean, id: number, username: string, avatar: string | null, permissions: { allLibraries: boolean, libraries: Array<string>, request: boolean, autoApprove: boolean, requestLimit: number, manageRequests: boolean, manageShows: boolean, downloads: boolean, editMetadata: boolean, watchTogether: boolean, shareLinks: boolean, clip: boolean, clipMaxLength: number, clipLimit: number, clipStorage: number, clipLinks: boolean } } | null };

export type PeopleQueryVariables = Exact<{ [key: string]: never; }>;


export type PeopleQuery = { users: Array<{ id: number, username: string, avatar: string | null }> };

export type InboxFieldsFragment = { unread: number, items: Array<{ id: number, kind: NotificationKind, priority: boolean, title: string, body: string | null, image: string | null, link: string | null, createdAt: number, expiresAt: number | null, readAt: number | null, actor: { id: number, username: string, avatar: string | null } | null }> };

export type InboxQueryVariables = Exact<{ [key: string]: never; }>;


export type InboxQuery = { notifications: { unread: number, items: Array<{ id: number, kind: NotificationKind, priority: boolean, title: string, body: string | null, image: string | null, link: string | null, createdAt: number, expiresAt: number | null, readAt: number | null, actor: { id: number, username: string, avatar: string | null } | null }> } };

export type MarkNotificationsReadMutationVariables = Exact<{
  ids?: Array<number> | number | null | undefined;
}>;


export type MarkNotificationsReadMutation = { markNotificationsRead: { unread: number, items: Array<{ id: number, kind: NotificationKind, priority: boolean, title: string, body: string | null, image: string | null, link: string | null, createdAt: number, expiresAt: number | null, readAt: number | null, actor: { id: number, username: string, avatar: string | null } | null }> } };

export type DeleteNotificationsMutationVariables = Exact<{
  id?: number | null | undefined;
}>;


export type DeleteNotificationsMutation = { deleteNotifications: { unread: number, items: Array<{ id: number, kind: NotificationKind, priority: boolean, title: string, body: string | null, image: string | null, link: string | null, createdAt: number, expiresAt: number | null, readAt: number | null, actor: { id: number, username: string, avatar: string | null } | null }> } };

export type LibraryAlbumsQueryVariables = Exact<{
  library: string;
  sort: AlbumSort;
}>;


export type LibraryAlbumsQuery = { albums: Array<{ id: number, name: string, artist: string, year: number | null, cover: string | null, trackCount: number, duration: number, compilation: boolean, starred: boolean, playCount: number, addedAt: number, artists: Array<{ id: number, name: string }> }> };

export type LibraryArtistsQueryVariables = Exact<{
  library: string;
}>;


export type LibraryArtistsQuery = { artists: Array<{ id: number, name: string, albumCount: number, trackCount: number, cover: string | null, starred: boolean }> };

export type LibrarySongsQueryVariables = Exact<{
  library: string;
  query: string;
}>;


export type LibrarySongsQuery = { songs: Array<{ id: number, title: string, artist: string, album: string, albumId: number | null, albumArtist: string | null, library: string, disc: number | null, number: number | null, year: number | null, duration: number, codec: string, suffix: string, lossless: boolean, bitrate: number | null, sampleRate: number | null, bitDepth: number | null, channels: number | null, size: number, file: string, flac: string, cover: string | null, starred: boolean, rating: number | null, playCount: number, artists: Array<{ id: number, name: string }>, gains: { trackGain: number | null, trackPeak: number | null, albumGain: number | null, albumPeak: number | null, pending: boolean } }> };

export type PlaylistsQueryVariables = Exact<{ [key: string]: never; }>;


export type PlaylistsQuery = { playlists: Array<{ id: number, name: string, comment: string | null, public: boolean, mine: boolean, trackCount: number, duration: number, covers: Array<string>, owner: { id: number, username: string } }> };

export type MusicTrackFragment = { id: number, title: string, artist: string, album: string, albumId: number | null, albumArtist: string | null, library: string, disc: number | null, number: number | null, year: number | null, duration: number, codec: string, suffix: string, lossless: boolean, bitrate: number | null, sampleRate: number | null, bitDepth: number | null, channels: number | null, size: number, file: string, flac: string, cover: string | null, starred: boolean, rating: number | null, playCount: number, artists: Array<{ id: number, name: string }>, gains: { trackGain: number | null, trackPeak: number | null, albumGain: number | null, albumPeak: number | null, pending: boolean } };

export type AlbumCardFragment = { id: number, name: string, artist: string, year: number | null, cover: string | null, trackCount: number, duration: number, compilation: boolean, starred: boolean, playCount: number, addedAt: number, artists: Array<{ id: number, name: string }> };

export type ArtistCardFragment = { id: number, name: string, albumCount: number, trackCount: number, cover: string | null, starred: boolean };

export type PlaylistCardFragment = { id: number, name: string, comment: string | null, public: boolean, mine: boolean, trackCount: number, duration: number, covers: Array<string>, owner: { id: number, username: string } };

export type PlayQueueQueryVariables = Exact<{ [key: string]: never; }>;


export type PlayQueueQuery = { playQueue: { current: number, position: number, shuffled: boolean, repeat: Repeat, changedBy: string | null, updatedAt: number, tracks: Array<{ id: number, title: string, artist: string, album: string, albumId: number | null, albumArtist: string | null, library: string, disc: number | null, number: number | null, year: number | null, duration: number, codec: string, suffix: string, lossless: boolean, bitrate: number | null, sampleRate: number | null, bitDepth: number | null, channels: number | null, size: number, file: string, flac: string, cover: string | null, starred: boolean, rating: number | null, playCount: number, artists: Array<{ id: number, name: string }>, gains: { trackGain: number | null, trackPeak: number | null, albumGain: number | null, albumPeak: number | null, pending: boolean } }> } };

export type TrackQueryVariables = Exact<{
  id: number;
}>;


export type TrackQuery = { track: { id: number, title: string, artist: string, album: string, albumId: number | null, albumArtist: string | null, library: string, disc: number | null, number: number | null, year: number | null, duration: number, codec: string, suffix: string, lossless: boolean, bitrate: number | null, sampleRate: number | null, bitDepth: number | null, channels: number | null, size: number, file: string, flac: string, cover: string | null, starred: boolean, rating: number | null, playCount: number, artists: Array<{ id: number, name: string }>, gains: { trackGain: number | null, trackPeak: number | null, albumGain: number | null, albumPeak: number | null, pending: boolean } } | null };

export type SavePlayQueueMutationVariables = Exact<{
  input: QueueInput;
}>;


export type SavePlayQueueMutation = { savePlayQueue: { updatedAt: number } };

export type MeasureLoudnessMutationVariables = Exact<{
  trackId: number;
}>;


export type MeasureLoudnessMutation = { measureLoudness: { id: number, title: string, artist: string, album: string, albumId: number | null, albumArtist: string | null, library: string, disc: number | null, number: number | null, year: number | null, duration: number, codec: string, suffix: string, lossless: boolean, bitrate: number | null, sampleRate: number | null, bitDepth: number | null, channels: number | null, size: number, file: string, flac: string, cover: string | null, starred: boolean, rating: number | null, playCount: number, artists: Array<{ id: number, name: string }>, gains: { trackGain: number | null, trackPeak: number | null, albumGain: number | null, albumPeak: number | null, pending: boolean } } };

export type PlayedMutationVariables = Exact<{
  trackId: number;
}>;


export type PlayedMutation = { played: boolean };

export type NowPlayingMutationVariables = Exact<{
  trackId?: number | null | undefined;
  position: number;
  paused: boolean;
}>;


export type NowPlayingMutation = { nowPlaying: boolean };

export type StarMutationVariables = Exact<{
  kind: MusicKind;
  id: number;
  starred: boolean;
}>;


export type StarMutation = { star: boolean };

export type LyricsQueryVariables = Exact<{
  trackId: number;
}>;


export type LyricsQuery = { lyrics: { synced: boolean, source: LyricsSource, lines: Array<{ start: number | null, text: string }> } | null };

export type SimilarTracksQueryVariables = Exact<{
  trackId: number;
  exclude: Array<number> | number;
}>;


export type SimilarTracksQuery = { similarTracks: Array<{ id: number, title: string, artist: string, album: string, albumId: number | null, albumArtist: string | null, library: string, disc: number | null, number: number | null, year: number | null, duration: number, codec: string, suffix: string, lossless: boolean, bitrate: number | null, sampleRate: number | null, bitDepth: number | null, channels: number | null, size: number, file: string, flac: string, cover: string | null, starred: boolean, rating: number | null, playCount: number, artists: Array<{ id: number, name: string }>, gains: { trackGain: number | null, trackPeak: number | null, albumGain: number | null, albumPeak: number | null, pending: boolean } }> };

export type AlbumTracksQueryVariables = Exact<{
  id: number;
}>;


export type AlbumTracksQuery = { album: { tracks: Array<{ id: number, title: string, artist: string, album: string, albumId: number | null, albumArtist: string | null, library: string, disc: number | null, number: number | null, year: number | null, duration: number, codec: string, suffix: string, lossless: boolean, bitrate: number | null, sampleRate: number | null, bitDepth: number | null, channels: number | null, size: number, file: string, flac: string, cover: string | null, starred: boolean, rating: number | null, playCount: number, artists: Array<{ id: number, name: string }>, gains: { trackGain: number | null, trackPeak: number | null, albumGain: number | null, albumPeak: number | null, pending: boolean } }> } | null };

export type PlaylistNamesQueryVariables = Exact<{ [key: string]: never; }>;


export type PlaylistNamesQuery = { playlists: Array<{ id: number, name: string, mine: boolean }> };

export type AddToPlaylistMutationVariables = Exact<{
  id: number;
  tracks: Array<number> | number;
}>;


export type AddToPlaylistMutation = { addToPlaylist: { id: number, name: string } };

export type CreatePlaylistMutationVariables = Exact<{
  name: string;
  tracks: Array<number> | number;
}>;


export type CreatePlaylistMutation = { createPlaylist: { id: number, name: string } };

export type StartListenRoomMutationVariables = Exact<{
  input: NewListenRoom;
}>;


export type StartListenRoomMutation = { startListenRoom: { code: string } };

export type ClipAllowanceQueryVariables = Exact<{ [key: string]: never; }>;


export type ClipAllowanceQuery = { clipAllowance: { canClip: boolean, canLink: boolean, maxLength: number, bytes: number, rendered: number, storage: number, limit: number, customDefaultFont: boolean } };

export type UpdateClipMutationVariables = Exact<{
  id: number;
  input: ClipPatch;
}>;


export type UpdateClipMutation = { updateClip: { id: number, screenshot: boolean, name: string, mine: boolean, canManage: boolean, start: number, end: number, audio: number | null, subtitles: string | null, state: ClipState, progress: number | null, error: string | null, bytes: number | null, width: number | null, height: number | null, fps: number | null, renderedAt: number | null, createdAt: number, sharedAt: number | null, public: boolean, link: string | null, linkLive: boolean, file: string, poster: string | null, owner: { id: number, username: string, avatar: string | null }, source: { name: string, kind: TitleKind, label: string | null, year: number | null, status: SourceStatus, video: { id: number } | null, title: { id: number } | null }, quality: { height: number, halfRate: boolean }, recipients: Array<{ sharedAt: number, hidden: boolean, user: { id: number, username: string, avatar: string | null } }> } };

export type ShareClipMutationVariables = Exact<{
  id: number;
  users: Array<number> | number;
}>;


export type ShareClipMutation = { shareClip: { id: number, screenshot: boolean, name: string, mine: boolean, canManage: boolean, start: number, end: number, audio: number | null, subtitles: string | null, state: ClipState, progress: number | null, error: string | null, bytes: number | null, width: number | null, height: number | null, fps: number | null, renderedAt: number | null, createdAt: number, sharedAt: number | null, public: boolean, link: string | null, linkLive: boolean, file: string, poster: string | null, owner: { id: number, username: string, avatar: string | null }, source: { name: string, kind: TitleKind, label: string | null, year: number | null, status: SourceStatus, video: { id: number } | null, title: { id: number } | null }, quality: { height: number, halfRate: boolean }, recipients: Array<{ sharedAt: number, hidden: boolean, user: { id: number, username: string, avatar: string | null } }> } };

export type CreateClipMutationVariables = Exact<{
  input: NewClip;
}>;


export type CreateClipMutation = { createClip: { id: number, screenshot: boolean, name: string, mine: boolean, canManage: boolean, start: number, end: number, audio: number | null, subtitles: string | null, state: ClipState, progress: number | null, error: string | null, bytes: number | null, width: number | null, height: number | null, fps: number | null, renderedAt: number | null, createdAt: number, sharedAt: number | null, public: boolean, link: string | null, linkLive: boolean, file: string, poster: string | null, owner: { id: number, username: string, avatar: string | null }, source: { name: string, kind: TitleKind, label: string | null, year: number | null, status: SourceStatus, video: { id: number } | null, title: { id: number } | null }, quality: { height: number, halfRate: boolean }, recipients: Array<{ sharedAt: number, hidden: boolean, user: { id: number, username: string, avatar: string | null } }> } };

export type PlaybackQueryVariables = Exact<{
  id: number;
}>;


export type PlaybackQuery = { video: { id: number, still: string, label: string | null, name: string | null, position: number | null, finished: boolean | null, title: { id: number, kind: TitleKind, name: string, backdrop: string | null }, previous: { id: number, label: string | null, name: string | null } | null, next: { id: number, still: string, label: string | null, name: string | null } | null, media: { duration: number | null, video: { index: number, codec: string, codecString: string | null, width: number, height: number, fps: number, bitDepth: number, hdr: boolean } | null, audio: Array<{ index: number, codec: string, codecString: string | null, channels: number, language: string | null, title: string | null, default: boolean }>, subtitles: Array<{ id: string, codec: string, language: string | null, title: string | null, default: boolean, forced: boolean, supported: boolean }>, fonts: Array<{ index: number, filename: string }>, chapters: Array<{ start: number, end: number, title: string | null }> } } | null, server: { transcoding: { vaapi: string | null, vaapiError: string | null, softwareH264: boolean } } };

export type RoomPlaybackQueryVariables = Exact<{
  code: string;
  id: number;
}>;


export type RoomPlaybackQuery = { room: { video: { id: number, still: string, label: string | null, name: string | null, position: number | null, finished: boolean | null, title: { id: number, kind: TitleKind, name: string, backdrop: string | null }, previous: { id: number, label: string | null, name: string | null } | null, next: { id: number, still: string, label: string | null, name: string | null } | null, media: { duration: number | null, video: { index: number, codec: string, codecString: string | null, width: number, height: number, fps: number, bitDepth: number, hdr: boolean } | null, audio: Array<{ index: number, codec: string, codecString: string | null, channels: number, language: string | null, title: string | null, default: boolean }>, subtitles: Array<{ id: string, codec: string, language: string | null, title: string | null, default: boolean, forced: boolean, supported: boolean }>, fonts: Array<{ index: number, filename: string }>, chapters: Array<{ start: number, end: number, title: string | null }> } } | null }, server: { transcoding: { vaapi: string | null, vaapiError: string | null, softwareH264: boolean } } };

export type SaveProgressMutationVariables = Exact<{
  videoId: number;
  position: number;
  duration: number;
}>;


export type SaveProgressMutation = { saveProgress: { id: number } };

export type TakeScreenshotMutationVariables = Exact<{
  input: NewScreenshot;
}>;


export type TakeScreenshotMutation = { takeScreenshot: { id: number, screenshot: boolean, name: string, mine: boolean, canManage: boolean, start: number, end: number, audio: number | null, subtitles: string | null, state: ClipState, progress: number | null, error: string | null, bytes: number | null, width: number | null, height: number | null, fps: number | null, renderedAt: number | null, createdAt: number, sharedAt: number | null, public: boolean, link: string | null, linkLive: boolean, file: string, poster: string | null, owner: { id: number, username: string, avatar: string | null }, source: { name: string, kind: TitleKind, label: string | null, year: number | null, status: SourceStatus, video: { id: number } | null, title: { id: number } | null }, quality: { height: number, halfRate: boolean }, recipients: Array<{ sharedAt: number, hidden: boolean, user: { id: number, username: string, avatar: string | null } }> } };

export type StartRoomMutationVariables = Exact<{
  input: NewRoom;
}>;


export type StartRoomMutation = { startRoom: { code: string } };

export type PlayerScheduleQueryVariables = Exact<{
  id: number;
}>;


export type PlayerScheduleQuery = { title: { series: { id: number, monitor: Monitor, status: string | null, next: { season: number, episode: number, absolute: number | null, name: string | null, airAt: number | null, aired: boolean, state: EpisodeState, attempts: number, searchedAt: number | null, nextSearch: number | null, downloadId: number | null, video: { id: number } | null } | null } | null } | null };

export type PlayerOverviewQueryVariables = Exact<{
  id: number;
  videoId: number;
}>;


export type PlayerOverviewQuery = { title: { overview: string | null } | null, video: { overview: string | null } | null };

export type RoomQueryVariables = Exact<{
  code: string;
}>;


export type RoomQuery = { room: { code: string, signedIn: boolean, isHost: boolean, canShare: boolean, canInvite: boolean, title: { id: number, name: string, backdrop: string | null } } };

export type EventsSubscriptionVariables = Exact<{ [key: string]: never; }>;


export type EventsSubscription = { events:
    | { __typename: 'ClipChanged', clipId: number, state: ClipState | null, progress: number | null }
    | { __typename: 'ConfigChanged', error: string | null }
    | { __typename: 'EpisodesImported', library: string }
    | { __typename: 'LibraryChanged', library: string }
    | { __typename: 'ListChanged', list: ChangedList }
    | { __typename: 'MetadataChanged', titleId: number, status: MetadataStatus }
    | { __typename: 'NotificationReceived', notification: { id: number, kind: NotificationKind, priority: boolean, title: string, body: string | null, image: string | null, link: string | null, createdAt: number, expiresAt: number | null, readAt: number | null, actor: { id: number, username: string, avatar: string | null } | null } }
    | { __typename: 'PlaybackChanged', client: string, trackId: number | null, position: number, paused: boolean }
    | { __typename: 'QueueChanged', by: string | null }
    | { __typename: 'ScanFinished', library: string }
    | { __typename: 'ScanStarted' }
    | { __typename: 'SeriesChanged', seriesId: number }
   };

export type AlbumQueryVariables = Exact<{
  id: number;
}>;


export type AlbumQuery = { album: { library: string, releaseDate: string | null, originalDate: string | null, genres: Array<string>, releaseTypes: Array<string>, labels: Array<string>, id: number, name: string, artist: string, year: number | null, cover: string | null, trackCount: number, duration: number, compilation: boolean, starred: boolean, playCount: number, addedAt: number, discTitles: Array<{ disc: number, title: string }>, tracks: Array<{ id: number, title: string, artist: string, album: string, albumId: number | null, albumArtist: string | null, library: string, disc: number | null, number: number | null, year: number | null, duration: number, codec: string, suffix: string, lossless: boolean, bitrate: number | null, sampleRate: number | null, bitDepth: number | null, channels: number | null, size: number, file: string, flac: string, cover: string | null, starred: boolean, rating: number | null, playCount: number, artists: Array<{ id: number, name: string }>, gains: { trackGain: number | null, trackPeak: number | null, albumGain: number | null, albumPeak: number | null, pending: boolean } }>, artists: Array<{ id: number, name: string }> } | null };

export type MoreByArtistQueryVariables = Exact<{
  id: number;
}>;


export type MoreByArtistQuery = { artist: { albums: Array<{ id: number, name: string, artist: string, year: number | null, cover: string | null, trackCount: number, duration: number, compilation: boolean, starred: boolean, playCount: number, addedAt: number, artists: Array<{ id: number, name: string }> }> } | null };

export type ArtistQueryVariables = Exact<{
  id: number;
}>;


export type ArtistQuery = { artist: { id: number, name: string, albumCount: number, trackCount: number, cover: string | null, starred: boolean, albums: Array<{ releaseTypes: Array<string>, id: number, name: string, artist: string, year: number | null, cover: string | null, trackCount: number, duration: number, compilation: boolean, starred: boolean, playCount: number, addedAt: number, artists: Array<{ id: number, name: string }> }>, appearsOn: Array<{ id: number, name: string, artist: string, year: number | null, cover: string | null, trackCount: number, duration: number, compilation: boolean, starred: boolean, playCount: number, addedAt: number, artists: Array<{ id: number, name: string }> }>, topTracks: Array<{ id: number, title: string, artist: string, album: string, albumId: number | null, albumArtist: string | null, library: string, disc: number | null, number: number | null, year: number | null, duration: number, codec: string, suffix: string, lossless: boolean, bitrate: number | null, sampleRate: number | null, bitDepth: number | null, channels: number | null, size: number, file: string, flac: string, cover: string | null, starred: boolean, rating: number | null, playCount: number, artists: Array<{ id: number, name: string }>, gains: { trackGain: number | null, trackPeak: number | null, albumGain: number | null, albumPeak: number | null, pending: boolean } }> } | null };

export type CalendarQueryVariables = Exact<{
  from: number;
  to: number;
}>;


export type CalendarQuery = { calendar: Array<{ seriesId: number, library: string, show: string, poster: string | null, backdrop: string | null, monitor: Monitor, season: number, episode: number, absolute: number | null, name: string | null, airAt: number, state: EpisodeState, title: { id: number } | null, video: { id: number } | null, download: { stage: TorrentStage, progress: number, downloadRate: number, eta: number | null } | null }> };

export type ClipsQueryVariables = Exact<{
  scope: ClipScope;
}>;


export type ClipsQuery = { clips: Array<{ id: number, screenshot: boolean, name: string, mine: boolean, canManage: boolean, start: number, end: number, audio: number | null, subtitles: string | null, state: ClipState, progress: number | null, error: string | null, bytes: number | null, width: number | null, height: number | null, fps: number | null, renderedAt: number | null, createdAt: number, sharedAt: number | null, public: boolean, link: string | null, linkLive: boolean, file: string, poster: string | null, owner: { id: number, username: string, avatar: string | null }, source: { name: string, kind: TitleKind, label: string | null, year: number | null, status: SourceStatus, video: { id: number } | null, title: { id: number } | null }, quality: { height: number, halfRate: boolean }, recipients: Array<{ sharedAt: number, hidden: boolean, user: { id: number, username: string, avatar: string | null } }> }>, clipAllowance: { canClip: boolean, canLink: boolean, maxLength: number, bytes: number, rendered: number, storage: number, limit: number, customDefaultFont: boolean } };

export type RenderClipMutationVariables = Exact<{
  id: number;
}>;


export type RenderClipMutation = { renderClip: { id: number, screenshot: boolean, name: string, mine: boolean, canManage: boolean, start: number, end: number, audio: number | null, subtitles: string | null, state: ClipState, progress: number | null, error: string | null, bytes: number | null, width: number | null, height: number | null, fps: number | null, renderedAt: number | null, createdAt: number, sharedAt: number | null, public: boolean, link: string | null, linkLive: boolean, file: string, poster: string | null, owner: { id: number, username: string, avatar: string | null }, source: { name: string, kind: TitleKind, label: string | null, year: number | null, status: SourceStatus, video: { id: number } | null, title: { id: number } | null }, quality: { height: number, halfRate: boolean }, recipients: Array<{ sharedAt: number, hidden: boolean, user: { id: number, username: string, avatar: string | null } }> } };

export type ClipsUpdateClipMutationVariables = Exact<{
  id: number;
  input: ClipPatch;
}>;


export type ClipsUpdateClipMutation = { updateClip: { id: number, screenshot: boolean, name: string, mine: boolean, canManage: boolean, start: number, end: number, audio: number | null, subtitles: string | null, state: ClipState, progress: number | null, error: string | null, bytes: number | null, width: number | null, height: number | null, fps: number | null, renderedAt: number | null, createdAt: number, sharedAt: number | null, public: boolean, link: string | null, linkLive: boolean, file: string, poster: string | null, owner: { id: number, username: string, avatar: string | null }, source: { name: string, kind: TitleKind, label: string | null, year: number | null, status: SourceStatus, video: { id: number } | null, title: { id: number } | null }, quality: { height: number, halfRate: boolean }, recipients: Array<{ sharedAt: number, hidden: boolean, user: { id: number, username: string, avatar: string | null } }> } };

export type ClipsShareClipMutationVariables = Exact<{
  id: number;
  users: Array<number> | number;
}>;


export type ClipsShareClipMutation = { shareClip: { id: number, screenshot: boolean, name: string, mine: boolean, canManage: boolean, start: number, end: number, audio: number | null, subtitles: string | null, state: ClipState, progress: number | null, error: string | null, bytes: number | null, width: number | null, height: number | null, fps: number | null, renderedAt: number | null, createdAt: number, sharedAt: number | null, public: boolean, link: string | null, linkLive: boolean, file: string, poster: string | null, owner: { id: number, username: string, avatar: string | null }, source: { name: string, kind: TitleKind, label: string | null, year: number | null, status: SourceStatus, video: { id: number } | null, title: { id: number } | null }, quality: { height: number, halfRate: boolean }, recipients: Array<{ sharedAt: number, hidden: boolean, user: { id: number, username: string, avatar: string | null } }> } };

export type UnshareClipMutationVariables = Exact<{
  id: number;
  userId: number;
}>;


export type UnshareClipMutation = { unshareClip: { id: number, screenshot: boolean, name: string, mine: boolean, canManage: boolean, start: number, end: number, audio: number | null, subtitles: string | null, state: ClipState, progress: number | null, error: string | null, bytes: number | null, width: number | null, height: number | null, fps: number | null, renderedAt: number | null, createdAt: number, sharedAt: number | null, public: boolean, link: string | null, linkLive: boolean, file: string, poster: string | null, owner: { id: number, username: string, avatar: string | null }, source: { name: string, kind: TitleKind, label: string | null, year: number | null, status: SourceStatus, video: { id: number } | null, title: { id: number } | null }, quality: { height: number, halfRate: boolean }, recipients: Array<{ sharedAt: number, hidden: boolean, user: { id: number, username: string, avatar: string | null } }> } };

export type DeleteClipMutationVariables = Exact<{
  id: number;
}>;


export type DeleteClipMutation = { deleteClip: number };

export type HideClipMutationVariables = Exact<{
  id: number;
}>;


export type HideClipMutation = { hideClip: number };

export type DiscoverQueryVariables = Exact<{
  library?: string | null | undefined;
  query?: string | null | undefined;
}>;


export type DiscoverQuery = { discover: { library: string, results: Array<{ category: MediaCategory, provider: Provider, id: string, name: string, romaji: string | null, year: number | null, poster: string | null, overview: string | null, library: string, titleId: number | null, seriesId: number | null, monitor: Monitor | null, requestState: RequestState | null, because: string | null }> } };

export type ForYouQueryVariables = Exact<{
  library?: string | null | undefined;
}>;


export type ForYouQuery = { forYou: { library: string, shelves: Array<{ key: string, name: string, results: Array<{ category: MediaCategory, provider: Provider, id: string, name: string, romaji: string | null, year: number | null, poster: string | null, overview: string | null, library: string, titleId: number | null, seriesId: number | null, monitor: Monitor | null, requestState: RequestState | null, because: string | null }> }> } };

export type DownloadsQueryVariables = Exact<{ [key: string]: never; }>;


export type DownloadsQuery = { downloads: Array<{ category: MediaCategory, id: number, name: string, seriesId: number | null, seriesName: string | null, poster: string | null, source: string | null, size: number | null, savePath: string, state: DownloadState, importState: ImportState, importError: string | null, importMode: string | null, error: string | null, addedAt: number, finishedAt: number | null, importedAt: number | null, title: { id: number } | null, episodes: Array<{ season: number, episode: number }>, requestedBy: { username: string } | null, live: { stage: TorrentStage, paused: boolean, progress: number, downloadRate: number, uploadRate: number, done: number, uploaded: number, ratio: number, peers: number, seeds: number, seedingSeconds: number, eta: number | null, pieces: Array<number> } | null, seedGoal: { ratio: number | null, seconds: number | null } }> };

export type EngineQueryVariables = Exact<{ [key: string]: never; }>;


export type EngineQuery = { downloadEngine: { version: string, downloadRate: number, uploadRate: number, active: number, killSwitch: string | null, listening: string | null, listenError: string | null, slowHours: boolean, downloadPath: string } };

export type DownloadsPauseMutationVariables = Exact<{
  ids: Array<number> | number;
}>;


export type DownloadsPauseMutation = { pauseDownloads: Array<{ id: number }> };

export type DownloadsResumeMutationVariables = Exact<{
  ids: Array<number> | number;
}>;


export type DownloadsResumeMutation = { resumeDownloads: Array<{ id: number }> };

export type DownloadsRecheckMutationVariables = Exact<{
  ids: Array<number> | number;
}>;


export type DownloadsRecheckMutation = { recheckDownloads: Array<{ id: number }> };

export type ImportDownloadMutationVariables = Exact<{
  id: number;
}>;


export type ImportDownloadMutation = { importDownload: { id: number } };

export type RemoveDownloadsMutationVariables = Exact<{
  ids: Array<number> | number;
  deleteFiles: boolean;
}>;


export type RemoveDownloadsMutation = { removeDownloads: Array<number> };

export type HomeQueryVariables = Exact<{ [key: string]: never; }>;


export type HomeQuery = { home: { continueWatching: Array<{ position: number, upNext: boolean, newEpisode: boolean, watchedAt: number | null, video: { id: number, label: string | null, name: string | null, still: string, duration: number | null, title: { id: number, name: string, poster: string | null, backdrop: string | null } } }>, recentlyAdded: Array<{ library: string, titles: Array<{ id: number, kind: TitleKind, library: string, name: string, year: number | null, poster: string | null, backdrop: string | null, watchedCount: number, videoCount: number, progress: number | null, freshCount: number }> }>, popularHere: Array<{ people: number, title: { id: number, kind: TitleKind, library: string, name: string, year: number | null, poster: string | null, backdrop: string | null, watchedCount: number, videoCount: number, progress: number | null, freshCount: number } }> } };

export type ComingUpQueryVariables = Exact<{
  from: number;
  to: number;
}>;


export type ComingUpQuery = { calendar: Array<{ seriesId: number, library: string, show: string, poster: string | null, backdrop: string | null, monitor: Monitor, season: number, episode: number, absolute: number | null, name: string | null, airAt: number, state: EpisodeState, title: { id: number } | null, video: { id: number } | null, download: { stage: TorrentStage, progress: number, downloadRate: number, eta: number | null } | null }> };

export type MusicHomeQueryVariables = Exact<{ [key: string]: never; }>;


export type MusicHomeQuery = { musicHome: { recentlyPlayed: Array<{ id: number, name: string, artist: string, year: number | null, cover: string | null, trackCount: number, duration: number, compilation: boolean, starred: boolean, playCount: number, addedAt: number, artists: Array<{ id: number, name: string }> }>, recentlyAdded: Array<{ id: number, name: string, artist: string, year: number | null, cover: string | null, trackCount: number, duration: number, compilation: boolean, starred: boolean, playCount: number, addedAt: number, artists: Array<{ id: number, name: string }> }> } };

export type InviteQueryVariables = Exact<{
  token: string;
}>;


export type InviteQuery = { invite: { expiresAt: number, remainingUses: number } | null };

export type AcceptInviteMutationVariables = Exact<{
  token: string;
  username: string;
  password: string;
}>;


export type AcceptInviteMutation = { acceptInvite: { user: { id: number } } };

export type LibraryQueryVariables = Exact<{
  name: string;
}>;


export type LibraryQuery = { library: { titles: Array<{ id: number, kind: TitleKind, library: string, name: string, year: number | null, poster: string | null, backdrop: string | null, watchedCount: number, videoCount: number, progress: number | null, freshCount: number }> } | null };

export type ListenRoomQueryVariables = Exact<{
  code: string;
}>;


export type ListenRoomQuery = { listenRoom: { code: string, hostName: string, signedIn: boolean, isHost: boolean, canShare: boolean, canInvite: boolean, tracks: Array<{ id: number, title: string, artist: string, album: string, albumId: number | null, albumArtist: string | null, library: string, disc: number | null, number: number | null, year: number | null, duration: number, codec: string, suffix: string, lossless: boolean, bitrate: number | null, sampleRate: number | null, bitDepth: number | null, channels: number | null, size: number, file: string, flac: string, cover: string | null, starred: boolean, rating: number | null, playCount: number, artists: Array<{ id: number, name: string }>, gains: { trackGain: number | null, trackPeak: number | null, albumGain: number | null, albumPeak: number | null, pending: boolean } }> } };

export type PlaylistQueryVariables = Exact<{
  id: number;
}>;


export type PlaylistQuery = { playlist: { id: number, name: string, comment: string | null, public: boolean, mine: boolean, trackCount: number, duration: number, covers: Array<string>, tracks: Array<{ id: number, title: string, artist: string, album: string, albumId: number | null, albumArtist: string | null, library: string, disc: number | null, number: number | null, year: number | null, duration: number, codec: string, suffix: string, lossless: boolean, bitrate: number | null, sampleRate: number | null, bitDepth: number | null, channels: number | null, size: number, file: string, flac: string, cover: string | null, starred: boolean, rating: number | null, playCount: number, artists: Array<{ id: number, name: string }>, gains: { trackGain: number | null, trackPeak: number | null, albumGain: number | null, albumPeak: number | null, pending: boolean } }>, owner: { id: number, username: string } } | null };

export type UpdatePlaylistMutationVariables = Exact<{
  id: number;
  input: PlaylistInput;
}>;


export type UpdatePlaylistMutation = { updatePlaylist: { id: number } };

export type DeletePlaylistMutationVariables = Exact<{
  id: number;
}>;


export type DeletePlaylistMutation = { deletePlaylist: boolean };

export type RequestsQueryVariables = Exact<{ [key: string]: never; }>;


export type RequestsQuery = { requests: Array<{ id: number, name: string, year: number | null, poster: string | null, library: string | null, state: RequestState, note: string | null, createdAt: number, have: number | null, aired: number | null, user: { id: number, username: string, avatar: string | null } | null, title: { id: number } | null }> };

export type ApproveRequestMutationVariables = Exact<{
  id: number;
}>;


export type ApproveRequestMutation = { approveRequest: { id: number } };

export type DeclineRequestMutationVariables = Exact<{
  id: number;
}>;


export type DeclineRequestMutation = { declineRequest: { id: number } };

export type DeleteRequestMutationVariables = Exact<{
  id: number;
}>;


export type DeleteRequestMutation = { deleteRequest: number };

export type ScanMutationVariables = Exact<{
  library?: string | null | undefined;
}>;


export type ScanMutation = { scan: boolean };

export type AddLibraryMutationVariables = Exact<{
  input: LibraryInput;
}>;


export type AddLibraryMutation = { addLibrary: { raw: string } };

export type UpdateLibraryMutationVariables = Exact<{
  name: string;
  input: LibraryInput;
}>;


export type UpdateLibraryMutation = { updateLibrary: { raw: string } };

export type RemoveLibraryMutationVariables = Exact<{
  name: string;
}>;


export type RemoveLibraryMutation = { removeLibrary: { raw: string } };

export type FoldersQueryVariables = Exact<{
  path?: string | null | undefined;
}>;


export type FoldersQuery = { folders: { path: string, parent: string | null, home: string | null, folders: Array<{ name: string, path: string }> } };

export type SaveServerMutationVariables = Exact<{
  patch: ConfigPatch;
}>;


export type SaveServerMutation = { updateSettings: { raw: string } };

export type PasskeysQueryVariables = Exact<{ [key: string]: never; }>;


export type PasskeysQuery = { viewer: { passkeys: Array<{ id: number, name: string, createdAt: number, lastUsed: number | null }> } | null };

export type StartPasskeyRegistrationMutationVariables = Exact<{
  name?: string | null | undefined;
}>;


export type StartPasskeyRegistrationMutation = { startPasskeyRegistration: { challenge: string, options: unknown } };

export type FinishPasskeyRegistrationMutationVariables = Exact<{
  challenge: string;
  credential: unknown;
}>;


export type FinishPasskeyRegistrationMutation = { finishPasskeyRegistration: Array<{ id: number }> };

export type DeletePasskeyMutationVariables = Exact<{
  id: number;
}>;


export type DeletePasskeyMutation = { deletePasskey: Array<{ id: number }> };

export type ChangePasswordMutationVariables = Exact<{
  current: string;
  new: string;
}>;


export type ChangePasswordMutation = { changePassword: boolean };

export type ReplaceConfigMutationVariables = Exact<{
  text: string;
}>;


export type ReplaceConfigMutation = { replaceConfig: { raw: string } };

export type SkippedFilesQueryVariables = Exact<{ [key: string]: never; }>;


export type SkippedFilesQuery = { skippedFiles: Array<{ library: string, path: string, reason: string }> };

export type TitleQueryVariables = Exact<{
  id: number;
}>;


export type TitleQuery = { title: { customPoster: boolean, customBackdrop: boolean, overview: string | null, genres: Array<string>, rating: number | null, path: string | null, matchState: MatchState, provider: Provider | null, providerId: string | null, libraryProvider: Provider | null, id: number, kind: TitleKind, library: string, name: string, year: number | null, poster: string | null, backdrop: string | null, watchedCount: number, videoCount: number, progress: number | null, freshCount: number, seasons: Array<{ number: number, name: string, title: string | null, overview: string | null, poster: string | null, episodes: Array<{ id: number, season: number | null, episode: number | null, episodeEnd: number | null, label: string | null, name: string | null, overview: string | null, still: string, customStill: boolean, airDate: string | null, duration: number | null, position: number | null, finished: boolean | null }> }>, movie: { id: number, season: number | null, episode: number | null, episodeEnd: number | null, label: string | null, name: string | null, overview: string | null, still: string, customStill: boolean, airDate: string | null, duration: number | null, position: number | null, finished: boolean | null } | null, nextUp: { resuming: boolean, video: { id: number, season: number | null, episode: number | null, episodeEnd: number | null, label: string | null, name: string | null, overview: string | null, still: string, customStill: boolean, airDate: string | null, duration: number | null, position: number | null, finished: boolean | null } } | null } | null };

export type SimilarQueryVariables = Exact<{
  id: number;
}>;


export type SimilarQuery = { title: { similar: { recommendations: Array<{ category: MediaCategory, provider: Provider, id: string, name: string, romaji: string | null, year: number | null, poster: string | null, overview: string | null, library: string, titleId: number | null, seriesId: number | null, monitor: Monitor | null, requestState: RequestState | null, because: string | null }>, alsoWatched: Array<{ id: number, kind: TitleKind, library: string, name: string, year: number | null, poster: string | null, backdrop: string | null, watchedCount: number, videoCount: number, progress: number | null, freshCount: number }> } } | null };

export type MatchCandidatesQueryVariables = Exact<{
  id: number;
  query?: string | null | undefined;
  provider?: Provider | null | undefined;
}>;


export type MatchCandidatesQuery = { title: { matchCandidates: { query: string, results: Array<{ provider: Provider, id: string, name: string, year: number | null, poster: string | null, overview: string | null }> } } | null };

export type TitleDownloadsQueryVariables = Exact<{ [key: string]: never; }>;


export type TitleDownloadsQuery = { downloads: Array<{ category: MediaCategory, id: number, name: string, seriesId: number | null, seriesName: string | null, poster: string | null, source: string | null, size: number | null, savePath: string, state: DownloadState, importState: ImportState, importError: string | null, importMode: string | null, error: string | null, addedAt: number, finishedAt: number | null, importedAt: number | null, title: { id: number } | null, episodes: Array<{ season: number, episode: number }>, requestedBy: { username: string } | null, live: { stage: TorrentStage, paused: boolean, progress: number, downloadRate: number, uploadRate: number, done: number, uploaded: number, ratio: number, peers: number, seeds: number, seedingSeconds: number, eta: number | null, pieces: Array<number> } | null, seedGoal: { ratio: number | null, seconds: number | null } }> };

export type SetWatchedMutationVariables = Exact<{
  videoIds: Array<number> | number;
  watched: boolean;
}>;


export type SetWatchedMutation = { setWatched: Array<{ id: number }> };

export type SetTitleWatchedMutationVariables = Exact<{
  id: number;
  watched: boolean;
}>;


export type SetTitleWatchedMutation = { setTitleWatched: { id: number } };

export type RefreshTitleMutationVariables = Exact<{
  id: number;
}>;


export type RefreshTitleMutation = { refreshTitle: { id: number } };

export type MatchTitleMutationVariables = Exact<{
  id: number;
  provider: Provider;
  providerId: string;
}>;


export type MatchTitleMutation = { matchTitle: { id: number } };

export type WantedQueryVariables = Exact<{ [key: string]: never; }>;


export type WantedQuery = { wanted: Array<{ seriesId: number, show: string, season: number, episode: number, name: string | null, airAt: number | null, aired: boolean, state: EpisodeState, attempts: number, searchedAt: number | null, nextSearch: number | null, title: { id: number } | null }> };

export class TypedDocumentString<TResult, TVariables>
  extends String
  implements DocumentTypeDecoration<TResult, TVariables>
{
  __apiType?: NonNullable<DocumentTypeDecoration<TResult, TVariables>['__apiType']>;
  private value: string;
  public __meta__?: Record<string, any> | undefined;

  constructor(value: string, __meta__?: Record<string, any> | undefined) {
    super(value);
    this.value = value;
    this.__meta__ = __meta__;
  }

  override toString(): string & DocumentTypeDecoration<TResult, TVariables> {
    return this.value;
  }
}
export const PersonFragmentDoc = new TypedDocumentString(`
    fragment Person on User {
  id
  username
  avatar
}
    `, {"fragmentName":"Person"}) as unknown as TypedDocumentString<PersonFragment, unknown>;
export const PermissionsFieldsFragmentDoc = new TypedDocumentString(`
    fragment PermissionsFields on Permissions {
  allLibraries
  libraries
  request
  autoApprove
  requestLimit
  manageRequests
  manageShows
  downloads
  editMetadata
  watchTogether
  shareLinks
  clip
  clipMaxLength
  clipLimit
  clipStorage
  clipLinks
}
    `, {"fragmentName":"PermissionsFields"}) as unknown as TypedDocumentString<PermissionsFieldsFragment, unknown>;
export const ViewerFragmentDoc = new TypedDocumentString(`
    fragment Viewer on User {
  ...Person
  isAdmin
  permissions {
    ...PermissionsFields
  }
}
    fragment Person on User {
  id
  username
  avatar
}
fragment PermissionsFields on Permissions {
  allLibraries
  libraries
  request
  autoApprove
  requestLimit
  manageRequests
  manageShows
  downloads
  editMetadata
  watchTogether
  shareLinks
  clip
  clipMaxLength
  clipLimit
  clipStorage
  clipLinks
}`, {"fragmentName":"Viewer"}) as unknown as TypedDocumentString<ViewerFragment, unknown>;
export const CardFragmentDoc = new TypedDocumentString(`
    fragment Card on Title {
  id
  kind
  library
  name
  year
  poster
  backdrop
  watchedCount
  videoCount
  progress
  freshCount
}
    `, {"fragmentName":"Card"}) as unknown as TypedDocumentString<CardFragment, unknown>;
export const VideoRowFragmentDoc = new TypedDocumentString(`
    fragment VideoRow on Video {
  id
  season
  episode
  episodeEnd
  label
  name
  overview
  still
  customStill
  airDate
  duration
  position
  finished
}
    `, {"fragmentName":"VideoRow"}) as unknown as TypedDocumentString<VideoRowFragment, unknown>;
export const TitleDetailFragmentDoc = new TypedDocumentString(`
    fragment TitleDetail on Title {
  ...Card
  customPoster
  customBackdrop
  overview
  genres
  rating
  path
  matchState
  provider
  providerId
  libraryProvider
  seasons {
    number
    name
    title
    overview
    poster
    episodes {
      ...VideoRow
    }
  }
  movie {
    ...VideoRow
  }
  nextUp {
    resuming
    video {
      ...VideoRow
    }
  }
}
    fragment Card on Title {
  id
  kind
  library
  name
  year
  poster
  backdrop
  watchedCount
  videoCount
  progress
  freshCount
}
fragment VideoRow on Video {
  id
  season
  episode
  episodeEnd
  label
  name
  overview
  still
  customStill
  airDate
  duration
  position
  finished
}`, {"fragmentName":"TitleDetail"}) as unknown as TypedDocumentString<TitleDetailFragment, unknown>;
export const PlaybackFragmentDoc = new TypedDocumentString(`
    fragment Playback on Video {
  id
  still
  label
  name
  position
  finished
  title {
    id
    kind
    name
    backdrop
  }
  previous {
    id
    label
    name
  }
  next {
    id
    still
    label
    name
  }
  media {
    duration
    video {
      index
      codec
      codecString
      width
      height
      fps
      bitDepth
      hdr
    }
    audio {
      index
      codec
      codecString
      channels
      language
      title
      default
    }
    subtitles {
      id
      codec
      language
      title
      default
      forced
      supported
    }
    fonts {
      index
      filename
    }
    chapters {
      start
      end
      title
    }
  }
}
    `, {"fragmentName":"Playback"}) as unknown as TypedDocumentString<PlaybackFragment, unknown>;
export const TranscodingFieldsFragmentDoc = new TypedDocumentString(`
    fragment TranscodingFields on Transcoding {
  vaapi
  vaapiError
  softwareH264
}
    `, {"fragmentName":"TranscodingFields"}) as unknown as TypedDocumentString<TranscodingFieldsFragment, unknown>;
export const DiscoverResultFieldsFragmentDoc = new TypedDocumentString(`
    fragment DiscoverResultFields on DiscoverResult {
  category
  provider
  id
  name
  romaji
  year
  poster
  overview
  library
  titleId
  seriesId
  monitor
  requestState
  because
}
    `, {"fragmentName":"DiscoverResultFields"}) as unknown as TypedDocumentString<DiscoverResultFieldsFragment, unknown>;
export const ClipFieldsFragmentDoc = new TypedDocumentString(`
    fragment ClipFields on Clip {
  id
  screenshot
  name
  mine
  canManage
  owner {
    ...Person
  }
  source {
    video {
      id
    }
    title {
      id
    }
    name
    kind
    label
    year
    status
  }
  start
  end
  audio
  subtitles
  quality {
    height
    halfRate
  }
  state
  progress
  error
  bytes
  width
  height
  fps
  renderedAt
  createdAt
  sharedAt
  public
  link
  linkLive
  recipients {
    user {
      ...Person
    }
    sharedAt
    hidden
  }
  file
  poster
}
    fragment Person on User {
  id
  username
  avatar
}`, {"fragmentName":"ClipFields"}) as unknown as TypedDocumentString<ClipFieldsFragment, unknown>;
export const ClipAllowanceFieldsFragmentDoc = new TypedDocumentString(`
    fragment ClipAllowanceFields on ClipAllowance {
  canClip
  canLink
  maxLength
  bytes
  rendered
  storage
  limit
  customDefaultFont
}
    `, {"fragmentName":"ClipAllowanceFields"}) as unknown as TypedDocumentString<ClipAllowanceFieldsFragment, unknown>;
export const DownloadFieldsFragmentDoc = new TypedDocumentString(`
    fragment DownloadFields on Download {
  category
  id
  name
  seriesId
  seriesName
  title {
    id
  }
  poster
  episodes {
    season
    episode
  }
  source
  size
  savePath
  state
  importState
  importError
  importMode
  error
  addedAt
  finishedAt
  importedAt
  requestedBy {
    username
  }
  live {
    stage
    paused
    progress
    downloadRate
    uploadRate
    done
    uploaded
    ratio
    peers
    seeds
    seedingSeconds
    eta
    pieces
  }
  seedGoal {
    ratio
    seconds
  }
}
    `, {"fragmentName":"DownloadFields"}) as unknown as TypedDocumentString<DownloadFieldsFragment, unknown>;
export const EngineFieldsFragmentDoc = new TypedDocumentString(`
    fragment EngineFields on DownloadEngine {
  version
  downloadRate
  uploadRate
  active
  killSwitch
  listening
  listenError
  slowHours
  downloadPath
}
    `, {"fragmentName":"EngineFields"}) as unknown as TypedDocumentString<EngineFieldsFragment, unknown>;
export const SeriesEpisodeFieldsFragmentDoc = new TypedDocumentString(`
    fragment SeriesEpisodeFields on SeriesEpisode {
  season
  episode
  absolute
  name
  airAt
  aired
  state
  attempts
  searchedAt
  nextSearch
  downloadId
  video {
    id
  }
}
    `, {"fragmentName":"SeriesEpisodeFields"}) as unknown as TypedDocumentString<SeriesEpisodeFieldsFragment, unknown>;
export const SeedingFieldsFragmentDoc = new TypedDocumentString(`
    fragment SeedingFields on Seeding {
  ratio
  time
  idle
  then
}
    `, {"fragmentName":"SeedingFields"}) as unknown as TypedDocumentString<SeedingFieldsFragment, unknown>;
export const SeriesFieldsFragmentDoc = new TypedDocumentString(`
    fragment SeriesFields on Series {
  id
  monitor
  status
  next {
    ...SeriesEpisodeFields
  }
  title {
    id
  }
  library
  managed
  path
  name
  year
  poster
  overview
  provider
  providerId
  profile
  effectiveProfile
  sources
  groups
  aliases
  knownAs
  numbering
  naming
  style {
    file
    folder
    agreement
    samples
  }
  seeding {
    ...SeedingFields
  }
  scheduleAt
  addedAt
  counts {
    have
    wanted
    missing
    grabbed
    total
    upcoming
    skipped
  }
  episodes {
    ...SeriesEpisodeFields
  }
}
    fragment SeriesEpisodeFields on SeriesEpisode {
  season
  episode
  absolute
  name
  airAt
  aired
  state
  attempts
  searchedAt
  nextSearch
  downloadId
  video {
    id
  }
}
fragment SeedingFields on Seeding {
  ratio
  time
  idle
  then
}`, {"fragmentName":"SeriesFields"}) as unknown as TypedDocumentString<SeriesFieldsFragment, unknown>;
export const ReleaseCandidateFieldsFragmentDoc = new TypedDocumentString(`
    fragment ReleaseCandidateFields on ReleaseCandidate {
  release {
    title
    source
    link
    infoHash
    size
    seeders
    leechers
    published
    page
  }
  attributes {
    group
    resolution
    codec
    source
    dualAudio
    version
    proper
    tenBit
  }
  episodes {
    season
    episode
  }
  batch
  verdict {
    accepted
    score
    rejections
    warnings
    nonstandard
  }
}
    `, {"fragmentName":"ReleaseCandidateFields"}) as unknown as TypedDocumentString<ReleaseCandidateFieldsFragment, unknown>;
export const CalendarEntryFieldsFragmentDoc = new TypedDocumentString(`
    fragment CalendarEntryFields on CalendarEntry {
  seriesId
  title {
    id
  }
  library
  show
  poster
  backdrop
  monitor
  season
  episode
  absolute
  name
  airAt
  state
  video {
    id
  }
  download {
    stage
    progress
    downloadRate
    eta
  }
}
    `, {"fragmentName":"CalendarEntryFields"}) as unknown as TypedDocumentString<CalendarEntryFieldsFragment, unknown>;
export const SettingsFieldsFragmentDoc = new TypedDocumentString(`
    fragment SettingsFields on Settings {
  network {
    host
    port
    cors
  }
  log {
    level
  }
  scan {
    watch
    interval
  }
  metadata {
    tmdbApiKey
    language
  }
  transcode {
    hardware
    vaapiDevice
  }
  clips {
    enabled
    path
    publicLinks
    concurrency
    maxStorage
    fontsDir
    defaultFont
  }
  music {
    onlineLyrics
    lyricsUrl
    analyzeLoudness
  }
  downloads {
    path
    import
    port
    upnp
    dht
    maxActive
    downloadLimit
    uploadLimit
    slowDownloadLimit
    slowUploadLimit
    slowFrom
    slowTo
    bindInterface
    proxy
    seeding {
      ...SeedingFields
    }
  }
  automation {
    defaultMonitor
    rssInterval
    retry {
      every
      until
    }
    renameSuggestions
  }
  requests {
    monitor
  }
  signIn {
    style
  }
  sources {
    name
    kind
    url
    feed
    apiKey
    categories
    enabled
    downloadPath
    seeding {
      ...SeedingFields
    }
  }
  profiles {
    name
    resolutions
    groups
    require
    reject
    minSize
    maxSize
    codecs
    preferDualAudio
    batches
    minSeeders
  }
  libraries {
    name
    path
    kind
    metadataProvider
    managed
    profile
    downloadPath
    resolvedPath
    exists
    error
    titleCount
    skippedCount
  }
  raw
  error
  paths {
    config
    data
    log
  }
}
    fragment SeedingFields on Seeding {
  ratio
  time
  idle
  then
}`, {"fragmentName":"SettingsFields"}) as unknown as TypedDocumentString<SettingsFieldsFragment, unknown>;
export const SchemeFieldsFragmentDoc = new TypedDocumentString(`
    fragment SchemeFields on ColorScheme {
  id
  name
  builtIn
  published
  editable
  code
  shareCode
  forkedFrom {
    id
    name
  }
  palette {
    seeds {
      name
      value
    }
    overrides {
      name
      value
    }
    tokens {
      name
      value
    }
    warnings {
      foreground
      background
      ratio
      minimum
    }
  }
}
    `, {"fragmentName":"SchemeFields"}) as unknown as TypedDocumentString<SchemeFieldsFragment, unknown>;
export const NotificationFieldsFragmentDoc = new TypedDocumentString(`
    fragment NotificationFields on Notification {
  id
  kind
  priority
  title
  body
  image
  link
  actor {
    ...Person
  }
  createdAt
  expiresAt
  readAt
}
    fragment Person on User {
  id
  username
  avatar
}`, {"fragmentName":"NotificationFields"}) as unknown as TypedDocumentString<NotificationFieldsFragment, unknown>;
export const InboxFieldsFragmentDoc = new TypedDocumentString(`
    fragment InboxFields on Inbox {
  items {
    ...NotificationFields
  }
  unread
}
    fragment Person on User {
  id
  username
  avatar
}
fragment NotificationFields on Notification {
  id
  kind
  priority
  title
  body
  image
  link
  actor {
    ...Person
  }
  createdAt
  expiresAt
  readAt
}`, {"fragmentName":"InboxFields"}) as unknown as TypedDocumentString<InboxFieldsFragment, unknown>;
export const MusicTrackFragmentDoc = new TypedDocumentString(`
    fragment MusicTrack on Track {
  id
  title
  artist
  artists {
    id
    name
  }
  album
  albumId
  albumArtist
  library
  disc
  number
  year
  duration
  codec
  suffix
  lossless
  bitrate
  sampleRate
  bitDepth
  channels
  size
  file
  flac
  cover
  gains {
    trackGain
    trackPeak
    albumGain
    albumPeak
    pending
  }
  starred
  rating
  playCount
}
    `, {"fragmentName":"MusicTrack"}) as unknown as TypedDocumentString<MusicTrackFragment, unknown>;
export const AlbumCardFragmentDoc = new TypedDocumentString(`
    fragment AlbumCard on Album {
  id
  name
  artist
  artists {
    id
    name
  }
  year
  cover
  trackCount
  duration
  compilation
  starred
  playCount
  addedAt
}
    `, {"fragmentName":"AlbumCard"}) as unknown as TypedDocumentString<AlbumCardFragment, unknown>;
export const ArtistCardFragmentDoc = new TypedDocumentString(`
    fragment ArtistCard on Artist {
  id
  name
  albumCount
  trackCount
  cover
  starred
}
    `, {"fragmentName":"ArtistCard"}) as unknown as TypedDocumentString<ArtistCardFragment, unknown>;
export const PlaylistCardFragmentDoc = new TypedDocumentString(`
    fragment PlaylistCard on Playlist {
  id
  name
  comment
  public
  mine
  trackCount
  duration
  covers
  owner {
    id
    username
  }
}
    `, {"fragmentName":"PlaylistCard"}) as unknown as TypedDocumentString<PlaylistCardFragment, unknown>;
export const ColorSchemesDocument = new TypedDocumentString(`
    query ColorSchemes {
  colorSchemes {
    ...SchemeFields
  }
}
    fragment SchemeFields on ColorScheme {
  id
  name
  builtIn
  published
  editable
  code
  shareCode
  forkedFrom {
    id
    name
  }
  palette {
    seeds {
      name
      value
    }
    overrides {
      name
      value
    }
    tokens {
      name
      value
    }
    warnings {
      foreground
      background
      ratio
      minimum
    }
  }
}`) as unknown as TypedDocumentString<ColorSchemesQuery, ColorSchemesQueryVariables>;
export const AppearanceSettingsDocument = new TypedDocumentString(`
    query AppearanceSettings {
  appearanceSettings {
    colors {
      mode
      single
      light
      dark
    }
    style
    mediaTint
  }
}
    `) as unknown as TypedDocumentString<AppearanceSettingsQuery, AppearanceSettingsQueryVariables>;
export const ServerAppearanceDocument = new TypedDocumentString(`
    query ServerAppearance {
  serverAppearance {
    colors {
      mode
      single
      light
      dark
    }
    style
  }
}
    `) as unknown as TypedDocumentString<ServerAppearanceQuery, ServerAppearanceQueryVariables>;
export const SetAppearanceDocument = new TypedDocumentString(`
    mutation SetAppearance($input: AppearanceSettingsInput!) {
  setAppearance(input: $input) {
    mode
  }
}
    `) as unknown as TypedDocumentString<SetAppearanceMutation, SetAppearanceMutationVariables>;
export const SetServerAppearanceDocument = new TypedDocumentString(`
    mutation SetServerAppearance($input: ServerAppearanceInput!) {
  setServerAppearance(input: $input) {
    style
  }
}
    `) as unknown as TypedDocumentString<SetServerAppearanceMutation, SetServerAppearanceMutationVariables>;
export const SaveSchemeDocument = new TypedDocumentString(`
    mutation SaveScheme($id: String, $input: SchemeInput!) {
  saveScheme(id: $id, input: $input) {
    ...SchemeFields
  }
}
    fragment SchemeFields on ColorScheme {
  id
  name
  builtIn
  published
  editable
  code
  shareCode
  forkedFrom {
    id
    name
  }
  palette {
    seeds {
      name
      value
    }
    overrides {
      name
      value
    }
    tokens {
      name
      value
    }
    warnings {
      foreground
      background
      ratio
      minimum
    }
  }
}`) as unknown as TypedDocumentString<SaveSchemeMutation, SaveSchemeMutationVariables>;
export const ForkSchemeDocument = new TypedDocumentString(`
    mutation ForkScheme($id: String!) {
  forkScheme(id: $id) {
    ...SchemeFields
  }
}
    fragment SchemeFields on ColorScheme {
  id
  name
  builtIn
  published
  editable
  code
  shareCode
  forkedFrom {
    id
    name
  }
  palette {
    seeds {
      name
      value
    }
    overrides {
      name
      value
    }
    tokens {
      name
      value
    }
    warnings {
      foreground
      background
      ratio
      minimum
    }
  }
}`) as unknown as TypedDocumentString<ForkSchemeMutation, ForkSchemeMutationVariables>;
export const ImportSchemeDocument = new TypedDocumentString(`
    mutation ImportScheme($code: String!, $name: String) {
  importScheme(code: $code, name: $name) {
    ...SchemeFields
  }
}
    fragment SchemeFields on ColorScheme {
  id
  name
  builtIn
  published
  editable
  code
  shareCode
  forkedFrom {
    id
    name
  }
  palette {
    seeds {
      name
      value
    }
    overrides {
      name
      value
    }
    tokens {
      name
      value
    }
    warnings {
      foreground
      background
      ratio
      minimum
    }
  }
}`) as unknown as TypedDocumentString<ImportSchemeMutation, ImportSchemeMutationVariables>;
export const DeleteSchemeDocument = new TypedDocumentString(`
    mutation DeleteScheme($id: String!) {
  deleteScheme(id: $id)
}
    `) as unknown as TypedDocumentString<DeleteSchemeMutation, DeleteSchemeMutationVariables>;
export const PublishSchemeDocument = new TypedDocumentString(`
    mutation PublishScheme($id: String!, $published: Boolean!) {
  publishScheme(id: $id, published: $published) {
    id
  }
}
    `) as unknown as TypedDocumentString<PublishSchemeMutation, PublishSchemeMutationVariables>;
export const DecodeSchemeDocument = new TypedDocumentString(`
    query DecodeScheme($code: String!) {
  decodeScheme(code: $code) {
    name
    code
    palette {
      tokens {
        name
        value
      }
      warnings {
        foreground
        background
        ratio
        minimum
      }
    }
  }
}
    `) as unknown as TypedDocumentString<DecodeSchemeQuery, DecodeSchemeQueryVariables>;
export const SetTitleArtworkDocument = new TypedDocumentString(`
    mutation SetTitleArtwork($id: Int!, $kind: TitleArtwork!, $image: Upload) {
  setTitleArtwork(id: $id, kind: $kind, image: $image) {
    id
  }
}
    `) as unknown as TypedDocumentString<SetTitleArtworkMutation, SetTitleArtworkMutationVariables>;
export const SetVideoArtworkDocument = new TypedDocumentString(`
    mutation SetVideoArtwork($videoId: Int!, $image: Upload) {
  setVideoArtwork(videoId: $videoId, image: $image) {
    id
  }
}
    `) as unknown as TypedDocumentString<SetVideoArtworkMutation, SetVideoArtworkMutationVariables>;
export const SetAvatarDocument = new TypedDocumentString(`
    mutation SetAvatar($image: Upload!, $userId: Int) {
  setAvatar(image: $image, userId: $userId) {
    id
    avatar
  }
}
    `) as unknown as TypedDocumentString<SetAvatarMutation, SetAvatarMutationVariables>;
export const RemoveAvatarDocument = new TypedDocumentString(`
    mutation RemoveAvatar($userId: Int) {
  removeAvatar(userId: $userId) {
    id
    avatar
  }
}
    `) as unknown as TypedDocumentString<RemoveAvatarMutation, RemoveAvatarMutationVariables>;
export const ClipStorageDocument = new TypedDocumentString(`
    query ClipStorage {
  clipStorage {
    usage {
      user {
        ...Person
      }
      bytes
      rendered
      clips
      storage
      limit
    }
    publicClips {
      ...ClipFields
    }
    bytes
    dir
  }
}
    fragment Person on User {
  id
  username
  avatar
}
fragment ClipFields on Clip {
  id
  screenshot
  name
  mine
  canManage
  owner {
    ...Person
  }
  source {
    video {
      id
    }
    title {
      id
    }
    name
    kind
    label
    year
    status
  }
  start
  end
  audio
  subtitles
  quality {
    height
    halfRate
  }
  state
  progress
  error
  bytes
  width
  height
  fps
  renderedAt
  createdAt
  sharedAt
  public
  link
  linkLive
  recipients {
    user {
      ...Person
    }
    sharedAt
    hidden
  }
  file
  poster
}`) as unknown as TypedDocumentString<ClipStorageQuery, ClipStorageQueryVariables>;
export const SaveClipsConfigDocument = new TypedDocumentString(`
    mutation SaveClipsConfig($clips: ClipsConfigInput!) {
  updateSettings(patch: { clips: $clips }) {
    clips {
      enabled
    }
  }
}
    `) as unknown as TypedDocumentString<SaveClipsConfigMutation, SaveClipsConfigMutationVariables>;
export const DropClipRendersDocument = new TypedDocumentString(`
    mutation DropClipRenders {
  dropClipRenders
}
    `) as unknown as TypedDocumentString<DropClipRendersMutation, DropClipRendersMutationVariables>;
export const UnpublishClipDocument = new TypedDocumentString(`
    mutation UnpublishClip($id: Int!) {
  updateClip(id: $id, input: { public: false }) {
    id
  }
}
    `) as unknown as TypedDocumentString<UnpublishClipMutation, UnpublishClipMutationVariables>;
export const AdminDeleteClipDocument = new TypedDocumentString(`
    mutation AdminDeleteClip($id: Int!) {
  deleteClip(id: $id)
}
    `) as unknown as TypedDocumentString<AdminDeleteClipMutation, AdminDeleteClipMutationVariables>;
export const AddSeriesDocument = new TypedDocumentString(`
    mutation AddSeries($input: NewSeries!) {
  addSeries(input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<AddSeriesMutation, AddSeriesMutationVariables>;
export const AiredEpisodesDocument = new TypedDocumentString(`
    query AiredEpisodes($provider: Provider!, $id: String!) {
  airedEpisodes(provider: $provider, id: $id)
}
    `) as unknown as TypedDocumentString<AiredEpisodesQuery, AiredEpisodesQueryVariables>;
export const CreateRequestDocument = new TypedDocumentString(`
    mutation CreateRequest($input: NewRequest!) {
  createRequest(input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<CreateRequestMutation, CreateRequestMutationVariables>;
export const SaveSectionDocument = new TypedDocumentString(`
    mutation SaveSection($patch: ConfigPatch!) {
  updateSettings(patch: $patch) {
    raw
  }
}
    `) as unknown as TypedDocumentString<SaveSectionMutation, SaveSectionMutationVariables>;
export const SettingsEngineDocument = new TypedDocumentString(`
    query SettingsEngine {
  downloadEngine {
    downloadPath
    killSwitch
  }
}
    `) as unknown as TypedDocumentString<SettingsEngineQuery, SettingsEngineQueryVariables>;
export const AddSourceDocument = new TypedDocumentString(`
    mutation AddSource($input: SourceInput!) {
  addSource(input: $input) {
    raw
  }
}
    `) as unknown as TypedDocumentString<AddSourceMutation, AddSourceMutationVariables>;
export const UpdateSourceDocument = new TypedDocumentString(`
    mutation UpdateSource($name: String!, $input: SourceInput!) {
  updateSource(name: $name, input: $input) {
    raw
  }
}
    `) as unknown as TypedDocumentString<UpdateSourceMutation, UpdateSourceMutationVariables>;
export const RemoveSourceDocument = new TypedDocumentString(`
    mutation RemoveSource($name: String!) {
  removeSource(name: $name) {
    raw
  }
}
    `) as unknown as TypedDocumentString<RemoveSourceMutation, RemoveSourceMutationVariables>;
export const DetectSourceDocument = new TypedDocumentString(`
    query DetectSource($url: String!, $apiKey: String) {
  detectSource(url: $url, apiKey: $apiKey) {
    kind
    url
    feed
    name
    searchable
    sample {
      link
      title
      size
      seeders
      published
    }
  }
}
    `) as unknown as TypedDocumentString<DetectSourceQuery, DetectSourceQueryVariables>;
export const AddProfileDocument = new TypedDocumentString(`
    mutation AddProfile($input: ProfileInput!) {
  addProfile(input: $input) {
    raw
  }
}
    `) as unknown as TypedDocumentString<AddProfileMutation, AddProfileMutationVariables>;
export const UpdateProfileDocument = new TypedDocumentString(`
    mutation UpdateProfile($name: String!, $input: ProfileInput!) {
  updateProfile(name: $name, input: $input) {
    raw
  }
}
    `) as unknown as TypedDocumentString<UpdateProfileMutation, UpdateProfileMutationVariables>;
export const RemoveProfileDocument = new TypedDocumentString(`
    mutation RemoveProfile($name: String!) {
  removeProfile(name: $name) {
    raw
  }
}
    `) as unknown as TypedDocumentString<RemoveProfileMutation, RemoveProfileMutationVariables>;
export const RenameSuggestionsDocument = new TypedDocumentString(`
    query RenameSuggestions {
  renameSuggestions {
    id
    library
    managed
    root
    src
    dst
    reason
    confidence
  }
}
    `) as unknown as TypedDocumentString<RenameSuggestionsQuery, RenameSuggestionsQueryVariables>;
export const FileHistoryDocument = new TypedDocumentString(`
    query FileHistory {
  fileHistory {
    batch
    label
    at
    count
    undone
    operations {
      kind
      src
      dst
    }
  }
}
    `) as unknown as TypedDocumentString<FileHistoryQuery, FileHistoryQueryVariables>;
export const RefreshRenameSuggestionsDocument = new TypedDocumentString(`
    mutation RefreshRenameSuggestions {
  refreshRenameSuggestions
}
    `) as unknown as TypedDocumentString<RefreshRenameSuggestionsMutation, RefreshRenameSuggestionsMutationVariables>;
export const ApplyRenamesDocument = new TypedDocumentString(`
    mutation ApplyRenames($ids: [Int!]!) {
  applyRenames(ids: $ids) {
    renamed
    problems
  }
}
    `) as unknown as TypedDocumentString<ApplyRenamesMutation, ApplyRenamesMutationVariables>;
export const DismissRenamesDocument = new TypedDocumentString(`
    mutation DismissRenames($ids: [Int!]!) {
  dismissRenames(ids: $ids)
}
    `) as unknown as TypedDocumentString<DismissRenamesMutation, DismissRenamesMutationVariables>;
export const UndoFileChangesDocument = new TypedDocumentString(`
    mutation UndoFileChanges($batch: String!) {
  undoFileChanges(batch: $batch) {
    undone
    problems
  }
}
    `) as unknown as TypedDocumentString<UndoFileChangesMutation, UndoFileChangesMutationVariables>;
export const InvitesDocument = new TypedDocumentString(`
    query Invites {
  invites {
    id
    label
    createdAt
    expiresAt
    maxUses
    uses
    revoked
  }
}
    `) as unknown as TypedDocumentString<InvitesQuery, InvitesQueryVariables>;
export const CreateInviteDocument = new TypedDocumentString(`
    mutation CreateInvite($label: String!, $maxUses: Int!, $expiresInHours: Int!) {
  createInvite(label: $label, maxUses: $maxUses, expiresInHours: $expiresInHours) {
    link
  }
}
    `) as unknown as TypedDocumentString<CreateInviteMutation, CreateInviteMutationVariables>;
export const RevokeInviteDocument = new TypedDocumentString(`
    mutation RevokeInvite($id: Int!) {
  revokeInvite(id: $id)
}
    `) as unknown as TypedDocumentString<RevokeInviteMutation, RevokeInviteMutationVariables>;
export const SignInProfilesDocument = new TypedDocumentString(`
    query SignInProfiles {
  signInProfiles {
    key
    avatar
    passkey
  }
}
    `) as unknown as TypedDocumentString<SignInProfilesQuery, SignInProfilesQueryVariables>;
export const SetupDocument = new TypedDocumentString(`
    mutation Setup($username: String!, $password: String!) {
  setup(username: $username, password: $password) {
    user {
      id
    }
  }
}
    `) as unknown as TypedDocumentString<SetupMutation, SetupMutationVariables>;
export const SignInDocument = new TypedDocumentString(`
    mutation SignIn($username: String, $profile: String, $password: String!) {
  signIn(username: $username, profile: $profile, password: $password) {
    user {
      id
    }
  }
}
    `) as unknown as TypedDocumentString<SignInMutation, SignInMutationVariables>;
export const StartPasskeySignInDocument = new TypedDocumentString(`
    mutation StartPasskeySignIn($username: String, $profile: String) {
  startPasskeySignIn(username: $username, profile: $profile) {
    challenge
    options
  }
}
    `) as unknown as TypedDocumentString<StartPasskeySignInMutation, StartPasskeySignInMutationVariables>;
export const FinishPasskeySignInDocument = new TypedDocumentString(`
    mutation FinishPasskeySignIn($challenge: String!, $credential: JSON!) {
  finishPasskeySignIn(challenge: $challenge, credential: $credential) {
    user {
      id
    }
  }
}
    `) as unknown as TypedDocumentString<FinishPasskeySignInMutation, FinishPasskeySignInMutationVariables>;
export const SaveMusicConfigDocument = new TypedDocumentString(`
    mutation SaveMusicConfig($music: MusicConfigInput!) {
  updateSettings(patch: { music: $music }) {
    music {
      onlineLyrics
    }
  }
}
    `) as unknown as TypedDocumentString<SaveMusicConfigMutation, SaveMusicConfigMutationVariables>;
export const AppPasswordsDocument = new TypedDocumentString(`
    query AppPasswords {
  appPasswords {
    id
    name
    createdAt
    lastUsed
    client
  }
}
    `) as unknown as TypedDocumentString<AppPasswordsQuery, AppPasswordsQueryVariables>;
export const CreateAppPasswordDocument = new TypedDocumentString(`
    mutation CreateAppPassword($name: String!) {
  createAppPassword(name: $name) {
    secret
    password {
      id
      name
    }
  }
}
    `) as unknown as TypedDocumentString<CreateAppPasswordMutation, CreateAppPasswordMutationVariables>;
export const DeleteAppPasswordDocument = new TypedDocumentString(`
    mutation DeleteAppPassword($id: Int!) {
  deleteAppPassword(id: $id)
}
    `) as unknown as TypedDocumentString<DeleteAppPasswordMutation, DeleteAppPasswordMutationVariables>;
export const UsersDocument = new TypedDocumentString(`
    query Users {
  users {
    ...Viewer
    createdAt
    lastSeen
    overrides {
      allLibraries
      libraries
      request
      autoApprove
      requestLimit
      manageRequests
      manageShows
      downloads
      editMetadata
      watchTogether
      shareLinks
      clip
      clipMaxLength
      clipLimit
      clipStorage
      clipLinks
    }
  }
}
    fragment Person on User {
  id
  username
  avatar
}
fragment PermissionsFields on Permissions {
  allLibraries
  libraries
  request
  autoApprove
  requestLimit
  manageRequests
  manageShows
  downloads
  editMetadata
  watchTogether
  shareLinks
  clip
  clipMaxLength
  clipLimit
  clipStorage
  clipLinks
}
fragment Viewer on User {
  ...Person
  isAdmin
  permissions {
    ...PermissionsFields
  }
}`) as unknown as TypedDocumentString<UsersQuery, UsersQueryVariables>;
export const PermissionDefaultsDocument = new TypedDocumentString(`
    query PermissionDefaults {
  permissionDefaults {
    ...PermissionsFields
  }
}
    fragment PermissionsFields on Permissions {
  allLibraries
  libraries
  request
  autoApprove
  requestLimit
  manageRequests
  manageShows
  downloads
  editMetadata
  watchTogether
  shareLinks
  clip
  clipMaxLength
  clipLimit
  clipStorage
  clipLinks
}`) as unknown as TypedDocumentString<PermissionDefaultsQuery, PermissionDefaultsQueryVariables>;
export const SetPermissionDefaultsDocument = new TypedDocumentString(`
    mutation SetPermissionDefaults($permissions: PermissionsInput!) {
  setPermissionDefaults(permissions: $permissions) {
    ...PermissionsFields
  }
}
    fragment PermissionsFields on Permissions {
  allLibraries
  libraries
  request
  autoApprove
  requestLimit
  manageRequests
  manageShows
  downloads
  editMetadata
  watchTogether
  shareLinks
  clip
  clipMaxLength
  clipLimit
  clipStorage
  clipLinks
}`) as unknown as TypedDocumentString<SetPermissionDefaultsMutation, SetPermissionDefaultsMutationVariables>;
export const UpdateUserDocument = new TypedDocumentString(`
    mutation UpdateUser($id: Int!, $input: UserPatch!) {
  updateUser(id: $id, input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<UpdateUserMutation, UpdateUserMutationVariables>;
export const DeleteUserDocument = new TypedDocumentString(`
    mutation DeleteUser($id: Int!) {
  deleteUser(id: $id)
}
    `) as unknown as TypedDocumentString<DeleteUserMutation, DeleteUserMutationVariables>;
export const CreateUserDocument = new TypedDocumentString(`
    mutation CreateUser($input: NewUser!) {
  createUser(input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<CreateUserMutation, CreateUserMutationVariables>;
export const TitleSeriesDocument = new TypedDocumentString(`
    query TitleSeries($id: Int!) {
  title(id: $id) {
    series {
      ...SeriesFields
    }
  }
}
    fragment SeriesEpisodeFields on SeriesEpisode {
  season
  episode
  absolute
  name
  airAt
  aired
  state
  attempts
  searchedAt
  nextSearch
  downloadId
  video {
    id
  }
}
fragment SeedingFields on Seeding {
  ratio
  time
  idle
  then
}
fragment SeriesFields on Series {
  id
  monitor
  status
  next {
    ...SeriesEpisodeFields
  }
  title {
    id
  }
  library
  managed
  path
  name
  year
  poster
  overview
  provider
  providerId
  profile
  effectiveProfile
  sources
  groups
  aliases
  knownAs
  numbering
  naming
  style {
    file
    folder
    agreement
    samples
  }
  seeding {
    ...SeedingFields
  }
  scheduleAt
  addedAt
  counts {
    have
    wanted
    missing
    grabbed
    total
    upcoming
    skipped
  }
  episodes {
    ...SeriesEpisodeFields
  }
}`) as unknown as TypedDocumentString<TitleSeriesQuery, TitleSeriesQueryVariables>;
export const TitleScheduleDocument = new TypedDocumentString(`
    query TitleSchedule($id: Int!) {
  title(id: $id) {
    series {
      id
      monitor
      status
      next {
        ...SeriesEpisodeFields
      }
    }
  }
}
    fragment SeriesEpisodeFields on SeriesEpisode {
  season
  episode
  absolute
  name
  airAt
  aired
  state
  attempts
  searchedAt
  nextSearch
  downloadId
  video {
    id
  }
}`) as unknown as TypedDocumentString<TitleScheduleQuery, TitleScheduleQueryVariables>;
export const ManageTitleDocument = new TypedDocumentString(`
    mutation ManageTitle($titleId: Int!) {
  manageTitle(titleId: $titleId) {
    id
  }
}
    `) as unknown as TypedDocumentString<ManageTitleMutation, ManageTitleMutationVariables>;
export const UpdateSeriesDocument = new TypedDocumentString(`
    mutation UpdateSeries($id: Int!, $patch: SeriesPatch!) {
  updateSeries(id: $id, patch: $patch) {
    ...SeriesFields
  }
}
    fragment SeriesEpisodeFields on SeriesEpisode {
  season
  episode
  absolute
  name
  airAt
  aired
  state
  attempts
  searchedAt
  nextSearch
  downloadId
  video {
    id
  }
}
fragment SeedingFields on Seeding {
  ratio
  time
  idle
  then
}
fragment SeriesFields on Series {
  id
  monitor
  status
  next {
    ...SeriesEpisodeFields
  }
  title {
    id
  }
  library
  managed
  path
  name
  year
  poster
  overview
  provider
  providerId
  profile
  effectiveProfile
  sources
  groups
  aliases
  knownAs
  numbering
  naming
  style {
    file
    folder
    agreement
    samples
  }
  seeding {
    ...SeedingFields
  }
  scheduleAt
  addedAt
  counts {
    have
    wanted
    missing
    grabbed
    total
    upcoming
    skipped
  }
  episodes {
    ...SeriesEpisodeFields
  }
}`) as unknown as TypedDocumentString<UpdateSeriesMutation, UpdateSeriesMutationVariables>;
export const RefreshSeriesScheduleDocument = new TypedDocumentString(`
    mutation RefreshSeriesSchedule($id: Int!) {
  refreshSeriesSchedule(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<RefreshSeriesScheduleMutation, RefreshSeriesScheduleMutationVariables>;
export const RemoveSeriesDocument = new TypedDocumentString(`
    mutation RemoveSeries($id: Int!) {
  removeSeries(id: $id)
}
    `) as unknown as TypedDocumentString<RemoveSeriesMutation, RemoveSeriesMutationVariables>;
export const NamingPreviewDocument = new TypedDocumentString(`
    query NamingPreview($id: Int!, $file: String!) {
  series(id: $id) {
    namingPreview(file: $file) {
      samples
      error
    }
  }
}
    `) as unknown as TypedDocumentString<NamingPreviewQuery, NamingPreviewQueryVariables>;
export const TransfersDocument = new TypedDocumentString(`
    query Transfers {
  downloadEngine {
    ...EngineFields
  }
  downloads {
    ...DownloadFields
  }
}
    fragment DownloadFields on Download {
  category
  id
  name
  seriesId
  seriesName
  title {
    id
  }
  poster
  episodes {
    season
    episode
  }
  source
  size
  savePath
  state
  importState
  importError
  importMode
  error
  addedAt
  finishedAt
  importedAt
  requestedBy {
    username
  }
  live {
    stage
    paused
    progress
    downloadRate
    uploadRate
    done
    uploaded
    ratio
    peers
    seeds
    seedingSeconds
    eta
    pieces
  }
  seedGoal {
    ratio
    seconds
  }
}
fragment EngineFields on DownloadEngine {
  version
  downloadRate
  uploadRate
  active
  killSwitch
  listening
  listenError
  slowHours
  downloadPath
}`) as unknown as TypedDocumentString<TransfersQuery, TransfersQueryVariables>;
export const SignOutDocument = new TypedDocumentString(`
    mutation SignOut {
  signOut
}
    `) as unknown as TypedDocumentString<SignOutMutation, SignOutMutationVariables>;
export const SearchDocument = new TypedDocumentString(`
    query Search($query: String!) {
  musicSearch(query: $query, limit: 5) {
    artists {
      id
      name
      cover
      albumCount
    }
    albums {
      id
      name
      artist
      cover
      year
    }
    tracks {
      ...MusicTrack
    }
  }
  search(query: $query) {
    titles {
      ...Card
    }
    videos {
      id
      label
      name
      title {
        name
      }
    }
  }
}
    fragment Card on Title {
  id
  kind
  library
  name
  year
  poster
  backdrop
  watchedCount
  videoCount
  progress
  freshCount
}
fragment MusicTrack on Track {
  id
  title
  artist
  artists {
    id
    name
  }
  album
  albumId
  albumArtist
  library
  disc
  number
  year
  duration
  codec
  suffix
  lossless
  bitrate
  sampleRate
  bitDepth
  channels
  size
  file
  flac
  cover
  gains {
    trackGain
    trackPeak
    albumGain
    albumPeak
    pending
  }
  starred
  rating
  playCount
}`) as unknown as TypedDocumentString<SearchQuery, SearchQueryVariables>;
export const RecentTitlesDocument = new TypedDocumentString(`
    query RecentTitles($ids: [Int!]!) {
  titles(ids: $ids) {
    id
  }
}
    `) as unknown as TypedDocumentString<RecentTitlesQuery, RecentTitlesQueryVariables>;
export const DownloadStatesDocument = new TypedDocumentString(`
    query DownloadStates {
  downloads {
    id
    state
  }
}
    `) as unknown as TypedDocumentString<DownloadStatesQuery, DownloadStatesQueryVariables>;
export const PauseDownloadsDocument = new TypedDocumentString(`
    mutation PauseDownloads($ids: [Int!]!) {
  pauseDownloads(ids: $ids) {
    id
  }
}
    `) as unknown as TypedDocumentString<PauseDownloadsMutation, PauseDownloadsMutationVariables>;
export const ResumeDownloadsDocument = new TypedDocumentString(`
    mutation ResumeDownloads($ids: [Int!]!) {
  resumeDownloads(ids: $ids) {
    id
  }
}
    `) as unknown as TypedDocumentString<ResumeDownloadsMutation, ResumeDownloadsMutationVariables>;
export const ReleasesDocument = new TypedDocumentString(`
    query Releases($seriesId: Int!, $season: Int!, $episodes: [Int!]!, $query: String) {
  series(id: $seriesId) {
    releases(season: $season, episodes: $episodes, query: $query) {
      ...ReleaseCandidateFields
    }
  }
}
    fragment ReleaseCandidateFields on ReleaseCandidate {
  release {
    title
    source
    link
    infoHash
    size
    seeders
    leechers
    published
    page
  }
  attributes {
    group
    resolution
    codec
    source
    dualAudio
    version
    proper
    tenBit
  }
  episodes {
    season
    episode
  }
  batch
  verdict {
    accepted
    score
    rejections
    warnings
    nonstandard
  }
}`) as unknown as TypedDocumentString<ReleasesQuery, ReleasesQueryVariables>;
export const GrabReleaseDocument = new TypedDocumentString(`
    mutation GrabRelease($release: ReleaseInput!, $seriesId: Int, $episodes: [EpisodeNumberInput!]!) {
  grabRelease(release: $release, seriesId: $seriesId, episodes: $episodes) {
    id
  }
}
    `) as unknown as TypedDocumentString<GrabReleaseMutation, GrabReleaseMutationVariables>;
export const DeleteDownloadedDocument = new TypedDocumentString(`
    mutation DeleteDownloaded($seriesId: Int!, $season: Int) {
  deleteDownloaded(seriesId: $seriesId, season: $season) {
    undone
    problems
  }
}
    `) as unknown as TypedDocumentString<DeleteDownloadedMutation, DeleteDownloadedMutationVariables>;
export const LookForAgainDocument = new TypedDocumentString(`
    mutation LookForAgain($seriesId: Int!, $season: Int, $episode: Int) {
  lookForAgain(seriesId: $seriesId, season: $season, episode: $episode)
}
    `) as unknown as TypedDocumentString<LookForAgainMutation, LookForAgainMutationVariables>;
export const LibrariesDocument = new TypedDocumentString(`
    query Libraries {
  libraries {
    name
    kind
    showCount
    movieCount
    albumCount
    trackCount
  }
}
    `) as unknown as TypedDocumentString<LibrariesQuery, LibrariesQueryVariables>;
export const SettingsDocument = new TypedDocumentString(`
    query Settings {
  settings {
    ...SettingsFields
  }
  server {
    transcoding {
      ...TranscodingFields
    }
  }
}
    fragment TranscodingFields on Transcoding {
  vaapi
  vaapiError
  softwareH264
}
fragment SeedingFields on Seeding {
  ratio
  time
  idle
  then
}
fragment SettingsFields on Settings {
  network {
    host
    port
    cors
  }
  log {
    level
  }
  scan {
    watch
    interval
  }
  metadata {
    tmdbApiKey
    language
  }
  transcode {
    hardware
    vaapiDevice
  }
  clips {
    enabled
    path
    publicLinks
    concurrency
    maxStorage
    fontsDir
    defaultFont
  }
  music {
    onlineLyrics
    lyricsUrl
    analyzeLoudness
  }
  downloads {
    path
    import
    port
    upnp
    dht
    maxActive
    downloadLimit
    uploadLimit
    slowDownloadLimit
    slowUploadLimit
    slowFrom
    slowTo
    bindInterface
    proxy
    seeding {
      ...SeedingFields
    }
  }
  automation {
    defaultMonitor
    rssInterval
    retry {
      every
      until
    }
    renameSuggestions
  }
  requests {
    monitor
  }
  signIn {
    style
  }
  sources {
    name
    kind
    url
    feed
    apiKey
    categories
    enabled
    downloadPath
    seeding {
      ...SeedingFields
    }
  }
  profiles {
    name
    resolutions
    groups
    require
    reject
    minSize
    maxSize
    codecs
    preferDualAudio
    batches
    minSeeders
  }
  libraries {
    name
    path
    kind
    metadataProvider
    managed
    profile
    downloadPath
    resolvedPath
    exists
    error
    titleCount
    skippedCount
  }
  raw
  error
  paths {
    config
    data
    log
  }
}`) as unknown as TypedDocumentString<SettingsQuery, SettingsQueryVariables>;
export const AppearanceDocument = new TypedDocumentString(`
    query Appearance {
  appearance {
    mode
    style
    mediaTint
    light {
      id
      palette {
        tokens {
          name
          value
        }
      }
    }
    dark {
      id
      palette {
        tokens {
          name
          value
        }
      }
    }
  }
}
    `) as unknown as TypedDocumentString<AppearanceQuery, AppearanceQueryVariables>;
export const ClipDocument = new TypedDocumentString(`
    query Clip($id: Int!) {
  clip(id: $id) {
    ...ClipFields
  }
}
    fragment Person on User {
  id
  username
  avatar
}
fragment ClipFields on Clip {
  id
  screenshot
  name
  mine
  canManage
  owner {
    ...Person
  }
  source {
    video {
      id
    }
    title {
      id
    }
    name
    kind
    label
    year
    status
  }
  start
  end
  audio
  subtitles
  quality {
    height
    halfRate
  }
  state
  progress
  error
  bytes
  width
  height
  fps
  renderedAt
  createdAt
  sharedAt
  public
  link
  linkLive
  recipients {
    user {
      ...Person
    }
    sharedAt
    hidden
  }
  file
  poster
}`) as unknown as TypedDocumentString<ClipQuery, ClipQueryVariables>;
export const RenderingClipsDocument = new TypedDocumentString(`
    query RenderingClips {
  clips(scope: RENDERING) {
    ...ClipFields
  }
}
    fragment Person on User {
  id
  username
  avatar
}
fragment ClipFields on Clip {
  id
  screenshot
  name
  mine
  canManage
  owner {
    ...Person
  }
  source {
    video {
      id
    }
    title {
      id
    }
    name
    kind
    label
    year
    status
  }
  start
  end
  audio
  subtitles
  quality {
    height
    halfRate
  }
  state
  progress
  error
  bytes
  width
  height
  fps
  renderedAt
  createdAt
  sharedAt
  public
  link
  linkLive
  recipients {
    user {
      ...Person
    }
    sharedAt
    hidden
  }
  file
  poster
}`) as unknown as TypedDocumentString<RenderingClipsQuery, RenderingClipsQueryVariables>;
export const StatusDocument = new TypedDocumentString(`
    query Status {
  server {
    setupRequired
    clips
    downloads
    sources
  }
  viewer {
    ...Viewer
  }
}
    fragment Person on User {
  id
  username
  avatar
}
fragment PermissionsFields on Permissions {
  allLibraries
  libraries
  request
  autoApprove
  requestLimit
  manageRequests
  manageShows
  downloads
  editMetadata
  watchTogether
  shareLinks
  clip
  clipMaxLength
  clipLimit
  clipStorage
  clipLinks
}
fragment Viewer on User {
  ...Person
  isAdmin
  permissions {
    ...PermissionsFields
  }
}`) as unknown as TypedDocumentString<StatusQuery, StatusQueryVariables>;
export const PeopleDocument = new TypedDocumentString(`
    query People {
  users {
    ...Person
  }
}
    fragment Person on User {
  id
  username
  avatar
}`) as unknown as TypedDocumentString<PeopleQuery, PeopleQueryVariables>;
export const InboxDocument = new TypedDocumentString(`
    query Inbox {
  notifications {
    ...InboxFields
  }
}
    fragment Person on User {
  id
  username
  avatar
}
fragment NotificationFields on Notification {
  id
  kind
  priority
  title
  body
  image
  link
  actor {
    ...Person
  }
  createdAt
  expiresAt
  readAt
}
fragment InboxFields on Inbox {
  items {
    ...NotificationFields
  }
  unread
}`) as unknown as TypedDocumentString<InboxQuery, InboxQueryVariables>;
export const MarkNotificationsReadDocument = new TypedDocumentString(`
    mutation MarkNotificationsRead($ids: [Int!]) {
  markNotificationsRead(ids: $ids) {
    ...InboxFields
  }
}
    fragment Person on User {
  id
  username
  avatar
}
fragment NotificationFields on Notification {
  id
  kind
  priority
  title
  body
  image
  link
  actor {
    ...Person
  }
  createdAt
  expiresAt
  readAt
}
fragment InboxFields on Inbox {
  items {
    ...NotificationFields
  }
  unread
}`) as unknown as TypedDocumentString<MarkNotificationsReadMutation, MarkNotificationsReadMutationVariables>;
export const DeleteNotificationsDocument = new TypedDocumentString(`
    mutation DeleteNotifications($id: Int) {
  deleteNotifications(id: $id) {
    ...InboxFields
  }
}
    fragment Person on User {
  id
  username
  avatar
}
fragment NotificationFields on Notification {
  id
  kind
  priority
  title
  body
  image
  link
  actor {
    ...Person
  }
  createdAt
  expiresAt
  readAt
}
fragment InboxFields on Inbox {
  items {
    ...NotificationFields
  }
  unread
}`) as unknown as TypedDocumentString<DeleteNotificationsMutation, DeleteNotificationsMutationVariables>;
export const LibraryAlbumsDocument = new TypedDocumentString(`
    query LibraryAlbums($library: String!, $sort: AlbumSort!) {
  albums(library: $library, sort: $sort, limit: 5000) {
    ...AlbumCard
  }
}
    fragment AlbumCard on Album {
  id
  name
  artist
  artists {
    id
    name
  }
  year
  cover
  trackCount
  duration
  compilation
  starred
  playCount
  addedAt
}`) as unknown as TypedDocumentString<LibraryAlbumsQuery, LibraryAlbumsQueryVariables>;
export const LibraryArtistsDocument = new TypedDocumentString(`
    query LibraryArtists($library: String!) {
  artists(library: $library) {
    ...ArtistCard
  }
}
    fragment ArtistCard on Artist {
  id
  name
  albumCount
  trackCount
  cover
  starred
}`) as unknown as TypedDocumentString<LibraryArtistsQuery, LibraryArtistsQueryVariables>;
export const LibrarySongsDocument = new TypedDocumentString(`
    query LibrarySongs($library: String!, $query: String!) {
  songs(library: $library, query: $query, limit: 300) {
    ...MusicTrack
  }
}
    fragment MusicTrack on Track {
  id
  title
  artist
  artists {
    id
    name
  }
  album
  albumId
  albumArtist
  library
  disc
  number
  year
  duration
  codec
  suffix
  lossless
  bitrate
  sampleRate
  bitDepth
  channels
  size
  file
  flac
  cover
  gains {
    trackGain
    trackPeak
    albumGain
    albumPeak
    pending
  }
  starred
  rating
  playCount
}`) as unknown as TypedDocumentString<LibrarySongsQuery, LibrarySongsQueryVariables>;
export const PlaylistsDocument = new TypedDocumentString(`
    query Playlists {
  playlists {
    ...PlaylistCard
  }
}
    fragment PlaylistCard on Playlist {
  id
  name
  comment
  public
  mine
  trackCount
  duration
  covers
  owner {
    id
    username
  }
}`) as unknown as TypedDocumentString<PlaylistsQuery, PlaylistsQueryVariables>;
export const PlayQueueDocument = new TypedDocumentString(`
    query PlayQueue {
  playQueue {
    tracks {
      ...MusicTrack
    }
    current
    position
    shuffled
    repeat
    changedBy
    updatedAt
  }
}
    fragment MusicTrack on Track {
  id
  title
  artist
  artists {
    id
    name
  }
  album
  albumId
  albumArtist
  library
  disc
  number
  year
  duration
  codec
  suffix
  lossless
  bitrate
  sampleRate
  bitDepth
  channels
  size
  file
  flac
  cover
  gains {
    trackGain
    trackPeak
    albumGain
    albumPeak
    pending
  }
  starred
  rating
  playCount
}`) as unknown as TypedDocumentString<PlayQueueQuery, PlayQueueQueryVariables>;
export const TrackDocument = new TypedDocumentString(`
    query Track($id: Int!) {
  track(id: $id) {
    ...MusicTrack
  }
}
    fragment MusicTrack on Track {
  id
  title
  artist
  artists {
    id
    name
  }
  album
  albumId
  albumArtist
  library
  disc
  number
  year
  duration
  codec
  suffix
  lossless
  bitrate
  sampleRate
  bitDepth
  channels
  size
  file
  flac
  cover
  gains {
    trackGain
    trackPeak
    albumGain
    albumPeak
    pending
  }
  starred
  rating
  playCount
}`) as unknown as TypedDocumentString<TrackQuery, TrackQueryVariables>;
export const SavePlayQueueDocument = new TypedDocumentString(`
    mutation SavePlayQueue($input: QueueInput!) {
  savePlayQueue(input: $input) {
    updatedAt
  }
}
    `) as unknown as TypedDocumentString<SavePlayQueueMutation, SavePlayQueueMutationVariables>;
export const MeasureLoudnessDocument = new TypedDocumentString(`
    mutation MeasureLoudness($trackId: Int!) {
  measureLoudness(trackId: $trackId) {
    ...MusicTrack
  }
}
    fragment MusicTrack on Track {
  id
  title
  artist
  artists {
    id
    name
  }
  album
  albumId
  albumArtist
  library
  disc
  number
  year
  duration
  codec
  suffix
  lossless
  bitrate
  sampleRate
  bitDepth
  channels
  size
  file
  flac
  cover
  gains {
    trackGain
    trackPeak
    albumGain
    albumPeak
    pending
  }
  starred
  rating
  playCount
}`) as unknown as TypedDocumentString<MeasureLoudnessMutation, MeasureLoudnessMutationVariables>;
export const PlayedDocument = new TypedDocumentString(`
    mutation Played($trackId: Int!) {
  played(trackId: $trackId)
}
    `) as unknown as TypedDocumentString<PlayedMutation, PlayedMutationVariables>;
export const NowPlayingDocument = new TypedDocumentString(`
    mutation NowPlaying($trackId: Int, $position: Float!, $paused: Boolean!) {
  nowPlaying(trackId: $trackId, position: $position, paused: $paused)
}
    `) as unknown as TypedDocumentString<NowPlayingMutation, NowPlayingMutationVariables>;
export const StarDocument = new TypedDocumentString(`
    mutation Star($kind: MusicKind!, $id: Int!, $starred: Boolean!) {
  star(kind: $kind, id: $id, starred: $starred)
}
    `) as unknown as TypedDocumentString<StarMutation, StarMutationVariables>;
export const LyricsDocument = new TypedDocumentString(`
    query Lyrics($trackId: Int!) {
  lyrics(trackId: $trackId) {
    synced
    source
    lines {
      start
      text
    }
  }
}
    `) as unknown as TypedDocumentString<LyricsQuery, LyricsQueryVariables>;
export const SimilarTracksDocument = new TypedDocumentString(`
    query SimilarTracks($trackId: Int!, $exclude: [Int!]!) {
  similarTracks(trackId: $trackId, count: 25, exclude: $exclude) {
    ...MusicTrack
  }
}
    fragment MusicTrack on Track {
  id
  title
  artist
  artists {
    id
    name
  }
  album
  albumId
  albumArtist
  library
  disc
  number
  year
  duration
  codec
  suffix
  lossless
  bitrate
  sampleRate
  bitDepth
  channels
  size
  file
  flac
  cover
  gains {
    trackGain
    trackPeak
    albumGain
    albumPeak
    pending
  }
  starred
  rating
  playCount
}`) as unknown as TypedDocumentString<SimilarTracksQuery, SimilarTracksQueryVariables>;
export const AlbumTracksDocument = new TypedDocumentString(`
    query AlbumTracks($id: Int!) {
  album(id: $id) {
    tracks {
      ...MusicTrack
    }
  }
}
    fragment MusicTrack on Track {
  id
  title
  artist
  artists {
    id
    name
  }
  album
  albumId
  albumArtist
  library
  disc
  number
  year
  duration
  codec
  suffix
  lossless
  bitrate
  sampleRate
  bitDepth
  channels
  size
  file
  flac
  cover
  gains {
    trackGain
    trackPeak
    albumGain
    albumPeak
    pending
  }
  starred
  rating
  playCount
}`) as unknown as TypedDocumentString<AlbumTracksQuery, AlbumTracksQueryVariables>;
export const PlaylistNamesDocument = new TypedDocumentString(`
    query PlaylistNames {
  playlists {
    id
    name
    mine
  }
}
    `) as unknown as TypedDocumentString<PlaylistNamesQuery, PlaylistNamesQueryVariables>;
export const AddToPlaylistDocument = new TypedDocumentString(`
    mutation AddToPlaylist($id: Int!, $tracks: [Int!]!) {
  addToPlaylist(id: $id, tracks: $tracks) {
    id
    name
  }
}
    `) as unknown as TypedDocumentString<AddToPlaylistMutation, AddToPlaylistMutationVariables>;
export const CreatePlaylistDocument = new TypedDocumentString(`
    mutation CreatePlaylist($name: String!, $tracks: [Int!]!) {
  createPlaylist(name: $name, tracks: $tracks) {
    id
    name
  }
}
    `) as unknown as TypedDocumentString<CreatePlaylistMutation, CreatePlaylistMutationVariables>;
export const StartListenRoomDocument = new TypedDocumentString(`
    mutation StartListenRoom($input: NewListenRoom!) {
  startListenRoom(input: $input) {
    code
  }
}
    `) as unknown as TypedDocumentString<StartListenRoomMutation, StartListenRoomMutationVariables>;
export const ClipAllowanceDocument = new TypedDocumentString(`
    query ClipAllowance {
  clipAllowance {
    ...ClipAllowanceFields
  }
}
    fragment ClipAllowanceFields on ClipAllowance {
  canClip
  canLink
  maxLength
  bytes
  rendered
  storage
  limit
  customDefaultFont
}`) as unknown as TypedDocumentString<ClipAllowanceQuery, ClipAllowanceQueryVariables>;
export const UpdateClipDocument = new TypedDocumentString(`
    mutation UpdateClip($id: Int!, $input: ClipPatch!) {
  updateClip(id: $id, input: $input) {
    ...ClipFields
  }
}
    fragment Person on User {
  id
  username
  avatar
}
fragment ClipFields on Clip {
  id
  screenshot
  name
  mine
  canManage
  owner {
    ...Person
  }
  source {
    video {
      id
    }
    title {
      id
    }
    name
    kind
    label
    year
    status
  }
  start
  end
  audio
  subtitles
  quality {
    height
    halfRate
  }
  state
  progress
  error
  bytes
  width
  height
  fps
  renderedAt
  createdAt
  sharedAt
  public
  link
  linkLive
  recipients {
    user {
      ...Person
    }
    sharedAt
    hidden
  }
  file
  poster
}`) as unknown as TypedDocumentString<UpdateClipMutation, UpdateClipMutationVariables>;
export const ShareClipDocument = new TypedDocumentString(`
    mutation ShareClip($id: Int!, $users: [Int!]!) {
  shareClip(id: $id, users: $users) {
    ...ClipFields
  }
}
    fragment Person on User {
  id
  username
  avatar
}
fragment ClipFields on Clip {
  id
  screenshot
  name
  mine
  canManage
  owner {
    ...Person
  }
  source {
    video {
      id
    }
    title {
      id
    }
    name
    kind
    label
    year
    status
  }
  start
  end
  audio
  subtitles
  quality {
    height
    halfRate
  }
  state
  progress
  error
  bytes
  width
  height
  fps
  renderedAt
  createdAt
  sharedAt
  public
  link
  linkLive
  recipients {
    user {
      ...Person
    }
    sharedAt
    hidden
  }
  file
  poster
}`) as unknown as TypedDocumentString<ShareClipMutation, ShareClipMutationVariables>;
export const CreateClipDocument = new TypedDocumentString(`
    mutation CreateClip($input: NewClip!) {
  createClip(input: $input) {
    ...ClipFields
  }
}
    fragment Person on User {
  id
  username
  avatar
}
fragment ClipFields on Clip {
  id
  screenshot
  name
  mine
  canManage
  owner {
    ...Person
  }
  source {
    video {
      id
    }
    title {
      id
    }
    name
    kind
    label
    year
    status
  }
  start
  end
  audio
  subtitles
  quality {
    height
    halfRate
  }
  state
  progress
  error
  bytes
  width
  height
  fps
  renderedAt
  createdAt
  sharedAt
  public
  link
  linkLive
  recipients {
    user {
      ...Person
    }
    sharedAt
    hidden
  }
  file
  poster
}`) as unknown as TypedDocumentString<CreateClipMutation, CreateClipMutationVariables>;
export const PlaybackDocument = new TypedDocumentString(`
    query Playback($id: Int!) {
  video(id: $id) {
    ...Playback
  }
  server {
    transcoding {
      ...TranscodingFields
    }
  }
}
    fragment Playback on Video {
  id
  still
  label
  name
  position
  finished
  title {
    id
    kind
    name
    backdrop
  }
  previous {
    id
    label
    name
  }
  next {
    id
    still
    label
    name
  }
  media {
    duration
    video {
      index
      codec
      codecString
      width
      height
      fps
      bitDepth
      hdr
    }
    audio {
      index
      codec
      codecString
      channels
      language
      title
      default
    }
    subtitles {
      id
      codec
      language
      title
      default
      forced
      supported
    }
    fonts {
      index
      filename
    }
    chapters {
      start
      end
      title
    }
  }
}
fragment TranscodingFields on Transcoding {
  vaapi
  vaapiError
  softwareH264
}`) as unknown as TypedDocumentString<PlaybackQuery, PlaybackQueryVariables>;
export const RoomPlaybackDocument = new TypedDocumentString(`
    query RoomPlayback($code: String!, $id: Int!) {
  room(code: $code) {
    video(id: $id) {
      ...Playback
    }
  }
  server {
    transcoding {
      ...TranscodingFields
    }
  }
}
    fragment Playback on Video {
  id
  still
  label
  name
  position
  finished
  title {
    id
    kind
    name
    backdrop
  }
  previous {
    id
    label
    name
  }
  next {
    id
    still
    label
    name
  }
  media {
    duration
    video {
      index
      codec
      codecString
      width
      height
      fps
      bitDepth
      hdr
    }
    audio {
      index
      codec
      codecString
      channels
      language
      title
      default
    }
    subtitles {
      id
      codec
      language
      title
      default
      forced
      supported
    }
    fonts {
      index
      filename
    }
    chapters {
      start
      end
      title
    }
  }
}
fragment TranscodingFields on Transcoding {
  vaapi
  vaapiError
  softwareH264
}`) as unknown as TypedDocumentString<RoomPlaybackQuery, RoomPlaybackQueryVariables>;
export const SaveProgressDocument = new TypedDocumentString(`
    mutation SaveProgress($videoId: Int!, $position: Float!, $duration: Float!) {
  saveProgress(videoId: $videoId, position: $position, duration: $duration) {
    id
  }
}
    `) as unknown as TypedDocumentString<SaveProgressMutation, SaveProgressMutationVariables>;
export const TakeScreenshotDocument = new TypedDocumentString(`
    mutation TakeScreenshot($input: NewScreenshot!) {
  takeScreenshot(input: $input) {
    ...ClipFields
  }
}
    fragment Person on User {
  id
  username
  avatar
}
fragment ClipFields on Clip {
  id
  screenshot
  name
  mine
  canManage
  owner {
    ...Person
  }
  source {
    video {
      id
    }
    title {
      id
    }
    name
    kind
    label
    year
    status
  }
  start
  end
  audio
  subtitles
  quality {
    height
    halfRate
  }
  state
  progress
  error
  bytes
  width
  height
  fps
  renderedAt
  createdAt
  sharedAt
  public
  link
  linkLive
  recipients {
    user {
      ...Person
    }
    sharedAt
    hidden
  }
  file
  poster
}`) as unknown as TypedDocumentString<TakeScreenshotMutation, TakeScreenshotMutationVariables>;
export const StartRoomDocument = new TypedDocumentString(`
    mutation StartRoom($input: NewRoom!) {
  startRoom(input: $input) {
    code
  }
}
    `) as unknown as TypedDocumentString<StartRoomMutation, StartRoomMutationVariables>;
export const PlayerScheduleDocument = new TypedDocumentString(`
    query PlayerSchedule($id: Int!) {
  title(id: $id) {
    series {
      id
      monitor
      status
      next {
        ...SeriesEpisodeFields
      }
    }
  }
}
    fragment SeriesEpisodeFields on SeriesEpisode {
  season
  episode
  absolute
  name
  airAt
  aired
  state
  attempts
  searchedAt
  nextSearch
  downloadId
  video {
    id
  }
}`) as unknown as TypedDocumentString<PlayerScheduleQuery, PlayerScheduleQueryVariables>;
export const PlayerOverviewDocument = new TypedDocumentString(`
    query PlayerOverview($id: Int!, $videoId: Int!) {
  title(id: $id) {
    overview
  }
  video(id: $videoId) {
    overview
  }
}
    `) as unknown as TypedDocumentString<PlayerOverviewQuery, PlayerOverviewQueryVariables>;
export const RoomDocument = new TypedDocumentString(`
    query Room($code: String!) {
  room(code: $code) {
    code
    signedIn
    isHost
    canShare
    canInvite
    title {
      id
      name
      backdrop
    }
  }
}
    `) as unknown as TypedDocumentString<RoomQuery, RoomQueryVariables>;
export const EventsDocument = new TypedDocumentString(`
    subscription Events {
  events {
    __typename
    ... on ConfigChanged {
      error
    }
    ... on ScanFinished {
      library
    }
    ... on LibraryChanged {
      library
    }
    ... on MetadataChanged {
      titleId
      status
    }
    ... on ListChanged {
      list
    }
    ... on SeriesChanged {
      seriesId
    }
    ... on EpisodesImported {
      library
    }
    ... on NotificationReceived {
      notification {
        ...NotificationFields
      }
    }
    ... on ClipChanged {
      clipId
      state
      progress
    }
    ... on QueueChanged {
      by
    }
    ... on PlaybackChanged {
      client
      trackId
      position
      paused
    }
  }
}
    fragment Person on User {
  id
  username
  avatar
}
fragment NotificationFields on Notification {
  id
  kind
  priority
  title
  body
  image
  link
  actor {
    ...Person
  }
  createdAt
  expiresAt
  readAt
}`) as unknown as TypedDocumentString<EventsSubscription, EventsSubscriptionVariables>;
export const AlbumDocument = new TypedDocumentString(`
    query Album($id: Int!) {
  album(id: $id) {
    ...AlbumCard
    library
    releaseDate
    originalDate
    genres
    releaseTypes
    labels
    discTitles {
      disc
      title
    }
    tracks {
      ...MusicTrack
    }
  }
}
    fragment MusicTrack on Track {
  id
  title
  artist
  artists {
    id
    name
  }
  album
  albumId
  albumArtist
  library
  disc
  number
  year
  duration
  codec
  suffix
  lossless
  bitrate
  sampleRate
  bitDepth
  channels
  size
  file
  flac
  cover
  gains {
    trackGain
    trackPeak
    albumGain
    albumPeak
    pending
  }
  starred
  rating
  playCount
}
fragment AlbumCard on Album {
  id
  name
  artist
  artists {
    id
    name
  }
  year
  cover
  trackCount
  duration
  compilation
  starred
  playCount
  addedAt
}`) as unknown as TypedDocumentString<AlbumQuery, AlbumQueryVariables>;
export const MoreByArtistDocument = new TypedDocumentString(`
    query MoreByArtist($id: Int!) {
  artist(id: $id) {
    albums {
      ...AlbumCard
    }
  }
}
    fragment AlbumCard on Album {
  id
  name
  artist
  artists {
    id
    name
  }
  year
  cover
  trackCount
  duration
  compilation
  starred
  playCount
  addedAt
}`) as unknown as TypedDocumentString<MoreByArtistQuery, MoreByArtistQueryVariables>;
export const ArtistDocument = new TypedDocumentString(`
    query Artist($id: Int!) {
  artist(id: $id) {
    ...ArtistCard
    albums {
      ...AlbumCard
      releaseTypes
    }
    appearsOn {
      ...AlbumCard
    }
    topTracks(count: 200) {
      ...MusicTrack
    }
  }
}
    fragment MusicTrack on Track {
  id
  title
  artist
  artists {
    id
    name
  }
  album
  albumId
  albumArtist
  library
  disc
  number
  year
  duration
  codec
  suffix
  lossless
  bitrate
  sampleRate
  bitDepth
  channels
  size
  file
  flac
  cover
  gains {
    trackGain
    trackPeak
    albumGain
    albumPeak
    pending
  }
  starred
  rating
  playCount
}
fragment AlbumCard on Album {
  id
  name
  artist
  artists {
    id
    name
  }
  year
  cover
  trackCount
  duration
  compilation
  starred
  playCount
  addedAt
}
fragment ArtistCard on Artist {
  id
  name
  albumCount
  trackCount
  cover
  starred
}`) as unknown as TypedDocumentString<ArtistQuery, ArtistQueryVariables>;
export const CalendarDocument = new TypedDocumentString(`
    query Calendar($from: Int!, $to: Int!) {
  calendar(from: $from, to: $to) {
    ...CalendarEntryFields
  }
}
    fragment CalendarEntryFields on CalendarEntry {
  seriesId
  title {
    id
  }
  library
  show
  poster
  backdrop
  monitor
  season
  episode
  absolute
  name
  airAt
  state
  video {
    id
  }
  download {
    stage
    progress
    downloadRate
    eta
  }
}`) as unknown as TypedDocumentString<CalendarQuery, CalendarQueryVariables>;
export const ClipsDocument = new TypedDocumentString(`
    query Clips($scope: ClipScope!) {
  clips(scope: $scope) {
    ...ClipFields
  }
  clipAllowance {
    ...ClipAllowanceFields
  }
}
    fragment Person on User {
  id
  username
  avatar
}
fragment ClipFields on Clip {
  id
  screenshot
  name
  mine
  canManage
  owner {
    ...Person
  }
  source {
    video {
      id
    }
    title {
      id
    }
    name
    kind
    label
    year
    status
  }
  start
  end
  audio
  subtitles
  quality {
    height
    halfRate
  }
  state
  progress
  error
  bytes
  width
  height
  fps
  renderedAt
  createdAt
  sharedAt
  public
  link
  linkLive
  recipients {
    user {
      ...Person
    }
    sharedAt
    hidden
  }
  file
  poster
}
fragment ClipAllowanceFields on ClipAllowance {
  canClip
  canLink
  maxLength
  bytes
  rendered
  storage
  limit
  customDefaultFont
}`) as unknown as TypedDocumentString<ClipsQuery, ClipsQueryVariables>;
export const RenderClipDocument = new TypedDocumentString(`
    mutation RenderClip($id: Int!) {
  renderClip(id: $id) {
    ...ClipFields
  }
}
    fragment Person on User {
  id
  username
  avatar
}
fragment ClipFields on Clip {
  id
  screenshot
  name
  mine
  canManage
  owner {
    ...Person
  }
  source {
    video {
      id
    }
    title {
      id
    }
    name
    kind
    label
    year
    status
  }
  start
  end
  audio
  subtitles
  quality {
    height
    halfRate
  }
  state
  progress
  error
  bytes
  width
  height
  fps
  renderedAt
  createdAt
  sharedAt
  public
  link
  linkLive
  recipients {
    user {
      ...Person
    }
    sharedAt
    hidden
  }
  file
  poster
}`) as unknown as TypedDocumentString<RenderClipMutation, RenderClipMutationVariables>;
export const ClipsUpdateClipDocument = new TypedDocumentString(`
    mutation ClipsUpdateClip($id: Int!, $input: ClipPatch!) {
  updateClip(id: $id, input: $input) {
    ...ClipFields
  }
}
    fragment Person on User {
  id
  username
  avatar
}
fragment ClipFields on Clip {
  id
  screenshot
  name
  mine
  canManage
  owner {
    ...Person
  }
  source {
    video {
      id
    }
    title {
      id
    }
    name
    kind
    label
    year
    status
  }
  start
  end
  audio
  subtitles
  quality {
    height
    halfRate
  }
  state
  progress
  error
  bytes
  width
  height
  fps
  renderedAt
  createdAt
  sharedAt
  public
  link
  linkLive
  recipients {
    user {
      ...Person
    }
    sharedAt
    hidden
  }
  file
  poster
}`) as unknown as TypedDocumentString<ClipsUpdateClipMutation, ClipsUpdateClipMutationVariables>;
export const ClipsShareClipDocument = new TypedDocumentString(`
    mutation ClipsShareClip($id: Int!, $users: [Int!]!) {
  shareClip(id: $id, users: $users) {
    ...ClipFields
  }
}
    fragment Person on User {
  id
  username
  avatar
}
fragment ClipFields on Clip {
  id
  screenshot
  name
  mine
  canManage
  owner {
    ...Person
  }
  source {
    video {
      id
    }
    title {
      id
    }
    name
    kind
    label
    year
    status
  }
  start
  end
  audio
  subtitles
  quality {
    height
    halfRate
  }
  state
  progress
  error
  bytes
  width
  height
  fps
  renderedAt
  createdAt
  sharedAt
  public
  link
  linkLive
  recipients {
    user {
      ...Person
    }
    sharedAt
    hidden
  }
  file
  poster
}`) as unknown as TypedDocumentString<ClipsShareClipMutation, ClipsShareClipMutationVariables>;
export const UnshareClipDocument = new TypedDocumentString(`
    mutation UnshareClip($id: Int!, $userId: Int!) {
  unshareClip(id: $id, userId: $userId) {
    ...ClipFields
  }
}
    fragment Person on User {
  id
  username
  avatar
}
fragment ClipFields on Clip {
  id
  screenshot
  name
  mine
  canManage
  owner {
    ...Person
  }
  source {
    video {
      id
    }
    title {
      id
    }
    name
    kind
    label
    year
    status
  }
  start
  end
  audio
  subtitles
  quality {
    height
    halfRate
  }
  state
  progress
  error
  bytes
  width
  height
  fps
  renderedAt
  createdAt
  sharedAt
  public
  link
  linkLive
  recipients {
    user {
      ...Person
    }
    sharedAt
    hidden
  }
  file
  poster
}`) as unknown as TypedDocumentString<UnshareClipMutation, UnshareClipMutationVariables>;
export const DeleteClipDocument = new TypedDocumentString(`
    mutation DeleteClip($id: Int!) {
  deleteClip(id: $id)
}
    `) as unknown as TypedDocumentString<DeleteClipMutation, DeleteClipMutationVariables>;
export const HideClipDocument = new TypedDocumentString(`
    mutation HideClip($id: Int!) {
  hideClip(id: $id)
}
    `) as unknown as TypedDocumentString<HideClipMutation, HideClipMutationVariables>;
export const DiscoverDocument = new TypedDocumentString(`
    query Discover($library: String, $query: String) {
  discover(library: $library, query: $query) {
    library
    results {
      ...DiscoverResultFields
    }
  }
}
    fragment DiscoverResultFields on DiscoverResult {
  category
  provider
  id
  name
  romaji
  year
  poster
  overview
  library
  titleId
  seriesId
  monitor
  requestState
  because
}`) as unknown as TypedDocumentString<DiscoverQuery, DiscoverQueryVariables>;
export const ForYouDocument = new TypedDocumentString(`
    query ForYou($library: String) {
  forYou(library: $library) {
    library
    shelves {
      key
      name
      results {
        ...DiscoverResultFields
      }
    }
  }
}
    fragment DiscoverResultFields on DiscoverResult {
  category
  provider
  id
  name
  romaji
  year
  poster
  overview
  library
  titleId
  seriesId
  monitor
  requestState
  because
}`) as unknown as TypedDocumentString<ForYouQuery, ForYouQueryVariables>;
export const DownloadsDocument = new TypedDocumentString(`
    query Downloads {
  downloads {
    ...DownloadFields
  }
}
    fragment DownloadFields on Download {
  category
  id
  name
  seriesId
  seriesName
  title {
    id
  }
  poster
  episodes {
    season
    episode
  }
  source
  size
  savePath
  state
  importState
  importError
  importMode
  error
  addedAt
  finishedAt
  importedAt
  requestedBy {
    username
  }
  live {
    stage
    paused
    progress
    downloadRate
    uploadRate
    done
    uploaded
    ratio
    peers
    seeds
    seedingSeconds
    eta
    pieces
  }
  seedGoal {
    ratio
    seconds
  }
}`) as unknown as TypedDocumentString<DownloadsQuery, DownloadsQueryVariables>;
export const EngineDocument = new TypedDocumentString(`
    query Engine {
  downloadEngine {
    ...EngineFields
  }
}
    fragment EngineFields on DownloadEngine {
  version
  downloadRate
  uploadRate
  active
  killSwitch
  listening
  listenError
  slowHours
  downloadPath
}`) as unknown as TypedDocumentString<EngineQuery, EngineQueryVariables>;
export const DownloadsPauseDocument = new TypedDocumentString(`
    mutation DownloadsPause($ids: [Int!]!) {
  pauseDownloads(ids: $ids) {
    id
  }
}
    `) as unknown as TypedDocumentString<DownloadsPauseMutation, DownloadsPauseMutationVariables>;
export const DownloadsResumeDocument = new TypedDocumentString(`
    mutation DownloadsResume($ids: [Int!]!) {
  resumeDownloads(ids: $ids) {
    id
  }
}
    `) as unknown as TypedDocumentString<DownloadsResumeMutation, DownloadsResumeMutationVariables>;
export const DownloadsRecheckDocument = new TypedDocumentString(`
    mutation DownloadsRecheck($ids: [Int!]!) {
  recheckDownloads(ids: $ids) {
    id
  }
}
    `) as unknown as TypedDocumentString<DownloadsRecheckMutation, DownloadsRecheckMutationVariables>;
export const ImportDownloadDocument = new TypedDocumentString(`
    mutation ImportDownload($id: Int!) {
  importDownload(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<ImportDownloadMutation, ImportDownloadMutationVariables>;
export const RemoveDownloadsDocument = new TypedDocumentString(`
    mutation RemoveDownloads($ids: [Int!]!, $deleteFiles: Boolean!) {
  removeDownloads(ids: $ids, deleteFiles: $deleteFiles)
}
    `) as unknown as TypedDocumentString<RemoveDownloadsMutation, RemoveDownloadsMutationVariables>;
export const HomeDocument = new TypedDocumentString(`
    query Home {
  home {
    continueWatching {
      position
      upNext
      newEpisode
      watchedAt
      video {
        id
        label
        name
        still
        duration
        title {
          id
          name
          poster
          backdrop
        }
      }
    }
    recentlyAdded {
      library
      titles {
        ...Card
      }
    }
    popularHere {
      people
      title {
        ...Card
      }
    }
  }
}
    fragment Card on Title {
  id
  kind
  library
  name
  year
  poster
  backdrop
  watchedCount
  videoCount
  progress
  freshCount
}`) as unknown as TypedDocumentString<HomeQuery, HomeQueryVariables>;
export const ComingUpDocument = new TypedDocumentString(`
    query ComingUp($from: Int!, $to: Int!) {
  calendar(from: $from, to: $to) {
    ...CalendarEntryFields
  }
}
    fragment CalendarEntryFields on CalendarEntry {
  seriesId
  title {
    id
  }
  library
  show
  poster
  backdrop
  monitor
  season
  episode
  absolute
  name
  airAt
  state
  video {
    id
  }
  download {
    stage
    progress
    downloadRate
    eta
  }
}`) as unknown as TypedDocumentString<ComingUpQuery, ComingUpQueryVariables>;
export const MusicHomeDocument = new TypedDocumentString(`
    query MusicHome {
  musicHome {
    recentlyPlayed {
      ...AlbumCard
    }
    recentlyAdded {
      ...AlbumCard
    }
  }
}
    fragment AlbumCard on Album {
  id
  name
  artist
  artists {
    id
    name
  }
  year
  cover
  trackCount
  duration
  compilation
  starred
  playCount
  addedAt
}`) as unknown as TypedDocumentString<MusicHomeQuery, MusicHomeQueryVariables>;
export const InviteDocument = new TypedDocumentString(`
    query Invite($token: String!) {
  invite(token: $token) {
    expiresAt
    remainingUses
  }
}
    `) as unknown as TypedDocumentString<InviteQuery, InviteQueryVariables>;
export const AcceptInviteDocument = new TypedDocumentString(`
    mutation AcceptInvite($token: String!, $username: String!, $password: String!) {
  acceptInvite(token: $token, username: $username, password: $password) {
    user {
      id
    }
  }
}
    `) as unknown as TypedDocumentString<AcceptInviteMutation, AcceptInviteMutationVariables>;
export const LibraryDocument = new TypedDocumentString(`
    query Library($name: String!) {
  library(name: $name) {
    titles {
      ...Card
    }
  }
}
    fragment Card on Title {
  id
  kind
  library
  name
  year
  poster
  backdrop
  watchedCount
  videoCount
  progress
  freshCount
}`) as unknown as TypedDocumentString<LibraryQuery, LibraryQueryVariables>;
export const ListenRoomDocument = new TypedDocumentString(`
    query ListenRoom($code: String!) {
  listenRoom(code: $code) {
    code
    hostName
    signedIn
    isHost
    canShare
    canInvite
    tracks {
      ...MusicTrack
    }
  }
}
    fragment MusicTrack on Track {
  id
  title
  artist
  artists {
    id
    name
  }
  album
  albumId
  albumArtist
  library
  disc
  number
  year
  duration
  codec
  suffix
  lossless
  bitrate
  sampleRate
  bitDepth
  channels
  size
  file
  flac
  cover
  gains {
    trackGain
    trackPeak
    albumGain
    albumPeak
    pending
  }
  starred
  rating
  playCount
}`) as unknown as TypedDocumentString<ListenRoomQuery, ListenRoomQueryVariables>;
export const PlaylistDocument = new TypedDocumentString(`
    query Playlist($id: Int!) {
  playlist(id: $id) {
    ...PlaylistCard
    tracks {
      ...MusicTrack
    }
  }
}
    fragment MusicTrack on Track {
  id
  title
  artist
  artists {
    id
    name
  }
  album
  albumId
  albumArtist
  library
  disc
  number
  year
  duration
  codec
  suffix
  lossless
  bitrate
  sampleRate
  bitDepth
  channels
  size
  file
  flac
  cover
  gains {
    trackGain
    trackPeak
    albumGain
    albumPeak
    pending
  }
  starred
  rating
  playCount
}
fragment PlaylistCard on Playlist {
  id
  name
  comment
  public
  mine
  trackCount
  duration
  covers
  owner {
    id
    username
  }
}`) as unknown as TypedDocumentString<PlaylistQuery, PlaylistQueryVariables>;
export const UpdatePlaylistDocument = new TypedDocumentString(`
    mutation UpdatePlaylist($id: Int!, $input: PlaylistInput!) {
  updatePlaylist(id: $id, input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<UpdatePlaylistMutation, UpdatePlaylistMutationVariables>;
export const DeletePlaylistDocument = new TypedDocumentString(`
    mutation DeletePlaylist($id: Int!) {
  deletePlaylist(id: $id)
}
    `) as unknown as TypedDocumentString<DeletePlaylistMutation, DeletePlaylistMutationVariables>;
export const RequestsDocument = new TypedDocumentString(`
    query Requests {
  requests {
    id
    user {
      ...Person
    }
    name
    year
    poster
    library
    state
    title {
      id
    }
    note
    createdAt
    have
    aired
  }
}
    fragment Person on User {
  id
  username
  avatar
}`) as unknown as TypedDocumentString<RequestsQuery, RequestsQueryVariables>;
export const ApproveRequestDocument = new TypedDocumentString(`
    mutation ApproveRequest($id: Int!) {
  approveRequest(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<ApproveRequestMutation, ApproveRequestMutationVariables>;
export const DeclineRequestDocument = new TypedDocumentString(`
    mutation DeclineRequest($id: Int!) {
  declineRequest(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<DeclineRequestMutation, DeclineRequestMutationVariables>;
export const DeleteRequestDocument = new TypedDocumentString(`
    mutation DeleteRequest($id: Int!) {
  deleteRequest(id: $id)
}
    `) as unknown as TypedDocumentString<DeleteRequestMutation, DeleteRequestMutationVariables>;
export const ScanDocument = new TypedDocumentString(`
    mutation Scan($library: String) {
  scan(library: $library)
}
    `) as unknown as TypedDocumentString<ScanMutation, ScanMutationVariables>;
export const AddLibraryDocument = new TypedDocumentString(`
    mutation AddLibrary($input: LibraryInput!) {
  addLibrary(input: $input) {
    raw
  }
}
    `) as unknown as TypedDocumentString<AddLibraryMutation, AddLibraryMutationVariables>;
export const UpdateLibraryDocument = new TypedDocumentString(`
    mutation UpdateLibrary($name: String!, $input: LibraryInput!) {
  updateLibrary(name: $name, input: $input) {
    raw
  }
}
    `) as unknown as TypedDocumentString<UpdateLibraryMutation, UpdateLibraryMutationVariables>;
export const RemoveLibraryDocument = new TypedDocumentString(`
    mutation RemoveLibrary($name: String!) {
  removeLibrary(name: $name) {
    raw
  }
}
    `) as unknown as TypedDocumentString<RemoveLibraryMutation, RemoveLibraryMutationVariables>;
export const FoldersDocument = new TypedDocumentString(`
    query Folders($path: String) {
  folders(path: $path) {
    path
    parent
    home
    folders {
      name
      path
    }
  }
}
    `) as unknown as TypedDocumentString<FoldersQuery, FoldersQueryVariables>;
export const SaveServerDocument = new TypedDocumentString(`
    mutation SaveServer($patch: ConfigPatch!) {
  updateSettings(patch: $patch) {
    raw
  }
}
    `) as unknown as TypedDocumentString<SaveServerMutation, SaveServerMutationVariables>;
export const PasskeysDocument = new TypedDocumentString(`
    query Passkeys {
  viewer {
    passkeys {
      id
      name
      createdAt
      lastUsed
    }
  }
}
    `) as unknown as TypedDocumentString<PasskeysQuery, PasskeysQueryVariables>;
export const StartPasskeyRegistrationDocument = new TypedDocumentString(`
    mutation StartPasskeyRegistration($name: String) {
  startPasskeyRegistration(name: $name) {
    challenge
    options
  }
}
    `) as unknown as TypedDocumentString<StartPasskeyRegistrationMutation, StartPasskeyRegistrationMutationVariables>;
export const FinishPasskeyRegistrationDocument = new TypedDocumentString(`
    mutation FinishPasskeyRegistration($challenge: String!, $credential: JSON!) {
  finishPasskeyRegistration(challenge: $challenge, credential: $credential) {
    id
  }
}
    `) as unknown as TypedDocumentString<FinishPasskeyRegistrationMutation, FinishPasskeyRegistrationMutationVariables>;
export const DeletePasskeyDocument = new TypedDocumentString(`
    mutation DeletePasskey($id: Int!) {
  deletePasskey(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<DeletePasskeyMutation, DeletePasskeyMutationVariables>;
export const ChangePasswordDocument = new TypedDocumentString(`
    mutation ChangePassword($current: String!, $new: String!) {
  changePassword(current: $current, new: $new)
}
    `) as unknown as TypedDocumentString<ChangePasswordMutation, ChangePasswordMutationVariables>;
export const ReplaceConfigDocument = new TypedDocumentString(`
    mutation ReplaceConfig($text: String!) {
  replaceConfig(text: $text) {
    raw
  }
}
    `) as unknown as TypedDocumentString<ReplaceConfigMutation, ReplaceConfigMutationVariables>;
export const SkippedFilesDocument = new TypedDocumentString(`
    query SkippedFiles {
  skippedFiles {
    library
    path
    reason
  }
}
    `) as unknown as TypedDocumentString<SkippedFilesQuery, SkippedFilesQueryVariables>;
export const TitleDocument = new TypedDocumentString(`
    query Title($id: Int!) {
  title(id: $id) {
    ...TitleDetail
  }
}
    fragment Card on Title {
  id
  kind
  library
  name
  year
  poster
  backdrop
  watchedCount
  videoCount
  progress
  freshCount
}
fragment VideoRow on Video {
  id
  season
  episode
  episodeEnd
  label
  name
  overview
  still
  customStill
  airDate
  duration
  position
  finished
}
fragment TitleDetail on Title {
  ...Card
  customPoster
  customBackdrop
  overview
  genres
  rating
  path
  matchState
  provider
  providerId
  libraryProvider
  seasons {
    number
    name
    title
    overview
    poster
    episodes {
      ...VideoRow
    }
  }
  movie {
    ...VideoRow
  }
  nextUp {
    resuming
    video {
      ...VideoRow
    }
  }
}`) as unknown as TypedDocumentString<TitleQuery, TitleQueryVariables>;
export const SimilarDocument = new TypedDocumentString(`
    query Similar($id: Int!) {
  title(id: $id) {
    similar {
      recommendations {
        ...DiscoverResultFields
      }
      alsoWatched {
        ...Card
      }
    }
  }
}
    fragment Card on Title {
  id
  kind
  library
  name
  year
  poster
  backdrop
  watchedCount
  videoCount
  progress
  freshCount
}
fragment DiscoverResultFields on DiscoverResult {
  category
  provider
  id
  name
  romaji
  year
  poster
  overview
  library
  titleId
  seriesId
  monitor
  requestState
  because
}`) as unknown as TypedDocumentString<SimilarQuery, SimilarQueryVariables>;
export const MatchCandidatesDocument = new TypedDocumentString(`
    query MatchCandidates($id: Int!, $query: String, $provider: Provider) {
  title(id: $id) {
    matchCandidates(query: $query, provider: $provider) {
      query
      results {
        provider
        id
        name
        year
        poster
        overview
      }
    }
  }
}
    `) as unknown as TypedDocumentString<MatchCandidatesQuery, MatchCandidatesQueryVariables>;
export const TitleDownloadsDocument = new TypedDocumentString(`
    query TitleDownloads {
  downloads {
    ...DownloadFields
  }
}
    fragment DownloadFields on Download {
  category
  id
  name
  seriesId
  seriesName
  title {
    id
  }
  poster
  episodes {
    season
    episode
  }
  source
  size
  savePath
  state
  importState
  importError
  importMode
  error
  addedAt
  finishedAt
  importedAt
  requestedBy {
    username
  }
  live {
    stage
    paused
    progress
    downloadRate
    uploadRate
    done
    uploaded
    ratio
    peers
    seeds
    seedingSeconds
    eta
    pieces
  }
  seedGoal {
    ratio
    seconds
  }
}`) as unknown as TypedDocumentString<TitleDownloadsQuery, TitleDownloadsQueryVariables>;
export const SetWatchedDocument = new TypedDocumentString(`
    mutation SetWatched($videoIds: [Int!]!, $watched: Boolean!) {
  setWatched(videoIds: $videoIds, watched: $watched) {
    id
  }
}
    `) as unknown as TypedDocumentString<SetWatchedMutation, SetWatchedMutationVariables>;
export const SetTitleWatchedDocument = new TypedDocumentString(`
    mutation SetTitleWatched($id: Int!, $watched: Boolean!) {
  setTitleWatched(id: $id, watched: $watched) {
    id
  }
}
    `) as unknown as TypedDocumentString<SetTitleWatchedMutation, SetTitleWatchedMutationVariables>;
export const RefreshTitleDocument = new TypedDocumentString(`
    mutation RefreshTitle($id: Int!) {
  refreshTitle(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<RefreshTitleMutation, RefreshTitleMutationVariables>;
export const MatchTitleDocument = new TypedDocumentString(`
    mutation MatchTitle($id: Int!, $provider: Provider!, $providerId: String!) {
  matchTitle(id: $id, provider: $provider, providerId: $providerId) {
    id
  }
}
    `) as unknown as TypedDocumentString<MatchTitleMutation, MatchTitleMutationVariables>;
export const WantedDocument = new TypedDocumentString(`
    query Wanted {
  wanted {
    seriesId
    title {
      id
    }
    show
    season
    episode
    name
    airAt
    aired
    state
    attempts
    searchedAt
    nextSearch
  }
}
    `) as unknown as TypedDocumentString<WantedQuery, WantedQueryVariables>;