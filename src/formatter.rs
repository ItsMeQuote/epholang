//! EphoFMT: the canonical source formatter for Epholang.
//!
//! This first implementation preserves expression spelling and comments while
//! normalizing block layout, indentation, line endings, and semicolon separators.

/// Format Epholang source code using the official layout rules.
pub fn format(source: &str) -> Result<String, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut output = String::new();
    let mut line = String::new();
    let mut indent = 0usize;
    let mut braces: Vec<bool> = Vec::new(); // true = block, false = literal
    let mut parens = 0usize;
    let mut brackets = 0usize;
    let mut i = 0usize;

    fn flush(output: &mut String, line: &mut String, indent: usize) {
        let trimmed = line.trim_matches(|c| c == ' ' || c == '\t' || c == '\r');
        if !trimmed.is_empty() {
            output.push_str(&"\t".repeat(indent));
            output.push_str(trimmed);
            output.push('\n');
        } else if !output.is_empty() && !output.ends_with("\n\n") {
            output.push('\n');
        }
        line.clear();
    }

    fn starts_block_header(line: &str) -> bool {
        let value = line.trim_start();
        value.starts_with("function ")
            || value == "function"
            || value.starts_with("if ")
            || value == "if"
            || value.starts_with("loop ")
            || value == "loop"
            || value.starts_with("otherwise")
            || value.contains("} otherwise")
    }

    while i < chars.len() {
        let ch = chars[i];

        // Strings (including interpolation expressions) are opaque.
        if ch == '"' {
            let start = i;
            i += 1;
            let mut closed = false;
            while i < chars.len() {
                if chars[i] == '\\' {
                    i = (i + 2).min(chars.len());
                    continue;
                }
                if chars[i] == '"' {
                    i += 1;
                    closed = true;
                    break;
                }
                i += 1;
            }
            if !closed {
                return Err(format!("Unclosed string starting at character {}", start + 1));
            }
            line.extend(chars[start..i].iter());
            continue;
        }

        // Preserve nested block comments as opaque text.
        if ch == '/' && chars.get(i + 1) == Some(&'*') {
            let start = i;
            i += 2;
            let mut depth = 1usize;
            while i < chars.len() && depth > 0 {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if depth != 0 {
                return Err(format!("Unclosed block comment starting at character {}", start + 1));
            }
            line.extend(chars[start..i].iter());
            continue;
        }

        match ch {
            '\r' => {
                if chars.get(i + 1) == Some(&'\n') {
                    i += 1;
                }
                if parens > 0 || brackets > 0 {
                    if !line.ends_with(' ') { line.push(' '); }
                } else {
                    flush(&mut output, &mut line, indent);
                }
            }
            '\n' => {
                if parens > 0 || brackets > 0 {
                    if !line.ends_with(' ') { line.push(' '); }
                } else {
                    flush(&mut output, &mut line, indent);
                }
            }
            ';' if parens == 0 && brackets == 0 => {
                flush(&mut output, &mut line, indent);
            }
            '(' => { parens += 1; line.push(ch); }
            ')' => { parens = parens.saturating_sub(1); line.push(ch); }
            '[' => { brackets += 1; line.push(ch); }
            ']' => { brackets = brackets.saturating_sub(1); line.push(ch); }
            '{' => {
                if starts_block_header(&line) {
                    while line.ends_with(' ') || line.ends_with('\t') { line.pop(); }
                    if !line.is_empty() { line.push(' '); }
                    line.push('{');
                    flush(&mut output, &mut line, indent);
                    braces.push(true);
                    indent += 1;
                } else {
                    line.push('{');
                    braces.push(false);
                }
            }
            '}' => {
                match braces.pop() {
                    Some(true) => {
                        flush(&mut output, &mut line, indent);
                        indent = indent.saturating_sub(1);
                        line.push('}');

                        let mut next = i + 1;
                        while next < chars.len()
                            && (chars[next] == ' ' || chars[next] == '\t'
                                || chars[next] == '\r' || chars[next] == '\n')
                        {
                            next += 1;
                        }
                        if chars[next..].starts_with(&['o', 't', 'h', 'e', 'r', 'w', 'i', 's', 'e'])
                            && chars.get(next + 8).is_none_or(|c| !c.is_ascii_alphanumeric() && *c != '_')
                        {
                            line.push_str(" otherwise");
                            i = next + 7;
                        }
                    }
                    Some(false) => line.push('}'),
                    None => return Err(format!("Unmatched closing brace at character {}", i + 1)),
                }
            }
            c if c.is_whitespace() => {
                if !line.is_empty() && !line.ends_with(' ') && !line.ends_with('\t') {
                    line.push(' ');
                }
            }
            _ => line.push(ch),
        }
        i += 1;
    }

    if !braces.is_empty() { return Err("Unclosed brace".to_string()); }
    if parens != 0 { return Err("Unclosed parenthesis".to_string()); }
    if brackets != 0 { return Err("Unclosed square bracket".to_string()); }

    flush(&mut output, &mut line, indent);
    while output.ends_with("\n\n") { output.pop(); }
    if !output.is_empty() && !output.ends_with('\n') { output.push('\n'); }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::format;

    #[test]
    fn separates_semicolon_statements() {
        assert_eq!(format("value str := \"hi\"; Output(value)").unwrap(),
            "value str := \"hi\"\nOutput(value)\n");
    }

    #[test]
    fn removes_trailing_semicolon() {
        assert_eq!(format("Output(\"hi\");").unwrap(), "Output(\"hi\")\n");
    }

    #[test]
    fn indents_blocks_with_tabs_and_joins_otherwise() {
        let source = "if ready {\nOutput(\"yes\")\n}\notherwise {\nOutput(\"no\")\n}";
        assert_eq!(format(source).unwrap(),
            "if ready {\n\tOutput(\"yes\")\n} otherwise {\n\tOutput(\"no\")\n}\n");
    }

    #[test]
    fn does_not_split_semicolons_inside_strings_or_comments() {
        assert_eq!(format("Output(\"a;b\") /* x;y */").unwrap(),
            "Output(\"a;b\") /* x;y */\n");
    }

    #[test]
    fn is_idempotent() {
        let once = format("function Hello() none {\nOutput(\"hi\");\n}").unwrap();
        assert_eq!(once, format(&once).unwrap());
    }

    #[test]
    fn preserves_list_literal_braces() {
        assert_eq!(format("values ls<int> := list<int>{1, 2, 3}").unwrap(),
            "values ls<int> := list<int>{1, 2, 3}\n");
    }
}
