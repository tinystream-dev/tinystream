// SPDX-License-Identifier: AGPL-3.0-or-later
// Which series a book is in, for the many that don't say: what their title
// (or else their file's name) says, as in "86—EIGHTY-SIX, Vol. 02: …".

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

static VOLUME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?P<series>.*?)[\s,:;.\-–—]*(?:\bvol(?:ume)?\.?\s*|\bbook\s+|\bv|#\s*)(?P<n>\d{1,4}(?:\.\d{1,2})?)(?:\D|$)")
        .unwrap()
});

static TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[[^\]]*\]|\{[^}]*\}").unwrap());

fn volume(s: &str) -> Option<(String, f64)> {
    let c = VOLUME.captures(s)?;
    let series = c["series"].trim();

    if !series.chars().any(char::is_alphanumeric) {
        return None;
    }

    Some((series.to_string(), c["n"].parse().ok()?))
}

/// The series and which book in it, from `title`, or else from the file's name.
pub fn guess(title: &str, path: &str) -> Option<(String, f64)> {
    volume(title).or_else(|| {
        let name = Path::new(path).file_name()?.to_string_lossy();
        let stem = name.split('.').next().unwrap_or_default().replace('_', " ");
        volume(&TAGS.replace_all(&stem, " "))
    })
}

#[cfg(test)]
mod tests {
    use super::guess;

    fn series(title: &str) -> Option<(String, f64)> {
        guess(title, "/books/book.epub")
    }

    #[test]
    fn titles_say_which_volume() {
        let eighty_six = Some(("86—EIGHTY-SIX".to_string(), 2.0));
        assert_eq!(series("86—EIGHTY-SIX, Vol. 02: Run Through the Battlefront (Start)"), eighty_six);
        assert_eq!(
            series("Mushoku Tensei: Jobless Reincarnation Vol. 10"),
            Some(("Mushoku Tensei: Jobless Reincarnation".into(), 10.0))
        );
        assert_eq!(series("Roshidere V3"), Some(("Roshidere".into(), 3.0)));
        assert_eq!(series("Roshidere, Vol. 8 - Glucose Translations"), Some(("Roshidere".into(), 8.0)));
        assert_eq!(series("Mistborn Book 2: The Well of Ascension"), Some(("Mistborn".into(), 2.0)));
        assert_eq!(series("Spice and Wolf, Volume 4.5"), Some(("Spice and Wolf".into(), 4.5)));
        assert_eq!(series("Roshidere #006"), Some(("Roshidere".into(), 6.0)));
    }

    #[test]
    fn most_titles_say_nothing() {
        assert_eq!(series("Moby Dick"), None);
        assert_eq!(series("The Book of Dust"), None);
        assert_eq!(series("Facebook 2"), None);
        assert_eq!(series("Vol. 3"), None);
        assert_eq!(series("Love v Death"), None);
    }

    #[test]
    fn or_else_the_file_name_does() {
        assert_eq!(
            guess("Roshidere", "/Light Novels/Alya/Roshidere #004 [Glucose TL].epub"),
            Some(("Roshidere".into(), 4.0))
        );
        assert_eq!(
            guess("Something", "/x/86--EIGHTY-SIX v01 [Yen Press] [Stick].epub"),
            Some(("86--EIGHTY-SIX".into(), 1.0))
        );
    }
}
