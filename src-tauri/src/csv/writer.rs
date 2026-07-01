//! Minimal RFC 4180 CSV row encoder (feature 009). A field is quoted iff it contains the delimiter,
//! a double quote, CR, or LF; quoting wraps the field in double quotes and doubles any embedded
//! quote. Output is UTF-8 with CRLF line endings. The delimiter is a single ASCII byte chosen per
//! preset (FR-033). Hand-rolled to avoid a new dependency (research R1).

/// Append one CSV row (fields joined by `delim`, terminated with CRLF) to `out`.
pub fn write_row<S: AsRef<str>>(out: &mut String, fields: &[S], delim: u8) {
    for (i, field) in fields.iter().enumerate() {
        if i > 0 {
            out.push(delim as char);
        }
        write_field(out, field.as_ref(), delim);
    }
    out.push_str("\r\n");
}

fn write_field(out: &mut String, field: &str, delim: u8) {
    let needs_quote = field
        .bytes()
        .any(|b| b == delim || b == b'"' || b == b'\r' || b == b'\n');
    if !needs_quote {
        out.push_str(field);
        return;
    }
    out.push('"');
    for c in field.chars() {
        if c == '"' {
            out.push('"');
        }
        out.push(c);
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(fields: &[&str], delim: u8) -> String {
        let mut s = String::new();
        write_row(&mut s, fields, delim);
        s
    }

    #[test]
    fn plain_fields_comma() {
        assert_eq!(row(&["a", "b", "c"], b','), "a,b,c\r\n");
    }

    #[test]
    fn semicolon_and_tab_delimiters() {
        assert_eq!(row(&["a", "b"], b';'), "a;b\r\n");
        assert_eq!(row(&["a", "b"], b'\t'), "a\tb\r\n");
    }

    #[test]
    fn quotes_field_containing_delimiter() {
        assert_eq!(row(&["a,b", "c"], b','), "\"a,b\",c\r\n");
        // The same value is NOT quoted when the delimiter is a semicolon.
        assert_eq!(row(&["a,b", "c"], b';'), "a,b;c\r\n");
    }

    #[test]
    fn escapes_embedded_quote() {
        assert_eq!(row(&["a\"b"], b','), "\"a\"\"b\"\r\n");
    }

    #[test]
    fn quotes_field_with_newline() {
        assert_eq!(row(&["a\nb"], b','), "\"a\nb\"\r\n");
        assert_eq!(row(&["a\r\nb"], b','), "\"a\r\nb\"\r\n");
    }

    #[test]
    fn empty_field_stays_empty() {
        assert_eq!(row(&["", "x"], b','), ",x\r\n");
    }
}
