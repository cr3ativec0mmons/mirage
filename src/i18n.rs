use gettextrs::{gettext, ngettext};

/// Replaces `{key}` placeholders in a translated string.
fn substitute(mut text: String, args: &[(&str, &str)]) -> String {
    for (key, value) in args {
        text = text.replace(&format!("{{{key}}}"), value);
    }
    text
}

/// Like `gettext`, with named `{placeholders}` that translators can reorder.
pub fn gettext_f(msgid: &str, args: &[(&str, &str)]) -> String {
    substitute(gettext(msgid), args)
}

/// Like `ngettext`, with named `{placeholders}` that translators can reorder.
pub fn ngettext_f(msgid: &str, msgid_plural: &str, n: u32, args: &[(&str, &str)]) -> String {
    substitute(ngettext(msgid, msgid_plural, n), args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_named_placeholders() {
        let text = substitute("{a} and {b}, {a}".to_owned(), &[("a", "x"), ("b", "y")]);
        assert_eq!(text, "x and y, x");
    }
}
