-- Quotes: a clip of a book's text, which is also a highlight in it.
--
-- They live with clips so they're sent, linked and dropped the same way.
-- source_path is the book's file, show_title its title, label the chapter,
-- and range_start to range_end the characters quoted (counted as reading
-- positions are). There's nothing to render: they're ready when made.
ALTER TABLE clips ADD COLUMN quote TEXT;              -- what was quoted, its blocks a line apart; NULL for the rest
ALTER TABLE clips ADD COLUMN author TEXT;
ALTER TABLE clips ADD COLUMN book_length INTEGER;     -- the book's, when it was quoted
CREATE INDEX clips_quotes ON clips(source_path) WHERE quote IS NOT NULL;
