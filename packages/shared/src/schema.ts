export type Maybe<T> = T | null;
export type InputMaybe<T> = Maybe<T>;
/** All built-in and custom scalars, mapped to their actual values */
export type Scalars = {
  ID: { input: string; output: string; }
  String: { input: string; output: string; }
  Boolean: { input: boolean; output: boolean; }
  Int: { input: number; output: number; }
  Float: { input: number; output: number; }
  Duration: { input: string; output: string; }
  /** A scalar that can represent any JSON value. */
  JSON: { input: unknown; output: unknown; }
  /** A multipart file upload */
  Upload: { input: Blob; output: Blob; }
};

export type Album = {
  addedAt: Scalars['Int']['output'];
  artist: Scalars['String']['output'];
  artists: Array<ArtistRef>;
  compilation: Scalars['Boolean']['output'];
  /** Missing when there's no picture in its folder or its files. */
  cover?: Maybe<Scalars['String']['output']>;
  /** The most vivid colour of the cover as "r g b", empty until it's been worked out. */
  coverTint?: Maybe<Scalars['String']['output']>;
  discTitles: Array<DiscTitle>;
  duration: Scalars['Float']['output'];
  genres: Array<Scalars['String']['output']>;
  id: Scalars['Int']['output'];
  labels: Array<Scalars['String']['output']>;
  lastPlayed?: Maybe<Scalars['Int']['output']>;
  library: Scalars['String']['output'];
  mbid?: Maybe<Scalars['String']['output']>;
  name: Scalars['String']['output'];
  originalDate?: Maybe<Scalars['String']['output']>;
  playCount: Scalars['Int']['output'];
  rating?: Maybe<Scalars['Int']['output']>;
  releaseDate?: Maybe<Scalars['String']['output']>;
  releaseTypes: Array<Scalars['String']['output']>;
  size: Scalars['Int']['output'];
  starred: Scalars['Boolean']['output'];
  trackCount: Scalars['Int']['output'];
  tracks: Array<Track>;
  year?: Maybe<Scalars['Int']['output']>;
};

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

export type AppPassword = {
  client?: Maybe<Scalars['String']['output']>;
  createdAt: Scalars['Int']['output'];
  id: Scalars['Int']['output'];
  lastUsed?: Maybe<Scalars['Int']['output']>;
  name: Scalars['String']['output'];
};

/** How tinystream looks for whoever's asking. */
export type Appearance = {
  dark: ColorScheme;
  /** The scheme for the system's light mode; the only one in single mode. */
  light: ColorScheme;
  mediaTint: Scalars['Boolean']['output'];
  mode: SchemeMode;
  style: ComponentStyle;
};

/** What someone picked for themselves; anything left out follows the server. */
export type AppearanceSettings = {
  colors?: Maybe<SchemeChoice>;
  /** Let the scheme colour what sits over artwork and video, too. */
  mediaTint: Scalars['Boolean']['output'];
  style?: Maybe<ComponentStyle>;
};

/** What someone picked for themselves; anything left out follows the server. */
export type AppearanceSettingsInput = {
  colors?: InputMaybe<SchemeChoiceInput>;
  /** Let the scheme colour what sits over artwork and video, too. */
  mediaTint: Scalars['Boolean']['input'];
  style?: InputMaybe<ComponentStyle>;
};

export type AppliedRenames = {
  batch: Scalars['String']['output'];
  problems: Array<Scalars['String']['output']>;
  renamed: Scalars['Int']['output'];
};

export type Artist = {
  albumCount: Scalars['Int']['output'];
  albums: Array<Album>;
  /** Albums by others that this artist is on. */
  appearsOn: Array<Album>;
  cover?: Maybe<Scalars['String']['output']>;
  /** The most vivid colour of the cover as "r g b", empty until it's been worked out. */
  coverTint?: Maybe<Scalars['String']['output']>;
  id: Scalars['Int']['output'];
  library: Scalars['String']['output'];
  mbid?: Maybe<Scalars['String']['output']>;
  name: Scalars['String']['output'];
  rating?: Maybe<Scalars['Int']['output']>;
  sortName: Scalars['String']['output'];
  starred: Scalars['Boolean']['output'];
  topTracks: Array<Track>;
  trackCount: Scalars['Int']['output'];
};


export type ArtistTopTracksArgs = {
  count?: Scalars['Int']['input'];
};

export type ArtistRef = {
  id: Scalars['Int']['output'];
  name: Scalars['String']['output'];
};

export type AudioTrack = {
  channels: Scalars['Int']['output'];
  codec: Scalars['String']['output'];
  codecString?: Maybe<Scalars['String']['output']>;
  default: Scalars['Boolean']['output'];
  index: Scalars['Int']['output'];
  language?: Maybe<Scalars['String']['output']>;
  title?: Maybe<Scalars['String']['output']>;
};

export type AutomationConfig = {
  defaultMonitor: Monitor;
  renameSuggestions: Scalars['Boolean']['output'];
  retry: Array<RetryStep>;
  rssInterval: Scalars['Duration']['output'];
};

export type AutomationConfigInput = {
  defaultMonitor: Monitor;
  renameSuggestions: Scalars['Boolean']['input'];
  retry: Array<RetryStepInput>;
  rssInterval: Scalars['Duration']['input'];
};

export type Book = {
  addedAt: Scalars['Int']['output'];
  authors: Array<Scalars['String']['output']>;
  chapterCount: Scalars['Int']['output'];
  /** The text, as the readers show it: JSON, as `packages/shared/src/books.ts` describes it. */
  content: Scalars['String']['output'];
  cover?: Maybe<Scalars['String']['output']>;
  /** The most vivid colour of the cover as "r g b", empty until it's been worked out. */
  coverTint?: Maybe<Scalars['String']['output']>;
  /** The book's own, or else what was found online. */
  description?: Maybe<Scalars['String']['output']>;
  /** Whether it's been looked up online yet. */
  fetched: Scalars['Boolean']['output'];
  /** The file as it is. */
  file: Scalars['String']['output'];
  /** Where the pictures in `content` are, each at this followed by its `src`. */
  files: Scalars['String']['output'];
  format: BookFormat;
  genres: Array<Scalars['String']['output']>;
  id: Scalars['Int']['output'];
  isbn?: Maybe<Scalars['String']['output']>;
  language?: Maybe<Scalars['String']['output']>;
  /** Characters of text, which reading positions count in. */
  length: Scalars['Int']['output'];
  library: Scalars['String']['output'];
  /** Pages in print, when that was found online. */
  pages?: Maybe<Scalars['Int']['output']>;
  /** Where what the book doesn't say came from: `apple-books` or `open-library`. */
  provider?: Maybe<Scalars['String']['output']>;
  /** The page about it where its details came from. */
  providerUrl?: Maybe<Scalars['String']['output']>;
  /** A date, or just a year. */
  published?: Maybe<Scalars['String']['output']>;
  publisher?: Maybe<Scalars['String']['output']>;
  /** Your quotes from it and the ones sent to you, in the order they're in the book. */
  quotes: Array<Clip>;
  /** Out of 10. */
  rating?: Maybe<Scalars['Float']['output']>;
  reading?: Maybe<ReadingState>;
  /** What the file says, or else what its title (or file name) does. */
  series?: Maybe<Scalars['String']['output']>;
  seriesIndex?: Maybe<Scalars['Float']['output']>;
  size: Scalars['Int']['output'];
  sortTitle: Scalars['String']['output'];
  title: Scalars['String']['output'];
  year?: Maybe<Scalars['Int']['output']>;
};

export type BookFormat =
  | 'EPUB'
  | 'FB2'
  | 'TXT';

export type BookShelf = {
  books: Array<Book>;
  library: Scalars['String']['output'];
};

export type CalendarEntry = {
  absolute?: Maybe<Scalars['Int']['output']>;
  airAt: Scalars['Int']['output'];
  backdrop?: Maybe<Scalars['String']['output']>;
  /** The most vivid colour of the backdrop as "r g b", empty until it's been worked out. */
  backdropTint?: Maybe<Scalars['String']['output']>;
  download?: Maybe<EpisodeDownload>;
  episode: Scalars['Int']['output'];
  library: Scalars['String']['output'];
  monitor: Monitor;
  name?: Maybe<Scalars['String']['output']>;
  poster?: Maybe<Scalars['String']['output']>;
  season: Scalars['Int']['output'];
  seriesId: Scalars['Int']['output'];
  show: Scalars['String']['output'];
  state: EpisodeState;
  title?: Maybe<Title>;
  video?: Maybe<Video>;
};

export type ChangedList =
  | 'APPEARANCE'
  | 'DOWNLOADS'
  | 'NOTIFICATIONS'
  | 'PLAYLISTS'
  | 'RENAME_SUGGESTIONS'
  | 'REQUESTS'
  | 'USERS';

export type Chapter = {
  end: Scalars['Float']['output'];
  start: Scalars['Float']['output'];
  title?: Maybe<Scalars['String']['output']>;
};

export type Clip = {
  audio?: Maybe<Scalars['Int']['output']>;
  bytes?: Maybe<Scalars['Int']['output']>;
  canManage: Scalars['Boolean']['output'];
  createdAt: Scalars['Int']['output'];
  end: Scalars['Float']['output'];
  error?: Maybe<Scalars['String']['output']>;
  file: Scalars['String']['output'];
  fps?: Maybe<Scalars['Float']['output']>;
  height?: Maybe<Scalars['Int']['output']>;
  id: Scalars['Int']['output'];
  kind: ClipKind;
  link?: Maybe<Scalars['String']['output']>;
  linkLive: Scalars['Boolean']['output'];
  mine: Scalars['Boolean']['output'];
  name: Scalars['String']['output'];
  owner: User;
  poster?: Maybe<Scalars['String']['output']>;
  progress?: Maybe<Scalars['Float']['output']>;
  public: Scalars['Boolean']['output'];
  quality: ClipQuality;
  quote?: Maybe<ClipQuote>;
  recipients: Array<ClipRecipient>;
  renderedAt?: Maybe<Scalars['Int']['output']>;
  screenshot: Scalars['Boolean']['output'];
  sharedAt?: Maybe<Scalars['Int']['output']>;
  source: ClipSource;
  start: Scalars['Float']['output'];
  state: ClipState;
  subtitles?: Maybe<Scalars['String']['output']>;
  width?: Maybe<Scalars['Int']['output']>;
};

export type ClipAllowance = {
  bytes: Scalars['Int']['output'];
  canClip: Scalars['Boolean']['output'];
  canLink: Scalars['Boolean']['output'];
  customDefaultFont: Scalars['Boolean']['output'];
  limit: Scalars['Int']['output'];
  maxLength: Scalars['Int']['output'];
  rendered: Scalars['Int']['output'];
  storage: Scalars['Int']['output'];
};

export type ClipChanged = {
  clipId: Scalars['Int']['output'];
  progress?: Maybe<Scalars['Float']['output']>;
  state?: Maybe<ClipState>;
};

export type ClipKind =
  | 'CLIP'
  | 'QUOTE'
  | 'SCREENSHOT';

export type ClipPatch = {
  name?: InputMaybe<Scalars['String']['input']>;
  public?: InputMaybe<Scalars['Boolean']['input']>;
  recipe?: InputMaybe<RecipeInput>;
};

export type ClipQuality = {
  halfRate: Scalars['Boolean']['output'];
  height: Scalars['Int']['output'];
};

export type ClipQuote = {
  author?: Maybe<Scalars['String']['output']>;
  /**
   * Where it is in the book as the book is now, in characters; null when the book was changed
   * and the text can't be found in it anymore.
   */
  range?: Maybe<QuoteRange>;
  /** What was quoted, its paragraphs a line apart. */
  text: Scalars['String']['output'];
};

export type ClipRecipient = {
  hidden: Scalars['Boolean']['output'];
  sharedAt: Scalars['Int']['output'];
  user: User;
};

export type ClipScope =
  | 'MINE'
  | 'RECEIVED'
  | 'RENDERING'
  | 'SENT';

export type ClipSource = {
  /** The book a quote is from, while it's there and you can see it. */
  book?: Maybe<Book>;
  kind: TitleKind;
  label?: Maybe<Scalars['String']['output']>;
  name: Scalars['String']['output'];
  status: SourceStatus;
  title?: Maybe<Title>;
  video?: Maybe<Video>;
  year?: Maybe<Scalars['Int']['output']>;
};

export type ClipState =
  | 'EVICTED'
  | 'FAILED'
  | 'QUEUED'
  | 'READY'
  | 'RENDERING';

export type ClipStorage = {
  bytes: Scalars['Int']['output'];
  dir?: Maybe<Scalars['String']['output']>;
  publicClips: Array<Clip>;
  usage: Array<ClipUsage>;
};

export type ClipUsage = {
  bytes: Scalars['Int']['output'];
  clips: Scalars['Int']['output'];
  limit: Scalars['Int']['output'];
  rendered: Scalars['Int']['output'];
  storage: Scalars['Int']['output'];
  user: User;
};

export type ClipsConfig = {
  concurrency: Scalars['Int']['output'];
  defaultFont?: Maybe<Scalars['String']['output']>;
  enabled: Scalars['Boolean']['output'];
  fontsDir?: Maybe<Scalars['String']['output']>;
  maxStorage: Scalars['Int']['output'];
  path?: Maybe<Scalars['String']['output']>;
  publicLinks: Scalars['Boolean']['output'];
};

export type ClipsConfigInput = {
  concurrency: Scalars['Int']['input'];
  defaultFont?: InputMaybe<Scalars['String']['input']>;
  enabled: Scalars['Boolean']['input'];
  fontsDir?: InputMaybe<Scalars['String']['input']>;
  maxStorage: Scalars['Int']['input'];
  path?: InputMaybe<Scalars['String']['input']>;
  publicLinks: Scalars['Boolean']['input'];
};

export type ColorScheme = {
  builtIn: Scalars['Boolean']['output'];
  /** `ts1.<payload>`: the scheme itself. Two schemes with the same code are the same scheme. */
  code: Scalars['String']['output'];
  editable: Scalars['Boolean']['output'];
  forkedFrom?: Maybe<SchemeOrigin>;
  id: Scalars['String']['output'];
  name: Scalars['String']['output'];
  palette: Palette;
  /** Everyone on this server can pick it. */
  published: Scalars['Boolean']['output'];
  /** The code with the name attached, for sharing. */
  shareCode: Scalars['String']['output'];
};

export type ComponentStyle =
  | 'FLAT'
  | 'GLASS'
  | 'LAYERED';

export type Confidence =
  | 'HIGH'
  | 'LOW';

export type ConfigChanged = {
  error?: Maybe<Scalars['String']['output']>;
};

export type ConfigPatch = {
  automation?: InputMaybe<AutomationConfigInput>;
  clips?: InputMaybe<ClipsConfigInput>;
  downloads?: InputMaybe<DownloadsConfigInput>;
  log?: InputMaybe<LogConfigInput>;
  metadata?: InputMaybe<MetadataConfigInput>;
  music?: InputMaybe<MusicConfigInput>;
  network?: InputMaybe<NetworkConfigInput>;
  requests?: InputMaybe<RequestsConfigInput>;
  scan?: InputMaybe<ScanConfigInput>;
  signIn?: InputMaybe<SignInConfigInput>;
  smtp?: InputMaybe<SmtpConfigInput>;
  transcode?: InputMaybe<TranscodeConfigInput>;
};

export type ConfiguredLibrary = {
  downloadPath?: Maybe<Scalars['String']['output']>;
  error?: Maybe<Scalars['String']['output']>;
  exists: Scalars['Boolean']['output'];
  kind: LibraryKind;
  managed: Scalars['Boolean']['output'];
  metadataProvider?: Maybe<Provider>;
  name: Scalars['String']['output'];
  path: Scalars['String']['output'];
  profile?: Maybe<Scalars['String']['output']>;
  resolvedPath?: Maybe<Scalars['String']['output']>;
  skippedCount: Scalars['Int']['output'];
  titleCount: Scalars['Int']['output'];
};

export type ContinueEntry = {
  newEpisode: Scalars['Boolean']['output'];
  position: Scalars['Float']['output'];
  upNext: Scalars['Boolean']['output'];
  video: Video;
  watchedAt?: Maybe<Scalars['Int']['output']>;
};

export type ContrastWarning = {
  background: Scalars['String']['output'];
  foreground: Scalars['String']['output'];
  minimum: Scalars['Float']['output'];
  ratio: Scalars['Float']['output'];
};

export type CreatedInvite = {
  invite: Invite;
  link: Scalars['String']['output'];
};

export type DecodedScheme = {
  code: Scalars['String']['output'];
  name?: Maybe<Scalars['String']['output']>;
  palette: Palette;
};

export type DetectedSource = {
  feed?: Maybe<Scalars['String']['output']>;
  kind: SourceKind;
  name?: Maybe<Scalars['String']['output']>;
  sample: Array<Release>;
  searchable: Scalars['Boolean']['output'];
  url: Scalars['String']['output'];
};

export type DiscTitle = {
  disc: Scalars['Int']['output'];
  title: Scalars['String']['output'];
};

export type DiscoverResult = {
  because?: Maybe<Scalars['String']['output']>;
  category: MediaCategory;
  id: Scalars['String']['output'];
  library: Scalars['String']['output'];
  monitor?: Maybe<Monitor>;
  name: Scalars['String']['output'];
  overview?: Maybe<Scalars['String']['output']>;
  poster?: Maybe<Scalars['String']['output']>;
  provider: Provider;
  requestState?: Maybe<RequestState>;
  romaji?: Maybe<Scalars['String']['output']>;
  seriesId?: Maybe<Scalars['Int']['output']>;
  titleId?: Maybe<Scalars['Int']['output']>;
  year?: Maybe<Scalars['Int']['output']>;
};

export type Discovery = {
  library: Scalars['String']['output'];
  results: Array<DiscoverResult>;
};

export type Download = {
  addedAt: Scalars['Int']['output'];
  category: MediaCategory;
  episodes: Array<EpisodeNumber>;
  error?: Maybe<Scalars['String']['output']>;
  files: Array<TorrentFile>;
  finishedAt?: Maybe<Scalars['Int']['output']>;
  id: Scalars['Int']['output'];
  importError?: Maybe<Scalars['String']['output']>;
  importMode?: Maybe<Scalars['String']['output']>;
  importState: ImportState;
  importedAt?: Maybe<Scalars['Int']['output']>;
  live?: Maybe<TorrentStatus>;
  name: Scalars['String']['output'];
  poster?: Maybe<Scalars['String']['output']>;
  requestedBy?: Maybe<User>;
  savePath: Scalars['String']['output'];
  seedGoal: SeedGoal;
  seriesId?: Maybe<Scalars['Int']['output']>;
  seriesName?: Maybe<Scalars['String']['output']>;
  size?: Maybe<Scalars['Int']['output']>;
  source?: Maybe<Scalars['String']['output']>;
  state: DownloadState;
  title?: Maybe<Title>;
};

export type DownloadEngine = {
  active: Scalars['Int']['output'];
  downloadPath: Scalars['String']['output'];
  downloadRate: Scalars['Int']['output'];
  killSwitch?: Maybe<Scalars['String']['output']>;
  listenError?: Maybe<Scalars['String']['output']>;
  listening?: Maybe<Scalars['String']['output']>;
  slowHours: Scalars['Boolean']['output'];
  uploadRate: Scalars['Int']['output'];
  version: Scalars['String']['output'];
};

export type DownloadState =
  | 'DONE'
  | 'DOWNLOADING'
  | 'FAILED'
  | 'PAUSED'
  | 'REMOVED'
  | 'SEEDING';

export type DownloadsConfig = {
  bindInterface?: Maybe<Scalars['String']['output']>;
  dht: Scalars['Boolean']['output'];
  downloadLimit: Scalars['Int']['output'];
  import: ImportMode;
  maxActive: Scalars['Int']['output'];
  path?: Maybe<Scalars['String']['output']>;
  port: Scalars['Int']['output'];
  proxy?: Maybe<Scalars['String']['output']>;
  seeding: Seeding;
  slowDownloadLimit: Scalars['Int']['output'];
  slowFrom?: Maybe<Scalars['String']['output']>;
  slowTo?: Maybe<Scalars['String']['output']>;
  slowUploadLimit: Scalars['Int']['output'];
  uploadLimit: Scalars['Int']['output'];
  upnp: Scalars['Boolean']['output'];
};

export type DownloadsConfigInput = {
  bindInterface?: InputMaybe<Scalars['String']['input']>;
  dht: Scalars['Boolean']['input'];
  downloadLimit: Scalars['Int']['input'];
  import: ImportMode;
  maxActive: Scalars['Int']['input'];
  path?: InputMaybe<Scalars['String']['input']>;
  port: Scalars['Int']['input'];
  proxy?: InputMaybe<Scalars['String']['input']>;
  seeding: SeedingInput;
  slowDownloadLimit: Scalars['Int']['input'];
  slowFrom?: InputMaybe<Scalars['String']['input']>;
  slowTo?: InputMaybe<Scalars['String']['input']>;
  slowUploadLimit: Scalars['Int']['input'];
  uploadLimit: Scalars['Int']['input'];
  upnp: Scalars['Boolean']['input'];
};

export type EpisodeCounts = {
  grabbed: Scalars['Int']['output'];
  have: Scalars['Int']['output'];
  missing: Scalars['Int']['output'];
  skipped: Scalars['Int']['output'];
  total: Scalars['Int']['output'];
  upcoming: Scalars['Int']['output'];
  wanted: Scalars['Int']['output'];
};

export type EpisodeDownload = {
  downloadRate: Scalars['Int']['output'];
  eta?: Maybe<Scalars['Int']['output']>;
  progress: Scalars['Float']['output'];
  stage: TorrentStage;
};

export type EpisodeNumber = {
  episode: Scalars['Int']['output'];
  season: Scalars['Int']['output'];
};

export type EpisodeNumberInput = {
  episode: Scalars['Int']['input'];
  season: Scalars['Int']['input'];
};

export type EpisodeState =
  | 'DONE'
  | 'GRABBED'
  | 'IDLE'
  | 'MISSING'
  | 'SKIPPED'
  | 'WANTED';

export type EpisodesImported = {
  episodes: Array<EpisodeNumber>;
  library: Scalars['String']['output'];
  show: Scalars['String']['output'];
  titleId?: Maybe<Scalars['Int']['output']>;
};

export type Event = ClipChanged | ConfigChanged | EpisodesImported | LibraryChanged | ListChanged | MetadataChanged | NotificationReceived | PlaybackChanged | QueueChanged | ScanFinished | ScanStarted | SeriesChanged;

export type FileOperation = {
  dst: Scalars['String']['output'];
  kind: Scalars['String']['output'];
  src?: Maybe<Scalars['String']['output']>;
};

export type Folder = {
  name: Scalars['String']['output'];
  path: Scalars['String']['output'];
};

export type FolderListing = {
  folders: Array<Folder>;
  home?: Maybe<Scalars['String']['output']>;
  parent?: Maybe<Scalars['String']['output']>;
  path: Scalars['String']['output'];
};

export type Font = {
  filename: Scalars['String']['output'];
  index: Scalars['Int']['output'];
};

export type ForYou = {
  library: Scalars['String']['output'];
  shelves: Array<ForYouShelf>;
};

export type ForYouShelf = {
  key: Scalars['String']['output'];
  name: Scalars['String']['output'];
  results: Array<DiscoverResult>;
};

export type Gains = {
  albumGain?: Maybe<Scalars['Float']['output']>;
  albumPeak?: Maybe<Scalars['Float']['output']>;
  /** Neither tagged nor measured yet; `measureLoudness` sorts that out. */
  pending: Scalars['Boolean']['output'];
  /** dB to bring the track to ReplayGain's reference loudness. */
  trackGain?: Maybe<Scalars['Float']['output']>;
  trackPeak?: Maybe<Scalars['Float']['output']>;
};

export type Genre = {
  albumCount: Scalars['Int']['output'];
  name: Scalars['String']['output'];
  trackCount: Scalars['Int']['output'];
};

export type Hardware =
  | 'AUTO'
  | 'SOFTWARE'
  | 'VAAPI';

export type HistoryBatch = {
  at: Scalars['Int']['output'];
  batch: Scalars['String']['output'];
  count: Scalars['Int']['output'];
  label: Scalars['String']['output'];
  operations: Array<FileOperation>;
  undone: Scalars['Boolean']['output'];
};

export type Home = {
  continueWatching: Array<ContinueEntry>;
  popularHere: Array<PopularTitle>;
  recentlyAdded: Array<Shelf>;
};

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

export type Inbox = {
  items: Array<Notification>;
  unread: Scalars['Int']['output'];
};

export type Invite = {
  createdAt: Scalars['Int']['output'];
  expiresAt: Scalars['Int']['output'];
  id: Scalars['Int']['output'];
  label: Scalars['String']['output'];
  maxUses: Scalars['Int']['output'];
  revoked: Scalars['Boolean']['output'];
  uses: Scalars['Int']['output'];
};

export type InvitePreview = {
  expiresAt: Scalars['Int']['output'];
  remainingUses: Scalars['Int']['output'];
};

export type Library = {
  albumCount: Scalars['Int']['output'];
  /** A series counting as one, as its books are shown. */
  bookCount: Scalars['Int']['output'];
  kind: LibraryKind;
  movieCount: Scalars['Int']['output'];
  name: Scalars['String']['output'];
  showCount: Scalars['Int']['output'];
  titles: Array<Title>;
  trackCount: Scalars['Int']['output'];
};

export type LibraryChanged = {
  library: Scalars['String']['output'];
};

export type LibraryInput = {
  downloadPath?: InputMaybe<Scalars['String']['input']>;
  kind?: LibraryKind;
  managed?: Scalars['Boolean']['input'];
  metadataProvider?: InputMaybe<Provider>;
  name: Scalars['String']['input'];
  path: Scalars['String']['input'];
  profile?: InputMaybe<Scalars['String']['input']>;
};

export type LibraryKind =
  | 'BOOKS'
  | 'MUSIC'
  | 'VIDEO';

export type ListChanged = {
  list: ChangedList;
};

export type ListenRoom = {
  canInvite: Scalars['Boolean']['output'];
  canShare: Scalars['Boolean']['output'];
  code: Scalars['String']['output'];
  hostName: Scalars['String']['output'];
  isHost: Scalars['Boolean']['output'];
  signedIn: Scalars['Boolean']['output'];
  /** What's queued, in order; the room's socket says which is playing. */
  tracks: Array<Track>;
};

export type LogConfig = {
  level: Scalars['String']['output'];
};

export type LogConfigInput = {
  level: Scalars['String']['input'];
};

export type LyricLine = {
  /** Milliseconds in; missing for plain lyrics. */
  start?: Maybe<Scalars['Int']['output']>;
  text: Scalars['String']['output'];
};

export type Lyrics = {
  lines: Array<LyricLine>;
  source: LyricsSource;
  synced: Scalars['Boolean']['output'];
};

export type LyricsSource =
  | 'EMBEDDED'
  | 'FILE'
  | 'ONLINE';

export type MatchCandidates = {
  provider: Provider;
  query: Scalars['String']['output'];
  results: Array<ProviderTitle>;
};

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

export type MediaInfo = {
  audio: Array<AudioTrack>;
  chapters: Array<Chapter>;
  duration?: Maybe<Scalars['Float']['output']>;
  fonts: Array<Font>;
  subtitles: Array<SubtitleTrack>;
  video?: Maybe<VideoTrack>;
};

export type MetadataChanged = {
  status: MetadataStatus;
  titleId: Scalars['Int']['output'];
};

export type MetadataConfig = {
  language: Scalars['String']['output'];
  tmdbApiKey?: Maybe<Scalars['String']['output']>;
};

export type MetadataConfigInput = {
  language: Scalars['String']['input'];
  tmdbApiKey?: InputMaybe<Scalars['String']['input']>;
};

export type MetadataStatus =
  | 'FAILED'
  | 'FETCHING'
  | 'UPDATED';

export type Monitor =
  | 'FUTURE'
  | 'MISSING'
  | 'NONE';

export type MusicConfig = {
  analyzeLoudness: Scalars['Boolean']['output'];
  lyricsUrl: Scalars['String']['output'];
  onlineLyrics: Scalars['Boolean']['output'];
};

export type MusicConfigInput = {
  analyzeLoudness: Scalars['Boolean']['input'];
  lyricsUrl: Scalars['String']['input'];
  onlineLyrics: Scalars['Boolean']['input'];
};

export type MusicHome = {
  recentlyAdded: Array<Album>;
  recentlyPlayed: Array<Album>;
  rediscover: Array<Album>;
};

export type MusicKind =
  | 'ALBUM'
  | 'ARTIST'
  | 'TRACK';

export type MusicSearch = {
  albums: Array<Album>;
  artists: Array<Artist>;
  tracks: Array<Track>;
};

export type Mutation = {
  acceptInvite: SignedIn;
  addLibrary: Settings;
  addProfile: Settings;
  addSeries: Series;
  addSource: Settings;
  addToPlaylist: Playlist;
  applyRenames: AppliedRenames;
  approveRequest: Request;
  cancelClipRender: Clip;
  changePassword: Scalars['Boolean']['output'];
  createAppPassword: NewAppPassword;
  createClip: Clip;
  /** With `email`, the link is also sent there; the address isn't kept. */
  createInvite: CreatedInvite;
  createPlaylist: Playlist;
  createQuote: Clip;
  createRequest: Request;
  createUser: User;
  declineRequest: Request;
  deleteAppPassword: Scalars['Boolean']['output'];
  deleteClip: Scalars['Int']['output'];
  deleteDownloaded: UndoReport;
  deleteNotifications: Inbox;
  deletePasskey: Array<Passkey>;
  deletePlaylist: Scalars['Boolean']['output'];
  deleteRequest: Scalars['Int']['output'];
  deleteScheme: Scalars['String']['output'];
  deleteUser: Scalars['Int']['output'];
  dismissRenames: Scalars['Boolean']['output'];
  dropClipRenders: Scalars['Int']['output'];
  finishPasskeyRegistration: Array<Passkey>;
  finishPasskeySignIn: SignedIn;
  /** An editable copy of any scheme you can see. */
  forkScheme: ColorScheme;
  grabRelease: Download;
  hideClip: Scalars['Int']['output'];
  importDownload: Download;
  importScheme: ColorScheme;
  lookForAgain: Scalars['Boolean']['output'];
  manageTitle: Series;
  markNotificationsRead: Inbox;
  matchTitle: Title;
  /** Measures a track's loudness now, if that's still to be done, for whoever's about to hear it. */
  measureLoudness: Track;
  /**
   * What's playing in tinystream's own player right now (the web's, or `client`, the app's), so
   * other apps see it; no track means it stopped.
   */
  nowPlaying: Scalars['Boolean']['output'];
  pauseDownloads: Array<Download>;
  /** Someone listened to (most of) a track. */
  played: Scalars['Boolean']['output'];
  /** Shares one of your schemes with everyone here, or takes it back. */
  publishScheme: ColorScheme;
  /** 1 to 5 stars; 0 takes the rating away. */
  rate: Scalars['Boolean']['output'];
  recheckDownloads: Array<Download>;
  /** Looks the book up online again. */
  refreshBook: Book;
  refreshRenameSuggestions: Scalars['Boolean']['output'];
  refreshSeriesSchedule: Series;
  refreshTitle: Title;
  removeAvatar: User;
  removeDownloads: Array<Scalars['Int']['output']>;
  removeLibrary: Settings;
  removeProfile: Settings;
  removeSeries: Scalars['Int']['output'];
  removeSource: Settings;
  renderClip: Clip;
  replaceConfig: Settings;
  /**
   * Emails a reset link to the account `who` (a username or an email address) names. Says the same
   * whether or not there's such an account, and sends in the background so the wait doesn't tell either.
   */
  requestPasswordReset: Scalars['Boolean']['output'];
  resetPassword: SignedIn;
  resumeDownloads: Array<Download>;
  revokeInvite: Scalars['Boolean']['output'];
  /** Saves the queue of tinystream's own player: the web's, or `client`, the app's. */
  savePlayQueue: PlayQueue;
  saveProgress: Video;
  /** Where someone is in a book, in characters of its `length`; reaching the end finishes it. */
  saveReading: Book;
  /** Creates a scheme, or changes one you may edit. */
  saveScheme: ColorScheme;
  scan: Scalars['Boolean']['output'];
  sendTestEmail: Scalars['Boolean']['output'];
  setAppearance: Appearance;
  setAvatar: User;
  /** Marks a book read, or not started (which forgets where it was left). */
  setBookFinished: Book;
  /** The address password resets go to; `None` takes it away. */
  setEmail?: Maybe<Scalars['String']['output']>;
  setPermissionDefaults: Permissions;
  /** What everyone sees unless they pick something else, signed-out pages included. */
  setServerAppearance: ServerAppearance;
  setTitleArtwork: Title;
  setTitleWatched: Title;
  setVideoArtwork: Video;
  setWatched: Array<Video>;
  setup: SignedIn;
  shareClip: Clip;
  signIn: SignedIn;
  signOut: Scalars['Boolean']['output'];
  star: Scalars['Boolean']['output'];
  startListenRoom: ListenRoom;
  startPasskeyRegistration: PasskeyChallenge;
  startPasskeySignIn: PasskeyChallenge;
  startRoom: Room;
  takeScreenshot: Clip;
  undoFileChanges: UndoReport;
  unshareClip: Clip;
  updateClip: Clip;
  updateLibrary: Settings;
  updatePlaylist: Playlist;
  updateProfile: Settings;
  updateSeries: Series;
  updateSettings: Settings;
  updateSource: Settings;
  updateUser: User;
};


export type MutationAcceptInviteArgs = {
  password: Scalars['String']['input'];
  token: Scalars['String']['input'];
  username: Scalars['String']['input'];
};


export type MutationAddLibraryArgs = {
  input: LibraryInput;
};


export type MutationAddProfileArgs = {
  input: ProfileInput;
};


export type MutationAddSeriesArgs = {
  input: NewSeries;
};


export type MutationAddSourceArgs = {
  input: SourceInput;
};


export type MutationAddToPlaylistArgs = {
  id: Scalars['Int']['input'];
  tracks: Array<Scalars['Int']['input']>;
};


export type MutationApplyRenamesArgs = {
  ids: Array<Scalars['Int']['input']>;
};


export type MutationApproveRequestArgs = {
  id: Scalars['Int']['input'];
  library?: InputMaybe<Scalars['String']['input']>;
};


export type MutationCancelClipRenderArgs = {
  id: Scalars['Int']['input'];
};


export type MutationChangePasswordArgs = {
  current: Scalars['String']['input'];
  new: Scalars['String']['input'];
};


export type MutationCreateAppPasswordArgs = {
  name: Scalars['String']['input'];
};


export type MutationCreateClipArgs = {
  input: NewClip;
};


export type MutationCreateInviteArgs = {
  email?: InputMaybe<Scalars['String']['input']>;
  expiresInHours: Scalars['Int']['input'];
  label: Scalars['String']['input'];
  maxUses: Scalars['Int']['input'];
};


export type MutationCreatePlaylistArgs = {
  name: Scalars['String']['input'];
  tracks?: Array<Scalars['Int']['input']>;
};


export type MutationCreateQuoteArgs = {
  input: NewQuote;
};


export type MutationCreateRequestArgs = {
  input: NewRequest;
};


export type MutationCreateUserArgs = {
  input: NewUser;
};


export type MutationDeclineRequestArgs = {
  id: Scalars['Int']['input'];
  note?: InputMaybe<Scalars['String']['input']>;
};


export type MutationDeleteAppPasswordArgs = {
  id: Scalars['Int']['input'];
};


export type MutationDeleteClipArgs = {
  id: Scalars['Int']['input'];
};


export type MutationDeleteDownloadedArgs = {
  season?: InputMaybe<Scalars['Int']['input']>;
  seriesId: Scalars['Int']['input'];
};


export type MutationDeleteNotificationsArgs = {
  id?: InputMaybe<Scalars['Int']['input']>;
};


export type MutationDeletePasskeyArgs = {
  id: Scalars['Int']['input'];
};


export type MutationDeletePlaylistArgs = {
  id: Scalars['Int']['input'];
};


export type MutationDeleteRequestArgs = {
  id: Scalars['Int']['input'];
};


export type MutationDeleteSchemeArgs = {
  id: Scalars['String']['input'];
};


export type MutationDeleteUserArgs = {
  id: Scalars['Int']['input'];
};


export type MutationDismissRenamesArgs = {
  ids: Array<Scalars['Int']['input']>;
};


export type MutationFinishPasskeyRegistrationArgs = {
  challenge: Scalars['String']['input'];
  credential: Scalars['JSON']['input'];
};


export type MutationFinishPasskeySignInArgs = {
  challenge: Scalars['String']['input'];
  credential: Scalars['JSON']['input'];
};


export type MutationForkSchemeArgs = {
  id: Scalars['String']['input'];
};


export type MutationGrabReleaseArgs = {
  episodes?: Array<EpisodeNumberInput>;
  release: ReleaseInput;
  seriesId?: InputMaybe<Scalars['Int']['input']>;
};


export type MutationHideClipArgs = {
  id: Scalars['Int']['input'];
};


export type MutationImportDownloadArgs = {
  id: Scalars['Int']['input'];
};


export type MutationImportSchemeArgs = {
  code: Scalars['String']['input'];
  name?: InputMaybe<Scalars['String']['input']>;
};


export type MutationLookForAgainArgs = {
  episode?: InputMaybe<Scalars['Int']['input']>;
  season?: InputMaybe<Scalars['Int']['input']>;
  seriesId: Scalars['Int']['input'];
};


export type MutationManageTitleArgs = {
  titleId: Scalars['Int']['input'];
};


export type MutationMarkNotificationsReadArgs = {
  ids?: InputMaybe<Array<Scalars['Int']['input']>>;
};


export type MutationMatchTitleArgs = {
  id: Scalars['Int']['input'];
  provider: Provider;
  providerId: Scalars['String']['input'];
};


export type MutationMeasureLoudnessArgs = {
  trackId: Scalars['Int']['input'];
};


export type MutationNowPlayingArgs = {
  client?: InputMaybe<Scalars['String']['input']>;
  paused?: Scalars['Boolean']['input'];
  position?: Scalars['Float']['input'];
  trackId?: InputMaybe<Scalars['Int']['input']>;
};


export type MutationPauseDownloadsArgs = {
  ids: Array<Scalars['Int']['input']>;
};


export type MutationPlayedArgs = {
  trackId: Scalars['Int']['input'];
};


export type MutationPublishSchemeArgs = {
  id: Scalars['String']['input'];
  published: Scalars['Boolean']['input'];
};


export type MutationRateArgs = {
  id: Scalars['Int']['input'];
  kind: MusicKind;
  rating: Scalars['Int']['input'];
};


export type MutationRecheckDownloadsArgs = {
  ids: Array<Scalars['Int']['input']>;
};


export type MutationRefreshBookArgs = {
  id: Scalars['Int']['input'];
};


export type MutationRefreshSeriesScheduleArgs = {
  id: Scalars['Int']['input'];
};


export type MutationRefreshTitleArgs = {
  id: Scalars['Int']['input'];
};


export type MutationRemoveAvatarArgs = {
  userId?: InputMaybe<Scalars['Int']['input']>;
};


export type MutationRemoveDownloadsArgs = {
  deleteFiles?: Scalars['Boolean']['input'];
  ids: Array<Scalars['Int']['input']>;
};


export type MutationRemoveLibraryArgs = {
  name: Scalars['String']['input'];
};


export type MutationRemoveProfileArgs = {
  name: Scalars['String']['input'];
};


export type MutationRemoveSeriesArgs = {
  id: Scalars['Int']['input'];
};


export type MutationRemoveSourceArgs = {
  name: Scalars['String']['input'];
};


export type MutationRenderClipArgs = {
  id: Scalars['Int']['input'];
};


export type MutationReplaceConfigArgs = {
  text: Scalars['String']['input'];
};


export type MutationRequestPasswordResetArgs = {
  who: Scalars['String']['input'];
};


export type MutationResetPasswordArgs = {
  password: Scalars['String']['input'];
  token: Scalars['String']['input'];
};


export type MutationResumeDownloadsArgs = {
  ids: Array<Scalars['Int']['input']>;
};


export type MutationRevokeInviteArgs = {
  id: Scalars['Int']['input'];
};


export type MutationSavePlayQueueArgs = {
  client?: InputMaybe<Scalars['String']['input']>;
  input: QueueInput;
};


export type MutationSaveProgressArgs = {
  duration: Scalars['Float']['input'];
  position: Scalars['Float']['input'];
  videoId: Scalars['Int']['input'];
};


export type MutationSaveReadingArgs = {
  id: Scalars['Int']['input'];
  position: Scalars['Int']['input'];
};


export type MutationSaveSchemeArgs = {
  id?: InputMaybe<Scalars['String']['input']>;
  input: SchemeInput;
};


export type MutationScanArgs = {
  library?: InputMaybe<Scalars['String']['input']>;
};


export type MutationSendTestEmailArgs = {
  to: Scalars['String']['input'];
};


export type MutationSetAppearanceArgs = {
  input: AppearanceSettingsInput;
};


export type MutationSetAvatarArgs = {
  image: Scalars['Upload']['input'];
  userId?: InputMaybe<Scalars['Int']['input']>;
};


export type MutationSetBookFinishedArgs = {
  finished: Scalars['Boolean']['input'];
  id: Scalars['Int']['input'];
};


export type MutationSetEmailArgs = {
  email?: InputMaybe<Scalars['String']['input']>;
  password: Scalars['String']['input'];
};


export type MutationSetPermissionDefaultsArgs = {
  permissions: PermissionsInput;
};


export type MutationSetServerAppearanceArgs = {
  input: ServerAppearanceInput;
};


export type MutationSetTitleArtworkArgs = {
  id: Scalars['Int']['input'];
  image?: InputMaybe<Scalars['Upload']['input']>;
  kind: TitleArtwork;
};


export type MutationSetTitleWatchedArgs = {
  id: Scalars['Int']['input'];
  watched: Scalars['Boolean']['input'];
};


export type MutationSetVideoArtworkArgs = {
  image?: InputMaybe<Scalars['Upload']['input']>;
  videoId: Scalars['Int']['input'];
};


export type MutationSetWatchedArgs = {
  videoIds: Array<Scalars['Int']['input']>;
  watched: Scalars['Boolean']['input'];
};


export type MutationSetupArgs = {
  password: Scalars['String']['input'];
  username: Scalars['String']['input'];
};


export type MutationShareClipArgs = {
  id: Scalars['Int']['input'];
  users: Array<Scalars['Int']['input']>;
};


export type MutationSignInArgs = {
  password: Scalars['String']['input'];
  profile?: InputMaybe<Scalars['String']['input']>;
  username?: InputMaybe<Scalars['String']['input']>;
};


export type MutationStarArgs = {
  id: Scalars['Int']['input'];
  kind: MusicKind;
  starred: Scalars['Boolean']['input'];
};


export type MutationStartListenRoomArgs = {
  input: NewListenRoom;
};


export type MutationStartPasskeyRegistrationArgs = {
  name?: InputMaybe<Scalars['String']['input']>;
};


export type MutationStartPasskeySignInArgs = {
  profile?: InputMaybe<Scalars['String']['input']>;
  username?: InputMaybe<Scalars['String']['input']>;
};


export type MutationStartRoomArgs = {
  input: NewRoom;
};


export type MutationTakeScreenshotArgs = {
  input: NewScreenshot;
};


export type MutationUndoFileChangesArgs = {
  batch: Scalars['String']['input'];
};


export type MutationUnshareClipArgs = {
  id: Scalars['Int']['input'];
  userId: Scalars['Int']['input'];
};


export type MutationUpdateClipArgs = {
  id: Scalars['Int']['input'];
  input: ClipPatch;
};


export type MutationUpdateLibraryArgs = {
  input: LibraryInput;
  name: Scalars['String']['input'];
};


export type MutationUpdatePlaylistArgs = {
  id: Scalars['Int']['input'];
  input: PlaylistInput;
};


export type MutationUpdateProfileArgs = {
  input: ProfileInput;
  name: Scalars['String']['input'];
};


export type MutationUpdateSeriesArgs = {
  id: Scalars['Int']['input'];
  patch: SeriesPatch;
};


export type MutationUpdateSettingsArgs = {
  patch: ConfigPatch;
};


export type MutationUpdateSourceArgs = {
  input: SourceInput;
  name: Scalars['String']['input'];
};


export type MutationUpdateUserArgs = {
  id: Scalars['Int']['input'];
  input: UserPatch;
};

export type NamingPreview = {
  error?: Maybe<Scalars['String']['output']>;
  samples: Array<Scalars['String']['output']>;
};

export type NamingStyle = {
  agreement?: Maybe<Scalars['Float']['output']>;
  file: Scalars['String']['output'];
  folder: Scalars['String']['output'];
  samples: Scalars['Int']['output'];
};

export type NetworkConfig = {
  cors: Array<Scalars['String']['output']>;
  host: Scalars['String']['output'];
  port: Scalars['Int']['output'];
};

export type NetworkConfigInput = {
  cors: Array<Scalars['String']['input']>;
  host: Scalars['String']['input'];
  port: Scalars['Int']['input'];
};

export type NewAppPassword = {
  password: AppPassword;
  /** Shown this once. */
  secret: Scalars['String']['output'];
};

export type NewClip = {
  name?: Scalars['String']['input'];
  public?: Scalars['Boolean']['input'];
  recipe: RecipeInput;
  recipients?: Array<Scalars['Int']['input']>;
  room?: InputMaybe<Scalars['String']['input']>;
  videoId: Scalars['Int']['input'];
};

export type NewListenRoom = {
  current?: Scalars['Int']['input'];
  paused?: Scalars['Boolean']['input'];
  position?: Scalars['Float']['input'];
  public?: Scalars['Boolean']['input'];
  tracks: Array<Scalars['Int']['input']>;
};

export type NewQuote = {
  bookId: Scalars['Int']['input'];
  end: Scalars['Int']['input'];
  /** A note of your own about it. */
  name?: Scalars['String']['input'];
  public?: Scalars['Boolean']['input'];
  recipients?: Array<Scalars['Int']['input']>;
  /** Characters into the book, as reading positions count them. */
  start: Scalars['Int']['input'];
};

export type NewRequest = {
  library: Scalars['String']['input'];
  name: Scalars['String']['input'];
  note?: InputMaybe<Scalars['String']['input']>;
  overview?: InputMaybe<Scalars['String']['input']>;
  poster?: InputMaybe<Scalars['String']['input']>;
  providerId: Scalars['String']['input'];
  year?: InputMaybe<Scalars['Int']['input']>;
};

export type NewRoom = {
  paused?: Scalars['Boolean']['input'];
  position?: Scalars['Float']['input'];
  public?: Scalars['Boolean']['input'];
  tracks?: TracksInput;
  videoId: Scalars['Int']['input'];
};

export type NewScreenshot = {
  at: Scalars['Float']['input'];
  room?: InputMaybe<Scalars['String']['input']>;
  subtitles?: InputMaybe<Scalars['String']['input']>;
  videoId: Scalars['Int']['input'];
};

export type NewSeries = {
  library: Scalars['String']['input'];
  monitor?: InputMaybe<Monitor>;
  name: Scalars['String']['input'];
  overview?: InputMaybe<Scalars['String']['input']>;
  poster?: InputMaybe<Scalars['String']['input']>;
  profile?: InputMaybe<Scalars['String']['input']>;
  provider: Provider;
  providerId: Scalars['String']['input'];
  year?: InputMaybe<Scalars['Int']['input']>;
};

export type NewUser = {
  isAdmin?: Scalars['Boolean']['input'];
  password: Scalars['String']['input'];
  permissions?: PermissionOverridesInput;
  username: Scalars['String']['input'];
};

export type NextUp = {
  resuming: Scalars['Boolean']['output'];
  video: Video;
};

export type Notification = {
  actor?: Maybe<User>;
  body?: Maybe<Scalars['String']['output']>;
  createdAt: Scalars['Int']['output'];
  expiresAt?: Maybe<Scalars['Int']['output']>;
  id: Scalars['Int']['output'];
  image?: Maybe<Scalars['String']['output']>;
  kind: NotificationKind;
  link?: Maybe<Scalars['String']['output']>;
  priority: Scalars['Boolean']['output'];
  readAt?: Maybe<Scalars['Int']['output']>;
  title: Scalars['String']['output'];
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

export type NotificationReceived = {
  notification: Notification;
};

export type Numbering =
  | 'ABSOLUTE'
  | 'AUTO'
  | 'SEASONAL';

export type Palette = {
  overrides: Array<Token>;
  seeds: Array<Token>;
  /** Every colour, derived from the seeds with the overrides applied. */
  tokens: Array<Token>;
  warnings: Array<ContrastWarning>;
};

export type Passkey = {
  createdAt: Scalars['Int']['output'];
  id: Scalars['Int']['output'];
  lastUsed?: Maybe<Scalars['Int']['output']>;
  name: Scalars['String']['output'];
};

export type PasskeyChallenge = {
  challenge: Scalars['String']['output'];
  options: Scalars['JSON']['output'];
};

export type PermissionOverrides = {
  allLibraries?: Maybe<Scalars['Boolean']['output']>;
  autoApprove?: Maybe<Scalars['Boolean']['output']>;
  clip?: Maybe<Scalars['Boolean']['output']>;
  clipLimit?: Maybe<Scalars['Int']['output']>;
  clipLinks?: Maybe<Scalars['Boolean']['output']>;
  clipMaxLength?: Maybe<Scalars['Int']['output']>;
  clipStorage?: Maybe<Scalars['Int']['output']>;
  downloads?: Maybe<Scalars['Boolean']['output']>;
  editMetadata?: Maybe<Scalars['Boolean']['output']>;
  libraries?: Maybe<Array<Scalars['String']['output']>>;
  manageRequests?: Maybe<Scalars['Boolean']['output']>;
  manageShows?: Maybe<Scalars['Boolean']['output']>;
  request?: Maybe<Scalars['Boolean']['output']>;
  requestLimit?: Maybe<Scalars['Int']['output']>;
  shareLinks?: Maybe<Scalars['Boolean']['output']>;
  watchTogether?: Maybe<Scalars['Boolean']['output']>;
};

export type PermissionOverridesInput = {
  allLibraries?: InputMaybe<Scalars['Boolean']['input']>;
  autoApprove?: InputMaybe<Scalars['Boolean']['input']>;
  clip?: InputMaybe<Scalars['Boolean']['input']>;
  clipLimit?: InputMaybe<Scalars['Int']['input']>;
  clipLinks?: InputMaybe<Scalars['Boolean']['input']>;
  clipMaxLength?: InputMaybe<Scalars['Int']['input']>;
  clipStorage?: InputMaybe<Scalars['Int']['input']>;
  downloads?: InputMaybe<Scalars['Boolean']['input']>;
  editMetadata?: InputMaybe<Scalars['Boolean']['input']>;
  libraries?: InputMaybe<Array<Scalars['String']['input']>>;
  manageRequests?: InputMaybe<Scalars['Boolean']['input']>;
  manageShows?: InputMaybe<Scalars['Boolean']['input']>;
  request?: InputMaybe<Scalars['Boolean']['input']>;
  requestLimit?: InputMaybe<Scalars['Int']['input']>;
  shareLinks?: InputMaybe<Scalars['Boolean']['input']>;
  watchTogether?: InputMaybe<Scalars['Boolean']['input']>;
};

export type Permissions = {
  allLibraries: Scalars['Boolean']['output'];
  autoApprove: Scalars['Boolean']['output'];
  clip: Scalars['Boolean']['output'];
  clipLimit: Scalars['Int']['output'];
  clipLinks: Scalars['Boolean']['output'];
  clipMaxLength: Scalars['Int']['output'];
  clipStorage: Scalars['Int']['output'];
  downloads: Scalars['Boolean']['output'];
  editMetadata: Scalars['Boolean']['output'];
  libraries: Array<Scalars['String']['output']>;
  manageRequests: Scalars['Boolean']['output'];
  manageShows: Scalars['Boolean']['output'];
  request: Scalars['Boolean']['output'];
  requestLimit: Scalars['Int']['output'];
  shareLinks: Scalars['Boolean']['output'];
  watchTogether: Scalars['Boolean']['output'];
};

export type PermissionsInput = {
  allLibraries: Scalars['Boolean']['input'];
  autoApprove: Scalars['Boolean']['input'];
  clip: Scalars['Boolean']['input'];
  clipLimit: Scalars['Int']['input'];
  clipLinks: Scalars['Boolean']['input'];
  clipMaxLength: Scalars['Int']['input'];
  clipStorage: Scalars['Int']['input'];
  downloads: Scalars['Boolean']['input'];
  editMetadata: Scalars['Boolean']['input'];
  libraries: Array<Scalars['String']['input']>;
  manageRequests: Scalars['Boolean']['input'];
  manageShows: Scalars['Boolean']['input'];
  request: Scalars['Boolean']['input'];
  requestLimit: Scalars['Int']['input'];
  shareLinks: Scalars['Boolean']['input'];
  watchTogether: Scalars['Boolean']['input'];
};

export type PlayQueue = {
  changedBy?: Maybe<Scalars['String']['output']>;
  current: Scalars['Int']['output'];
  position: Scalars['Float']['output'];
  repeat: Repeat;
  shuffled: Scalars['Boolean']['output'];
  tracks: Array<Track>;
  updatedAt: Scalars['Int']['output'];
};

export type PlaybackChanged = {
  client: Scalars['String']['output'];
  paused: Scalars['Boolean']['output'];
  /** Seconds in, as of when it was sent. */
  position: Scalars['Float']['output'];
  trackId?: Maybe<Scalars['Int']['output']>;
};

export type Playlist = {
  comment?: Maybe<Scalars['String']['output']>;
  /** Up to four album covers from it, for a mosaic. */
  covers: Array<Scalars['String']['output']>;
  duration: Scalars['Float']['output'];
  id: Scalars['Int']['output'];
  mine: Scalars['Boolean']['output'];
  name: Scalars['String']['output'];
  owner: PlaylistOwner;
  public: Scalars['Boolean']['output'];
  trackCount: Scalars['Int']['output'];
  tracks: Array<Track>;
  updatedAt: Scalars['Int']['output'];
};

export type PlaylistInput = {
  comment?: InputMaybe<Scalars['String']['input']>;
  name?: InputMaybe<Scalars['String']['input']>;
  public?: InputMaybe<Scalars['Boolean']['input']>;
  /** Replaces what's in it. */
  tracks?: InputMaybe<Array<Scalars['Int']['input']>>;
};

export type PlaylistOwner = {
  id: Scalars['Int']['output'];
  username: Scalars['String']['output'];
};

export type PopularTitle = {
  people: Scalars['Int']['output'];
  title: Title;
};

export type Profile = {
  batches: Scalars['Boolean']['output'];
  codecs: Array<Scalars['String']['output']>;
  groups: Array<Scalars['String']['output']>;
  maxSize?: Maybe<Scalars['Int']['output']>;
  minSeeders: Scalars['Int']['output'];
  minSize?: Maybe<Scalars['Int']['output']>;
  name: Scalars['String']['output'];
  preferDualAudio: Scalars['Boolean']['output'];
  reject: Array<Scalars['String']['output']>;
  require: Array<Scalars['String']['output']>;
  resolutions: Array<Scalars['String']['output']>;
};

export type ProfileInput = {
  batches: Scalars['Boolean']['input'];
  codecs: Array<Scalars['String']['input']>;
  groups: Array<Scalars['String']['input']>;
  maxSize?: InputMaybe<Scalars['Int']['input']>;
  minSeeders: Scalars['Int']['input'];
  minSize?: InputMaybe<Scalars['Int']['input']>;
  name: Scalars['String']['input'];
  preferDualAudio: Scalars['Boolean']['input'];
  reject: Array<Scalars['String']['input']>;
  require: Array<Scalars['String']['input']>;
  resolutions: Array<Scalars['String']['input']>;
};

export type Provider =
  | 'ANILIST'
  | 'TMDB';

export type ProviderTitle = {
  category: MediaCategory;
  id: Scalars['String']['output'];
  name: Scalars['String']['output'];
  overview?: Maybe<Scalars['String']['output']>;
  poster?: Maybe<Scalars['String']['output']>;
  provider: Provider;
  romaji?: Maybe<Scalars['String']['output']>;
  year?: Maybe<Scalars['Int']['output']>;
};

export type Query = {
  airedEpisodes: Scalars['Int']['output'];
  album?: Maybe<Album>;
  albums: Array<Album>;
  allSeries: Array<Series>;
  appPasswords: Array<AppPassword>;
  /** How tinystream should look right now. Works signed out too, with the server's defaults. */
  appearance: Appearance;
  appearanceSettings: AppearanceSettings;
  artist?: Maybe<Artist>;
  /** Artists with albums of their own. */
  artists: Array<Artist>;
  book?: Maybe<Book>;
  /** A book library's books, a series' volumes together. */
  books: Array<Book>;
  calendar: Array<CalendarEntry>;
  clip?: Maybe<Clip>;
  clipAllowance: ClipAllowance;
  clipStorage: ClipStorage;
  /** `kind` only those of a kind, and `book` only quotes from that book. */
  clips: Array<Clip>;
  colorScheme?: Maybe<ColorScheme>;
  /** Built-ins, then published schemes, then your own. */
  colorSchemes: Array<ColorScheme>;
  /** What a code holds, to look at before importing it. */
  decodeScheme: DecodedScheme;
  detectSource: DetectedSource;
  discover: Discovery;
  download?: Maybe<Download>;
  downloadEngine: DownloadEngine;
  downloads: Array<Download>;
  fileHistory: Array<HistoryBatch>;
  folders: FolderListing;
  forYou: ForYou;
  genres: Array<Genre>;
  home: Home;
  invite?: Maybe<InvitePreview>;
  invites: Array<Invite>;
  libraries: Array<Library>;
  library?: Maybe<Library>;
  listenRoom: ListenRoom;
  lyrics?: Maybe<Lyrics>;
  musicHome: MusicHome;
  musicSearch: MusicSearch;
  /** The books added last to each book library. */
  newBooks: Array<BookShelf>;
  notifications: Inbox;
  /** Whose password a reset link is for, while it still works. */
  passwordReset?: Maybe<Scalars['String']['output']>;
  permissionDefaults: Permissions;
  playQueue: PlayQueue;
  playlist?: Maybe<Playlist>;
  playlists: Array<Playlist>;
  /** Books part read, the latest first. */
  readingNow: Array<Book>;
  renameSuggestions: Array<RenameSuggestion>;
  requests: Array<Request>;
  room: Room;
  search: SearchResults;
  series?: Maybe<Series>;
  server: Server;
  serverAppearance: ServerAppearance;
  settings: Settings;
  signInProfiles: Array<SignInProfile>;
  /** More like a track, from what's here: for radio when the queue runs out. */
  similarTracks: Array<Track>;
  skippedFiles: Array<SkippedFile>;
  songs: Array<Track>;
  title?: Maybe<Title>;
  /** The titles among `ids` that still exist and are visible, in the given order. */
  titles: Array<Title>;
  track?: Maybe<Track>;
  /** The tracks among `ids` that still exist and are visible, in the given order. */
  tracks: Array<Track>;
  user?: Maybe<User>;
  users: Array<User>;
  video?: Maybe<Video>;
  viewer?: Maybe<User>;
  wanted: Array<WantedEpisode>;
};


export type QueryAiredEpisodesArgs = {
  id: Scalars['String']['input'];
  provider: Provider;
};


export type QueryAlbumArgs = {
  id: Scalars['Int']['input'];
};


export type QueryAlbumsArgs = {
  genre?: InputMaybe<Scalars['String']['input']>;
  library?: InputMaybe<Scalars['String']['input']>;
  limit?: Scalars['Int']['input'];
  offset?: Scalars['Int']['input'];
  sort?: AlbumSort;
};


export type QueryArtistArgs = {
  id: Scalars['Int']['input'];
};


export type QueryArtistsArgs = {
  library?: InputMaybe<Scalars['String']['input']>;
};


export type QueryBookArgs = {
  id: Scalars['Int']['input'];
};


export type QueryBooksArgs = {
  library: Scalars['String']['input'];
};


export type QueryCalendarArgs = {
  from?: InputMaybe<Scalars['Int']['input']>;
  to?: InputMaybe<Scalars['Int']['input']>;
};


export type QueryClipArgs = {
  id: Scalars['Int']['input'];
};


export type QueryClipsArgs = {
  book?: InputMaybe<Scalars['Int']['input']>;
  kind?: InputMaybe<ClipKind>;
  scope?: ClipScope;
};


export type QueryColorSchemeArgs = {
  id: Scalars['String']['input'];
};


export type QueryDecodeSchemeArgs = {
  code: Scalars['String']['input'];
};


export type QueryDetectSourceArgs = {
  apiKey?: InputMaybe<Scalars['String']['input']>;
  url: Scalars['String']['input'];
};


export type QueryDiscoverArgs = {
  library?: InputMaybe<Scalars['String']['input']>;
  query?: InputMaybe<Scalars['String']['input']>;
};


export type QueryDownloadArgs = {
  id: Scalars['Int']['input'];
};


export type QueryFoldersArgs = {
  path?: InputMaybe<Scalars['String']['input']>;
};


export type QueryForYouArgs = {
  library?: InputMaybe<Scalars['String']['input']>;
};


export type QueryGenresArgs = {
  library?: InputMaybe<Scalars['String']['input']>;
};


export type QueryInviteArgs = {
  token: Scalars['String']['input'];
};


export type QueryLibraryArgs = {
  name: Scalars['String']['input'];
};


export type QueryListenRoomArgs = {
  code: Scalars['String']['input'];
};


export type QueryLyricsArgs = {
  trackId: Scalars['Int']['input'];
};


export type QueryMusicSearchArgs = {
  limit?: Scalars['Int']['input'];
  query: Scalars['String']['input'];
};


export type QueryNewBooksArgs = {
  limit?: Scalars['Int']['input'];
};


export type QueryNotificationsArgs = {
  limit?: Scalars['Int']['input'];
};


export type QueryPasswordResetArgs = {
  token: Scalars['String']['input'];
};


export type QueryPlaylistArgs = {
  id: Scalars['Int']['input'];
};


export type QueryReadingNowArgs = {
  limit?: Scalars['Int']['input'];
};


export type QueryRoomArgs = {
  code: Scalars['String']['input'];
};


export type QuerySearchArgs = {
  query: Scalars['String']['input'];
};


export type QuerySeriesArgs = {
  id: Scalars['Int']['input'];
};


export type QuerySimilarTracksArgs = {
  count?: Scalars['Int']['input'];
  exclude?: Array<Scalars['Int']['input']>;
  trackId: Scalars['Int']['input'];
};


export type QuerySongsArgs = {
  library?: InputMaybe<Scalars['String']['input']>;
  limit?: Scalars['Int']['input'];
  offset?: Scalars['Int']['input'];
  query?: Scalars['String']['input'];
};


export type QueryTitleArgs = {
  id: Scalars['Int']['input'];
};


export type QueryTitlesArgs = {
  ids: Array<Scalars['Int']['input']>;
};


export type QueryTrackArgs = {
  id: Scalars['Int']['input'];
};


export type QueryTracksArgs = {
  ids: Array<Scalars['Int']['input']>;
};


export type QueryUserArgs = {
  id: Scalars['Int']['input'];
};


export type QueryVideoArgs = {
  id: Scalars['Int']['input'];
};

export type QueueChanged = {
  by?: Maybe<Scalars['String']['output']>;
};

export type QueueInput = {
  current: Scalars['Int']['input'];
  position: Scalars['Float']['input'];
  repeat?: Repeat;
  shuffled?: Scalars['Boolean']['input'];
  tracks: Array<Scalars['Int']['input']>;
};

export type QuoteRange = {
  end: Scalars['Int']['output'];
  start: Scalars['Int']['output'];
};

export type ReadingState = {
  finished: Scalars['Boolean']['output'];
  /** Characters in, of the book's `length`. */
  position: Scalars['Int']['output'];
  /** From 0 to 1. */
  progress: Scalars['Float']['output'];
  updatedAt: Scalars['Int']['output'];
};

export type RecipeInput = {
  audio?: InputMaybe<Scalars['Int']['input']>;
  end: Scalars['Float']['input'];
  halfRate?: Scalars['Boolean']['input'];
  height: Scalars['Int']['input'];
  start: Scalars['Float']['input'];
  subtitles?: InputMaybe<Scalars['String']['input']>;
};

export type Release = {
  infoHash?: Maybe<Scalars['String']['output']>;
  leechers?: Maybe<Scalars['Int']['output']>;
  link: Scalars['String']['output'];
  page?: Maybe<Scalars['String']['output']>;
  published?: Maybe<Scalars['Int']['output']>;
  seeders?: Maybe<Scalars['Int']['output']>;
  size?: Maybe<Scalars['Int']['output']>;
  source: Scalars['String']['output'];
  title: Scalars['String']['output'];
};

export type ReleaseAttributes = {
  codec?: Maybe<Scalars['String']['output']>;
  dualAudio: Scalars['Boolean']['output'];
  group?: Maybe<Scalars['String']['output']>;
  proper: Scalars['Boolean']['output'];
  resolution?: Maybe<Scalars['Int']['output']>;
  source?: Maybe<Scalars['String']['output']>;
  tenBit: Scalars['Boolean']['output'];
  version: Scalars['Int']['output'];
};

export type ReleaseCandidate = {
  attributes: ReleaseAttributes;
  batch: Scalars['Boolean']['output'];
  episodes: Array<EpisodeNumber>;
  release: Release;
  verdict: Verdict;
};

export type ReleaseInput = {
  infoHash?: InputMaybe<Scalars['String']['input']>;
  leechers?: InputMaybe<Scalars['Int']['input']>;
  link: Scalars['String']['input'];
  page?: InputMaybe<Scalars['String']['input']>;
  published?: InputMaybe<Scalars['Int']['input']>;
  seeders?: InputMaybe<Scalars['Int']['input']>;
  size?: InputMaybe<Scalars['Int']['input']>;
  source: Scalars['String']['input'];
  title: Scalars['String']['input'];
};

export type RenameSuggestion = {
  confidence: Confidence;
  dst: Scalars['String']['output'];
  id: Scalars['Int']['output'];
  library: Scalars['String']['output'];
  managed: Scalars['Boolean']['output'];
  reason: Scalars['String']['output'];
  root?: Maybe<Scalars['String']['output']>;
  src: Scalars['String']['output'];
};

export type Repeat =
  | 'ALL'
  | 'OFF'
  | 'ONE';

export type Request = {
  aired?: Maybe<Scalars['Int']['output']>;
  createdAt: Scalars['Int']['output'];
  decidedAt?: Maybe<Scalars['Int']['output']>;
  have?: Maybe<Scalars['Int']['output']>;
  id: Scalars['Int']['output'];
  library?: Maybe<Scalars['String']['output']>;
  name: Scalars['String']['output'];
  note?: Maybe<Scalars['String']['output']>;
  overview?: Maybe<Scalars['String']['output']>;
  poster?: Maybe<Scalars['String']['output']>;
  provider: Provider;
  providerId: Scalars['String']['output'];
  seriesId?: Maybe<Scalars['Int']['output']>;
  state: RequestState;
  title?: Maybe<Title>;
  user?: Maybe<User>;
  year?: Maybe<Scalars['Int']['output']>;
};

export type RequestState =
  | 'APPROVED'
  | 'DECLINED'
  | 'PENDING';

export type RequestsConfig = {
  monitor: Monitor;
};

export type RequestsConfigInput = {
  monitor: Monitor;
};

export type RetryStep = {
  every: Scalars['Duration']['output'];
  until: Scalars['Duration']['output'];
};

export type RetryStepInput = {
  every: Scalars['Duration']['input'];
  until: Scalars['Duration']['input'];
};

export type Room = {
  canInvite: Scalars['Boolean']['output'];
  canShare: Scalars['Boolean']['output'];
  code: Scalars['String']['output'];
  isHost: Scalars['Boolean']['output'];
  signedIn: Scalars['Boolean']['output'];
  title: Title;
  video?: Maybe<Video>;
};


export type RoomVideoArgs = {
  id?: InputMaybe<Scalars['Int']['input']>;
};

export type ScanConfig = {
  interval?: Maybe<Scalars['Duration']['output']>;
  watch: Scalars['Boolean']['output'];
};

export type ScanConfigInput = {
  interval?: InputMaybe<Scalars['Duration']['input']>;
  watch: Scalars['Boolean']['input'];
};

export type ScanFinished = {
  library: Scalars['String']['output'];
  skipped: Scalars['Int']['output'];
  titles: Scalars['Int']['output'];
  videos: Scalars['Int']['output'];
};

export type ScanStarted = {
  library: Scalars['String']['output'];
};

/** Which schemes to use, by id: a built-in's name or a saved scheme's number. */
export type SchemeChoice = {
  dark: Scalars['String']['output'];
  light: Scalars['String']['output'];
  mode: SchemeMode;
  single: Scalars['String']['output'];
};

/** Which schemes to use, by id: a built-in's name or a saved scheme's number. */
export type SchemeChoiceInput = {
  dark: Scalars['String']['input'];
  light: Scalars['String']['input'];
  mode: SchemeMode;
  single: Scalars['String']['input'];
};

export type SchemeInput = {
  name: Scalars['String']['input'];
  overrides: Array<TokenInput>;
  /** Every seed, opaque. */
  seeds: Array<TokenInput>;
};

export type SchemeMode =
  | 'SINGLE'
  /** One scheme for the system's light mode, one for its dark mode. */
  | 'SYSTEM';

export type SchemeOrigin = {
  /** Gone when the parent was deleted or you can't see it any more. */
  id?: Maybe<Scalars['String']['output']>;
  name: Scalars['String']['output'];
};

export type SearchResults = {
  titles: Array<Title>;
  videos: Array<Video>;
};

export type Season = {
  episodes: Array<Video>;
  name: Scalars['String']['output'];
  number: Scalars['Int']['output'];
  overview?: Maybe<Scalars['String']['output']>;
  poster?: Maybe<Scalars['String']['output']>;
  title?: Maybe<Scalars['String']['output']>;
};

export type SeedAction =
  | 'PAUSE'
  | 'REMOVE';

export type SeedGoal = {
  ratio?: Maybe<Scalars['Float']['output']>;
  seconds?: Maybe<Scalars['Int']['output']>;
};

export type Seeding = {
  idle?: Maybe<Scalars['Duration']['output']>;
  ratio?: Maybe<Scalars['Float']['output']>;
  then: SeedAction;
  time?: Maybe<Scalars['Duration']['output']>;
};

export type SeedingInput = {
  idle?: InputMaybe<Scalars['Duration']['input']>;
  ratio?: InputMaybe<Scalars['Float']['input']>;
  then: SeedAction;
  time?: InputMaybe<Scalars['Duration']['input']>;
};

export type Series = {
  addedAt: Scalars['Int']['output'];
  aliases: Array<Scalars['String']['output']>;
  counts: EpisodeCounts;
  effectiveProfile: Scalars['String']['output'];
  episodes: Array<SeriesEpisode>;
  groups: Array<Scalars['String']['output']>;
  id: Scalars['Int']['output'];
  knownAs: Array<Scalars['String']['output']>;
  library: Scalars['String']['output'];
  managed: Scalars['Boolean']['output'];
  monitor: Monitor;
  name: Scalars['String']['output'];
  naming?: Maybe<Scalars['String']['output']>;
  namingPreview: NamingPreview;
  next?: Maybe<SeriesEpisode>;
  numbering: Numbering;
  overview?: Maybe<Scalars['String']['output']>;
  path: Scalars['String']['output'];
  poster?: Maybe<Scalars['String']['output']>;
  profile?: Maybe<Scalars['String']['output']>;
  provider?: Maybe<Provider>;
  providerId?: Maybe<Scalars['String']['output']>;
  releases: Array<ReleaseCandidate>;
  scheduleAt?: Maybe<Scalars['Int']['output']>;
  seeding?: Maybe<Seeding>;
  sources: Array<Scalars['String']['output']>;
  status?: Maybe<Scalars['String']['output']>;
  style: NamingStyle;
  title?: Maybe<Title>;
  year?: Maybe<Scalars['Int']['output']>;
};


export type SeriesNamingPreviewArgs = {
  file: Scalars['String']['input'];
  folder?: InputMaybe<Scalars['String']['input']>;
};


export type SeriesReleasesArgs = {
  episodes?: Array<Scalars['Int']['input']>;
  query?: InputMaybe<Scalars['String']['input']>;
  season: Scalars['Int']['input'];
};

export type SeriesChanged = {
  seriesId: Scalars['Int']['output'];
};

export type SeriesEpisode = {
  absolute?: Maybe<Scalars['Int']['output']>;
  airAt?: Maybe<Scalars['Int']['output']>;
  aired: Scalars['Boolean']['output'];
  attempts: Scalars['Int']['output'];
  downloadId?: Maybe<Scalars['Int']['output']>;
  episode: Scalars['Int']['output'];
  name?: Maybe<Scalars['String']['output']>;
  nextSearch?: Maybe<Scalars['Int']['output']>;
  searchedAt?: Maybe<Scalars['Int']['output']>;
  season: Scalars['Int']['output'];
  state: EpisodeState;
  video?: Maybe<Video>;
};

export type SeriesPatch = {
  aliases?: InputMaybe<Array<Scalars['String']['input']>>;
  groups?: InputMaybe<Array<Scalars['String']['input']>>;
  monitor?: InputMaybe<Monitor>;
  naming?: InputMaybe<Scalars['String']['input']>;
  numbering?: InputMaybe<Numbering>;
  profile?: InputMaybe<Scalars['String']['input']>;
  seeding?: InputMaybe<SeedingInput>;
  sources?: InputMaybe<Array<Scalars['String']['input']>>;
};

export type Server = {
  clips: Scalars['Boolean']['output'];
  downloads: Scalars['Boolean']['output'];
  email: Scalars['Boolean']['output'];
  setupRequired: Scalars['Boolean']['output'];
  signInStyle: SignInStyle;
  smtp: Scalars['Boolean']['output'];
  sources: Scalars['Int']['output'];
  transcoding: Transcoding;
  version: Scalars['String']['output'];
};

export type ServerAppearance = {
  colors: SchemeChoice;
  style: ComponentStyle;
};

export type ServerAppearanceInput = {
  colors: SchemeChoiceInput;
  style: ComponentStyle;
};

export type ServerPaths = {
  config: Scalars['String']['output'];
  data: Scalars['String']['output'];
  log: Scalars['String']['output'];
};

export type Settings = {
  automation: AutomationConfig;
  clips: ClipsConfig;
  downloads: DownloadsConfig;
  error?: Maybe<Scalars['String']['output']>;
  libraries: Array<ConfiguredLibrary>;
  log: LogConfig;
  metadata: MetadataConfig;
  music: MusicConfig;
  network: NetworkConfig;
  paths: ServerPaths;
  profiles: Array<Profile>;
  raw: Scalars['String']['output'];
  requests: RequestsConfig;
  scan: ScanConfig;
  signIn: SignInConfig;
  smtp: SmtpConfig;
  sources: Array<Source>;
  transcode: TranscodeConfig;
};

export type Shelf = {
  library: Scalars['String']['output'];
  titles: Array<Title>;
};

export type SignInConfig = {
  style: SignInStyle;
};

export type SignInConfigInput = {
  style: SignInStyle;
};

export type SignInProfile = {
  avatar?: Maybe<Scalars['String']['output']>;
  key: Scalars['String']['output'];
  passkey: Scalars['Boolean']['output'];
};

export type SignInStyle =
  | 'PROFILES'
  | 'USERNAME';

export type SignedIn = {
  token: Scalars['String']['output'];
  user: User;
};

export type Similar = {
  alsoWatched: Array<Title>;
  recommendations: Array<DiscoverResult>;
};

export type SkippedFile = {
  library: Scalars['String']['output'];
  path: Scalars['String']['output'];
  reason: Scalars['String']['output'];
};

export type SmtpConfig = {
  from?: Maybe<Scalars['String']['output']>;
  host?: Maybe<Scalars['String']['output']>;
  password?: Maybe<Scalars['String']['output']>;
  port?: Maybe<Scalars['Int']['output']>;
  publicUrl?: Maybe<Scalars['String']['output']>;
  security: SmtpSecurity;
  username?: Maybe<Scalars['String']['output']>;
};

export type SmtpConfigInput = {
  from?: InputMaybe<Scalars['String']['input']>;
  host?: InputMaybe<Scalars['String']['input']>;
  password?: InputMaybe<Scalars['String']['input']>;
  port?: InputMaybe<Scalars['Int']['input']>;
  publicUrl?: InputMaybe<Scalars['String']['input']>;
  security: SmtpSecurity;
  username?: InputMaybe<Scalars['String']['input']>;
};

export type SmtpSecurity =
  | 'NONE'
  | 'STARTTLS'
  | 'TLS';

export type Source = {
  apiKey?: Maybe<Scalars['String']['output']>;
  categories: Array<Scalars['Int']['output']>;
  downloadPath?: Maybe<Scalars['String']['output']>;
  enabled: Scalars['Boolean']['output'];
  feed?: Maybe<Scalars['String']['output']>;
  kind: SourceKind;
  name: Scalars['String']['output'];
  seeding?: Maybe<Seeding>;
  url: Scalars['String']['output'];
};

export type SourceInput = {
  apiKey?: InputMaybe<Scalars['String']['input']>;
  categories: Array<Scalars['Int']['input']>;
  downloadPath?: InputMaybe<Scalars['String']['input']>;
  enabled?: Scalars['Boolean']['input'];
  feed?: InputMaybe<Scalars['String']['input']>;
  kind: SourceKind;
  name: Scalars['String']['input'];
  seeding?: InputMaybe<SeedingInput>;
  url: Scalars['String']['input'];
};

export type SourceKind =
  | 'RSS'
  | 'TORZNAB';

export type SourceStatus =
  | 'CHANGED'
  | 'GONE'
  | 'OK';

export type Subscription = {
  events: Event;
};

export type SubtitleTrack = {
  codec: Scalars['String']['output'];
  default: Scalars['Boolean']['output'];
  forced: Scalars['Boolean']['output'];
  id: Scalars['String']['output'];
  language?: Maybe<Scalars['String']['output']>;
  supported: Scalars['Boolean']['output'];
  title?: Maybe<Scalars['String']['output']>;
};

export type Title = {
  backdrop?: Maybe<Scalars['String']['output']>;
  /** The same for the backdrop. */
  backdropTint?: Maybe<Scalars['String']['output']>;
  customBackdrop: Scalars['Boolean']['output'];
  customPoster: Scalars['Boolean']['output'];
  freshCount: Scalars['Int']['output'];
  genres: Array<Scalars['String']['output']>;
  id: Scalars['Int']['output'];
  kind: TitleKind;
  library: Scalars['String']['output'];
  libraryProvider?: Maybe<Provider>;
  matchCandidates: MatchCandidates;
  matchState: MatchState;
  movie?: Maybe<Video>;
  name: Scalars['String']['output'];
  nextUp?: Maybe<NextUp>;
  overview?: Maybe<Scalars['String']['output']>;
  path?: Maybe<Scalars['String']['output']>;
  poster?: Maybe<Scalars['String']['output']>;
  /** The most vivid colour of the poster as "r g b", empty until it's been worked out. */
  posterTint?: Maybe<Scalars['String']['output']>;
  progress?: Maybe<Scalars['Float']['output']>;
  provider?: Maybe<Provider>;
  providerId?: Maybe<Scalars['String']['output']>;
  rating?: Maybe<Scalars['Float']['output']>;
  seasons: Array<Season>;
  series?: Maybe<Series>;
  similar: Similar;
  videoCount: Scalars['Int']['output'];
  videos: Array<Video>;
  watchedCount: Scalars['Int']['output'];
  year?: Maybe<Scalars['Int']['output']>;
};


export type TitleMatchCandidatesArgs = {
  provider?: InputMaybe<Provider>;
  query?: InputMaybe<Scalars['String']['input']>;
};

export type TitleArtwork =
  | 'BACKDROP'
  | 'POSTER';

export type TitleKind =
  | 'MOVIE'
  | 'SHOW';

export type Token = {
  name: Scalars['String']['output'];
  value: Scalars['String']['output'];
};

export type TokenInput = {
  name: Scalars['String']['input'];
  value: Scalars['String']['input'];
};

export type TorrentFile = {
  done: Scalars['Int']['output'];
  path: Scalars['String']['output'];
  size: Scalars['Int']['output'];
};

export type TorrentStage =
  | 'CHECKING'
  | 'DOWNLOADING'
  | 'METADATA'
  | 'SEEDING';

export type TorrentStatus = {
  done: Scalars['Int']['output'];
  downloadRate: Scalars['Int']['output'];
  eta?: Maybe<Scalars['Int']['output']>;
  paused: Scalars['Boolean']['output'];
  peers: Scalars['Int']['output'];
  pieces: Array<Scalars['Int']['output']>;
  progress: Scalars['Float']['output'];
  ratio: Scalars['Float']['output'];
  seedingSeconds: Scalars['Int']['output'];
  seeds: Scalars['Int']['output'];
  stage: TorrentStage;
  uploadRate: Scalars['Int']['output'];
  uploaded: Scalars['Int']['output'];
};

export type Track = {
  addedAt: Scalars['Int']['output'];
  album: Scalars['String']['output'];
  albumArtist?: Maybe<Scalars['String']['output']>;
  albumId?: Maybe<Scalars['Int']['output']>;
  /** The artists as tagged, for display. */
  artist: Scalars['String']['output'];
  artists: Array<ArtistRef>;
  bitDepth?: Maybe<Scalars['Int']['output']>;
  /** kbit/s. */
  bitrate?: Maybe<Scalars['Int']['output']>;
  channels?: Maybe<Scalars['Int']['output']>;
  /** flac, alac, mp3, aac, opus, vorbis, pcm, ape, wavpack, musepack… */
  codec: Scalars['String']['output'];
  composers: Array<ArtistRef>;
  cover?: Maybe<Scalars['String']['output']>;
  /** The most vivid colour of the cover as "r g b", empty until it's been worked out. */
  coverTint?: Maybe<Scalars['String']['output']>;
  disc?: Maybe<Scalars['Int']['output']>;
  duration: Scalars['Float']['output'];
  /** The file as it is. */
  file: Scalars['String']['output'];
  /** The same as FLAC, made once and kept, for players that can't read the original. */
  flac: Scalars['String']['output'];
  gains: Gains;
  genres: Array<Scalars['String']['output']>;
  id: Scalars['Int']['output'];
  lastPlayed?: Maybe<Scalars['Int']['output']>;
  library: Scalars['String']['output'];
  lossless: Scalars['Boolean']['output'];
  mbid?: Maybe<Scalars['String']['output']>;
  number?: Maybe<Scalars['Int']['output']>;
  playCount: Scalars['Int']['output'];
  rating?: Maybe<Scalars['Int']['output']>;
  sampleRate?: Maybe<Scalars['Int']['output']>;
  size: Scalars['Int']['output'];
  starred: Scalars['Boolean']['output'];
  /** The same, converted; add `?format=flac|opus|mp3|aac&start=`. */
  stream: Scalars['String']['output'];
  suffix: Scalars['String']['output'];
  title: Scalars['String']['output'];
  year?: Maybe<Scalars['Int']['output']>;
};

export type TracksInput = {
  audio?: InputMaybe<Scalars['Int']['input']>;
  audioChannels?: InputMaybe<Scalars['Int']['input']>;
  audioCodec?: InputMaybe<Scalars['String']['input']>;
  audioLanguage?: InputMaybe<Scalars['String']['input']>;
  audioTitle?: InputMaybe<Scalars['String']['input']>;
  subtitle?: InputMaybe<Scalars['String']['input']>;
  subtitleCodec?: InputMaybe<Scalars['String']['input']>;
  subtitleForced?: Scalars['Boolean']['input'];
  subtitleLanguage?: InputMaybe<Scalars['String']['input']>;
  subtitleTitle?: InputMaybe<Scalars['String']['input']>;
};

export type TranscodeConfig = {
  hardware: Hardware;
  vaapiDevice: Scalars['String']['output'];
};

export type TranscodeConfigInput = {
  hardware: Hardware;
  vaapiDevice: Scalars['String']['input'];
};

export type Transcoding = {
  softwareH264: Scalars['Boolean']['output'];
  vaapi?: Maybe<Scalars['String']['output']>;
  vaapiError?: Maybe<Scalars['String']['output']>;
};

export type UndoReport = {
  problems: Array<Scalars['String']['output']>;
  undone: Scalars['Int']['output'];
};

export type User = {
  avatar?: Maybe<Scalars['String']['output']>;
  createdAt: Scalars['Int']['output'];
  email?: Maybe<Scalars['String']['output']>;
  id: Scalars['Int']['output'];
  isAdmin: Scalars['Boolean']['output'];
  lastSeen?: Maybe<Scalars['Int']['output']>;
  overrides: PermissionOverrides;
  passkeys: Array<Passkey>;
  permissions: Permissions;
  username: Scalars['String']['output'];
};

export type UserPatch = {
  isAdmin?: InputMaybe<Scalars['Boolean']['input']>;
  password?: InputMaybe<Scalars['String']['input']>;
  permissions?: InputMaybe<PermissionOverridesInput>;
  username?: InputMaybe<Scalars['String']['input']>;
};

export type Verdict = {
  accepted: Scalars['Boolean']['output'];
  nonstandard: Scalars['Boolean']['output'];
  rejections: Array<Scalars['String']['output']>;
  score: Scalars['Int']['output'];
  warnings: Array<Scalars['String']['output']>;
};

export type Video = {
  airDate?: Maybe<Scalars['String']['output']>;
  customStill: Scalars['Boolean']['output'];
  duration?: Maybe<Scalars['Float']['output']>;
  episode?: Maybe<Scalars['Int']['output']>;
  episodeEnd?: Maybe<Scalars['Int']['output']>;
  finished?: Maybe<Scalars['Boolean']['output']>;
  id: Scalars['Int']['output'];
  label?: Maybe<Scalars['String']['output']>;
  media: MediaInfo;
  name?: Maybe<Scalars['String']['output']>;
  next?: Maybe<Video>;
  overview?: Maybe<Scalars['String']['output']>;
  position?: Maybe<Scalars['Float']['output']>;
  previous?: Maybe<Video>;
  season?: Maybe<Scalars['Int']['output']>;
  still: Scalars['String']['output'];
  title: Title;
  watchedAt?: Maybe<Scalars['Int']['output']>;
};

export type VideoTrack = {
  bitDepth: Scalars['Int']['output'];
  codec: Scalars['String']['output'];
  codecString?: Maybe<Scalars['String']['output']>;
  fps: Scalars['Float']['output'];
  hdr: Scalars['Boolean']['output'];
  height: Scalars['Int']['output'];
  index: Scalars['Int']['output'];
  width: Scalars['Int']['output'];
};

export type WantedEpisode = {
  airAt?: Maybe<Scalars['Int']['output']>;
  aired: Scalars['Boolean']['output'];
  attempts: Scalars['Int']['output'];
  episode: Scalars['Int']['output'];
  name?: Maybe<Scalars['String']['output']>;
  nextSearch?: Maybe<Scalars['Int']['output']>;
  searchedAt?: Maybe<Scalars['Int']['output']>;
  season: Scalars['Int']['output'];
  seriesId: Scalars['Int']['output'];
  show: Scalars['String']['output'];
  state: EpisodeState;
  title?: Maybe<Title>;
};
