-- Books. What a file says about itself is read on every scan; what was found
-- about it online is kept until it's looked up again.

CREATE TABLE books (
    id             INTEGER PRIMARY KEY,
    library        TEXT NOT NULL,
    path           TEXT NOT NULL UNIQUE,
    size           INTEGER NOT NULL,
    mtime          INTEGER NOT NULL,
    format         TEXT NOT NULL,                    -- epub, fb2, txt
    title          TEXT NOT NULL,
    sort_title     TEXT NOT NULL,
    authors        TEXT NOT NULL DEFAULT '[]',       -- as JSON
    series         TEXT,
    series_index   REAL,
    description    TEXT,
    publisher      TEXT,
    published      TEXT,                             -- a date, or just a year
    language       TEXT,
    isbn           TEXT,
    genres         TEXT NOT NULL DEFAULT '[]',
    length         INTEGER NOT NULL,                 -- characters of text, which reading positions count in
    chapters       INTEGER NOT NULL DEFAULT 0,
    embedded_cover INTEGER NOT NULL DEFAULT 0,
    provider       TEXT,                             -- apple-books or open-library, when found online
    provider_id    TEXT,
    overview       TEXT,                             -- the description found online
    subjects       TEXT NOT NULL DEFAULT '[]',
    online_cover   TEXT,
    online_date    TEXT,
    rating         REAL,                             -- out of 10
    pages          INTEGER,
    fetched_at     INTEGER,                          -- NULL until it's been looked up
    added_at       INTEGER NOT NULL
);
CREATE INDEX books_library ON books(library, sort_title);
CREATE INDEX books_added ON books(library, added_at);
CREATE INDEX books_unfetched ON books(id) WHERE fetched_at IS NULL;

-- Where people are in them. Like progress, kept by path so it outlives a
-- library being removed and added back.
CREATE TABLE reading (
    user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    book_path  TEXT NOT NULL,
    position   INTEGER NOT NULL,                     -- characters in
    length     INTEGER NOT NULL,                     -- the book's, when this was saved
    finished   INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, book_path)
);
CREATE INDEX reading_recent ON reading(user_id, updated_at);
