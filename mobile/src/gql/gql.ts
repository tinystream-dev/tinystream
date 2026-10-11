/* eslint-disable */
import * as types from './graphql';



/**
 * Map of all GraphQL operations in the project.
 *
 * This map has several performance disadvantages:
 * 1. It is not tree-shakeable, so it will include all operations in the project.
 * 2. It is not minifiable, so the string of a GraphQL query will be multiple times inside the bundle.
 * 3. It does not support dead code elimination, so it will add unused operations.
 *
 * Therefore it is highly recommended to use the babel or swc plugin for production.
 * Learn more about it here: https://the-guild.dev/graphql/codegen/plugins/presets/preset-client#reducing-bundle-size
 */
type Documents = {
    "\n  query Calendar($from: Int!, $to: Int!) {\n    calendar(from: $from, to: $to) {\n      ...CalendarEntryFields\n    }\n  }\n": typeof types.CalendarDocument,
    "\n  query Downloads {\n    downloads {\n      ...DownloadFields\n    }\n  }\n": typeof types.DownloadsDocument,
    "\n  query Engine {\n    downloadEngine {\n      ...EngineFields\n    }\n  }\n": typeof types.EngineDocument,
    "\n  mutation DownloadsPause($ids: [Int!]!) {\n    pauseDownloads(ids: $ids) {\n      id\n    }\n  }\n": typeof types.DownloadsPauseDocument,
    "\n  mutation DownloadsResume($ids: [Int!]!) {\n    resumeDownloads(ids: $ids) {\n      id\n    }\n  }\n": typeof types.DownloadsResumeDocument,
    "\n  mutation DownloadsRecheck($ids: [Int!]!) {\n    recheckDownloads(ids: $ids) {\n      id\n    }\n  }\n": typeof types.DownloadsRecheckDocument,
    "\n  mutation ImportDownload($id: Int!) {\n    importDownload(id: $id) {\n      id\n    }\n  }\n": typeof types.ImportDownloadDocument,
    "\n  mutation RemoveDownloads($ids: [Int!]!, $deleteFiles: Boolean!) {\n    removeDownloads(ids: $ids, deleteFiles: $deleteFiles)\n  }\n": typeof types.RemoveDownloadsDocument,
    "\n  query Home {\n    home {\n      continueWatching {\n        position\n        upNext\n        newEpisode\n        watchedAt\n        video {\n          id\n          label\n          name\n          still\n          duration\n          title {\n            id\n            name\n            poster\n            backdrop\n            posterTint\n            backdropTint\n          }\n        }\n      }\n      recentlyAdded {\n        library\n        titles {\n          ...Card\n        }\n      }\n      popularHere {\n        people\n        title {\n          ...Card\n        }\n      }\n    }\n  }\n": typeof types.HomeDocument,
    "\n  query ComingUp($from: Int!, $to: Int!) {\n    calendar(from: $from, to: $to) {\n      ...CalendarEntryFields\n    }\n  }\n": typeof types.ComingUpDocument,
    "\n  query MusicHome {\n    musicHome {\n      recentlyPlayed {\n        id\n        name\n        artist\n        cover\n      }\n      recentlyAdded {\n        id\n        name\n        artist\n        cover\n      }\n    }\n  }\n": typeof types.MusicHomeDocument,
    "\n  query Library($name: String!) {\n    library(name: $name) {\n      titles {\n        ...Card\n      }\n    }\n  }\n": typeof types.LibraryDocument,
    "\n  query Transfers {\n    downloadEngine {\n      downloadRate\n      killSwitch\n    }\n    downloads {\n      id\n      state\n      importState\n    }\n  }\n": typeof types.TransfersDocument,
    "\n  query RequestCount {\n    requests {\n      id\n      state\n    }\n  }\n": typeof types.RequestCountDocument,
    "\n  query Requests {\n    requests {\n      id\n      user {\n        ...Person\n      }\n      name\n      year\n      poster\n      library\n      state\n      title {\n        id\n      }\n      note\n      createdAt\n      have\n      aired\n    }\n  }\n": typeof types.RequestsDocument,
    "\n  mutation ApproveRequest($id: Int!) {\n    approveRequest(id: $id) {\n      id\n    }\n  }\n": typeof types.ApproveRequestDocument,
    "\n  mutation DeclineRequest($id: Int!) {\n    declineRequest(id: $id) {\n      id\n    }\n  }\n": typeof types.DeclineRequestDocument,
    "\n  mutation DeleteRequest($id: Int!) {\n    deleteRequest(id: $id)\n  }\n": typeof types.DeleteRequestDocument,
    "\n  query Search($query: String!) {\n    musicSearch(query: $query, limit: 5) {\n      artists {\n        id\n        name\n        cover\n        albumCount\n      }\n      albums {\n        id\n        name\n        artist\n        cover\n        year\n      }\n      tracks {\n        id\n        title\n        artist\n        album\n        cover\n      }\n    }\n    search(query: $query) {\n      titles {\n        ...Card\n      }\n      videos {\n        id\n        label\n        name\n        title {\n          name\n        }\n      }\n    }\n  }\n": typeof types.SearchDocument,
    "\n  query RecentTitles($ids: [Int!]!) {\n    titles(ids: $ids) {\n      id\n    }\n  }\n": typeof types.RecentTitlesDocument,
    "\n  query DownloadStates {\n    downloads {\n      id\n      state\n    }\n  }\n": typeof types.DownloadStatesDocument,
    "\n  mutation PauseDownloads($ids: [Int!]!) {\n    pauseDownloads(ids: $ids) {\n      id\n    }\n  }\n": typeof types.PauseDownloadsDocument,
    "\n  mutation ResumeDownloads($ids: [Int!]!) {\n    resumeDownloads(ids: $ids) {\n      id\n    }\n  }\n": typeof types.ResumeDownloadsDocument,
    "\n  query Discover($library: String, $query: String) {\n    discover(library: $library, query: $query) {\n      library\n      results {\n        ...DiscoverResultFields\n      }\n    }\n  }\n": typeof types.DiscoverDocument,
    "\n  query ForYou($library: String) {\n    forYou(library: $library) {\n      library\n      shelves {\n        key\n        name\n        results {\n          ...DiscoverResultFields\n        }\n      }\n    }\n  }\n": typeof types.ForYouDocument,
    "\n  query Similar($id: Int!) {\n    title(id: $id) {\n      similar {\n        recommendations {\n          ...DiscoverResultFields\n        }\n        alsoWatched {\n          ...Card\n        }\n      }\n    }\n  }\n": typeof types.SimilarDocument,
    "\n  query MatchCandidates($id: Int!, $query: String, $provider: Provider) {\n    title(id: $id) {\n      matchCandidates(query: $query, provider: $provider) {\n        query\n        results {\n          provider\n          id\n          name\n          year\n          poster\n          overview\n        }\n      }\n    }\n  }\n": typeof types.MatchCandidatesDocument,
    "\n  query TitleDownloads {\n    downloads {\n      ...DownloadFields\n    }\n  }\n": typeof types.TitleDownloadsDocument,
    "\n  mutation SetWatched($videoIds: [Int!]!, $watched: Boolean!) {\n    setWatched(videoIds: $videoIds, watched: $watched) {\n      id\n    }\n  }\n": typeof types.SetWatchedDocument,
    "\n  mutation SetTitleWatched($id: Int!, $watched: Boolean!) {\n    setTitleWatched(id: $id, watched: $watched) {\n      id\n    }\n  }\n": typeof types.SetTitleWatchedDocument,
    "\n  mutation RefreshTitle($id: Int!) {\n    refreshTitle(id: $id) {\n      id\n    }\n  }\n": typeof types.RefreshTitleDocument,
    "\n  mutation MatchTitle($id: Int!, $provider: Provider!, $providerId: String!) {\n    matchTitle(id: $id, provider: $provider, providerId: $providerId) {\n      id\n    }\n  }\n": typeof types.MatchTitleDocument,
    "\n  query Wanted {\n    wanted {\n      seriesId\n      title {\n        id\n      }\n      show\n      season\n      episode\n      name\n      airAt\n      aired\n      state\n      attempts\n      searchedAt\n      nextSearch\n    }\n  }\n": typeof types.WantedDocument,
    "\n  query Attention($requests: Boolean!, $downloads: Boolean!) {\n    requests @include(if: $requests) {\n      id\n      state\n    }\n    downloads @include(if: $downloads) {\n      id\n      state\n      importState\n    }\n  }\n": typeof types.AttentionDocument,
    "\n  query SignInServer {\n    server {\n      setupRequired\n      signInStyle\n    }\n  }\n": typeof types.SignInServerDocument,
    "\n  query SignInProfiles {\n    signInProfiles {\n      key\n      avatar\n      passkey\n    }\n  }\n": typeof types.SignInProfilesDocument,
    "\n  mutation Setup($username: String!, $password: String!) {\n    setup(username: $username, password: $password) {\n      user {\n        ...Person\n      }\n      token\n    }\n  }\n": typeof types.SetupDocument,
    "\n  mutation SignIn($username: String, $profile: String, $password: String!) {\n    signIn(username: $username, profile: $profile, password: $password) {\n      user {\n        ...Person\n      }\n      token\n    }\n  }\n": typeof types.SignInDocument,
    "\n  mutation SetTitleArtwork($id: Int!, $kind: TitleArtwork!, $image: Upload) {\n    setTitleArtwork(id: $id, kind: $kind, image: $image) {\n      id\n    }\n  }\n": typeof types.SetTitleArtworkDocument,
    "\n  mutation SetVideoArtwork($videoId: Int!, $image: Upload) {\n    setVideoArtwork(videoId: $videoId, image: $image) {\n      id\n    }\n  }\n": typeof types.SetVideoArtworkDocument,
    "\n  mutation SetAvatar($image: Upload!, $userId: Int) {\n    setAvatar(image: $image, userId: $userId) {\n      id\n      avatar\n    }\n  }\n": typeof types.SetAvatarDocument,
    "\n  mutation RemoveAvatar($userId: Int) {\n    removeAvatar(userId: $userId) {\n      id\n      avatar\n    }\n  }\n": typeof types.RemoveAvatarDocument,
    "\n  mutation AddSeries($input: NewSeries!) {\n    addSeries(input: $input) {\n      id\n    }\n  }\n": typeof types.AddSeriesDocument,
    "\n  query AiredEpisodes($provider: Provider!, $id: String!) {\n    airedEpisodes(provider: $provider, id: $id)\n  }\n": typeof types.AiredEpisodesDocument,
    "\n  mutation CreateRequest($input: NewRequest!) {\n    createRequest(input: $input) {\n      id\n    }\n  }\n": typeof types.CreateRequestDocument,
    "\n  subscription Events {\n    events {\n      __typename\n      ... on ConfigChanged {\n        error\n      }\n      ... on ScanFinished {\n        library\n      }\n      ... on LibraryChanged {\n        library\n      }\n      ... on MetadataChanged {\n        titleId\n        status\n      }\n      ... on ListChanged {\n        list\n      }\n      ... on SeriesChanged {\n        seriesId\n      }\n      ... on EpisodesImported {\n        library\n      }\n      ... on NotificationReceived {\n        notification {\n          ...NotificationFields\n        }\n      }\n      ... on ClipChanged {\n        clipId\n      }\n    }\n  }\n": typeof types.EventsDocument,
    "\n  query TitleSeries($id: Int!) {\n    title(id: $id) {\n      series {\n        ...SeriesFields\n      }\n    }\n  }\n": typeof types.TitleSeriesDocument,
    "\n  query TitleSchedule($id: Int!) {\n    title(id: $id) {\n      series {\n        id\n        monitor\n        status\n        next {\n          ...SeriesEpisodeFields\n        }\n      }\n    }\n  }\n": typeof types.TitleScheduleDocument,
    "\n  mutation ManageTitle($titleId: Int!) {\n    manageTitle(titleId: $titleId) {\n      id\n    }\n  }\n": typeof types.ManageTitleDocument,
    "\n  mutation UpdateSeries($id: Int!, $patch: SeriesPatch!) {\n    updateSeries(id: $id, patch: $patch) {\n      ...SeriesFields\n    }\n  }\n": typeof types.UpdateSeriesDocument,
    "\n  mutation RefreshSeriesSchedule($id: Int!) {\n    refreshSeriesSchedule(id: $id) {\n      id\n    }\n  }\n": typeof types.RefreshSeriesScheduleDocument,
    "\n  mutation RemoveSeries($id: Int!) {\n    removeSeries(id: $id)\n  }\n": typeof types.RemoveSeriesDocument,
    "\n  query NamingPreview($id: Int!, $file: String!) {\n    series(id: $id) {\n      namingPreview(file: $file) {\n        samples\n        error\n      }\n    }\n  }\n": typeof types.NamingPreviewDocument,
    "\n  query Releases($seriesId: Int!, $season: Int!, $episodes: [Int!]!, $query: String) {\n    series(id: $seriesId) {\n      releases(season: $season, episodes: $episodes, query: $query) {\n        ...ReleaseCandidateFields\n      }\n    }\n  }\n": typeof types.ReleasesDocument,
    "\n  mutation GrabRelease($release: ReleaseInput!, $seriesId: Int, $episodes: [EpisodeNumberInput!]!) {\n    grabRelease(release: $release, seriesId: $seriesId, episodes: $episodes) {\n      id\n    }\n  }\n": typeof types.GrabReleaseDocument,
    "\n  mutation DeleteDownloaded($seriesId: Int!, $season: Int) {\n    deleteDownloaded(seriesId: $seriesId, season: $season) {\n      undone\n      problems\n    }\n  }\n": typeof types.DeleteDownloadedDocument,
    "\n  mutation LookForAgain($seriesId: Int!, $season: Int, $episode: Int) {\n    lookForAgain(seriesId: $seriesId, season: $season, episode: $episode)\n  }\n": typeof types.LookForAgainDocument,
    "\n  fragment InboxFields on Inbox {\n    items {\n      ...NotificationFields\n    }\n    unread\n  }\n": typeof types.InboxFieldsFragmentDoc,
    "\n  query Inbox {\n    notifications {\n      ...InboxFields\n    }\n  }\n": typeof types.InboxDocument,
    "\n  mutation MarkNotificationsRead($ids: [Int!]) {\n    markNotificationsRead(ids: $ids) {\n      ...InboxFields\n    }\n  }\n": typeof types.MarkNotificationsReadDocument,
    "\n  mutation DeleteNotifications($id: Int) {\n    deleteNotifications(id: $id) {\n      ...InboxFields\n    }\n  }\n": typeof types.DeleteNotificationsDocument,
    "\n  query Playback($id: Int!) {\n    video(id: $id) {\n      ...Playback\n    }\n    server {\n      transcoding {\n        ...TranscodingFields\n      }\n    }\n  }\n": typeof types.PlaybackDocument,
    "\n  mutation SaveProgress($videoId: Int!, $position: Float!, $duration: Float!) {\n    saveProgress(videoId: $videoId, position: $position, duration: $duration) {\n      id\n    }\n  }\n": typeof types.SaveProgressDocument,
    "\n  mutation TakeScreenshot($input: NewScreenshot!) {\n    takeScreenshot(input: $input) {\n      ...ClipFields\n    }\n  }\n": typeof types.TakeScreenshotDocument,
    "\n  query PlayerSchedule($id: Int!) {\n    title(id: $id) {\n      series {\n        id\n        monitor\n        status\n        next {\n          ...SeriesEpisodeFields\n        }\n      }\n    }\n  }\n": typeof types.PlayerScheduleDocument,
    "\n  query PlayerOverview($id: Int!, $videoId: Int!) {\n    title(id: $id) {\n      overview\n    }\n    video(id: $videoId) {\n      overview\n    }\n  }\n": typeof types.PlayerOverviewDocument,
    "\n  query Status {\n    server {\n      version\n      setupRequired\n      clips\n      downloads\n      sources\n    }\n    viewer {\n      ...Viewer\n    }\n  }\n": typeof types.StatusDocument,
    "\n  query Appearance {\n    appearance {\n      mode\n      style\n      mediaTint\n      light {\n        id\n        palette {\n          tokens {\n            name\n            value\n          }\n        }\n      }\n      dark {\n        id\n        palette {\n          tokens {\n            name\n            value\n          }\n        }\n      }\n    }\n  }\n": typeof types.AppearanceDocument,
    "\n  query Libraries {\n    libraries {\n      name\n      kind\n      showCount\n      movieCount\n      albumCount\n      trackCount\n    }\n  }\n": typeof types.LibrariesDocument,
    "\n  query Settings {\n    settings {\n      ...SettingsFields\n    }\n    server {\n      transcoding {\n        ...TranscodingFields\n      }\n    }\n  }\n": typeof types.SettingsDocument,
    "\n  query Title($id: Int!) {\n    title(id: $id) {\n      ...TitleDetail\n    }\n  }\n": typeof types.TitleDocument,
    "\n  query People {\n    users {\n      ...Person\n    }\n  }\n": typeof types.PeopleDocument,
    "\n  mutation SignOut {\n    signOut\n  }\n": typeof types.SignOutDocument,
    "\n  query Passkeys {\n    viewer {\n      passkeys {\n        id\n        name\n        createdAt\n        lastUsed\n      }\n    }\n  }\n": typeof types.PasskeysDocument,
    "\n  mutation DeletePasskey($id: Int!) {\n    deletePasskey(id: $id) {\n      id\n    }\n  }\n": typeof types.DeletePasskeyDocument,
    "\n  mutation ChangePassword($current: String!, $new: String!) {\n    changePassword(current: $current, new: $new)\n  }\n": typeof types.ChangePasswordDocument,
    "\n  query AppPasswords {\n    appPasswords {\n      id\n      name\n      createdAt\n      lastUsed\n      client\n    }\n  }\n": typeof types.AppPasswordsDocument,
    "\n  mutation CreateAppPassword($name: String!) {\n    createAppPassword(name: $name) {\n      secret\n      password {\n        id\n        name\n      }\n    }\n  }\n": typeof types.CreateAppPasswordDocument,
    "\n  mutation DeleteAppPassword($id: Int!) {\n    deleteAppPassword(id: $id)\n  }\n": typeof types.DeleteAppPasswordDocument,
    "\n  fragment SchemeFields on ColorScheme {\n    id\n    name\n    builtIn\n    published\n    editable\n    code\n    shareCode\n    forkedFrom {\n      id\n      name\n    }\n    palette {\n      seeds {\n        name\n        value\n      }\n      overrides {\n        name\n        value\n      }\n      tokens {\n        name\n        value\n      }\n      warnings {\n        foreground\n        background\n        ratio\n        minimum\n      }\n    }\n  }\n": typeof types.SchemeFieldsFragmentDoc,
    "\n  query ColorSchemes {\n    colorSchemes {\n      ...SchemeFields\n    }\n  }\n": typeof types.ColorSchemesDocument,
    "\n  query AppearanceSettings {\n    appearanceSettings {\n      colors {\n        mode\n        single\n        light\n        dark\n      }\n      style\n      mediaTint\n    }\n  }\n": typeof types.AppearanceSettingsDocument,
    "\n  query ServerAppearance {\n    serverAppearance {\n      colors {\n        mode\n        single\n        light\n        dark\n      }\n      style\n    }\n  }\n": typeof types.ServerAppearanceDocument,
    "\n  mutation SetAppearance($input: AppearanceSettingsInput!) {\n    setAppearance(input: $input) {\n      mode\n    }\n  }\n": typeof types.SetAppearanceDocument,
    "\n  mutation SetServerAppearance($input: ServerAppearanceInput!) {\n    setServerAppearance(input: $input) {\n      style\n    }\n  }\n": typeof types.SetServerAppearanceDocument,
    "\n  mutation SaveScheme($id: String, $input: SchemeInput!) {\n    saveScheme(id: $id, input: $input) {\n      ...SchemeFields\n    }\n  }\n": typeof types.SaveSchemeDocument,
    "\n  mutation ForkScheme($id: String!) {\n    forkScheme(id: $id) {\n      ...SchemeFields\n    }\n  }\n": typeof types.ForkSchemeDocument,
    "\n  mutation ImportScheme($code: String!, $name: String) {\n    importScheme(code: $code, name: $name) {\n      ...SchemeFields\n    }\n  }\n": typeof types.ImportSchemeDocument,
    "\n  mutation DeleteScheme($id: String!) {\n    deleteScheme(id: $id)\n  }\n": typeof types.DeleteSchemeDocument,
    "\n  mutation PublishScheme($id: String!, $published: Boolean!) {\n    publishScheme(id: $id, published: $published) {\n      id\n    }\n  }\n": typeof types.PublishSchemeDocument,
    "\n  query DecodeScheme($code: String!) {\n    decodeScheme(code: $code) {\n      name\n      code\n      palette {\n        tokens {\n          name\n          value\n        }\n        warnings {\n          foreground\n          background\n          ratio\n          minimum\n        }\n      }\n    }\n  }\n": typeof types.DecodeSchemeDocument,
    "\n  query SettingsEngine {\n    downloadEngine {\n      downloadPath\n      killSwitch\n    }\n  }\n": typeof types.SettingsEngineDocument,
    "\n  mutation AddSource($input: SourceInput!) {\n    addSource(input: $input) {\n      raw\n    }\n  }\n": typeof types.AddSourceDocument,
    "\n  mutation UpdateSource($name: String!, $input: SourceInput!) {\n    updateSource(name: $name, input: $input) {\n      raw\n    }\n  }\n": typeof types.UpdateSourceDocument,
    "\n  mutation RemoveSource($name: String!) {\n    removeSource(name: $name) {\n      raw\n    }\n  }\n": typeof types.RemoveSourceDocument,
    "\n  query DetectSource($url: String!, $apiKey: String) {\n    detectSource(url: $url, apiKey: $apiKey) {\n      kind\n      url\n      feed\n      name\n      searchable\n      sample {\n        link\n        title\n        size\n        seeders\n        published\n      }\n    }\n  }\n": typeof types.DetectSourceDocument,
    "\n  mutation AddProfile($input: ProfileInput!) {\n    addProfile(input: $input) {\n      raw\n    }\n  }\n": typeof types.AddProfileDocument,
    "\n  mutation UpdateProfile($name: String!, $input: ProfileInput!) {\n    updateProfile(name: $name, input: $input) {\n      raw\n    }\n  }\n": typeof types.UpdateProfileDocument,
    "\n  mutation RemoveProfile($name: String!) {\n    removeProfile(name: $name) {\n      raw\n    }\n  }\n": typeof types.RemoveProfileDocument,
    "\n  query RenameSuggestions {\n    renameSuggestions {\n      id\n      library\n      managed\n      root\n      src\n      dst\n      reason\n      confidence\n    }\n  }\n": typeof types.RenameSuggestionsDocument,
    "\n  query FileHistory {\n    fileHistory {\n      batch\n      label\n      at\n      count\n      undone\n      operations {\n        kind\n        src\n        dst\n      }\n    }\n  }\n": typeof types.FileHistoryDocument,
    "\n  mutation RefreshRenameSuggestions {\n    refreshRenameSuggestions\n  }\n": typeof types.RefreshRenameSuggestionsDocument,
    "\n  mutation ApplyRenames($ids: [Int!]!) {\n    applyRenames(ids: $ids) {\n      renamed\n      problems\n    }\n  }\n": typeof types.ApplyRenamesDocument,
    "\n  mutation DismissRenames($ids: [Int!]!) {\n    dismissRenames(ids: $ids)\n  }\n": typeof types.DismissRenamesDocument,
    "\n  mutation UndoFileChanges($batch: String!) {\n    undoFileChanges(batch: $batch) {\n      undone\n      problems\n    }\n  }\n": typeof types.UndoFileChangesDocument,
    "\n  mutation Scan($library: String) {\n    scan(library: $library)\n  }\n": typeof types.ScanDocument,
    "\n  mutation AddLibrary($input: LibraryInput!) {\n    addLibrary(input: $input) {\n      raw\n    }\n  }\n": typeof types.AddLibraryDocument,
    "\n  mutation UpdateLibrary($name: String!, $input: LibraryInput!) {\n    updateLibrary(name: $name, input: $input) {\n      raw\n    }\n  }\n": typeof types.UpdateLibraryDocument,
    "\n  mutation RemoveLibrary($name: String!) {\n    removeLibrary(name: $name) {\n      raw\n    }\n  }\n": typeof types.RemoveLibraryDocument,
    "\n  query Folders($path: String) {\n    folders(path: $path) {\n      path\n      parent\n      home\n      folders {\n        name\n        path\n      }\n    }\n  }\n": typeof types.FoldersDocument,
    "\n  mutation ReplaceConfig($text: String!) {\n    replaceConfig(text: $text) {\n      raw\n    }\n  }\n": typeof types.ReplaceConfigDocument,
    "\n  query SkippedFiles {\n    skippedFiles {\n      library\n      path\n      reason\n    }\n  }\n": typeof types.SkippedFilesDocument,
    "\n  query Users {\n    users {\n      ...Viewer\n      createdAt\n      lastSeen\n      overrides {\n        allLibraries\n        libraries\n        request\n        autoApprove\n        requestLimit\n        manageRequests\n        manageShows\n        downloads\n        editMetadata\n        watchTogether\n        shareLinks\n        clip\n        clipMaxLength\n        clipLimit\n        clipStorage\n        clipLinks\n      }\n    }\n  }\n": typeof types.UsersDocument,
    "\n  query PermissionDefaults {\n    permissionDefaults {\n      ...PermissionsFields\n    }\n  }\n": typeof types.PermissionDefaultsDocument,
    "\n  mutation SetPermissionDefaults($permissions: PermissionsInput!) {\n    setPermissionDefaults(permissions: $permissions) {\n      ...PermissionsFields\n    }\n  }\n": typeof types.SetPermissionDefaultsDocument,
    "\n  mutation UpdateUser($id: Int!, $input: UserPatch!) {\n    updateUser(id: $id, input: $input) {\n      id\n    }\n  }\n": typeof types.UpdateUserDocument,
    "\n  mutation DeleteUser($id: Int!) {\n    deleteUser(id: $id)\n  }\n": typeof types.DeleteUserDocument,
    "\n  mutation CreateUser($input: NewUser!) {\n    createUser(input: $input) {\n      id\n    }\n  }\n": typeof types.CreateUserDocument,
    "\n  mutation SaveSection($patch: ConfigPatch!) {\n    updateSettings(patch: $patch) {\n      raw\n    }\n  }\n": typeof types.SaveSectionDocument,
    "fragment Person on User {\n  id\n  username\n  avatar\n}\n\nfragment PermissionsFields on Permissions {\n  allLibraries\n  libraries\n  request\n  autoApprove\n  requestLimit\n  manageRequests\n  manageShows\n  downloads\n  editMetadata\n  watchTogether\n  shareLinks\n  clip\n  clipMaxLength\n  clipLimit\n  clipStorage\n  clipLinks\n}\n\nfragment Viewer on User {\n  ...Person\n  isAdmin\n  permissions {\n    ...PermissionsFields\n  }\n}\n\nfragment Card on Title {\n  id\n  kind\n  library\n  name\n  year\n  poster\n  backdrop\n  watchedCount\n  videoCount\n  progress\n  freshCount\n}\n\nfragment VideoRow on Video {\n  id\n  season\n  episode\n  episodeEnd\n  label\n  name\n  overview\n  still\n  customStill\n  airDate\n  duration\n  position\n  finished\n}\n\nfragment TitleDetail on Title {\n  ...Card\n  posterTint\n  backdropTint\n  customPoster\n  customBackdrop\n  overview\n  genres\n  rating\n  path\n  matchState\n  provider\n  providerId\n  libraryProvider\n  seasons {\n    number\n    name\n    title\n    overview\n    poster\n    episodes {\n      ...VideoRow\n    }\n  }\n  movie {\n    ...VideoRow\n  }\n  nextUp {\n    resuming\n    video {\n      ...VideoRow\n    }\n  }\n}\n\nfragment Playback on Video {\n  id\n  still\n  label\n  name\n  position\n  finished\n  title {\n    id\n    kind\n    name\n    backdrop\n  }\n  previous {\n    id\n    label\n    name\n  }\n  next {\n    id\n    still\n    label\n    name\n  }\n  media {\n    duration\n    video {\n      index\n      codec\n      codecString\n      width\n      height\n      fps\n      bitDepth\n      hdr\n    }\n    audio {\n      index\n      codec\n      codecString\n      channels\n      language\n      title\n      default\n    }\n    subtitles {\n      id\n      codec\n      language\n      title\n      default\n      forced\n      supported\n    }\n    fonts {\n      index\n      filename\n    }\n    chapters {\n      start\n      end\n      title\n    }\n  }\n}\n\nfragment TranscodingFields on Transcoding {\n  vaapi\n  vaapiError\n  softwareH264\n}\n\nfragment DiscoverResultFields on DiscoverResult {\n  category\n  provider\n  id\n  name\n  romaji\n  year\n  poster\n  overview\n  library\n  titleId\n  seriesId\n  monitor\n  requestState\n  because\n}\n\nfragment ClipFields on Clip {\n  id\n  screenshot\n  name\n  mine\n  canManage\n  owner {\n    ...Person\n  }\n  source {\n    video {\n      id\n    }\n    title {\n      id\n    }\n    name\n    kind\n    label\n    year\n    status\n  }\n  start\n  end\n  audio\n  subtitles\n  quality {\n    height\n    halfRate\n  }\n  state\n  progress\n  error\n  bytes\n  width\n  height\n  fps\n  renderedAt\n  createdAt\n  sharedAt\n  public\n  link\n  linkLive\n  recipients {\n    user {\n      ...Person\n    }\n    sharedAt\n    hidden\n  }\n  file\n  poster\n}\n\nfragment ClipAllowanceFields on ClipAllowance {\n  canClip\n  canLink\n  maxLength\n  bytes\n  rendered\n  storage\n  limit\n  customDefaultFont\n}\n\nfragment NotificationFields on Notification {\n  id\n  kind\n  priority\n  title\n  body\n  image\n  link\n  actor {\n    ...Person\n  }\n  createdAt\n  expiresAt\n  readAt\n}\n\nfragment DownloadFields on Download {\n  category\n  id\n  name\n  seriesId\n  seriesName\n  title {\n    id\n  }\n  poster\n  episodes {\n    season\n    episode\n  }\n  source\n  size\n  savePath\n  state\n  importState\n  importError\n  importMode\n  error\n  addedAt\n  finishedAt\n  importedAt\n  requestedBy {\n    username\n  }\n  live {\n    stage\n    paused\n    progress\n    downloadRate\n    uploadRate\n    done\n    uploaded\n    ratio\n    peers\n    seeds\n    seedingSeconds\n    eta\n    pieces\n  }\n  seedGoal {\n    ratio\n    seconds\n  }\n}\n\nfragment EngineFields on DownloadEngine {\n  version\n  downloadRate\n  uploadRate\n  active\n  killSwitch\n  listening\n  listenError\n  slowHours\n  downloadPath\n}\n\nfragment SeriesEpisodeFields on SeriesEpisode {\n  season\n  episode\n  absolute\n  name\n  airAt\n  aired\n  state\n  attempts\n  searchedAt\n  nextSearch\n  downloadId\n  video {\n    id\n  }\n}\n\nfragment SeedingFields on Seeding {\n  ratio\n  time\n  idle\n  then\n}\n\nfragment SeriesFields on Series {\n  id\n  monitor\n  status\n  next {\n    ...SeriesEpisodeFields\n  }\n  title {\n    id\n  }\n  library\n  managed\n  path\n  name\n  year\n  poster\n  overview\n  provider\n  providerId\n  profile\n  effectiveProfile\n  sources\n  groups\n  aliases\n  knownAs\n  numbering\n  naming\n  style {\n    file\n    folder\n    agreement\n    samples\n  }\n  seeding {\n    ...SeedingFields\n  }\n  scheduleAt\n  addedAt\n  counts {\n    have\n    wanted\n    missing\n    grabbed\n    total\n    upcoming\n    skipped\n  }\n  episodes {\n    ...SeriesEpisodeFields\n  }\n}\n\nfragment ReleaseCandidateFields on ReleaseCandidate {\n  release {\n    title\n    source\n    link\n    infoHash\n    size\n    seeders\n    leechers\n    published\n    page\n  }\n  attributes {\n    group\n    resolution\n    codec\n    source\n    dualAudio\n    version\n    proper\n    tenBit\n  }\n  episodes {\n    season\n    episode\n  }\n  batch\n  verdict {\n    accepted\n    score\n    rejections\n    warnings\n    nonstandard\n  }\n}\n\nfragment CalendarEntryFields on CalendarEntry {\n  seriesId\n  title {\n    id\n  }\n  library\n  show\n  poster\n  backdrop\n  backdropTint\n  monitor\n  season\n  episode\n  absolute\n  name\n  airAt\n  state\n  video {\n    id\n  }\n  download {\n    stage\n    progress\n    downloadRate\n    eta\n  }\n}\n\nfragment SettingsFields on Settings {\n  network {\n    host\n    port\n    cors\n  }\n  log {\n    level\n  }\n  scan {\n    watch\n    interval\n  }\n  metadata {\n    tmdbApiKey\n    language\n  }\n  transcode {\n    hardware\n    vaapiDevice\n  }\n  clips {\n    enabled\n    path\n    publicLinks\n    concurrency\n    maxStorage\n    fontsDir\n    defaultFont\n  }\n  music {\n    onlineLyrics\n    lyricsUrl\n    analyzeLoudness\n  }\n  downloads {\n    path\n    import\n    port\n    upnp\n    dht\n    maxActive\n    downloadLimit\n    uploadLimit\n    slowDownloadLimit\n    slowUploadLimit\n    slowFrom\n    slowTo\n    bindInterface\n    proxy\n    seeding {\n      ...SeedingFields\n    }\n  }\n  automation {\n    defaultMonitor\n    rssInterval\n    retry {\n      every\n      until\n    }\n    renameSuggestions\n  }\n  requests {\n    monitor\n  }\n  signIn {\n    style\n  }\n  sources {\n    name\n    kind\n    url\n    feed\n    apiKey\n    categories\n    enabled\n    downloadPath\n    seeding {\n      ...SeedingFields\n    }\n  }\n  profiles {\n    name\n    resolutions\n    groups\n    require\n    reject\n    minSize\n    maxSize\n    codecs\n    preferDualAudio\n    batches\n    minSeeders\n  }\n  libraries {\n    name\n    path\n    kind\n    metadataProvider\n    managed\n    profile\n    downloadPath\n    resolvedPath\n    exists\n    error\n    titleCount\n    skippedCount\n  }\n  raw\n  error\n  paths {\n    config\n    data\n    log\n  }\n}": typeof types.PersonFragmentDoc,
};
const documents: Documents = {
    "\n  query Calendar($from: Int!, $to: Int!) {\n    calendar(from: $from, to: $to) {\n      ...CalendarEntryFields\n    }\n  }\n": types.CalendarDocument,
    "\n  query Downloads {\n    downloads {\n      ...DownloadFields\n    }\n  }\n": types.DownloadsDocument,
    "\n  query Engine {\n    downloadEngine {\n      ...EngineFields\n    }\n  }\n": types.EngineDocument,
    "\n  mutation DownloadsPause($ids: [Int!]!) {\n    pauseDownloads(ids: $ids) {\n      id\n    }\n  }\n": types.DownloadsPauseDocument,
    "\n  mutation DownloadsResume($ids: [Int!]!) {\n    resumeDownloads(ids: $ids) {\n      id\n    }\n  }\n": types.DownloadsResumeDocument,
    "\n  mutation DownloadsRecheck($ids: [Int!]!) {\n    recheckDownloads(ids: $ids) {\n      id\n    }\n  }\n": types.DownloadsRecheckDocument,
    "\n  mutation ImportDownload($id: Int!) {\n    importDownload(id: $id) {\n      id\n    }\n  }\n": types.ImportDownloadDocument,
    "\n  mutation RemoveDownloads($ids: [Int!]!, $deleteFiles: Boolean!) {\n    removeDownloads(ids: $ids, deleteFiles: $deleteFiles)\n  }\n": types.RemoveDownloadsDocument,
    "\n  query Home {\n    home {\n      continueWatching {\n        position\n        upNext\n        newEpisode\n        watchedAt\n        video {\n          id\n          label\n          name\n          still\n          duration\n          title {\n            id\n            name\n            poster\n            backdrop\n            posterTint\n            backdropTint\n          }\n        }\n      }\n      recentlyAdded {\n        library\n        titles {\n          ...Card\n        }\n      }\n      popularHere {\n        people\n        title {\n          ...Card\n        }\n      }\n    }\n  }\n": types.HomeDocument,
    "\n  query ComingUp($from: Int!, $to: Int!) {\n    calendar(from: $from, to: $to) {\n      ...CalendarEntryFields\n    }\n  }\n": types.ComingUpDocument,
    "\n  query MusicHome {\n    musicHome {\n      recentlyPlayed {\n        id\n        name\n        artist\n        cover\n      }\n      recentlyAdded {\n        id\n        name\n        artist\n        cover\n      }\n    }\n  }\n": types.MusicHomeDocument,
    "\n  query Library($name: String!) {\n    library(name: $name) {\n      titles {\n        ...Card\n      }\n    }\n  }\n": types.LibraryDocument,
    "\n  query Transfers {\n    downloadEngine {\n      downloadRate\n      killSwitch\n    }\n    downloads {\n      id\n      state\n      importState\n    }\n  }\n": types.TransfersDocument,
    "\n  query RequestCount {\n    requests {\n      id\n      state\n    }\n  }\n": types.RequestCountDocument,
    "\n  query Requests {\n    requests {\n      id\n      user {\n        ...Person\n      }\n      name\n      year\n      poster\n      library\n      state\n      title {\n        id\n      }\n      note\n      createdAt\n      have\n      aired\n    }\n  }\n": types.RequestsDocument,
    "\n  mutation ApproveRequest($id: Int!) {\n    approveRequest(id: $id) {\n      id\n    }\n  }\n": types.ApproveRequestDocument,
    "\n  mutation DeclineRequest($id: Int!) {\n    declineRequest(id: $id) {\n      id\n    }\n  }\n": types.DeclineRequestDocument,
    "\n  mutation DeleteRequest($id: Int!) {\n    deleteRequest(id: $id)\n  }\n": types.DeleteRequestDocument,
    "\n  query Search($query: String!) {\n    musicSearch(query: $query, limit: 5) {\n      artists {\n        id\n        name\n        cover\n        albumCount\n      }\n      albums {\n        id\n        name\n        artist\n        cover\n        year\n      }\n      tracks {\n        id\n        title\n        artist\n        album\n        cover\n      }\n    }\n    search(query: $query) {\n      titles {\n        ...Card\n      }\n      videos {\n        id\n        label\n        name\n        title {\n          name\n        }\n      }\n    }\n  }\n": types.SearchDocument,
    "\n  query RecentTitles($ids: [Int!]!) {\n    titles(ids: $ids) {\n      id\n    }\n  }\n": types.RecentTitlesDocument,
    "\n  query DownloadStates {\n    downloads {\n      id\n      state\n    }\n  }\n": types.DownloadStatesDocument,
    "\n  mutation PauseDownloads($ids: [Int!]!) {\n    pauseDownloads(ids: $ids) {\n      id\n    }\n  }\n": types.PauseDownloadsDocument,
    "\n  mutation ResumeDownloads($ids: [Int!]!) {\n    resumeDownloads(ids: $ids) {\n      id\n    }\n  }\n": types.ResumeDownloadsDocument,
    "\n  query Discover($library: String, $query: String) {\n    discover(library: $library, query: $query) {\n      library\n      results {\n        ...DiscoverResultFields\n      }\n    }\n  }\n": types.DiscoverDocument,
    "\n  query ForYou($library: String) {\n    forYou(library: $library) {\n      library\n      shelves {\n        key\n        name\n        results {\n          ...DiscoverResultFields\n        }\n      }\n    }\n  }\n": types.ForYouDocument,
    "\n  query Similar($id: Int!) {\n    title(id: $id) {\n      similar {\n        recommendations {\n          ...DiscoverResultFields\n        }\n        alsoWatched {\n          ...Card\n        }\n      }\n    }\n  }\n": types.SimilarDocument,
    "\n  query MatchCandidates($id: Int!, $query: String, $provider: Provider) {\n    title(id: $id) {\n      matchCandidates(query: $query, provider: $provider) {\n        query\n        results {\n          provider\n          id\n          name\n          year\n          poster\n          overview\n        }\n      }\n    }\n  }\n": types.MatchCandidatesDocument,
    "\n  query TitleDownloads {\n    downloads {\n      ...DownloadFields\n    }\n  }\n": types.TitleDownloadsDocument,
    "\n  mutation SetWatched($videoIds: [Int!]!, $watched: Boolean!) {\n    setWatched(videoIds: $videoIds, watched: $watched) {\n      id\n    }\n  }\n": types.SetWatchedDocument,
    "\n  mutation SetTitleWatched($id: Int!, $watched: Boolean!) {\n    setTitleWatched(id: $id, watched: $watched) {\n      id\n    }\n  }\n": types.SetTitleWatchedDocument,
    "\n  mutation RefreshTitle($id: Int!) {\n    refreshTitle(id: $id) {\n      id\n    }\n  }\n": types.RefreshTitleDocument,
    "\n  mutation MatchTitle($id: Int!, $provider: Provider!, $providerId: String!) {\n    matchTitle(id: $id, provider: $provider, providerId: $providerId) {\n      id\n    }\n  }\n": types.MatchTitleDocument,
    "\n  query Wanted {\n    wanted {\n      seriesId\n      title {\n        id\n      }\n      show\n      season\n      episode\n      name\n      airAt\n      aired\n      state\n      attempts\n      searchedAt\n      nextSearch\n    }\n  }\n": types.WantedDocument,
    "\n  query Attention($requests: Boolean!, $downloads: Boolean!) {\n    requests @include(if: $requests) {\n      id\n      state\n    }\n    downloads @include(if: $downloads) {\n      id\n      state\n      importState\n    }\n  }\n": types.AttentionDocument,
    "\n  query SignInServer {\n    server {\n      setupRequired\n      signInStyle\n    }\n  }\n": types.SignInServerDocument,
    "\n  query SignInProfiles {\n    signInProfiles {\n      key\n      avatar\n      passkey\n    }\n  }\n": types.SignInProfilesDocument,
    "\n  mutation Setup($username: String!, $password: String!) {\n    setup(username: $username, password: $password) {\n      user {\n        ...Person\n      }\n      token\n    }\n  }\n": types.SetupDocument,
    "\n  mutation SignIn($username: String, $profile: String, $password: String!) {\n    signIn(username: $username, profile: $profile, password: $password) {\n      user {\n        ...Person\n      }\n      token\n    }\n  }\n": types.SignInDocument,
    "\n  mutation SetTitleArtwork($id: Int!, $kind: TitleArtwork!, $image: Upload) {\n    setTitleArtwork(id: $id, kind: $kind, image: $image) {\n      id\n    }\n  }\n": types.SetTitleArtworkDocument,
    "\n  mutation SetVideoArtwork($videoId: Int!, $image: Upload) {\n    setVideoArtwork(videoId: $videoId, image: $image) {\n      id\n    }\n  }\n": types.SetVideoArtworkDocument,
    "\n  mutation SetAvatar($image: Upload!, $userId: Int) {\n    setAvatar(image: $image, userId: $userId) {\n      id\n      avatar\n    }\n  }\n": types.SetAvatarDocument,
    "\n  mutation RemoveAvatar($userId: Int) {\n    removeAvatar(userId: $userId) {\n      id\n      avatar\n    }\n  }\n": types.RemoveAvatarDocument,
    "\n  mutation AddSeries($input: NewSeries!) {\n    addSeries(input: $input) {\n      id\n    }\n  }\n": types.AddSeriesDocument,
    "\n  query AiredEpisodes($provider: Provider!, $id: String!) {\n    airedEpisodes(provider: $provider, id: $id)\n  }\n": types.AiredEpisodesDocument,
    "\n  mutation CreateRequest($input: NewRequest!) {\n    createRequest(input: $input) {\n      id\n    }\n  }\n": types.CreateRequestDocument,
    "\n  subscription Events {\n    events {\n      __typename\n      ... on ConfigChanged {\n        error\n      }\n      ... on ScanFinished {\n        library\n      }\n      ... on LibraryChanged {\n        library\n      }\n      ... on MetadataChanged {\n        titleId\n        status\n      }\n      ... on ListChanged {\n        list\n      }\n      ... on SeriesChanged {\n        seriesId\n      }\n      ... on EpisodesImported {\n        library\n      }\n      ... on NotificationReceived {\n        notification {\n          ...NotificationFields\n        }\n      }\n      ... on ClipChanged {\n        clipId\n      }\n    }\n  }\n": types.EventsDocument,
    "\n  query TitleSeries($id: Int!) {\n    title(id: $id) {\n      series {\n        ...SeriesFields\n      }\n    }\n  }\n": types.TitleSeriesDocument,
    "\n  query TitleSchedule($id: Int!) {\n    title(id: $id) {\n      series {\n        id\n        monitor\n        status\n        next {\n          ...SeriesEpisodeFields\n        }\n      }\n    }\n  }\n": types.TitleScheduleDocument,
    "\n  mutation ManageTitle($titleId: Int!) {\n    manageTitle(titleId: $titleId) {\n      id\n    }\n  }\n": types.ManageTitleDocument,
    "\n  mutation UpdateSeries($id: Int!, $patch: SeriesPatch!) {\n    updateSeries(id: $id, patch: $patch) {\n      ...SeriesFields\n    }\n  }\n": types.UpdateSeriesDocument,
    "\n  mutation RefreshSeriesSchedule($id: Int!) {\n    refreshSeriesSchedule(id: $id) {\n      id\n    }\n  }\n": types.RefreshSeriesScheduleDocument,
    "\n  mutation RemoveSeries($id: Int!) {\n    removeSeries(id: $id)\n  }\n": types.RemoveSeriesDocument,
    "\n  query NamingPreview($id: Int!, $file: String!) {\n    series(id: $id) {\n      namingPreview(file: $file) {\n        samples\n        error\n      }\n    }\n  }\n": types.NamingPreviewDocument,
    "\n  query Releases($seriesId: Int!, $season: Int!, $episodes: [Int!]!, $query: String) {\n    series(id: $seriesId) {\n      releases(season: $season, episodes: $episodes, query: $query) {\n        ...ReleaseCandidateFields\n      }\n    }\n  }\n": types.ReleasesDocument,
    "\n  mutation GrabRelease($release: ReleaseInput!, $seriesId: Int, $episodes: [EpisodeNumberInput!]!) {\n    grabRelease(release: $release, seriesId: $seriesId, episodes: $episodes) {\n      id\n    }\n  }\n": types.GrabReleaseDocument,
    "\n  mutation DeleteDownloaded($seriesId: Int!, $season: Int) {\n    deleteDownloaded(seriesId: $seriesId, season: $season) {\n      undone\n      problems\n    }\n  }\n": types.DeleteDownloadedDocument,
    "\n  mutation LookForAgain($seriesId: Int!, $season: Int, $episode: Int) {\n    lookForAgain(seriesId: $seriesId, season: $season, episode: $episode)\n  }\n": types.LookForAgainDocument,
    "\n  fragment InboxFields on Inbox {\n    items {\n      ...NotificationFields\n    }\n    unread\n  }\n": types.InboxFieldsFragmentDoc,
    "\n  query Inbox {\n    notifications {\n      ...InboxFields\n    }\n  }\n": types.InboxDocument,
    "\n  mutation MarkNotificationsRead($ids: [Int!]) {\n    markNotificationsRead(ids: $ids) {\n      ...InboxFields\n    }\n  }\n": types.MarkNotificationsReadDocument,
    "\n  mutation DeleteNotifications($id: Int) {\n    deleteNotifications(id: $id) {\n      ...InboxFields\n    }\n  }\n": types.DeleteNotificationsDocument,
    "\n  query Playback($id: Int!) {\n    video(id: $id) {\n      ...Playback\n    }\n    server {\n      transcoding {\n        ...TranscodingFields\n      }\n    }\n  }\n": types.PlaybackDocument,
    "\n  mutation SaveProgress($videoId: Int!, $position: Float!, $duration: Float!) {\n    saveProgress(videoId: $videoId, position: $position, duration: $duration) {\n      id\n    }\n  }\n": types.SaveProgressDocument,
    "\n  mutation TakeScreenshot($input: NewScreenshot!) {\n    takeScreenshot(input: $input) {\n      ...ClipFields\n    }\n  }\n": types.TakeScreenshotDocument,
    "\n  query PlayerSchedule($id: Int!) {\n    title(id: $id) {\n      series {\n        id\n        monitor\n        status\n        next {\n          ...SeriesEpisodeFields\n        }\n      }\n    }\n  }\n": types.PlayerScheduleDocument,
    "\n  query PlayerOverview($id: Int!, $videoId: Int!) {\n    title(id: $id) {\n      overview\n    }\n    video(id: $videoId) {\n      overview\n    }\n  }\n": types.PlayerOverviewDocument,
    "\n  query Status {\n    server {\n      version\n      setupRequired\n      clips\n      downloads\n      sources\n    }\n    viewer {\n      ...Viewer\n    }\n  }\n": types.StatusDocument,
    "\n  query Appearance {\n    appearance {\n      mode\n      style\n      mediaTint\n      light {\n        id\n        palette {\n          tokens {\n            name\n            value\n          }\n        }\n      }\n      dark {\n        id\n        palette {\n          tokens {\n            name\n            value\n          }\n        }\n      }\n    }\n  }\n": types.AppearanceDocument,
    "\n  query Libraries {\n    libraries {\n      name\n      kind\n      showCount\n      movieCount\n      albumCount\n      trackCount\n    }\n  }\n": types.LibrariesDocument,
    "\n  query Settings {\n    settings {\n      ...SettingsFields\n    }\n    server {\n      transcoding {\n        ...TranscodingFields\n      }\n    }\n  }\n": types.SettingsDocument,
    "\n  query Title($id: Int!) {\n    title(id: $id) {\n      ...TitleDetail\n    }\n  }\n": types.TitleDocument,
    "\n  query People {\n    users {\n      ...Person\n    }\n  }\n": types.PeopleDocument,
    "\n  mutation SignOut {\n    signOut\n  }\n": types.SignOutDocument,
    "\n  query Passkeys {\n    viewer {\n      passkeys {\n        id\n        name\n        createdAt\n        lastUsed\n      }\n    }\n  }\n": types.PasskeysDocument,
    "\n  mutation DeletePasskey($id: Int!) {\n    deletePasskey(id: $id) {\n      id\n    }\n  }\n": types.DeletePasskeyDocument,
    "\n  mutation ChangePassword($current: String!, $new: String!) {\n    changePassword(current: $current, new: $new)\n  }\n": types.ChangePasswordDocument,
    "\n  query AppPasswords {\n    appPasswords {\n      id\n      name\n      createdAt\n      lastUsed\n      client\n    }\n  }\n": types.AppPasswordsDocument,
    "\n  mutation CreateAppPassword($name: String!) {\n    createAppPassword(name: $name) {\n      secret\n      password {\n        id\n        name\n      }\n    }\n  }\n": types.CreateAppPasswordDocument,
    "\n  mutation DeleteAppPassword($id: Int!) {\n    deleteAppPassword(id: $id)\n  }\n": types.DeleteAppPasswordDocument,
    "\n  fragment SchemeFields on ColorScheme {\n    id\n    name\n    builtIn\n    published\n    editable\n    code\n    shareCode\n    forkedFrom {\n      id\n      name\n    }\n    palette {\n      seeds {\n        name\n        value\n      }\n      overrides {\n        name\n        value\n      }\n      tokens {\n        name\n        value\n      }\n      warnings {\n        foreground\n        background\n        ratio\n        minimum\n      }\n    }\n  }\n": types.SchemeFieldsFragmentDoc,
    "\n  query ColorSchemes {\n    colorSchemes {\n      ...SchemeFields\n    }\n  }\n": types.ColorSchemesDocument,
    "\n  query AppearanceSettings {\n    appearanceSettings {\n      colors {\n        mode\n        single\n        light\n        dark\n      }\n      style\n      mediaTint\n    }\n  }\n": types.AppearanceSettingsDocument,
    "\n  query ServerAppearance {\n    serverAppearance {\n      colors {\n        mode\n        single\n        light\n        dark\n      }\n      style\n    }\n  }\n": types.ServerAppearanceDocument,
    "\n  mutation SetAppearance($input: AppearanceSettingsInput!) {\n    setAppearance(input: $input) {\n      mode\n    }\n  }\n": types.SetAppearanceDocument,
    "\n  mutation SetServerAppearance($input: ServerAppearanceInput!) {\n    setServerAppearance(input: $input) {\n      style\n    }\n  }\n": types.SetServerAppearanceDocument,
    "\n  mutation SaveScheme($id: String, $input: SchemeInput!) {\n    saveScheme(id: $id, input: $input) {\n      ...SchemeFields\n    }\n  }\n": types.SaveSchemeDocument,
    "\n  mutation ForkScheme($id: String!) {\n    forkScheme(id: $id) {\n      ...SchemeFields\n    }\n  }\n": types.ForkSchemeDocument,
    "\n  mutation ImportScheme($code: String!, $name: String) {\n    importScheme(code: $code, name: $name) {\n      ...SchemeFields\n    }\n  }\n": types.ImportSchemeDocument,
    "\n  mutation DeleteScheme($id: String!) {\n    deleteScheme(id: $id)\n  }\n": types.DeleteSchemeDocument,
    "\n  mutation PublishScheme($id: String!, $published: Boolean!) {\n    publishScheme(id: $id, published: $published) {\n      id\n    }\n  }\n": types.PublishSchemeDocument,
    "\n  query DecodeScheme($code: String!) {\n    decodeScheme(code: $code) {\n      name\n      code\n      palette {\n        tokens {\n          name\n          value\n        }\n        warnings {\n          foreground\n          background\n          ratio\n          minimum\n        }\n      }\n    }\n  }\n": types.DecodeSchemeDocument,
    "\n  query SettingsEngine {\n    downloadEngine {\n      downloadPath\n      killSwitch\n    }\n  }\n": types.SettingsEngineDocument,
    "\n  mutation AddSource($input: SourceInput!) {\n    addSource(input: $input) {\n      raw\n    }\n  }\n": types.AddSourceDocument,
    "\n  mutation UpdateSource($name: String!, $input: SourceInput!) {\n    updateSource(name: $name, input: $input) {\n      raw\n    }\n  }\n": types.UpdateSourceDocument,
    "\n  mutation RemoveSource($name: String!) {\n    removeSource(name: $name) {\n      raw\n    }\n  }\n": types.RemoveSourceDocument,
    "\n  query DetectSource($url: String!, $apiKey: String) {\n    detectSource(url: $url, apiKey: $apiKey) {\n      kind\n      url\n      feed\n      name\n      searchable\n      sample {\n        link\n        title\n        size\n        seeders\n        published\n      }\n    }\n  }\n": types.DetectSourceDocument,
    "\n  mutation AddProfile($input: ProfileInput!) {\n    addProfile(input: $input) {\n      raw\n    }\n  }\n": types.AddProfileDocument,
    "\n  mutation UpdateProfile($name: String!, $input: ProfileInput!) {\n    updateProfile(name: $name, input: $input) {\n      raw\n    }\n  }\n": types.UpdateProfileDocument,
    "\n  mutation RemoveProfile($name: String!) {\n    removeProfile(name: $name) {\n      raw\n    }\n  }\n": types.RemoveProfileDocument,
    "\n  query RenameSuggestions {\n    renameSuggestions {\n      id\n      library\n      managed\n      root\n      src\n      dst\n      reason\n      confidence\n    }\n  }\n": types.RenameSuggestionsDocument,
    "\n  query FileHistory {\n    fileHistory {\n      batch\n      label\n      at\n      count\n      undone\n      operations {\n        kind\n        src\n        dst\n      }\n    }\n  }\n": types.FileHistoryDocument,
    "\n  mutation RefreshRenameSuggestions {\n    refreshRenameSuggestions\n  }\n": types.RefreshRenameSuggestionsDocument,
    "\n  mutation ApplyRenames($ids: [Int!]!) {\n    applyRenames(ids: $ids) {\n      renamed\n      problems\n    }\n  }\n": types.ApplyRenamesDocument,
    "\n  mutation DismissRenames($ids: [Int!]!) {\n    dismissRenames(ids: $ids)\n  }\n": types.DismissRenamesDocument,
    "\n  mutation UndoFileChanges($batch: String!) {\n    undoFileChanges(batch: $batch) {\n      undone\n      problems\n    }\n  }\n": types.UndoFileChangesDocument,
    "\n  mutation Scan($library: String) {\n    scan(library: $library)\n  }\n": types.ScanDocument,
    "\n  mutation AddLibrary($input: LibraryInput!) {\n    addLibrary(input: $input) {\n      raw\n    }\n  }\n": types.AddLibraryDocument,
    "\n  mutation UpdateLibrary($name: String!, $input: LibraryInput!) {\n    updateLibrary(name: $name, input: $input) {\n      raw\n    }\n  }\n": types.UpdateLibraryDocument,
    "\n  mutation RemoveLibrary($name: String!) {\n    removeLibrary(name: $name) {\n      raw\n    }\n  }\n": types.RemoveLibraryDocument,
    "\n  query Folders($path: String) {\n    folders(path: $path) {\n      path\n      parent\n      home\n      folders {\n        name\n        path\n      }\n    }\n  }\n": types.FoldersDocument,
    "\n  mutation ReplaceConfig($text: String!) {\n    replaceConfig(text: $text) {\n      raw\n    }\n  }\n": types.ReplaceConfigDocument,
    "\n  query SkippedFiles {\n    skippedFiles {\n      library\n      path\n      reason\n    }\n  }\n": types.SkippedFilesDocument,
    "\n  query Users {\n    users {\n      ...Viewer\n      createdAt\n      lastSeen\n      overrides {\n        allLibraries\n        libraries\n        request\n        autoApprove\n        requestLimit\n        manageRequests\n        manageShows\n        downloads\n        editMetadata\n        watchTogether\n        shareLinks\n        clip\n        clipMaxLength\n        clipLimit\n        clipStorage\n        clipLinks\n      }\n    }\n  }\n": types.UsersDocument,
    "\n  query PermissionDefaults {\n    permissionDefaults {\n      ...PermissionsFields\n    }\n  }\n": types.PermissionDefaultsDocument,
    "\n  mutation SetPermissionDefaults($permissions: PermissionsInput!) {\n    setPermissionDefaults(permissions: $permissions) {\n      ...PermissionsFields\n    }\n  }\n": types.SetPermissionDefaultsDocument,
    "\n  mutation UpdateUser($id: Int!, $input: UserPatch!) {\n    updateUser(id: $id, input: $input) {\n      id\n    }\n  }\n": types.UpdateUserDocument,
    "\n  mutation DeleteUser($id: Int!) {\n    deleteUser(id: $id)\n  }\n": types.DeleteUserDocument,
    "\n  mutation CreateUser($input: NewUser!) {\n    createUser(input: $input) {\n      id\n    }\n  }\n": types.CreateUserDocument,
    "\n  mutation SaveSection($patch: ConfigPatch!) {\n    updateSettings(patch: $patch) {\n      raw\n    }\n  }\n": types.SaveSectionDocument,
    "fragment Person on User {\n  id\n  username\n  avatar\n}\n\nfragment PermissionsFields on Permissions {\n  allLibraries\n  libraries\n  request\n  autoApprove\n  requestLimit\n  manageRequests\n  manageShows\n  downloads\n  editMetadata\n  watchTogether\n  shareLinks\n  clip\n  clipMaxLength\n  clipLimit\n  clipStorage\n  clipLinks\n}\n\nfragment Viewer on User {\n  ...Person\n  isAdmin\n  permissions {\n    ...PermissionsFields\n  }\n}\n\nfragment Card on Title {\n  id\n  kind\n  library\n  name\n  year\n  poster\n  backdrop\n  watchedCount\n  videoCount\n  progress\n  freshCount\n}\n\nfragment VideoRow on Video {\n  id\n  season\n  episode\n  episodeEnd\n  label\n  name\n  overview\n  still\n  customStill\n  airDate\n  duration\n  position\n  finished\n}\n\nfragment TitleDetail on Title {\n  ...Card\n  posterTint\n  backdropTint\n  customPoster\n  customBackdrop\n  overview\n  genres\n  rating\n  path\n  matchState\n  provider\n  providerId\n  libraryProvider\n  seasons {\n    number\n    name\n    title\n    overview\n    poster\n    episodes {\n      ...VideoRow\n    }\n  }\n  movie {\n    ...VideoRow\n  }\n  nextUp {\n    resuming\n    video {\n      ...VideoRow\n    }\n  }\n}\n\nfragment Playback on Video {\n  id\n  still\n  label\n  name\n  position\n  finished\n  title {\n    id\n    kind\n    name\n    backdrop\n  }\n  previous {\n    id\n    label\n    name\n  }\n  next {\n    id\n    still\n    label\n    name\n  }\n  media {\n    duration\n    video {\n      index\n      codec\n      codecString\n      width\n      height\n      fps\n      bitDepth\n      hdr\n    }\n    audio {\n      index\n      codec\n      codecString\n      channels\n      language\n      title\n      default\n    }\n    subtitles {\n      id\n      codec\n      language\n      title\n      default\n      forced\n      supported\n    }\n    fonts {\n      index\n      filename\n    }\n    chapters {\n      start\n      end\n      title\n    }\n  }\n}\n\nfragment TranscodingFields on Transcoding {\n  vaapi\n  vaapiError\n  softwareH264\n}\n\nfragment DiscoverResultFields on DiscoverResult {\n  category\n  provider\n  id\n  name\n  romaji\n  year\n  poster\n  overview\n  library\n  titleId\n  seriesId\n  monitor\n  requestState\n  because\n}\n\nfragment ClipFields on Clip {\n  id\n  screenshot\n  name\n  mine\n  canManage\n  owner {\n    ...Person\n  }\n  source {\n    video {\n      id\n    }\n    title {\n      id\n    }\n    name\n    kind\n    label\n    year\n    status\n  }\n  start\n  end\n  audio\n  subtitles\n  quality {\n    height\n    halfRate\n  }\n  state\n  progress\n  error\n  bytes\n  width\n  height\n  fps\n  renderedAt\n  createdAt\n  sharedAt\n  public\n  link\n  linkLive\n  recipients {\n    user {\n      ...Person\n    }\n    sharedAt\n    hidden\n  }\n  file\n  poster\n}\n\nfragment ClipAllowanceFields on ClipAllowance {\n  canClip\n  canLink\n  maxLength\n  bytes\n  rendered\n  storage\n  limit\n  customDefaultFont\n}\n\nfragment NotificationFields on Notification {\n  id\n  kind\n  priority\n  title\n  body\n  image\n  link\n  actor {\n    ...Person\n  }\n  createdAt\n  expiresAt\n  readAt\n}\n\nfragment DownloadFields on Download {\n  category\n  id\n  name\n  seriesId\n  seriesName\n  title {\n    id\n  }\n  poster\n  episodes {\n    season\n    episode\n  }\n  source\n  size\n  savePath\n  state\n  importState\n  importError\n  importMode\n  error\n  addedAt\n  finishedAt\n  importedAt\n  requestedBy {\n    username\n  }\n  live {\n    stage\n    paused\n    progress\n    downloadRate\n    uploadRate\n    done\n    uploaded\n    ratio\n    peers\n    seeds\n    seedingSeconds\n    eta\n    pieces\n  }\n  seedGoal {\n    ratio\n    seconds\n  }\n}\n\nfragment EngineFields on DownloadEngine {\n  version\n  downloadRate\n  uploadRate\n  active\n  killSwitch\n  listening\n  listenError\n  slowHours\n  downloadPath\n}\n\nfragment SeriesEpisodeFields on SeriesEpisode {\n  season\n  episode\n  absolute\n  name\n  airAt\n  aired\n  state\n  attempts\n  searchedAt\n  nextSearch\n  downloadId\n  video {\n    id\n  }\n}\n\nfragment SeedingFields on Seeding {\n  ratio\n  time\n  idle\n  then\n}\n\nfragment SeriesFields on Series {\n  id\n  monitor\n  status\n  next {\n    ...SeriesEpisodeFields\n  }\n  title {\n    id\n  }\n  library\n  managed\n  path\n  name\n  year\n  poster\n  overview\n  provider\n  providerId\n  profile\n  effectiveProfile\n  sources\n  groups\n  aliases\n  knownAs\n  numbering\n  naming\n  style {\n    file\n    folder\n    agreement\n    samples\n  }\n  seeding {\n    ...SeedingFields\n  }\n  scheduleAt\n  addedAt\n  counts {\n    have\n    wanted\n    missing\n    grabbed\n    total\n    upcoming\n    skipped\n  }\n  episodes {\n    ...SeriesEpisodeFields\n  }\n}\n\nfragment ReleaseCandidateFields on ReleaseCandidate {\n  release {\n    title\n    source\n    link\n    infoHash\n    size\n    seeders\n    leechers\n    published\n    page\n  }\n  attributes {\n    group\n    resolution\n    codec\n    source\n    dualAudio\n    version\n    proper\n    tenBit\n  }\n  episodes {\n    season\n    episode\n  }\n  batch\n  verdict {\n    accepted\n    score\n    rejections\n    warnings\n    nonstandard\n  }\n}\n\nfragment CalendarEntryFields on CalendarEntry {\n  seriesId\n  title {\n    id\n  }\n  library\n  show\n  poster\n  backdrop\n  backdropTint\n  monitor\n  season\n  episode\n  absolute\n  name\n  airAt\n  state\n  video {\n    id\n  }\n  download {\n    stage\n    progress\n    downloadRate\n    eta\n  }\n}\n\nfragment SettingsFields on Settings {\n  network {\n    host\n    port\n    cors\n  }\n  log {\n    level\n  }\n  scan {\n    watch\n    interval\n  }\n  metadata {\n    tmdbApiKey\n    language\n  }\n  transcode {\n    hardware\n    vaapiDevice\n  }\n  clips {\n    enabled\n    path\n    publicLinks\n    concurrency\n    maxStorage\n    fontsDir\n    defaultFont\n  }\n  music {\n    onlineLyrics\n    lyricsUrl\n    analyzeLoudness\n  }\n  downloads {\n    path\n    import\n    port\n    upnp\n    dht\n    maxActive\n    downloadLimit\n    uploadLimit\n    slowDownloadLimit\n    slowUploadLimit\n    slowFrom\n    slowTo\n    bindInterface\n    proxy\n    seeding {\n      ...SeedingFields\n    }\n  }\n  automation {\n    defaultMonitor\n    rssInterval\n    retry {\n      every\n      until\n    }\n    renameSuggestions\n  }\n  requests {\n    monitor\n  }\n  signIn {\n    style\n  }\n  sources {\n    name\n    kind\n    url\n    feed\n    apiKey\n    categories\n    enabled\n    downloadPath\n    seeding {\n      ...SeedingFields\n    }\n  }\n  profiles {\n    name\n    resolutions\n    groups\n    require\n    reject\n    minSize\n    maxSize\n    codecs\n    preferDualAudio\n    batches\n    minSeeders\n  }\n  libraries {\n    name\n    path\n    kind\n    metadataProvider\n    managed\n    profile\n    downloadPath\n    resolvedPath\n    exists\n    error\n    titleCount\n    skippedCount\n  }\n  raw\n  error\n  paths {\n    config\n    data\n    log\n  }\n}": types.PersonFragmentDoc,
};

/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Calendar($from: Int!, $to: Int!) {\n    calendar(from: $from, to: $to) {\n      ...CalendarEntryFields\n    }\n  }\n"): typeof import('./graphql').CalendarDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Downloads {\n    downloads {\n      ...DownloadFields\n    }\n  }\n"): typeof import('./graphql').DownloadsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Engine {\n    downloadEngine {\n      ...EngineFields\n    }\n  }\n"): typeof import('./graphql').EngineDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation DownloadsPause($ids: [Int!]!) {\n    pauseDownloads(ids: $ids) {\n      id\n    }\n  }\n"): typeof import('./graphql').DownloadsPauseDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation DownloadsResume($ids: [Int!]!) {\n    resumeDownloads(ids: $ids) {\n      id\n    }\n  }\n"): typeof import('./graphql').DownloadsResumeDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation DownloadsRecheck($ids: [Int!]!) {\n    recheckDownloads(ids: $ids) {\n      id\n    }\n  }\n"): typeof import('./graphql').DownloadsRecheckDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation ImportDownload($id: Int!) {\n    importDownload(id: $id) {\n      id\n    }\n  }\n"): typeof import('./graphql').ImportDownloadDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation RemoveDownloads($ids: [Int!]!, $deleteFiles: Boolean!) {\n    removeDownloads(ids: $ids, deleteFiles: $deleteFiles)\n  }\n"): typeof import('./graphql').RemoveDownloadsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Home {\n    home {\n      continueWatching {\n        position\n        upNext\n        newEpisode\n        watchedAt\n        video {\n          id\n          label\n          name\n          still\n          duration\n          title {\n            id\n            name\n            poster\n            backdrop\n            posterTint\n            backdropTint\n          }\n        }\n      }\n      recentlyAdded {\n        library\n        titles {\n          ...Card\n        }\n      }\n      popularHere {\n        people\n        title {\n          ...Card\n        }\n      }\n    }\n  }\n"): typeof import('./graphql').HomeDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query ComingUp($from: Int!, $to: Int!) {\n    calendar(from: $from, to: $to) {\n      ...CalendarEntryFields\n    }\n  }\n"): typeof import('./graphql').ComingUpDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query MusicHome {\n    musicHome {\n      recentlyPlayed {\n        id\n        name\n        artist\n        cover\n      }\n      recentlyAdded {\n        id\n        name\n        artist\n        cover\n      }\n    }\n  }\n"): typeof import('./graphql').MusicHomeDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Library($name: String!) {\n    library(name: $name) {\n      titles {\n        ...Card\n      }\n    }\n  }\n"): typeof import('./graphql').LibraryDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Transfers {\n    downloadEngine {\n      downloadRate\n      killSwitch\n    }\n    downloads {\n      id\n      state\n      importState\n    }\n  }\n"): typeof import('./graphql').TransfersDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query RequestCount {\n    requests {\n      id\n      state\n    }\n  }\n"): typeof import('./graphql').RequestCountDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Requests {\n    requests {\n      id\n      user {\n        ...Person\n      }\n      name\n      year\n      poster\n      library\n      state\n      title {\n        id\n      }\n      note\n      createdAt\n      have\n      aired\n    }\n  }\n"): typeof import('./graphql').RequestsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation ApproveRequest($id: Int!) {\n    approveRequest(id: $id) {\n      id\n    }\n  }\n"): typeof import('./graphql').ApproveRequestDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation DeclineRequest($id: Int!) {\n    declineRequest(id: $id) {\n      id\n    }\n  }\n"): typeof import('./graphql').DeclineRequestDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation DeleteRequest($id: Int!) {\n    deleteRequest(id: $id)\n  }\n"): typeof import('./graphql').DeleteRequestDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Search($query: String!) {\n    musicSearch(query: $query, limit: 5) {\n      artists {\n        id\n        name\n        cover\n        albumCount\n      }\n      albums {\n        id\n        name\n        artist\n        cover\n        year\n      }\n      tracks {\n        id\n        title\n        artist\n        album\n        cover\n      }\n    }\n    search(query: $query) {\n      titles {\n        ...Card\n      }\n      videos {\n        id\n        label\n        name\n        title {\n          name\n        }\n      }\n    }\n  }\n"): typeof import('./graphql').SearchDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query RecentTitles($ids: [Int!]!) {\n    titles(ids: $ids) {\n      id\n    }\n  }\n"): typeof import('./graphql').RecentTitlesDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query DownloadStates {\n    downloads {\n      id\n      state\n    }\n  }\n"): typeof import('./graphql').DownloadStatesDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation PauseDownloads($ids: [Int!]!) {\n    pauseDownloads(ids: $ids) {\n      id\n    }\n  }\n"): typeof import('./graphql').PauseDownloadsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation ResumeDownloads($ids: [Int!]!) {\n    resumeDownloads(ids: $ids) {\n      id\n    }\n  }\n"): typeof import('./graphql').ResumeDownloadsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Discover($library: String, $query: String) {\n    discover(library: $library, query: $query) {\n      library\n      results {\n        ...DiscoverResultFields\n      }\n    }\n  }\n"): typeof import('./graphql').DiscoverDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query ForYou($library: String) {\n    forYou(library: $library) {\n      library\n      shelves {\n        key\n        name\n        results {\n          ...DiscoverResultFields\n        }\n      }\n    }\n  }\n"): typeof import('./graphql').ForYouDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Similar($id: Int!) {\n    title(id: $id) {\n      similar {\n        recommendations {\n          ...DiscoverResultFields\n        }\n        alsoWatched {\n          ...Card\n        }\n      }\n    }\n  }\n"): typeof import('./graphql').SimilarDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query MatchCandidates($id: Int!, $query: String, $provider: Provider) {\n    title(id: $id) {\n      matchCandidates(query: $query, provider: $provider) {\n        query\n        results {\n          provider\n          id\n          name\n          year\n          poster\n          overview\n        }\n      }\n    }\n  }\n"): typeof import('./graphql').MatchCandidatesDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query TitleDownloads {\n    downloads {\n      ...DownloadFields\n    }\n  }\n"): typeof import('./graphql').TitleDownloadsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation SetWatched($videoIds: [Int!]!, $watched: Boolean!) {\n    setWatched(videoIds: $videoIds, watched: $watched) {\n      id\n    }\n  }\n"): typeof import('./graphql').SetWatchedDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation SetTitleWatched($id: Int!, $watched: Boolean!) {\n    setTitleWatched(id: $id, watched: $watched) {\n      id\n    }\n  }\n"): typeof import('./graphql').SetTitleWatchedDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation RefreshTitle($id: Int!) {\n    refreshTitle(id: $id) {\n      id\n    }\n  }\n"): typeof import('./graphql').RefreshTitleDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation MatchTitle($id: Int!, $provider: Provider!, $providerId: String!) {\n    matchTitle(id: $id, provider: $provider, providerId: $providerId) {\n      id\n    }\n  }\n"): typeof import('./graphql').MatchTitleDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Wanted {\n    wanted {\n      seriesId\n      title {\n        id\n      }\n      show\n      season\n      episode\n      name\n      airAt\n      aired\n      state\n      attempts\n      searchedAt\n      nextSearch\n    }\n  }\n"): typeof import('./graphql').WantedDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Attention($requests: Boolean!, $downloads: Boolean!) {\n    requests @include(if: $requests) {\n      id\n      state\n    }\n    downloads @include(if: $downloads) {\n      id\n      state\n      importState\n    }\n  }\n"): typeof import('./graphql').AttentionDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query SignInServer {\n    server {\n      setupRequired\n      signInStyle\n    }\n  }\n"): typeof import('./graphql').SignInServerDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query SignInProfiles {\n    signInProfiles {\n      key\n      avatar\n      passkey\n    }\n  }\n"): typeof import('./graphql').SignInProfilesDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation Setup($username: String!, $password: String!) {\n    setup(username: $username, password: $password) {\n      user {\n        ...Person\n      }\n      token\n    }\n  }\n"): typeof import('./graphql').SetupDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation SignIn($username: String, $profile: String, $password: String!) {\n    signIn(username: $username, profile: $profile, password: $password) {\n      user {\n        ...Person\n      }\n      token\n    }\n  }\n"): typeof import('./graphql').SignInDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation SetTitleArtwork($id: Int!, $kind: TitleArtwork!, $image: Upload) {\n    setTitleArtwork(id: $id, kind: $kind, image: $image) {\n      id\n    }\n  }\n"): typeof import('./graphql').SetTitleArtworkDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation SetVideoArtwork($videoId: Int!, $image: Upload) {\n    setVideoArtwork(videoId: $videoId, image: $image) {\n      id\n    }\n  }\n"): typeof import('./graphql').SetVideoArtworkDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation SetAvatar($image: Upload!, $userId: Int) {\n    setAvatar(image: $image, userId: $userId) {\n      id\n      avatar\n    }\n  }\n"): typeof import('./graphql').SetAvatarDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation RemoveAvatar($userId: Int) {\n    removeAvatar(userId: $userId) {\n      id\n      avatar\n    }\n  }\n"): typeof import('./graphql').RemoveAvatarDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation AddSeries($input: NewSeries!) {\n    addSeries(input: $input) {\n      id\n    }\n  }\n"): typeof import('./graphql').AddSeriesDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query AiredEpisodes($provider: Provider!, $id: String!) {\n    airedEpisodes(provider: $provider, id: $id)\n  }\n"): typeof import('./graphql').AiredEpisodesDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation CreateRequest($input: NewRequest!) {\n    createRequest(input: $input) {\n      id\n    }\n  }\n"): typeof import('./graphql').CreateRequestDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  subscription Events {\n    events {\n      __typename\n      ... on ConfigChanged {\n        error\n      }\n      ... on ScanFinished {\n        library\n      }\n      ... on LibraryChanged {\n        library\n      }\n      ... on MetadataChanged {\n        titleId\n        status\n      }\n      ... on ListChanged {\n        list\n      }\n      ... on SeriesChanged {\n        seriesId\n      }\n      ... on EpisodesImported {\n        library\n      }\n      ... on NotificationReceived {\n        notification {\n          ...NotificationFields\n        }\n      }\n      ... on ClipChanged {\n        clipId\n      }\n    }\n  }\n"): typeof import('./graphql').EventsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query TitleSeries($id: Int!) {\n    title(id: $id) {\n      series {\n        ...SeriesFields\n      }\n    }\n  }\n"): typeof import('./graphql').TitleSeriesDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query TitleSchedule($id: Int!) {\n    title(id: $id) {\n      series {\n        id\n        monitor\n        status\n        next {\n          ...SeriesEpisodeFields\n        }\n      }\n    }\n  }\n"): typeof import('./graphql').TitleScheduleDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation ManageTitle($titleId: Int!) {\n    manageTitle(titleId: $titleId) {\n      id\n    }\n  }\n"): typeof import('./graphql').ManageTitleDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation UpdateSeries($id: Int!, $patch: SeriesPatch!) {\n    updateSeries(id: $id, patch: $patch) {\n      ...SeriesFields\n    }\n  }\n"): typeof import('./graphql').UpdateSeriesDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation RefreshSeriesSchedule($id: Int!) {\n    refreshSeriesSchedule(id: $id) {\n      id\n    }\n  }\n"): typeof import('./graphql').RefreshSeriesScheduleDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation RemoveSeries($id: Int!) {\n    removeSeries(id: $id)\n  }\n"): typeof import('./graphql').RemoveSeriesDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query NamingPreview($id: Int!, $file: String!) {\n    series(id: $id) {\n      namingPreview(file: $file) {\n        samples\n        error\n      }\n    }\n  }\n"): typeof import('./graphql').NamingPreviewDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Releases($seriesId: Int!, $season: Int!, $episodes: [Int!]!, $query: String) {\n    series(id: $seriesId) {\n      releases(season: $season, episodes: $episodes, query: $query) {\n        ...ReleaseCandidateFields\n      }\n    }\n  }\n"): typeof import('./graphql').ReleasesDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation GrabRelease($release: ReleaseInput!, $seriesId: Int, $episodes: [EpisodeNumberInput!]!) {\n    grabRelease(release: $release, seriesId: $seriesId, episodes: $episodes) {\n      id\n    }\n  }\n"): typeof import('./graphql').GrabReleaseDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation DeleteDownloaded($seriesId: Int!, $season: Int) {\n    deleteDownloaded(seriesId: $seriesId, season: $season) {\n      undone\n      problems\n    }\n  }\n"): typeof import('./graphql').DeleteDownloadedDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation LookForAgain($seriesId: Int!, $season: Int, $episode: Int) {\n    lookForAgain(seriesId: $seriesId, season: $season, episode: $episode)\n  }\n"): typeof import('./graphql').LookForAgainDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  fragment InboxFields on Inbox {\n    items {\n      ...NotificationFields\n    }\n    unread\n  }\n"): typeof import('./graphql').InboxFieldsFragmentDoc;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Inbox {\n    notifications {\n      ...InboxFields\n    }\n  }\n"): typeof import('./graphql').InboxDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation MarkNotificationsRead($ids: [Int!]) {\n    markNotificationsRead(ids: $ids) {\n      ...InboxFields\n    }\n  }\n"): typeof import('./graphql').MarkNotificationsReadDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation DeleteNotifications($id: Int) {\n    deleteNotifications(id: $id) {\n      ...InboxFields\n    }\n  }\n"): typeof import('./graphql').DeleteNotificationsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Playback($id: Int!) {\n    video(id: $id) {\n      ...Playback\n    }\n    server {\n      transcoding {\n        ...TranscodingFields\n      }\n    }\n  }\n"): typeof import('./graphql').PlaybackDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation SaveProgress($videoId: Int!, $position: Float!, $duration: Float!) {\n    saveProgress(videoId: $videoId, position: $position, duration: $duration) {\n      id\n    }\n  }\n"): typeof import('./graphql').SaveProgressDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation TakeScreenshot($input: NewScreenshot!) {\n    takeScreenshot(input: $input) {\n      ...ClipFields\n    }\n  }\n"): typeof import('./graphql').TakeScreenshotDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query PlayerSchedule($id: Int!) {\n    title(id: $id) {\n      series {\n        id\n        monitor\n        status\n        next {\n          ...SeriesEpisodeFields\n        }\n      }\n    }\n  }\n"): typeof import('./graphql').PlayerScheduleDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query PlayerOverview($id: Int!, $videoId: Int!) {\n    title(id: $id) {\n      overview\n    }\n    video(id: $videoId) {\n      overview\n    }\n  }\n"): typeof import('./graphql').PlayerOverviewDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Status {\n    server {\n      version\n      setupRequired\n      clips\n      downloads\n      sources\n    }\n    viewer {\n      ...Viewer\n    }\n  }\n"): typeof import('./graphql').StatusDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Appearance {\n    appearance {\n      mode\n      style\n      mediaTint\n      light {\n        id\n        palette {\n          tokens {\n            name\n            value\n          }\n        }\n      }\n      dark {\n        id\n        palette {\n          tokens {\n            name\n            value\n          }\n        }\n      }\n    }\n  }\n"): typeof import('./graphql').AppearanceDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Libraries {\n    libraries {\n      name\n      kind\n      showCount\n      movieCount\n      albumCount\n      trackCount\n    }\n  }\n"): typeof import('./graphql').LibrariesDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Settings {\n    settings {\n      ...SettingsFields\n    }\n    server {\n      transcoding {\n        ...TranscodingFields\n      }\n    }\n  }\n"): typeof import('./graphql').SettingsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Title($id: Int!) {\n    title(id: $id) {\n      ...TitleDetail\n    }\n  }\n"): typeof import('./graphql').TitleDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query People {\n    users {\n      ...Person\n    }\n  }\n"): typeof import('./graphql').PeopleDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation SignOut {\n    signOut\n  }\n"): typeof import('./graphql').SignOutDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Passkeys {\n    viewer {\n      passkeys {\n        id\n        name\n        createdAt\n        lastUsed\n      }\n    }\n  }\n"): typeof import('./graphql').PasskeysDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation DeletePasskey($id: Int!) {\n    deletePasskey(id: $id) {\n      id\n    }\n  }\n"): typeof import('./graphql').DeletePasskeyDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation ChangePassword($current: String!, $new: String!) {\n    changePassword(current: $current, new: $new)\n  }\n"): typeof import('./graphql').ChangePasswordDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query AppPasswords {\n    appPasswords {\n      id\n      name\n      createdAt\n      lastUsed\n      client\n    }\n  }\n"): typeof import('./graphql').AppPasswordsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation CreateAppPassword($name: String!) {\n    createAppPassword(name: $name) {\n      secret\n      password {\n        id\n        name\n      }\n    }\n  }\n"): typeof import('./graphql').CreateAppPasswordDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation DeleteAppPassword($id: Int!) {\n    deleteAppPassword(id: $id)\n  }\n"): typeof import('./graphql').DeleteAppPasswordDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  fragment SchemeFields on ColorScheme {\n    id\n    name\n    builtIn\n    published\n    editable\n    code\n    shareCode\n    forkedFrom {\n      id\n      name\n    }\n    palette {\n      seeds {\n        name\n        value\n      }\n      overrides {\n        name\n        value\n      }\n      tokens {\n        name\n        value\n      }\n      warnings {\n        foreground\n        background\n        ratio\n        minimum\n      }\n    }\n  }\n"): typeof import('./graphql').SchemeFieldsFragmentDoc;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query ColorSchemes {\n    colorSchemes {\n      ...SchemeFields\n    }\n  }\n"): typeof import('./graphql').ColorSchemesDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query AppearanceSettings {\n    appearanceSettings {\n      colors {\n        mode\n        single\n        light\n        dark\n      }\n      style\n      mediaTint\n    }\n  }\n"): typeof import('./graphql').AppearanceSettingsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query ServerAppearance {\n    serverAppearance {\n      colors {\n        mode\n        single\n        light\n        dark\n      }\n      style\n    }\n  }\n"): typeof import('./graphql').ServerAppearanceDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation SetAppearance($input: AppearanceSettingsInput!) {\n    setAppearance(input: $input) {\n      mode\n    }\n  }\n"): typeof import('./graphql').SetAppearanceDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation SetServerAppearance($input: ServerAppearanceInput!) {\n    setServerAppearance(input: $input) {\n      style\n    }\n  }\n"): typeof import('./graphql').SetServerAppearanceDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation SaveScheme($id: String, $input: SchemeInput!) {\n    saveScheme(id: $id, input: $input) {\n      ...SchemeFields\n    }\n  }\n"): typeof import('./graphql').SaveSchemeDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation ForkScheme($id: String!) {\n    forkScheme(id: $id) {\n      ...SchemeFields\n    }\n  }\n"): typeof import('./graphql').ForkSchemeDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation ImportScheme($code: String!, $name: String) {\n    importScheme(code: $code, name: $name) {\n      ...SchemeFields\n    }\n  }\n"): typeof import('./graphql').ImportSchemeDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation DeleteScheme($id: String!) {\n    deleteScheme(id: $id)\n  }\n"): typeof import('./graphql').DeleteSchemeDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation PublishScheme($id: String!, $published: Boolean!) {\n    publishScheme(id: $id, published: $published) {\n      id\n    }\n  }\n"): typeof import('./graphql').PublishSchemeDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query DecodeScheme($code: String!) {\n    decodeScheme(code: $code) {\n      name\n      code\n      palette {\n        tokens {\n          name\n          value\n        }\n        warnings {\n          foreground\n          background\n          ratio\n          minimum\n        }\n      }\n    }\n  }\n"): typeof import('./graphql').DecodeSchemeDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query SettingsEngine {\n    downloadEngine {\n      downloadPath\n      killSwitch\n    }\n  }\n"): typeof import('./graphql').SettingsEngineDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation AddSource($input: SourceInput!) {\n    addSource(input: $input) {\n      raw\n    }\n  }\n"): typeof import('./graphql').AddSourceDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation UpdateSource($name: String!, $input: SourceInput!) {\n    updateSource(name: $name, input: $input) {\n      raw\n    }\n  }\n"): typeof import('./graphql').UpdateSourceDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation RemoveSource($name: String!) {\n    removeSource(name: $name) {\n      raw\n    }\n  }\n"): typeof import('./graphql').RemoveSourceDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query DetectSource($url: String!, $apiKey: String) {\n    detectSource(url: $url, apiKey: $apiKey) {\n      kind\n      url\n      feed\n      name\n      searchable\n      sample {\n        link\n        title\n        size\n        seeders\n        published\n      }\n    }\n  }\n"): typeof import('./graphql').DetectSourceDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation AddProfile($input: ProfileInput!) {\n    addProfile(input: $input) {\n      raw\n    }\n  }\n"): typeof import('./graphql').AddProfileDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation UpdateProfile($name: String!, $input: ProfileInput!) {\n    updateProfile(name: $name, input: $input) {\n      raw\n    }\n  }\n"): typeof import('./graphql').UpdateProfileDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation RemoveProfile($name: String!) {\n    removeProfile(name: $name) {\n      raw\n    }\n  }\n"): typeof import('./graphql').RemoveProfileDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query RenameSuggestions {\n    renameSuggestions {\n      id\n      library\n      managed\n      root\n      src\n      dst\n      reason\n      confidence\n    }\n  }\n"): typeof import('./graphql').RenameSuggestionsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query FileHistory {\n    fileHistory {\n      batch\n      label\n      at\n      count\n      undone\n      operations {\n        kind\n        src\n        dst\n      }\n    }\n  }\n"): typeof import('./graphql').FileHistoryDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation RefreshRenameSuggestions {\n    refreshRenameSuggestions\n  }\n"): typeof import('./graphql').RefreshRenameSuggestionsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation ApplyRenames($ids: [Int!]!) {\n    applyRenames(ids: $ids) {\n      renamed\n      problems\n    }\n  }\n"): typeof import('./graphql').ApplyRenamesDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation DismissRenames($ids: [Int!]!) {\n    dismissRenames(ids: $ids)\n  }\n"): typeof import('./graphql').DismissRenamesDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation UndoFileChanges($batch: String!) {\n    undoFileChanges(batch: $batch) {\n      undone\n      problems\n    }\n  }\n"): typeof import('./graphql').UndoFileChangesDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation Scan($library: String) {\n    scan(library: $library)\n  }\n"): typeof import('./graphql').ScanDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation AddLibrary($input: LibraryInput!) {\n    addLibrary(input: $input) {\n      raw\n    }\n  }\n"): typeof import('./graphql').AddLibraryDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation UpdateLibrary($name: String!, $input: LibraryInput!) {\n    updateLibrary(name: $name, input: $input) {\n      raw\n    }\n  }\n"): typeof import('./graphql').UpdateLibraryDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation RemoveLibrary($name: String!) {\n    removeLibrary(name: $name) {\n      raw\n    }\n  }\n"): typeof import('./graphql').RemoveLibraryDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Folders($path: String) {\n    folders(path: $path) {\n      path\n      parent\n      home\n      folders {\n        name\n        path\n      }\n    }\n  }\n"): typeof import('./graphql').FoldersDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation ReplaceConfig($text: String!) {\n    replaceConfig(text: $text) {\n      raw\n    }\n  }\n"): typeof import('./graphql').ReplaceConfigDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query SkippedFiles {\n    skippedFiles {\n      library\n      path\n      reason\n    }\n  }\n"): typeof import('./graphql').SkippedFilesDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Users {\n    users {\n      ...Viewer\n      createdAt\n      lastSeen\n      overrides {\n        allLibraries\n        libraries\n        request\n        autoApprove\n        requestLimit\n        manageRequests\n        manageShows\n        downloads\n        editMetadata\n        watchTogether\n        shareLinks\n        clip\n        clipMaxLength\n        clipLimit\n        clipStorage\n        clipLinks\n      }\n    }\n  }\n"): typeof import('./graphql').UsersDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query PermissionDefaults {\n    permissionDefaults {\n      ...PermissionsFields\n    }\n  }\n"): typeof import('./graphql').PermissionDefaultsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation SetPermissionDefaults($permissions: PermissionsInput!) {\n    setPermissionDefaults(permissions: $permissions) {\n      ...PermissionsFields\n    }\n  }\n"): typeof import('./graphql').SetPermissionDefaultsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation UpdateUser($id: Int!, $input: UserPatch!) {\n    updateUser(id: $id, input: $input) {\n      id\n    }\n  }\n"): typeof import('./graphql').UpdateUserDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation DeleteUser($id: Int!) {\n    deleteUser(id: $id)\n  }\n"): typeof import('./graphql').DeleteUserDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation CreateUser($input: NewUser!) {\n    createUser(input: $input) {\n      id\n    }\n  }\n"): typeof import('./graphql').CreateUserDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation SaveSection($patch: ConfigPatch!) {\n    updateSettings(patch: $patch) {\n      raw\n    }\n  }\n"): typeof import('./graphql').SaveSectionDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "fragment Person on User {\n  id\n  username\n  avatar\n}\n\nfragment PermissionsFields on Permissions {\n  allLibraries\n  libraries\n  request\n  autoApprove\n  requestLimit\n  manageRequests\n  manageShows\n  downloads\n  editMetadata\n  watchTogether\n  shareLinks\n  clip\n  clipMaxLength\n  clipLimit\n  clipStorage\n  clipLinks\n}\n\nfragment Viewer on User {\n  ...Person\n  isAdmin\n  permissions {\n    ...PermissionsFields\n  }\n}\n\nfragment Card on Title {\n  id\n  kind\n  library\n  name\n  year\n  poster\n  backdrop\n  watchedCount\n  videoCount\n  progress\n  freshCount\n}\n\nfragment VideoRow on Video {\n  id\n  season\n  episode\n  episodeEnd\n  label\n  name\n  overview\n  still\n  customStill\n  airDate\n  duration\n  position\n  finished\n}\n\nfragment TitleDetail on Title {\n  ...Card\n  posterTint\n  backdropTint\n  customPoster\n  customBackdrop\n  overview\n  genres\n  rating\n  path\n  matchState\n  provider\n  providerId\n  libraryProvider\n  seasons {\n    number\n    name\n    title\n    overview\n    poster\n    episodes {\n      ...VideoRow\n    }\n  }\n  movie {\n    ...VideoRow\n  }\n  nextUp {\n    resuming\n    video {\n      ...VideoRow\n    }\n  }\n}\n\nfragment Playback on Video {\n  id\n  still\n  label\n  name\n  position\n  finished\n  title {\n    id\n    kind\n    name\n    backdrop\n  }\n  previous {\n    id\n    label\n    name\n  }\n  next {\n    id\n    still\n    label\n    name\n  }\n  media {\n    duration\n    video {\n      index\n      codec\n      codecString\n      width\n      height\n      fps\n      bitDepth\n      hdr\n    }\n    audio {\n      index\n      codec\n      codecString\n      channels\n      language\n      title\n      default\n    }\n    subtitles {\n      id\n      codec\n      language\n      title\n      default\n      forced\n      supported\n    }\n    fonts {\n      index\n      filename\n    }\n    chapters {\n      start\n      end\n      title\n    }\n  }\n}\n\nfragment TranscodingFields on Transcoding {\n  vaapi\n  vaapiError\n  softwareH264\n}\n\nfragment DiscoverResultFields on DiscoverResult {\n  category\n  provider\n  id\n  name\n  romaji\n  year\n  poster\n  overview\n  library\n  titleId\n  seriesId\n  monitor\n  requestState\n  because\n}\n\nfragment ClipFields on Clip {\n  id\n  screenshot\n  name\n  mine\n  canManage\n  owner {\n    ...Person\n  }\n  source {\n    video {\n      id\n    }\n    title {\n      id\n    }\n    name\n    kind\n    label\n    year\n    status\n  }\n  start\n  end\n  audio\n  subtitles\n  quality {\n    height\n    halfRate\n  }\n  state\n  progress\n  error\n  bytes\n  width\n  height\n  fps\n  renderedAt\n  createdAt\n  sharedAt\n  public\n  link\n  linkLive\n  recipients {\n    user {\n      ...Person\n    }\n    sharedAt\n    hidden\n  }\n  file\n  poster\n}\n\nfragment ClipAllowanceFields on ClipAllowance {\n  canClip\n  canLink\n  maxLength\n  bytes\n  rendered\n  storage\n  limit\n  customDefaultFont\n}\n\nfragment NotificationFields on Notification {\n  id\n  kind\n  priority\n  title\n  body\n  image\n  link\n  actor {\n    ...Person\n  }\n  createdAt\n  expiresAt\n  readAt\n}\n\nfragment DownloadFields on Download {\n  category\n  id\n  name\n  seriesId\n  seriesName\n  title {\n    id\n  }\n  poster\n  episodes {\n    season\n    episode\n  }\n  source\n  size\n  savePath\n  state\n  importState\n  importError\n  importMode\n  error\n  addedAt\n  finishedAt\n  importedAt\n  requestedBy {\n    username\n  }\n  live {\n    stage\n    paused\n    progress\n    downloadRate\n    uploadRate\n    done\n    uploaded\n    ratio\n    peers\n    seeds\n    seedingSeconds\n    eta\n    pieces\n  }\n  seedGoal {\n    ratio\n    seconds\n  }\n}\n\nfragment EngineFields on DownloadEngine {\n  version\n  downloadRate\n  uploadRate\n  active\n  killSwitch\n  listening\n  listenError\n  slowHours\n  downloadPath\n}\n\nfragment SeriesEpisodeFields on SeriesEpisode {\n  season\n  episode\n  absolute\n  name\n  airAt\n  aired\n  state\n  attempts\n  searchedAt\n  nextSearch\n  downloadId\n  video {\n    id\n  }\n}\n\nfragment SeedingFields on Seeding {\n  ratio\n  time\n  idle\n  then\n}\n\nfragment SeriesFields on Series {\n  id\n  monitor\n  status\n  next {\n    ...SeriesEpisodeFields\n  }\n  title {\n    id\n  }\n  library\n  managed\n  path\n  name\n  year\n  poster\n  overview\n  provider\n  providerId\n  profile\n  effectiveProfile\n  sources\n  groups\n  aliases\n  knownAs\n  numbering\n  naming\n  style {\n    file\n    folder\n    agreement\n    samples\n  }\n  seeding {\n    ...SeedingFields\n  }\n  scheduleAt\n  addedAt\n  counts {\n    have\n    wanted\n    missing\n    grabbed\n    total\n    upcoming\n    skipped\n  }\n  episodes {\n    ...SeriesEpisodeFields\n  }\n}\n\nfragment ReleaseCandidateFields on ReleaseCandidate {\n  release {\n    title\n    source\n    link\n    infoHash\n    size\n    seeders\n    leechers\n    published\n    page\n  }\n  attributes {\n    group\n    resolution\n    codec\n    source\n    dualAudio\n    version\n    proper\n    tenBit\n  }\n  episodes {\n    season\n    episode\n  }\n  batch\n  verdict {\n    accepted\n    score\n    rejections\n    warnings\n    nonstandard\n  }\n}\n\nfragment CalendarEntryFields on CalendarEntry {\n  seriesId\n  title {\n    id\n  }\n  library\n  show\n  poster\n  backdrop\n  backdropTint\n  monitor\n  season\n  episode\n  absolute\n  name\n  airAt\n  state\n  video {\n    id\n  }\n  download {\n    stage\n    progress\n    downloadRate\n    eta\n  }\n}\n\nfragment SettingsFields on Settings {\n  network {\n    host\n    port\n    cors\n  }\n  log {\n    level\n  }\n  scan {\n    watch\n    interval\n  }\n  metadata {\n    tmdbApiKey\n    language\n  }\n  transcode {\n    hardware\n    vaapiDevice\n  }\n  clips {\n    enabled\n    path\n    publicLinks\n    concurrency\n    maxStorage\n    fontsDir\n    defaultFont\n  }\n  music {\n    onlineLyrics\n    lyricsUrl\n    analyzeLoudness\n  }\n  downloads {\n    path\n    import\n    port\n    upnp\n    dht\n    maxActive\n    downloadLimit\n    uploadLimit\n    slowDownloadLimit\n    slowUploadLimit\n    slowFrom\n    slowTo\n    bindInterface\n    proxy\n    seeding {\n      ...SeedingFields\n    }\n  }\n  automation {\n    defaultMonitor\n    rssInterval\n    retry {\n      every\n      until\n    }\n    renameSuggestions\n  }\n  requests {\n    monitor\n  }\n  signIn {\n    style\n  }\n  sources {\n    name\n    kind\n    url\n    feed\n    apiKey\n    categories\n    enabled\n    downloadPath\n    seeding {\n      ...SeedingFields\n    }\n  }\n  profiles {\n    name\n    resolutions\n    groups\n    require\n    reject\n    minSize\n    maxSize\n    codecs\n    preferDualAudio\n    batches\n    minSeeders\n  }\n  libraries {\n    name\n    path\n    kind\n    metadataProvider\n    managed\n    profile\n    downloadPath\n    resolvedPath\n    exists\n    error\n    titleCount\n    skippedCount\n  }\n  raw\n  error\n  paths {\n    config\n    data\n    log\n  }\n}"): typeof import('./graphql').PersonFragmentDoc;


export function graphql(source: string) {
  return (documents as any)[source] ?? {};
}
