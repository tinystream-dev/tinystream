# Library and reader batch

Implemented #63, #64, #73, #67, #75 and the desktop reader for #94. OpenSubtitles (#78) was explicitly excluded; no OpenSubtitles changes are included.

- Incomplete video, book and music directory walks abort before catalog pruning.
- Partial imports retain their failures, expose a partial state and retry without repeating completed transfers or announcements.
- Library events and reconnects reconcile mounted web queries.
- Clearing the manual-match search stays cleared.
- Downloads and engine snapshots refresh when entering Downloads.
- Desktop book details and reading use the existing authenticated book API. The reader supports character-based progress, chapter navigation, text sizes, focus mode, images, formatted text, selection, copying, saving quotes and existing clip management for sharing.

## Verification

- Server: 156 tests passed, 2 ignored; schema consistency passed.
- Desktop: 124 app tests and 5 mpv integration tests passed; 20 app tests and the native local-video test remained ignored by default.
- The new ignored Vulkan reader render test was run explicitly and passed. Native reader, focus and selection renders were inspected; header overlap and focus scroll anchoring were corrected.
- Web: TypeScript check and 10 tests passed.
- Mobile: TypeScript check and 30 tests passed.
- Shared package: TypeScript check and 27 tests passed.
- Rust formatting and whitespace checks passed.

The reader was validated with synthetic server-format documents and headless native rendering. A live server EPUB/FB2 reading session, cross-device progress synchronization and sharing were not exercised end to end. No release builds, publishing or pushes were performed.
