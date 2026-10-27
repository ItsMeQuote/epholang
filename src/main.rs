use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

static OUTPUT_LINE_OPEN: AtomicBool = AtomicBool::new(false);

// ============================================================
// Spans and diagnostics
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
    line: usize,
    column: usize,
    length: usize,
}

impl Span {
    fn new(line: usize, column: usize, length: usize) -> Self {
        Self {
            line,
            column,
            length,
        }
    }

    fn through(self, last: Span) -> Self {
        if self.line == last.line {
            Self {
                line: self.line,
                column: self.column,
                length: (last.column + last.length)
                    .saturating_sub(self.column)
                    .max(1),
            }
        } else {
            self
        }
    }
}

#[derive(Debug, Clone)]
struct Diagnostic {
    span: Span,
    message: String,
}

impl Diagnostic {
    fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }
}

type EResult<T> = Result<T, Diagnostic>;

fn print_diagnostic(
    prefix: &str,
    path: &str,
    source: &str,
    diagnostic: &Diagnostic,
) {
    let span = diagnostic.span;

    eprintln!("{}: {}", prefix, diagnostic.message);
    eprintln!(" --> {}:{}:{}", path, span.line, span.column);

    let raw_line = source
        .split('\n')
        .nth(span.line.saturating_sub(1))
        .unwrap_or("");

    let raw_line = raw_line.strip_suffix('\r').unwrap_or(raw_line);

    // A tab counts as one column in source positions,
    // but is displayed as four spaces.
    let indentation: usize = raw_line
        .chars()
        .take(span.column.saturating_sub(1))
        .map(|ch| if ch == '\t' { 4 } else { 1 })
        .sum();

    let underline_length: usize = raw_line
        .chars()
        .skip(span.column.saturating_sub(1))
        .take(span.length.max(1))
        .map(|ch| if ch == '\t' { 4 } else { 1 })
        .sum();

    let displayed_line = raw_line.replace('\t', "    ");
    let width = span.line.to_string().len();
    let gutter = " ".repeat(width);

    eprintln!("{} |", gutter);
    eprintln!(
        "{:>width$} | {}",
        span.line,
        displayed_line,
        width = width
    );
    eprintln!(
        "{} | {}{}",
        gutter,
        " ".repeat(indentation),
        "^".repeat(underline_length.max(1))
    );
}

// ============================================================
// Lexer
// ============================================================

#[derive(Debug, Clone, PartialEq)]
enum StringPart {
    Text(String),
    Code(Vec<Token>),
}

#[derive(Debug, Clone, PartialEq)]
enum TokenKind {
    Identifier(String),
    Number(f64),
    String(String),
    Interpolated(Vec<StringPart>),
    Symbol(String),
    Newline,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
struct Token {
    kind: TokenKind,
    span: Span,
}

// Every character remembers its position in the original file.
// This matters inside interpolation: \" becomes a single ",
// yet the positions of the following characters must not shift.
#[derive(Debug, Clone, Copy)]
struct SourceChar {
    ch: char,
    span: Span,
}

fn pair(
    chars: &[SourceChar],
    i: usize,
    first: char,
    second: char,
) -> bool {
    chars.get(i).is_some_and(|c| c.ch == first)
        && chars.get(i + 1).is_some_and(|c| c.ch == second)
}

fn range_span(chars: &[SourceChar], start: usize, end: usize) -> Span {
    chars[start].span.through(chars[end - 1].span)
}

fn lex(source: &str) -> EResult<Vec<Token>> {
    let mut chars = Vec::new();
    let (mut line, mut column) = (1, 1);

    for ch in source.chars() {
        chars.push(SourceChar {
            ch,
            span: Span::new(line, column, 1),
        });

        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    lex_chars(&chars, Span::new(line, column, 1))
}

fn scan_interpolation(
    chars: &[SourceChar],
    i: &mut usize,
    opening_span: Span,
) -> EResult<(Vec<SourceChar>, Span)> {
    let mut code = Vec::new();
    let mut in_string = false;
    let mut brace_depth: usize = 0;
    let mut comment_depth: usize = 0;
    let mut comment_start = opening_span;

    while *i < chars.len() {
        let current = chars[*i];

        if current.ch == '\n' {
            return Err(Diagnostic::new(
                current.span,
                "Interpolation must be on one line",
            ));
        }

        // Quotes and braces inside comments
        // do not terminate the interpolation.
        if comment_depth > 0 {
            if pair(chars, *i, '/', '*') {
                comment_depth += 1;
                code.push(chars[*i]);
                code.push(chars[*i + 1]);
                *i += 2;
                continue;
            }

            if pair(chars, *i, '*', '/') {
                comment_depth -= 1;
                code.push(chars[*i]);
                code.push(chars[*i + 1]);
                *i += 2;
                continue;
            }

            code.push(current);
            *i += 1;
            continue;
        }

        if current.ch == '\\' {
            if let Some(next) = chars.get(*i + 1).copied() {
                if next.ch == '\n' {
                    return Err(Diagnostic::new(
                        next.span,
                        "Interpolation must be on one line",
                    ));
                }

                if next.ch == '"' {
                    in_string = !in_string;

                    code.push(SourceChar {
                        ch: '"',
                        span: current.span.through(next.span),
                    });
                } else {
                    code.push(current);
                    code.push(next);
                }

                *i += 2;
                continue;
            }
        }

        if current.ch == '"' {
            return Err(Diagnostic::new(
                current.span,
                "Use \\\" for strings inside interpolation",
            ));
        }

        if in_string {
            code.push(current);
            *i += 1;
            continue;
        }

        if pair(chars, *i, '/', '*') {
            comment_depth = 1;
            comment_start = range_span(chars, *i, *i + 2);
            code.push(chars[*i]);
            code.push(chars[*i + 1]);
            *i += 2;
            continue;
        }

        if current.ch == '{' {
            brace_depth += 1;
        } else if current.ch == '}' {
            if brace_depth == 0 {
                *i += 1;
                return Ok((code, current.span));
            }

            brace_depth -= 1;
        }

        code.push(current);
        *i += 1;
    }

    if comment_depth > 0 {
        Err(Diagnostic::new(comment_start, "Unclosed comment"))
    } else {
        Err(Diagnostic::new(
            opening_span,
            "Unclosed interpolation",
        ))
    }
}

fn lex_chars(
    chars: &[SourceChar],
    eof_span: Span,
) -> EResult<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i].ch;

        if matches!(c, ' ' | '\t' | '\r') {
            i += 1;
            continue;
        }

        if c == '\n' {
            tokens.push(Token {
                kind: TokenKind::Newline,
                span: chars[i].span,
            });
            i += 1;
            continue;
        }

        if pair(chars, i, '/', '*') {
            let opening_span = range_span(chars, i, i + 2);
            let mut depth: usize = 1;
            i += 2;

            while i < chars.len() && depth > 0 {
                if pair(chars, i, '/', '*') {
                    depth += 1;
                    i += 2;
                } else if pair(chars, i, '*', '/') {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }

            if depth != 0 {
                return Err(Diagnostic::new(
                    opening_span,
                    "Unclosed comment",
                ));
            }

            continue;
        }

        let start = i;

        if c == '"' {
            i += 1;

            let mut parts = Vec::new();
            let mut text = String::new();
            let mut closed = false;

            while i < chars.len() {
                let ch = chars[i].ch;

                if ch == '"' {
                    i += 1;
                    closed = true;
                    break;
                }

                if ch == '\n' {
                    return Err(Diagnostic::new(
                        chars[i].span,
                        "Newline inside string",
                    ));
                }

                if ch == '\\' && i + 1 < chars.len() {
                    if chars[i + 1].ch == '\n' {
                        return Err(Diagnostic::new(
                            chars[i + 1].span,
                            "Newline inside string",
                        ));
                    }

                    match chars[i + 1].ch {
                        'n' => text.push('\n'),
                        't' => text.push('\t'),
                        '"' => text.push('"'),
                        '\\' => text.push('\\'),
                        '#' => text.push('#'),
                        '}' => text.push('}'),
                        other => {
                            text.push('\\');
                            text.push(other);
                        }
                    }

                    i += 2;
                    continue;
                }

                if pair(chars, i, '#', '{') {
                    let opening_span = range_span(chars, i, i + 2);
                    i += 2;

                    let (code, code_eof) =
                        scan_interpolation(chars, &mut i, opening_span)?;

                    if code.iter().all(|c| c.ch.is_whitespace()) {
                        return Err(Diagnostic::new(
                            opening_span,
                            "Empty interpolation",
                        ));
                    }

                    let code_tokens = lex_chars(&code, code_eof)?;

                    if !text.is_empty() {
                        parts.push(StringPart::Text(
                            std::mem::take(&mut text),
                        ));
                    }

                    parts.push(StringPart::Code(code_tokens));
                    continue;
                }

                text.push(ch);
                i += 1;
            }

            if !closed {
                return Err(Diagnostic::new(
                    chars[start].span,
                    "Unclosed string",
                ));
            }

            let kind = if parts.is_empty() {
                TokenKind::String(text)
            } else {
                if !text.is_empty() {
                    parts.push(StringPart::Text(text));
                }

                TokenKind::Interpolated(parts)
            };

            tokens.push(Token {
                kind,
                span: range_span(chars, start, i),
            });

            continue;
        }

        if c.is_ascii_digit()
            || (c == '.'
                && chars
                    .get(i + 1)
                    .is_some_and(|c| c.ch.is_ascii_digit()))
        {
            while i < chars.len() && chars[i].ch.is_ascii_digit() {
                i += 1;
            }

            if i < chars.len()
                && chars[i].ch == '.'
                && chars
                    .get(i + 1)
                    .is_some_and(|c| c.ch.is_ascii_digit())
            {
                i += 1;

                while i < chars.len() && chars[i].ch.is_ascii_digit() {
                    i += 1;
                }
            }

            let span = range_span(chars, start, i);
            let text: String =
                chars[start..i].iter().map(|c| c.ch).collect();

            let number = text
                .parse::<f64>()
                .map_err(|_| Diagnostic::new(span, "Invalid number"))?;

            if !number.is_finite() {
                return Err(Diagnostic::new(
                    span,
                    "Number out of range",
                ));
            }

            tokens.push(Token {
                kind: TokenKind::Number(number),
                span,
            });

            continue;
        }

        if c.is_ascii_alphabetic() || c == '_' {
            while i < chars.len()
                && (chars[i].ch.is_ascii_alphanumeric()
                    || chars[i].ch == '_')
            {
                i += 1;
            }

            let name: String =
                chars[start..i].iter().map(|c| c.ch).collect();

            tokens.push(Token {
                kind: TokenKind::Identifier(name),
                span: range_span(chars, start, i),
            });

            continue;
        }

        let two: String =
            chars.iter().skip(i).take(2).map(|c| c.ch).collect();

        if two == "<=" || two == ">=" {
            return Err(Diagnostic::new(
                range_span(chars, i, i + 2),
                format!("Operator '{}' is not supported", two),
            ));
        }

        if ["==", "!=", ":=", "&&", "||"].contains(&two.as_str()) {
            tokens.push(Token {
                kind: TokenKind::Symbol(two),
                span: range_span(chars, i, i + 2),
            });

            i += 2;
            continue;
        }

        if "+-*/%^(){}[],;=<>!".contains(c) {
            tokens.push(Token {
                kind: TokenKind::Symbol(c.to_string()),
                span: chars[i].span,
            });

            i += 1;
            continue;
        }

        return Err(Diagnostic::new(
            chars[i].span,
            format!("Unexpected character '{}'", c),
        ));
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        span: eof_span,
    });

    Ok(tokens)
}

// ============================================================
// Values and types
// ============================================================

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Num(f64),
    Str(String),
    Bool(bool),
    Null,
    Type(String),
    List(String, Vec<Value>),
}

impl Value {
    fn type_name(&self) -> String {
        match self {
            Value::Num(_) => "num".into(),
            Value::Str(_) => "str".into(),
            Value::Bool(_) => "bool".into(),
            Value::Null => "none".into(),
            Value::Type(_) => "type".into(),
            Value::List(inner, _) => format!("ls<{}>", inner),
        }
    }

    fn display(&self) -> String {
        match self {
            Value::Num(n) if *n == 0.0 => "0".into(),
            Value::Num(n) if n.fract() == 0.0 => format!("{:.0}", n),

            Value::Num(n) => {
                let mut text = format!("{:.15}", n);

                while text.contains('.') && text.ends_with('0') {
                    text.pop();
                }

                if text.ends_with('.') {
                    text.pop();
                }

                text
            }

            Value::Str(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".into(),
            Value::Type(t) => t.clone(),

            Value::List(_, elements) => {
                let elements: Vec<String> =
                    elements.iter().map(Value::display).collect();

                format!("list{{{}}}", elements.join(", "))
            }
        }
    }
}

const ELEMENT_TYPES: [&str; 4] = ["num", "str", "bool", "none"];

fn valid_type(name: &str) -> bool {
    matches!(name, "num" | "str" | "bool" | "none" | "type")
        || list_inner(name)
            .is_some_and(|inner| ELEMENT_TYPES.contains(&inner))
}

fn list_inner(name: &str) -> Option<&str> {
    name.strip_prefix("ls<")?.strip_suffix('>')
}

#[derive(Debug, Clone)]
struct Name {
    text: String,
    span: Span,
}

#[derive(Debug, Clone)]
struct TypeAnn {
    name: String,
    span: Span,
}

fn check_reserved(name: &Name) -> EResult<()> {
    let reserved = [
        "num", "str", "bool", "none", "type", "ls", "list",
        "length", "output", "insert", "remove", "typeOf",
        "true", "false", "null", "cycle", "if", "otherwise",
        "return", "func", "function", "in",
    ];

    if reserved.contains(&name.text.as_str()) {
        Err(Diagnostic::new(
            name.span,
            format!(
                "'{}' is a reserved keyword and cannot be used as a name",
                name.text
            ),
        ))
    } else {
        Ok(())
    }
}

// ============================================================
// AST
// ============================================================

#[derive(Debug, Clone)]
struct Parameter {
    name: Name,
    declared_type: TypeAnn,
    default: Option<Expr>,
}

#[derive(Debug, Clone)]
struct FunctionDef {
    name: Name,
    parameters: Vec<Parameter>,
    return_type: TypeAnn,
    body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
enum InterpolatedPart {
    Text(String),
    Expr(Expr),
}

#[derive(Debug, Clone)]
struct Expr {
    kind: ExprKind,
    span: Span,
}

impl Expr {
    fn new(kind: ExprKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone)]
enum ExprKind {
    Literal(Value),
    Interpolate(Vec<InterpolatedPart>),
    ListLiteral(String, Vec<Expr>),
    Variable(String),
    Unary(String, Box<Expr>),
    Binary(Box<Expr>, String, Box<Expr>),
    Call(String, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone)]
struct Stmt {
    kind: Statement,
    span: Span,
}

#[derive(Debug, Clone)]
enum Statement {
    Declare {
        name: Name,
        declared_type: TypeAnn,
        value: Expr,
    },

    Assign {
        name: Name,
        value: Expr,
    },

    AssignIndex {
        name: Name,
        index: Expr,
        value: Expr,
    },

    Expression(Expr),

    Return {
        declared_type: TypeAnn,
        value: Expr,
    },

    Function(FunctionDef),

    If {
        branches: Vec<(Expr, Vec<Stmt>)>,
        otherwise_body: Option<Vec<Stmt>>,
    },

    While {
        condition: Expr,
        body: Vec<Stmt>,
    },

    CycleIn {
        item_name: Name,
        list_expr: Expr,
        body: Vec<Stmt>,
    },
}

// ============================================================
// Parser
// ============================================================

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn look(&self, offset: usize) -> &TokenKind {
        &self.tokens[
            (self.pos + offset).min(self.tokens.len() - 1)
        ]
        .kind
    }

    fn advance(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();

        if !matches!(&token.kind, TokenKind::Eof) {
            self.pos += 1;
        }

        token
    }

    fn symbol(&self, value: &str) -> bool {
        matches!(
            &self.current().kind,
            TokenKind::Symbol(s) if s == value
        )
    }

    fn identifier_is(&self, value: &str) -> bool {
        matches!(
            &self.current().kind,
            TokenKind::Identifier(s) if s == value
        )
    }

    fn skip_newlines(&mut self) {
        while matches!(&self.current().kind, TokenKind::Newline)
            || self.symbol(";")
        {
            self.advance();
        }
    }

    fn expect_symbol(&mut self, value: &str) -> EResult<Token> {
        if self.symbol(value) {
            Ok(self.advance())
        } else {
            Err(Diagnostic::new(
                self.current().span,
                format!("Expected '{}'", value),
            ))
        }
    }

    fn expect_name(&mut self) -> EResult<Name> {
        let token = self.advance();

        match token.kind {
            TokenKind::Identifier(text) => Ok(Name {
                text,
                span: token.span,
            }),

            _ => Err(Diagnostic::new(
                token.span,
                "Expected an identifier",
            )),
        }
    }

    fn parse_element_type(&mut self) -> EResult<String> {
        self.expect_symbol("<")?;

        let inner = self.expect_name()?;

        if !ELEMENT_TYPES.contains(&inner.text.as_str()) {
            return Err(Diagnostic::new(
                inner.span,
                "Invalid list element type",
            ));
        }

        self.expect_symbol(">")?;
        Ok(inner.text)
    }

    fn parse_type(&mut self) -> EResult<TypeAnn> {
        let name = self.expect_name()?;
        let span = name.span;

        let text = if name.text == "ls" {
            let inner = self.parse_element_type()?;
            format!("ls<{}>", inner)
        } else {
            name.text
        };

        Ok(TypeAnn { name: text, span })
    }

    fn parse_program(&mut self) -> EResult<Vec<Stmt>> {
        let mut statements = Vec::new();
        self.skip_newlines();

        while !matches!(&self.current().kind, TokenKind::Eof) {
            statements.push(self.parse_statement()?);

            if !matches!(
                &self.current().kind,
                TokenKind::Eof | TokenKind::Newline
            ) && !self.symbol(";")
            {
                return Err(Diagnostic::new(
                    self.current().span,
                    "Expected a newline or ';'",
                ));
            }

            self.skip_newlines();
        }

        Ok(statements)
    }

    fn parse_block(&mut self) -> EResult<Vec<Stmt>> {
        self.expect_symbol("{")?;
        self.skip_newlines();

        let mut statements = Vec::new();

        while !self.symbol("}") {
            if matches!(&self.current().kind, TokenKind::Eof) {
                return Err(Diagnostic::new(
                    self.current().span,
                    "Unclosed block",
                ));
            }

            statements.push(self.parse_statement()?);

            if !self.symbol("}")
                && !matches!(
                    &self.current().kind,
                    TokenKind::Newline
                )
                && !self.symbol(";")
            {
                return Err(Diagnostic::new(
                    self.current().span,
                    "Expected a newline or ';' inside the block",
                ));
            }

            self.skip_newlines();
        }

        self.expect_symbol("}")?;
        Ok(statements)
    }

    fn parse_function(&mut self, name: Name) -> EResult<Statement> {
        self.advance(); // func
        self.advance(); // :=
        self.advance(); // function

        self.expect_symbol("(")?;

        let mut parameters: Vec<Parameter> = Vec::new();
        let mut saw_default = false;

        if !self.symbol(")") {
            loop {
                let parameter_name = self.expect_name()?;
                check_reserved(&parameter_name)?;

                let parameter_type = self.parse_type()?;

                if parameter_type.name == "type" {
                    return Err(Diagnostic::new(
                        parameter_type.span,
                        "Function parameters cannot be of type 'type'",
                    ));
                }

                if !valid_type(&parameter_type.name)
                    || parameter_type.name == "none"
                {
                    return Err(Diagnostic::new(
                        parameter_type.span,
                        "Invalid parameter type",
                    ));
                }

                let default = if self.symbol("=") {
                    self.advance();
                    saw_default = true;
                    Some(self.parse_expression(0)?)
                } else {
                    if saw_default {
                        return Err(Diagnostic::new(
                            parameter_name.span,
                            "Required parameters cannot follow default parameters",
                        ));
                    }

                    None
                };

                if parameters
                    .iter()
                    .any(|p| p.name.text == parameter_name.text)
                {
                    return Err(Diagnostic::new(
                        parameter_name.span,
                        "Duplicate parameter name",
                    ));
                }

                parameters.push(Parameter {
                    name: parameter_name,
                    declared_type: parameter_type,
                    default,
                });

                if self.symbol(",") {
                    self.advance();

                    if self.symbol(")") {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        self.expect_symbol(")")?;

        let return_type = self.parse_type()?;

        if !valid_type(&return_type.name) {
            return Err(Diagnostic::new(
                return_type.span,
                "Invalid return type",
            ));
        }

        let body = self.parse_block()?;

        Ok(Statement::Function(FunctionDef {
            name,
            parameters,
            return_type,
            body,
        }))
    }

    fn parse_if(&mut self) -> EResult<Statement> {
        self.advance();

        let condition = self.parse_expression(0)?;
        let body = self.parse_block()?;

        let mut branches = vec![(condition, body)];
        let mut otherwise_body = None;

        loop {
            let saved = self.pos;
            self.skip_newlines();

            if !self.identifier_is("otherwise") {
                self.pos = saved;
                break;
            }

            self.advance();

            if self.identifier_is("if") {
                self.advance();

                let condition = self.parse_expression(0)?;
                let body = self.parse_block()?;

                branches.push((condition, body));
            } else {
                otherwise_body = Some(self.parse_block()?);
                break;
            }
        }

        Ok(Statement::If {
            branches,
            otherwise_body,
        })
    }

    fn parse_cycle(&mut self) -> EResult<Statement> {
        self.advance();

        if matches!(&self.current().kind, TokenKind::Identifier(_))
            && matches!(
                self.look(1),
                TokenKind::Identifier(s) if s == "in"
            )
        {
            let item_name = self.expect_name()?;
            check_reserved(&item_name)?;

            self.advance(); // in

            let list_expr = self.parse_expression(0)?;
            let body = self.parse_block()?;

            Ok(Statement::CycleIn {
                item_name,
                list_expr,
                body,
            })
        } else {
            let condition = self.parse_expression(0)?;
            let body = self.parse_block()?;

            Ok(Statement::While { condition, body })
        }
    }

    fn parse_statement(&mut self) -> EResult<Stmt> {
        let span = self.current().span;
        let kind = self.parse_statement_kind()?;

        Ok(Stmt { kind, span })
    }

    fn parse_statement_kind(&mut self) -> EResult<Statement> {
        if matches!(&self.current().kind, TokenKind::Identifier(_))
            && matches!(
                self.look(1),
                TokenKind::Identifier(s) if s == "func"
            )
            && matches!(
                self.look(2),
                TokenKind::Symbol(s) if s == ":="
            )
            && matches!(
                self.look(3),
                TokenKind::Identifier(s) if s == "function"
            )
        {
            let name = self.expect_name()?;
            check_reserved(&name)?;
            return self.parse_function(name);
        }

        if self.identifier_is("if") {
            return self.parse_if();
        }

        if self.identifier_is("cycle") {
            return self.parse_cycle();
        }

        if self.identifier_is("otherwise") {
            return Err(Diagnostic::new(
                self.current().span,
                "'otherwise' without a preceding 'if'",
            ));
        }

        if self.identifier_is("return") {
            self.advance();

            let starts_with_type = matches!(
                &self.current().kind,
                TokenKind::Identifier(s)
                    if matches!(
                        s.as_str(),
                        "num" | "str" | "bool" | "none" | "type" | "ls"
                    )
            );

            if !starts_with_type {
                return Err(Diagnostic::new(
                    self.current().span,
                    "Expected a return type after 'return' (e.g. 'return num x')",
                ));
            }

            let declared_type = self.parse_type()?;
            let value = self.parse_expression(0)?;

            return Ok(Statement::Return {
                declared_type,
                value,
            });
        }

        if matches!(&self.current().kind, TokenKind::Identifier(_)) {
            let is_declaration =
                if let TokenKind::Identifier(t) = self.look(1) {
                    if t == "ls" {
                        matches!(
                            self.look(2),
                            TokenKind::Symbol(s) if s == "<"
                        )
                    } else {
                        matches!(
                            self.look(2),
                            TokenKind::Symbol(s) if s == ":="
                        )
                    }
                } else {
                    false
                };

            if is_declaration {
                let name = self.expect_name()?;
                check_reserved(&name)?;

                let declared_type = self.parse_type()?;

                if declared_type.name == "type" {
                    return Err(Diagnostic::new(
                        declared_type.span,
                        "Cannot declare a variable of type 'type'",
                    ));
                }

                if !valid_type(&declared_type.name) {
                    return Err(Diagnostic::new(
                        declared_type.span,
                        "Invalid variable type",
                    ));
                }

                self.expect_symbol(":=")?;

                return Ok(Statement::Declare {
                    name,
                    declared_type,
                    value: self.parse_expression(0)?,
                });
            }
        }

        let expr = self.parse_expression(0)?;

        if self.symbol("=") {
            self.advance();

            let value = self.parse_expression(0)?;
            let target_span = expr.span;

            return match expr.kind {
                ExprKind::Variable(text) => Ok(Statement::Assign {
                    name: Name {
                        text,
                        span: target_span,
                    },
                    value,
                }),

                ExprKind::Index(array, index) => {
                    let array_span = array.span;

                    if let ExprKind::Variable(text) = array.kind {
                        Ok(Statement::AssignIndex {
                            name: Name {
                                text,
                                span: array_span,
                            },
                            index: *index,
                            value,
                        })
                    } else {
                        Err(Diagnostic::new(
                            array_span,
                            "Can only assign to a variable's index",
                        ))
                    }
                }

                _ => Err(Diagnostic::new(
                    target_span,
                    "Invalid assignment target",
                )),
            };
        }

        Ok(Statement::Expression(expr))
    }

    fn precedence(op: &str) -> u8 {
        match op {
            "||" => 1,
            "&&" => 2,
            "==" | "!=" | "<" | ">" => 3,
            "+" | "-" => 4,
            "*" | "/" | "%" => 5,
            "^" => 6,
            _ => 0,
        }
    }

    fn parse_expression(&mut self, min_prec: u8) -> EResult<Expr> {
        let mut left = self.parse_prefix()?;

        loop {
            let op = match &self.current().kind {
                TokenKind::Symbol(s)
                    if Self::precedence(s) >= min_prec
                        && Self::precedence(s) > 0 =>
                {
                    s.clone()
                }

                _ => break,
            };

            let op_token = self.advance();
            let prec = Self::precedence(&op);
            let next_min = if op == "^" { prec } else { prec + 1 };

            let right = self.parse_expression(next_min)?;

            left = Expr::new(
                ExprKind::Binary(
                    Box::new(left),
                    op,
                    Box::new(right),
                ),
                op_token.span,
            );
        }

        Ok(left)
    }

    fn parse_prefix(&mut self) -> EResult<Expr> {
        if self.symbol("!") {
            let token = self.advance();
            let operand = self.parse_prefix()?;

            return Ok(Expr::new(
                ExprKind::Unary("!".into(), Box::new(operand)),
                token.span,
            ));
        }

        let mut expr = self.parse_primary()?;

        while self.symbol("[") {
            let bracket = self.advance();
            let index = self.parse_expression(0)?;

            self.expect_symbol("]")?;

            expr = Expr::new(
                ExprKind::Index(Box::new(expr), Box::new(index)),
                bracket.span,
            );
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> EResult<Expr> {
        let token = self.advance();
        let span = token.span;

        let kind = match token.kind {
            TokenKind::Number(n) => ExprKind::Literal(Value::Num(n)),
            TokenKind::String(s) => ExprKind::Literal(Value::Str(s)),

            TokenKind::Interpolated(parts) => {
                let mut result = Vec::new();

                for part in parts {
                    match part {
                        StringPart::Text(text) => {
                            result.push(InterpolatedPart::Text(text));
                        }

                        StringPart::Code(tokens) => {
                            let mut sub = Parser::new(tokens);
                            let expr = sub.parse_expression(0)?;

                            if !matches!(
                                &sub.current().kind,
                                TokenKind::Eof
                            ) {
                                return Err(Diagnostic::new(
                                    sub.current().span,
                                    "Unexpected token inside interpolation",
                                ));
                            }

                            result.push(InterpolatedPart::Expr(expr));
                        }
                    }
                }

                ExprKind::Interpolate(result)
            }

            TokenKind::Identifier(name) if name == "list" => {
                let inner = self.parse_element_type()?;
                self.expect_symbol("{")?;

                let mut elements = Vec::new();

                if !self.symbol("}") {
                    loop {
                        elements.push(self.parse_expression(0)?);

                        if self.symbol(",") {
                            self.advance();

                            if self.symbol("}") {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }

                self.expect_symbol("}")?;
                ExprKind::ListLiteral(inner, elements)
            }

            TokenKind::Identifier(name)
                if matches!(
                    name.as_str(),
                    "num" | "str" | "bool" | "none" | "type"
                ) =>
            {
                ExprKind::Literal(Value::Type(name))
            }

            TokenKind::Identifier(name) if name == "ls" => {
                let inner = self.parse_element_type()?;
                ExprKind::Literal(Value::Type(format!("ls<{}>", inner)))
            }

            TokenKind::Identifier(name) if name == "true" => {
                ExprKind::Literal(Value::Bool(true))
            }

            TokenKind::Identifier(name) if name == "false" => {
                ExprKind::Literal(Value::Bool(false))
            }

            TokenKind::Identifier(name) if name == "null" => {
                ExprKind::Literal(Value::Null)
            }

            TokenKind::Identifier(name) => {
                if self.symbol("(") {
                    self.advance();

                    let mut args = Vec::new();

                    if !self.symbol(")") {
                        loop {
                            args.push(self.parse_expression(0)?);

                            if self.symbol(",") {
                                self.advance();

                                if self.symbol(")") {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    }

                    self.expect_symbol(")")?;
                    ExprKind::Call(name, args)
                } else {
                    ExprKind::Variable(name)
                }
            }

            TokenKind::Symbol(op) if op == "(" => {
                if self.symbol("-") {
                    let minus = self.advance();
                    let operand = self.parse_expression(7)?;

                    self.expect_symbol(")")?;

                    return Ok(Expr::new(
                        ExprKind::Unary("-".into(), Box::new(operand)),
                        minus.span,
                    ));
                }

                let expr = self.parse_expression(0)?;
                self.expect_symbol(")")?;

                return Ok(expr);
            }

            _ => {
                return Err(Diagnostic::new(
                    span,
                    "Expected an expression (unary minus must be wrapped in parens, e.g. (-1))",
                ));
            }
        };

        Ok(Expr::new(kind, span))
    }
}

// ============================================================
// Static checking
// ============================================================

fn stmt_guarantees_return(stmt: &Stmt) -> bool {
    match &stmt.kind {
        Statement::Return { .. } => true,

        Statement::If {
            branches,
            otherwise_body: Some(body),
        } => {
            !branches.is_empty()
                && branches
                    .iter()
                    .all(|(_, body)| block_guarantees_return(body))
                && block_guarantees_return(body)
        }

        _ => false,
    }
}

fn block_guarantees_return(statements: &[Stmt]) -> bool {
    statements.iter().any(stmt_guarantees_return)
}

struct VarInfo {
    ty: Option<String>,
    used: bool,
    kind: &'static str,
    span: Span,
}

struct FuncInfo {
    params: Vec<(String, bool)>,
    return_type: String,
    used: bool,
    span: Span,
}

struct Checker {
    scopes: Vec<HashMap<String, VarInfo>>,
    functions: HashMap<String, FuncInfo>,
    function_order: Vec<String>,
    current_function: Option<(String, String)>,
    errors: Vec<Diagnostic>,
}

impl Checker {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            function_order: Vec::new(),
            current_function: None,
            errors: Vec::new(),
        }
    }

    fn err(&mut self, span: Span, message: impl Into<String>) {
        self.errors.push(Diagnostic::new(span, message));
    }

    fn check_program(mut self, program: &[Stmt]) -> Vec<Diagnostic> {
        self.check_block(program);

        let module = self.scopes.pop().unwrap();
        self.report_unused(module);

        for name in self.function_order.clone() {
            let info = &self.functions[&name];

            if !info.used {
                let span = info.span;
                self.err(span, format!("Unused function '{}'", name));
            }
        }

        self.errors
            .sort_by_key(|e| (e.span.line, e.span.column));

        self.errors
    }

    fn report_unused(&mut self, scope: HashMap<String, VarInfo>) {
        let mut unused: Vec<_> = scope
            .into_iter()
            .filter(|(_, info)| !info.used)
            .collect();

        unused.sort_by_key(|(_, info)| {
            (info.span.line, info.span.column)
        });

        for (name, info) in unused {
            let message = match (info.kind, &self.current_function) {
                ("parameter", Some((function, _))) => {
                    format!(
                        "Unused parameter '{}' in function '{}'",
                        name, function
                    )
                }

                (kind, _) => format!("Unused {} '{}'", kind, name),
            };

            self.err(info.span, message);
        }
    }

    fn declare(
        &mut self,
        name: &Name,
        ty: Option<String>,
        kind: &'static str,
    ) {
        if self.functions.contains_key(&name.text) {
            self.err(
                name.span,
                format!("'{}' is already declared as a function", name.text),
            );
            return;
        }

        let last = self.scopes.len() - 1;

        if self.scopes[last].contains_key(&name.text) {
            self.err(
                name.span,
                format!("'{}' is already declared in this scope", name.text),
            );
            return;
        }

        if self.scopes[..last]
            .iter()
            .any(|scope| scope.contains_key(&name.text))
        {
            self.err(
                name.span,
                format!(
                    "'{}' is already declared in an outer scope",
                    name.text
                ),
            );
            return;
        }

        self.scopes[last].insert(
            name.text.clone(),
            VarInfo {
                ty,
                used: false,
                kind,
                span: name.span,
            },
        );
    }

    fn target_type(&mut self, name: &Name) -> Option<String> {
        for index in (0..self.scopes.len()).rev() {
            if let Some(info) = self.scopes[index].get(&name.text) {
                let ty = info.ty.clone();

                if index == 0 && self.current_function.is_some() {
                    self.err(
                        name.span,
                        format!(
                            "Cannot assign to global variable '{}' inside a function",
                            name.text
                        ),
                    );
                    return None;
                }

                return ty;
            }
        }

        self.err(
            name.span,
            format!("Undefined variable '{}'", name.text),
        );

        None
    }

    fn check_scoped_block(&mut self, body: &[Stmt]) {
        self.scopes.push(HashMap::new());
        self.check_block(body);

        let scope = self.scopes.pop().unwrap();
        self.report_unused(scope);
    }

    fn check_block(&mut self, statements: &[Stmt]) {
        let mut unreachable = false;

        for stmt in statements {
            if unreachable {
                if let Some((function, _)) =
                    self.current_function.clone()
                {
                    self.err(
                        stmt.span,
                        format!(
                            "Unreachable statement after return in function '{}'",
                            function
                        ),
                    );
                }
            }

            self.check_stmt(stmt);

            if stmt_guarantees_return(stmt) {
                unreachable = true;
            }
        }
    }

    fn expect_bool(&mut self, expr: &Expr, what: &str) {
        if let Some(ty) = self.expr_type(expr) {
            if ty != "bool" {
                self.err(
                    expr.span,
                    format!("{} condition must be bool, got {}", what, ty),
                );
            }
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            Statement::Function(definition) => {
                self.check_function(definition);
            }

            Statement::Declare {
                name,
                declared_type,
                value,
            } => {
                if let Some(ty) = self.expr_type(value) {
                    if ty != declared_type.name {
                        self.err(
                            value.span,
                            format!(
                                "Variable '{}' expects {}, got {}",
                                name.text, declared_type.name, ty
                            ),
                        );
                    }
                }

                self.declare(
                    name,
                    Some(declared_type.name.clone()),
                    "variable",
                );
            }

            Statement::Assign { name, value } => {
                let value_type = self.expr_type(value);
                let target_type = self.target_type(name);

                if let (Some(value_type), Some(target_type)) =
                    (value_type, target_type)
                {
                    if value_type != target_type {
                        self.err(
                            value.span,
                            format!(
                                "Cannot assign {} to '{}' of type {}",
                                value_type, name.text, target_type
                            ),
                        );
                    }
                }
            }

            Statement::AssignIndex { name, index, value } => {
                self.check_index_expr(index);

                let value_type = self.expr_type(value);
                let target_type = self.target_type(name);

                if let Some(target_type) = target_type {
                    match list_inner(&target_type) {
                        Some(inner) => {
                            if let Some(value_type) = value_type {
                                if value_type != inner {
                                    self.err(
                                        value.span,
                                        format!(
                                            "Cannot assign {} to element of {}",
                                            value_type, target_type
                                        ),
                                    );
                                }
                            }
                        }

                        None => self.err(
                            name.span,
                            format!("Cannot index into {}", target_type),
                        ),
                    }
                }
            }

            Statement::Expression(expr) => {
                self.expr_type(expr);
            }

            Statement::Return {
                declared_type,
                value,
            } => {
                let value_type = self.expr_type(value);

                match self.current_function.clone() {
                    None => self.err(
                        stmt.span,
                        "return is only allowed inside a function",
                    ),

                    Some((function, return_type)) => {
                        if declared_type.name != return_type {
                            self.err(
                                declared_type.span,
                                format!(
                                    "return declares {}, but function '{}' returns {}",
                                    declared_type.name, function, return_type
                                ),
                            );
                        }
                    }
                }

                if let Some(value_type) = value_type {
                    if value_type != declared_type.name {
                        self.err(
                            value.span,
                            format!(
                                "return declares {}, but the value is {}",
                                declared_type.name, value_type
                            ),
                        );
                    }
                }
            }

            Statement::If {
                branches,
                otherwise_body,
            } => {
                for (condition, body) in branches {
                    self.expect_bool(condition, "if");
                    self.check_scoped_block(body);
                }

                if let Some(body) = otherwise_body {
                    self.check_scoped_block(body);
                }
            }

            Statement::While { condition, body } => {
                self.expect_bool(condition, "cycle");
                self.check_scoped_block(body);
            }

            Statement::CycleIn {
                item_name,
                list_expr,
                body,
            } => {
                let item_type = match self.expr_type(list_expr) {
                    Some(ty) => match list_inner(&ty) {
                        Some(inner) => Some(inner.to_string()),

                        None => {
                            self.err(
                                list_expr.span,
                                format!(
                                    "cycle .. in .. requires a list, got {}",
                                    ty
                                ),
                            );
                            None
                        }
                    },

                    None => None,
                };

                // Keep checking the body even if the source
                // expression has a wrong type.
                self.scopes.push(HashMap::new());
                self.declare(item_name, item_type, "loop variable");
                self.check_block(body);

                let scope = self.scopes.pop().unwrap();
                self.report_unused(scope);
            }
        }
    }

    fn check_function(&mut self, definition: &FunctionDef) {
        let name = &definition.name;

        if self.current_function.is_some() || self.scopes.len() != 1 {
            self.err(
                name.span,
                "Functions can only be declared at the top level",
            );
            return;
        }

        if self.functions.contains_key(&name.text)
            || self.scopes[0].contains_key(&name.text)
        {
            self.err(
                name.span,
                format!("'{}' is already declared", name.text),
            );
            return;
        }

        for parameter in &definition.parameters {
            if let Some(default) = &parameter.default {
                if let Some(ty) = self.expr_type(default) {
                    if ty != parameter.declared_type.name {
                        self.err(
                            default.span,
                            format!(
                                "Default value of '{}' expects {}, got {}",
                                parameter.name.text,
                                parameter.declared_type.name,
                                ty
                            ),
                        );
                    }
                }
            }
        }

        self.functions.insert(
            name.text.clone(),
            FuncInfo {
                params: definition
                    .parameters
                    .iter()
                    .map(|p| {
                        (
                            p.declared_type.name.clone(),
                            p.default.is_some(),
                        )
                    })
                    .collect(),
                return_type: definition.return_type.name.clone(),
                used: false,
                span: name.span,
            },
        );

        self.function_order.push(name.text.clone());

        self.current_function = Some((
            name.text.clone(),
            definition.return_type.name.clone(),
        ));

        self.scopes.push(HashMap::new());

        for parameter in &definition.parameters {
            self.declare(
                &parameter.name,
                Some(parameter.declared_type.name.clone()),
                "parameter",
            );
        }

        self.check_block(&definition.body);

        if !block_guarantees_return(&definition.body) {
            self.err(
                name.span,
                format!(
                    "Function '{}' does not return a value on every path",
                    name.text
                ),
            );
        }

        let scope = self.scopes.pop().unwrap();
        self.report_unused(scope);
        self.current_function = None;
    }

    fn check_index_expr(&mut self, index: &Expr) {
        match &index.kind {
            ExprKind::Literal(Value::Num(n)) if *n <= 0.0 => {
                self.err(
                    index.span,
                    "Index must be positive and non-zero",
                );
            }

            ExprKind::Literal(Value::Num(n))
                if n.fract() != 0.0 =>
            {
                self.err(
                    index.span,
                    format!("Index must be an integer, got {}", n),
                );
            }

            ExprKind::Unary(op, inner)
                if op == "-"
                    && matches!(
                        &inner.kind,
                        ExprKind::Literal(Value::Num(_))
                    ) =>
            {
                self.err(
                    index.span,
                    "Index must be positive and non-zero",
                );
            }

            _ => {}
        }

        if let Some(ty) = self.expr_type(index) {
            if ty != "num" {
                self.err(
                    index.span,
                    format!("Index must be num, got {}", ty),
                );
            }
        }
    }

    fn expr_type(&mut self, expr: &Expr) -> Option<String> {
        match &expr.kind {
            ExprKind::Literal(value) => Some(value.type_name()),

            ExprKind::Interpolate(parts) => {
                for part in parts {
                    if let InterpolatedPart::Expr(inner) = part {
                        self.expr_type(inner);
                    }
                }

                Some("str".into())
            }

            ExprKind::ListLiteral(inner, elements) => {
                for element in elements {
                    if let Some(ty) = self.expr_type(element) {
                        if ty != *inner {
                            self.err(
                                element.span,
                                format!(
                                    "List element type mismatch: expected {}, got {}",
                                    inner, ty
                                ),
                            );
                        }
                    }
                }

                Some(format!("ls<{}>", inner))
            }

            ExprKind::Variable(name) => {
                for scope in self.scopes.iter_mut().rev() {
                    if let Some(info) = scope.get_mut(name) {
                        info.used = true;
                        return info.ty.clone();
                    }
                }

                self.err(
                    expr.span,
                    format!("Undefined variable '{}'", name),
                );
                None
            }

            ExprKind::Unary(op, operand) => {
                let ty = self.expr_type(operand)?;

                match (op.as_str(), ty.as_str()) {
                    ("-", "num") => Some("num".into()),
                    ("!", "bool") => Some("bool".into()),

                    ("-", _) => {
                        self.err(
                            expr.span,
                            format!("Unary '-' requires num, got {}", ty),
                        );
                        None
                    }

                    _ => {
                        self.err(
                            expr.span,
                            format!("Unary '!' requires bool, got {}", ty),
                        );
                        None
                    }
                }
            }

            ExprKind::Binary(left, op, right) => {
                let left_type = self.expr_type(left);
                let right_type = self.expr_type(right);

                let (a, b) = (left_type?, right_type?);

                let result = match op.as_str() {
                    "+" | "-" | "*" | "/" | "%" | "^"
                        if a == "num" && b == "num" =>
                    {
                        "num"
                    }

                    "<" | ">" if a == "num" && b == "num" => "bool",

                    "&&" | "||" if a == "bool" && b == "bool" => {
                        "bool"
                    }

                    "==" | "!="
                        if a == b
                            && matches!(
                                a.as_str(),
                                "num" | "str" | "bool" | "type"
                            ) =>
                    {
                        "bool"
                    }

                    "==" | "!="
                        if list_inner(&a).is_some()
                            && list_inner(&b).is_some() =>
                    {
                        "bool"
                    }

                    _ => {
                        self.err(
                            expr.span,
                            format!(
                                "Operator '{}' cannot be applied to {} and {}",
                                op, a, b
                            ),
                        );
                        return None;
                    }
                };

                Some(result.into())
            }

            ExprKind::Index(array, index) => {
                let array_type = self.expr_type(array);
                self.check_index_expr(index);

                let array_type = array_type?;

                match list_inner(&array_type) {
                    Some(inner) => Some(inner.to_string()),

                    None => {
                        self.err(
                            array.span,
                            format!("Cannot index into {}", array_type),
                        );
                        None
                    }
                }
            }

            ExprKind::Call(name, args) => {
                self.call_type(name, args, expr.span)
            }
        }
    }

    fn expect_count(
        &mut self,
        name: &str,
        args: &[Expr],
        count: usize,
        span: Span,
    ) -> bool {
        if args.len() == count {
            return true;
        }

        self.err(
            span,
            format!(
                "{}() expects {} argument(s), got {}",
                name,
                count,
                args.len()
            ),
        );

        for arg in args {
            self.expr_type(arg);
        }

        false
    }

    fn call_type(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Option<String> {
        match name {
            "output" | "typeOf" => {
                if self.expect_count(name, args, 1, span) {
                    self.expr_type(&args[0]);
                }

                Some(
                    if name == "output" { "none" } else { "type" }
                        .into(),
                )
            }

            "length" => {
                if self.expect_count(name, args, 1, span) {
                    if let Some(ty) = self.expr_type(&args[0]) {
                        if ty != "str" && list_inner(&ty).is_none() {
                            self.err(
                                args[0].span,
                                format!(
                                    "length() requires a string or list, got {}",
                                    ty
                                ),
                            );
                        }
                    }
                }

                Some("num".into())
            }

            "insert" | "remove" => {
                let count = if name == "insert" { 3 } else { 2 };

                if !self.expect_count(name, args, count, span) {
                    return None;
                }

                let list_type = self.expr_type(&args[0]);
                self.check_index_expr(&args[1]);

                let value_type = if name == "insert" {
                    self.expr_type(&args[2])
                } else {
                    None
                };

                let list_type = list_type?;

                let inner = match list_inner(&list_type) {
                    Some(inner) => inner,

                    None => {
                        self.err(
                            args[0].span,
                            format!(
                                "{}() expects a list as first argument, got {}",
                                name, list_type
                            ),
                        );
                        return None;
                    }
                };

                if let Some(ty) = value_type {
                    if ty != inner {
                        self.err(
                            args[2].span,
                            format!(
                                "Cannot insert {} into {}",
                                ty, list_type
                            ),
                        );
                    }
                }

                Some(list_type)
            }

            _ => {
                let arg_types: Vec<Option<String>> = args
                    .iter()
                    .map(|arg| self.expr_type(arg))
                    .collect();

                let recursive = matches!(
                    &self.current_function,
                    Some((current, _)) if current == name
                );

                let (params, return_type) =
                    match self.functions.get_mut(name) {
                        Some(info) => {
                            if !recursive {
                                info.used = true;
                            }

                            (
                                info.params.clone(),
                                info.return_type.clone(),
                            )
                        }

                        None => {
                            self.err(
                                span,
                                format!("Unknown function '{}'", name),
                            );
                            return None;
                        }
                    };

                let required = params
                    .iter()
                    .filter(|(_, default)| !*default)
                    .count();

                if args.len() < required || args.len() > params.len() {
                    self.err(
                        span,
                        format!(
                            "Function '{}' expects {} to {} argument(s), got {}",
                            name,
                            required,
                            params.len(),
                            args.len()
                        ),
                    );
                }

                for (index, ((arg, ty), (expected, _))) in args
                    .iter()
                    .zip(arg_types.iter())
                    .zip(params.iter())
                    .enumerate()
                {
                    if let Some(ty) = ty {
                        if ty != expected {
                            self.err(
                                arg.span,
                                format!(
                                    "Argument {} of '{}' expects {}, got {}",
                                    index + 1,
                                    name,
                                    expected,
                                    ty
                                ),
                            );
                        }
                    }
                }

                Some(return_type)
            }
        }
    }
}

// ============================================================
// Interpreter
// ============================================================

struct Variable {
    declared_type: String,
    value: Value,
}

struct Interpreter {
    scopes: Vec<HashMap<String, Variable>>,
    functions: HashMap<String, FunctionDef>,
    return_types: Vec<String>,
}

fn resolve_index(
    n: f64,
    length: usize,
    is_insert: bool,
    span: Span,
) -> EResult<usize> {
    if !n.is_finite() {
        return Err(Diagnostic::new(
            span,
            "Index must be a finite number",
        ));
    }

    if n.fract() != 0.0 {
        return Err(Diagnostic::new(
            span,
            format!("Index must be an integer, got {}", n),
        ));
    }

    if n <= 0.0 {
        return Err(Diagnostic::new(
            span,
            "Index must be positive and non-zero",
        ));
    }

    let maximum = if is_insert {
        length.checked_add(1).ok_or_else(|| {
            Diagnostic::new(span, "List size out of range")
        })?
    } else {
        length
    };

    if n > maximum as f64 {
        return Err(Diagnostic::new(
            span,
            format!(
                "Index {} out of bounds (max {})",
                Value::Num(n).display(),
                maximum
            ),
        ));
    }

    let index = n as usize;

    if index == 0 || index > maximum {
        return Err(Diagnostic::new(
            span,
            "Index out of bounds",
        ));
    }

    Ok(index - 1)
}

impl Interpreter {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            return_types: Vec::new(),
        }
    }

    fn lookup(&self, name: &str) -> Option<&Variable> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
    }

    fn target_mut(&mut self, name: &Name) -> EResult<&mut Variable> {
        let index = self
            .scopes
            .iter()
            .rposition(|scope| scope.contains_key(&name.text))
            .ok_or_else(|| {
                Diagnostic::new(
                    name.span,
                    format!("Undefined variable '{}'", name.text),
                )
            })?;

        if index == 0 && !self.return_types.is_empty() {
            return Err(Diagnostic::new(
                name.span,
                format!(
                    "Cannot assign to global variable '{}' inside a function",
                    name.text
                ),
            ));
        }

        self.scopes[index]
            .get_mut(&name.text)
            .ok_or_else(|| {
                Diagnostic::new(
                    name.span,
                    format!("Undefined variable '{}'", name.text),
                )
            })
    }

    fn run(&mut self, program: &[Stmt]) -> EResult<()> {
        self.execute_block(program)?;
        Ok(())
    }

    fn run_scoped(
        &mut self,
        body: &[Stmt],
        scope: HashMap<String, Variable>,
    ) -> EResult<Option<Value>> {
        self.scopes.push(scope);

        let result = self.execute_block(body);

        self.scopes.pop();
        result
    }

    fn execute_block(
        &mut self,
        statements: &[Stmt],
    ) -> EResult<Option<Value>> {
        for stmt in statements {
            if let Some(value) = self.execute_stmt(stmt)? {
                return Ok(Some(value));
            }
        }

        Ok(None)
    }

    fn execute_stmt(&mut self, stmt: &Stmt) -> EResult<Option<Value>> {
        match &stmt.kind {
            Statement::Function(definition) => {
                if self.scopes.len() != 1
                    || !self.return_types.is_empty()
                {
                    return Err(Diagnostic::new(
                        definition.name.span,
                        "Functions can only be declared at the top level",
                    ));
                }

                if self.functions.contains_key(&definition.name.text)
                    || self.lookup(&definition.name.text).is_some()
                {
                    return Err(Diagnostic::new(
                        definition.name.span,
                        format!(
                            "'{}' is already declared",
                            definition.name.text
                        ),
                    ));
                }

                self.functions.insert(
                    definition.name.text.clone(),
                    definition.clone(),
                );
            }

            Statement::Declare {
                name,
                declared_type,
                value,
            } => {
                let evaluated = self.evaluate(value)?;

                if evaluated.type_name() != declared_type.name {
                    return Err(Diagnostic::new(
                        value.span,
                        format!(
                            "Variable '{}' expects {}, got {}",
                            name.text,
                            declared_type.name,
                            evaluated.type_name()
                        ),
                    ));
                }

                let scope = self.scopes.last_mut().unwrap();

                if scope.contains_key(&name.text) {
                    return Err(Diagnostic::new(
                        name.span,
                        format!(
                            "'{}' is already declared in this scope",
                            name.text
                        ),
                    ));
                }

                scope.insert(
                    name.text.clone(),
                    Variable {
                        declared_type: declared_type.name.clone(),
                        value: evaluated,
                    },
                );
            }

            Statement::Assign { name, value } => {
                let new_value = self.evaluate(value)?;
                let variable = self.target_mut(name)?;

                if new_value.type_name() != variable.declared_type {
                    return Err(Diagnostic::new(
                        value.span,
                        format!(
                            "Cannot assign {} to '{}' of type {}",
                            new_value.type_name(),
                            name.text,
                            variable.declared_type
                        ),
                    ));
                }

                variable.value = new_value;
            }

            Statement::AssignIndex { name, index, value } => {
                let index_value = self.evaluate(index)?;
                let new_value = self.evaluate(value)?;
                let variable = self.target_mut(name)?;

                match &mut variable.value {
                    Value::List(inner, elements) => {
                        if new_value.type_name() != *inner {
                            return Err(Diagnostic::new(
                                value.span,
                                format!(
                                    "Cannot assign {} to element of ls<{}>",
                                    new_value.type_name(),
                                    inner
                                ),
                            ));
                        }

                        let n = match index_value {
                            Value::Num(n) => n,

                            other => {
                                return Err(Diagnostic::new(
                                    index.span,
                                    format!(
                                        "Index must be num, got {}",
                                        other.type_name()
                                    ),
                                ));
                            }
                        };

                        let resolved = resolve_index(
                            n,
                            elements.len(),
                            false,
                            index.span,
                        )?;

                        elements[resolved] = new_value;
                    }

                    other => {
                        return Err(Diagnostic::new(
                            name.span,
                            format!(
                                "Cannot index into {}",
                                other.type_name()
                            ),
                        ));
                    }
                }
            }

            Statement::Expression(expr) => {
                self.evaluate(expr)?;
            }

            Statement::Return {
                declared_type,
                value,
            } => {
                let expected = self
                    .return_types
                    .last()
                    .cloned()
                    .ok_or_else(|| {
                        Diagnostic::new(
                            stmt.span,
                            "return is only allowed inside a function",
                        )
                    })?;

                if declared_type.name != expected {
                    return Err(Diagnostic::new(
                        declared_type.span,
                        format!(
                            "return declares {}, expected {}",
                            declared_type.name, expected
                        ),
                    ));
                }

                let result = self.evaluate(value)?;

                if result.type_name() != declared_type.name {
                    return Err(Diagnostic::new(
                        value.span,
                        format!(
                            "return declares {}, but the value is {}",
                            declared_type.name,
                            result.type_name()
                        ),
                    ));
                }

                return Ok(Some(result));
            }

            Statement::If {
                branches,
                otherwise_body,
            } => {
                for (condition, body) in branches {
                    let matched = match self.evaluate(condition)? {
                        Value::Bool(value) => value,

                        other => {
                            return Err(Diagnostic::new(
                                condition.span,
                                format!(
                                    "if condition must be bool, got {}",
                                    other.type_name()
                                ),
                            ));
                        }
                    };

                    if matched {
                        return self.run_scoped(body, HashMap::new());
                    }
                }

                if let Some(body) = otherwise_body {
                    return self.run_scoped(body, HashMap::new());
                }
            }

            Statement::While { condition, body } => loop {
                let keep_going = match self.evaluate(condition)? {
                    Value::Bool(value) => value,

                    other => {
                        return Err(Diagnostic::new(
                            condition.span,
                            format!(
                                "cycle condition must be bool, got {}",
                                other.type_name()
                            ),
                        ));
                    }
                };

                if !keep_going {
                    break;
                }

                if let Some(value) =
                    self.run_scoped(body, HashMap::new())?
                {
                    return Ok(Some(value));
                }
            },

            Statement::CycleIn {
                item_name,
                list_expr,
                body,
            } => {
                let (inner, elements) =
                    match self.evaluate(list_expr)? {
                        Value::List(inner, elements) => {
                            (inner, elements)
                        }

                        other => {
                            return Err(Diagnostic::new(
                                list_expr.span,
                                format!(
                                    "cycle .. in .. requires a list, got {}",
                                    other.type_name()
                                ),
                            ));
                        }
                    };

                // elements is a separate snapshot of the list.
                for element in elements {
                    let mut scope = HashMap::new();

                    scope.insert(
                        item_name.text.clone(),
                        Variable {
                            declared_type: inner.clone(),
                            value: element,
                        },
                    );

                    if let Some(value) = self.run_scoped(body, scope)? {
                        return Ok(Some(value));
                    }
                }
            }
        }

        Ok(None)
    }

    fn evaluate(&mut self, expr: &Expr) -> EResult<Value> {
        match &expr.kind {
            ExprKind::Literal(value) => Ok(value.clone()),

            ExprKind::Interpolate(parts) => {
                let mut result = String::new();

                for part in parts {
                    match part {
                        InterpolatedPart::Text(text) => {
                            result.push_str(text);
                        }

                        InterpolatedPart::Expr(inner) => {
                            result.push_str(
                                &self.evaluate(inner)?.display(),
                            );
                        }
                    }
                }

                Ok(Value::Str(result))
            }

            ExprKind::ListLiteral(inner, elements) => {
                let mut values = Vec::new();

                for element in elements {
                    let value = self.evaluate(element)?;

                    if value.type_name() != *inner {
                        return Err(Diagnostic::new(
                            element.span,
                            format!(
                                "List element type mismatch: expected {}, got {}",
                                inner,
                                value.type_name()
                            ),
                        ));
                    }

                    values.push(value);
                }

                Ok(Value::List(inner.clone(), values))
            }

            ExprKind::Variable(name) => self
                .lookup(name)
                .map(|variable| variable.value.clone())
                .ok_or_else(|| {
                    Diagnostic::new(
                        expr.span,
                        format!("Undefined variable '{}'", name),
                    )
                }),

            ExprKind::Index(array, index) => {
                let array_value = self.evaluate(array)?;
                let index_value = self.evaluate(index)?;

                let elements = match array_value {
                    Value::List(_, elements) => elements,

                    other => {
                        return Err(Diagnostic::new(
                            array.span,
                            format!(
                                "Cannot index into {}",
                                other.type_name()
                            ),
                        ));
                    }
                };

                let n = match index_value {
                    Value::Num(n) => n,

                    other => {
                        return Err(Diagnostic::new(
                            index.span,
                            format!(
                                "Index must be num, got {}",
                                other.type_name()
                            ),
                        ));
                    }
                };

                let index = resolve_index(
                    n,
                    elements.len(),
                    false,
                    index.span,
                )?;

                Ok(elements[index].clone())
            }

            ExprKind::Unary(op, operand) => {
                let value = self.evaluate(operand)?;

                match (op.as_str(), value) {
                    ("-", Value::Num(n)) => {
                        let result = -n;

                        if !result.is_finite() {
                            return Err(Diagnostic::new(
                                expr.span,
                                "Numeric result out of range",
                            ));
                        }

                        Ok(Value::Num(result))
                    }

                    ("!", Value::Bool(value)) => {
                        Ok(Value::Bool(!value))
                    }

                    ("-", other) => Err(Diagnostic::new(
                        expr.span,
                        format!(
                            "Unary '-' requires num, got {}",
                            other.type_name()
                        ),
                    )),

                    (_, other) => Err(Diagnostic::new(
                        expr.span,
                        format!(
                            "Unary '!' requires bool, got {}",
                            other.type_name()
                        ),
                    )),
                }
            }

            ExprKind::Binary(left, op, right) => {
                let a = self.evaluate(left)?;

                if op == "&&" || op == "||" {
                    match &a {
                        Value::Bool(false) if op == "&&" => {
                            return Ok(Value::Bool(false));
                        }

                        Value::Bool(true) if op == "||" => {
                            return Ok(Value::Bool(true));
                        }

                        Value::Bool(_) => {}

                        other => {
                            return Err(Diagnostic::new(
                                expr.span,
                                format!(
                                    "Operator '{}' requires bool, got {}",
                                    op,
                                    other.type_name()
                                ),
                            ));
                        }
                    }
                }

                let b = self.evaluate(right)?;

                let result = match (a, op.as_str(), b) {
                    (Value::Num(a), "+", Value::Num(b)) => {
                        Value::Num(a + b)
                    }

                    (Value::Num(a), "-", Value::Num(b)) => {
                        Value::Num(a - b)
                    }

                    (Value::Num(a), "*", Value::Num(b)) => {
                        Value::Num(a * b)
                    }

                    (Value::Num(_), "/", Value::Num(b))
                        if b == 0.0 =>
                    {
                        return Err(Diagnostic::new(
                            expr.span,
                            "Division by zero",
                        ));
                    }

                    (Value::Num(a), "/", Value::Num(b)) => {
                        Value::Num(a / b)
                    }

                    (Value::Num(_), "%", Value::Num(b))
                        if b == 0.0 =>
                    {
                        return Err(Diagnostic::new(
                            expr.span,
                            "Modulo by zero",
                        ));
                    }

                    (Value::Num(a), "%", Value::Num(b)) => {
                        Value::Num(a % b)
                    }

                    (Value::Num(a), "^", Value::Num(b)) => {
                        Value::Num(a.powf(b))
                    }

                    (Value::Num(a), "==", Value::Num(b)) => {
                        Value::Bool(a == b)
                    }

                    (Value::Num(a), "!=", Value::Num(b)) => {
                        Value::Bool(a != b)
                    }

                    (Value::Num(a), "<", Value::Num(b)) => {
                        Value::Bool(a < b)
                    }

                    (Value::Num(a), ">", Value::Num(b)) => {
                        Value::Bool(a > b)
                    }

                    (Value::Str(a), "==", Value::Str(b)) => {
                        Value::Bool(a == b)
                    }

                    (Value::Str(a), "!=", Value::Str(b)) => {
                        Value::Bool(a != b)
                    }

                    (Value::Bool(a), "==", Value::Bool(b)) => {
                        Value::Bool(a == b)
                    }

                    (Value::Bool(a), "!=", Value::Bool(b)) => {
                        Value::Bool(a != b)
                    }

                    (Value::Bool(a), "&&", Value::Bool(b)) => {
                        Value::Bool(a && b)
                    }

                    (Value::Bool(a), "||", Value::Bool(b)) => {
                        Value::Bool(a || b)
                    }

                    (Value::Type(a), "==", Value::Type(b)) => {
                        Value::Bool(a == b)
                    }

                    (Value::Type(a), "!=", Value::Type(b)) => {
                        Value::Bool(a != b)
                    }

                    (
                        Value::List(a_type, a),
                        "==",
                        Value::List(b_type, b),
                    ) => Value::Bool(a_type == b_type && a == b),

                    (
                        Value::List(a_type, a),
                        "!=",
                        Value::List(b_type, b),
                    ) => Value::Bool(a_type != b_type || a != b),

                    (a, op, b) => {
                        return Err(Diagnostic::new(
                            expr.span,
                            format!(
                                "Operator '{}' cannot be applied to {} and {}",
                                op,
                                a.type_name(),
                                b.type_name()
                            ),
                        ));
                    }
                };

                if let Value::Num(n) = &result {
                    if !n.is_finite() {
                        return Err(Diagnostic::new(
                            expr.span,
                            "Numeric result out of range",
                        ));
                    }
                }

                Ok(result)
            }

            ExprKind::Call(name, args) => {
                self.call(name, args, expr.span)
            }
        }
    }

    fn check_count(
        name: &str,
        actual: usize,
        expected: usize,
        span: Span,
    ) -> EResult<()> {
        if actual == expected {
            Ok(())
        } else {
            Err(Diagnostic::new(
                span,
                format!(
                    "{}() expects {} argument(s), got {}",
                    name, expected, actual
                ),
            ))
        }
    }

    fn call(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> EResult<Value> {
        match name {
            "output" => {
                Self::check_count(name, args.len(), 1, span)?;

                let value = self.evaluate(&args[0])?;
                let text = value.display();

                let mut stdout = io::stdout().lock();

                write!(stdout, "{}", text).map_err(|e| {
                    Diagnostic::new(
                        span,
                        format!("Failed to write output: {}", e),
                    )
                })?;

                stdout.flush().map_err(|e| {
                    Diagnostic::new(
                        span,
                        format!("Failed to flush output: {}", e),
                    )
                })?;

                if !text.is_empty() {
                    OUTPUT_LINE_OPEN.store(
                        !text.ends_with('\n'),
                        Ordering::Relaxed,
                    );
                }

                return Ok(Value::Null);
            }

            "typeOf" => {
                Self::check_count(name, args.len(), 1, span)?;

                let value = self.evaluate(&args[0])?;
                return Ok(Value::Type(value.type_name()));
            }

            "length" => {
                Self::check_count(name, args.len(), 1, span)?;

                return match self.evaluate(&args[0])? {
                    Value::Str(text) => {
                        Ok(Value::Num(text.chars().count() as f64))
                    }

                    Value::List(_, elements) => {
                        Ok(Value::Num(elements.len() as f64))
                    }

                    other => Err(Diagnostic::new(
                        args[0].span,
                        format!(
                            "length() requires a string or list, got {}",
                            other.type_name()
                        ),
                    )),
                };
            }

            "insert" => {
                Self::check_count(name, args.len(), 3, span)?;

                let list = self.evaluate(&args[0])?;
                let index = self.evaluate(&args[1])?;
                let element = self.evaluate(&args[2])?;

                let (inner, mut elements) = match list {
                    Value::List(inner, elements) => (inner, elements),

                    other => {
                        return Err(Diagnostic::new(
                            args[0].span,
                            format!(
                                "insert() expects a list, got {}",
                                other.type_name()
                            ),
                        ));
                    }
                };

                let n = match index {
                    Value::Num(n) => n,

                    other => {
                        return Err(Diagnostic::new(
                            args[1].span,
                            format!(
                                "Index must be num, got {}",
                                other.type_name()
                            ),
                        ));
                    }
                };

                if element.type_name() != inner {
                    return Err(Diagnostic::new(
                        args[2].span,
                        format!(
                            "Cannot insert {} into ls<{}>",
                            element.type_name(),
                            inner
                        ),
                    ));
                }

                let index = resolve_index(
                    n,
                    elements.len(),
                    true,
                    args[1].span,
                )?;

                elements.insert(index, element);

                return Ok(Value::List(inner, elements));
            }

            "remove" => {
                Self::check_count(name, args.len(), 2, span)?;

                let list = self.evaluate(&args[0])?;
                let index = self.evaluate(&args[1])?;

                let (inner, mut elements) = match list {
                    Value::List(inner, elements) => (inner, elements),

                    other => {
                        return Err(Diagnostic::new(
                            args[0].span,
                            format!(
                                "remove() expects a list, got {}",
                                other.type_name()
                            ),
                        ));
                    }
                };

                let n = match index {
                    Value::Num(n) => n,

                    other => {
                        return Err(Diagnostic::new(
                            args[1].span,
                            format!(
                                "Index must be num, got {}",
                                other.type_name()
                            ),
                        ));
                    }
                };

                let index = resolve_index(
                    n,
                    elements.len(),
                    false,
                    args[1].span,
                )?;

                elements.remove(index);

                return Ok(Value::List(inner, elements));
            }

            _ => {}
        }

        let definition = self
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| {
                Diagnostic::new(
                    span,
                    format!("Unknown function '{}'", name),
                )
            })?;

        let required = definition
            .parameters
            .iter()
            .filter(|p| p.default.is_none())
            .count();

        if args.len() < required
            || args.len() > definition.parameters.len()
        {
            return Err(Diagnostic::new(
                span,
                format!(
                    "Function '{}' expects {} to {} argument(s), got {}",
                    name,
                    required,
                    definition.parameters.len(),
                    args.len()
                ),
            ));
        }

        // Remember each argument's position along with its value.
        let mut values = Vec::new();

        for arg in args {
            values.push((self.evaluate(arg)?, arg.span));
        }

        // A function cannot see the caller's local variables.
        let saved_scopes = self.scopes.split_off(1);

        self.return_types
            .push(definition.return_type.name.clone());

        let result = self.call_inner(&definition, values);

        self.return_types.pop();
        self.scopes.truncate(1);
        self.scopes.extend(saved_scopes);

        result
    }

    fn call_inner(
        &mut self,
        definition: &FunctionDef,
        values: Vec<(Value, Span)>,
    ) -> EResult<Value> {
        let mut frame = HashMap::new();
        let mut values = values.into_iter();

        for parameter in &definition.parameters {
            let (value, value_span) = match values.next() {
                Some(value) => value,

                None => match &parameter.default {
                    Some(default) => {
                        (self.evaluate(default)?, default.span)
                    }

                    None => {
                        return Err(Diagnostic::new(
                            parameter.name.span,
                            format!(
                                "Missing argument '{}' for '{}'",
                                parameter.name.text,
                                definition.name.text
                            ),
                        ));
                    }
                },
            };

            if value.type_name() != parameter.declared_type.name {
                return Err(Diagnostic::new(
                    value_span,
                    format!(
                        "Argument '{}' of '{}' expects {}, got {}",
                        parameter.name.text,
                        definition.name.text,
                        parameter.declared_type.name,
                        value.type_name()
                    ),
                ));
            }

            frame.insert(
                parameter.name.text.clone(),
                Variable {
                    declared_type: parameter.declared_type.name.clone(),
                    value,
                },
            );
        }

        self.scopes.push(frame);

        let returned = self.execute_block(&definition.body)?;

        let value = returned.ok_or_else(|| {
            Diagnostic::new(
                definition.name.span,
                format!(
                    "Function '{}' finished without return",
                    definition.name.text
                ),
            )
        })?;

        if value.type_name() != definition.return_type.name {
            return Err(Diagnostic::new(
                definition.return_type.span,
                format!(
                    "Function '{}' declares return type {}, got {}",
                    definition.name.text,
                    definition.return_type.name,
                    value.type_name()
                ),
            ));
        }

        Ok(value)
    }
}

// ============================================================
// Entry point and commands
// ============================================================

enum Failure {
    Syntax(Diagnostic),
    Static(Vec<Diagnostic>),
    Runtime(Diagnostic),
}

fn parse_source(source: &str) -> Result<Vec<Stmt>, Failure> {
    let tokens = lex(source).map_err(Failure::Syntax)?;

    Parser::new(tokens)
        .parse_program()
        .map_err(Failure::Syntax)
}

fn check_source(source: &str) -> Result<Vec<Stmt>, Failure> {
    let program = parse_source(source)?;
    let errors = Checker::new().check_program(&program);

    if errors.is_empty() {
        Ok(program)
    } else {
        Err(Failure::Static(errors))
    }
}

fn execute(source: &str) -> Result<(), Failure> {
    let program = check_source(source)?;

    Interpreter::new()
        .run(&program)
        .map_err(Failure::Runtime)
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        eprintln!("Usage:");
        eprintln!("  epholang tokens <file.epho>");
        eprintln!("  epholang parse <file.epho>");
        eprintln!("  epholang check <file.epho>");
        eprintln!("  epholang run <file.epho>");
        std::process::exit(1);
    }

    let command = &args[1];
    let path = &args[2];

    let source = match fs::read_to_string(path) {
        Ok(source) => source,

        Err(e) => {
            eprintln!("Cannot read '{}': {}", path, e);
            std::process::exit(1);
        }
    };

    let result = match command.as_str() {
        "tokens" => lex(&source)
            .map_err(Failure::Syntax)
            .map(|tokens| {
                for token in tokens {
                    println!("{:?}", token);
                }
            }),

        "parse" => parse_source(&source).map(|program| {
            for stmt in program {
                println!("{:#?}", stmt);
            }
        }),

        "check" => check_source(&source)
            .map(|_| println!("OK: no static errors")),

        "run" => execute(&source),

        _ => {
            eprintln!("Unknown command '{}'", command);
            std::process::exit(1);
        }
    };

    if let Err(failure) = result {
        if OUTPUT_LINE_OPEN.load(Ordering::Relaxed) {
            println!();
        }

        let _ = io::stdout().flush();

        match failure {
            Failure::Syntax(diagnostic) => {
                print_diagnostic(
                    "Syntax error",
                    path,
                    &source,
                    &diagnostic,
                );
            }

            Failure::Static(diagnostics) => {
                for diagnostic in diagnostics {
                    print_diagnostic(
                        "Static error",
                        path,
                        &source,
                        &diagnostic,
                    );
                }
            }

            Failure::Runtime(diagnostic) => {
                print_diagnostic(
                    "Runtime error",
                    path,
                    &source,
                    &diagnostic,
                );
            }
        }

        std::process::exit(1);
    }
}