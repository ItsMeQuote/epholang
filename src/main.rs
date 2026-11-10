use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
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

fn print_diagnostic(prefix: &str, path: &str, source: &str, diagnostic: &Diagnostic) {
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
    eprintln!("{:>width$} | {}", span.line, displayed_line, width = width);
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
    Integer(i64),
    Float(f64),
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

fn pair(chars: &[SourceChar], i: usize, first: char, second: char) -> bool {
    chars.get(i).is_some_and(|c| c.ch == first) && chars.get(i + 1).is_some_and(|c| c.ch == second)
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
        Err(Diagnostic::new(opening_span, "Unclosed interpolation"))
    }
}

fn lex_chars(chars: &[SourceChar], eof_span: Span) -> EResult<Vec<Token>> {
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
                return Err(Diagnostic::new(opening_span, "Unclosed comment"));
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
                    return Err(Diagnostic::new(chars[i].span, "Newline inside string"));
                }

                if ch == '\\' && i + 1 < chars.len() {
                    if chars[i + 1].ch == '\n' {
                        return Err(Diagnostic::new(chars[i + 1].span, "Newline inside string"));
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

                    let (code, code_eof) = scan_interpolation(chars, &mut i, opening_span)?;

                    if code.iter().all(|c| c.ch.is_whitespace()) {
                        return Err(Diagnostic::new(opening_span, "Empty interpolation"));
                    }

                    let code_tokens = lex_chars(&code, code_eof)?;

                    if !text.is_empty() {
                        parts.push(StringPart::Text(std::mem::take(&mut text)));
                    }

                    parts.push(StringPart::Code(code_tokens));
                    continue;
                }

                text.push(ch);
                i += 1;
            }

            if !closed {
                return Err(Diagnostic::new(chars[start].span, "Unclosed string"));
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
            || (c == '.' && chars.get(i + 1).is_some_and(|c| c.ch.is_ascii_digit()))
        {
            while i < chars.len() && chars[i].ch.is_ascii_digit() {
                i += 1;
            }

            if i < chars.len()
                && chars[i].ch == '.'
                && chars.get(i + 1).is_some_and(|c| c.ch.is_ascii_digit())
            {
                i += 1;

                while i < chars.len() && chars[i].ch.is_ascii_digit() {
                    i += 1;
                }
            }

            let span = range_span(chars, start, i);
            let text: String = chars[start..i].iter().map(|c| c.ch).collect();

            let kind = if text.contains('.') {
                let value = text
                    .parse::<f64>()
                    .map_err(|_| Diagnostic::new(span, "Invalid floating-point number"))?;
                if !value.is_finite() {
                    return Err(Diagnostic::new(span, "Floating-point number out of range"));
                }
                TokenKind::Float(value)
            } else {
                let value = text
                    .parse::<i64>()
                    .map_err(|_| Diagnostic::new(span, "Integer literal out of i64 range"))?;
                TokenKind::Integer(value)
            };

            tokens.push(Token { kind, span });

            continue;
        }

        if c.is_ascii_alphabetic() || c == '_' {
            while i < chars.len() && (chars[i].ch.is_ascii_alphanumeric() || chars[i].ch == '_') {
                i += 1;
            }

            let name: String = chars[start..i].iter().map(|c| c.ch).collect();

            tokens.push(Token {
                kind: TokenKind::Identifier(name),
                span: range_span(chars, start, i),
            });

            continue;
        }

        let two: String = chars.iter().skip(i).take(2).map(|c| c.ch).collect();

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

        if "+-*/%^(){}[],;=<>!:".contains(c) {
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
    Int(i64),
    Flt(f64),
    Str(String),
    Bool(bool),
    Null,
    Type(String),
    List(String, Vec<Value>),
}

impl Value {
    fn type_name(&self) -> String {
        match self {
            Value::Int(_) => "int".into(),
            Value::Flt(_) => "flt".into(),
            Value::Str(_) => "str".into(),
            Value::Bool(_) => "bool".into(),
            Value::Null => "none".into(),
            Value::Type(_) => "type".into(),
            Value::List(inner, _) => format!("ls<{}>", inner),
        }
    }

    fn display(&self) -> String {
        match self {
            Value::Int(n) => n.to_string(),
            Value::Flt(n) => n.to_string(),

            Value::Str(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".into(),
            Value::Type(t) => t.clone(),

            Value::List(_, elements) => {
                let elements: Vec<String> = elements.iter().map(Value::display).collect();

                format!("list{{{}}}", elements.join(", "))
            }
        }
    }
}

// Only simple types and one-level lists are public in 0.2.0.
// A structural type model is prepared for a future union-type release.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Type {
    Int,
    Flt,
    Str,
    Bool,
    None,
    Meta,
    List(Box<Type>),
    Union(Vec<Type>),
}

impl Type {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "int" => Some(Self::Int),
            "flt" => Some(Self::Flt),
            "str" => Some(Self::Str),
            "bool" => Some(Self::Bool),
            "none" => Some(Self::None),
            "type" => Some(Self::Meta),
            _ => {
                let inner = name.strip_prefix("ls<")?.strip_suffix('>')?;
                match Self::from_name(inner)? {
                    ty @ (Self::Int | Self::Flt | Self::Str | Self::Bool | Self::None) => {
                        Some(Self::List(Box::new(ty)))
                    }
                    _ => None,
                }
            }
        }
    }
}

const ELEMENT_TYPES: [&str; 5] = ["int", "flt", "str", "bool", "none"];

fn valid_type(name: &str) -> bool {
    Type::from_name(name).is_some()
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
        "int",
        "flt",
        "str",
        "bool",
        "none",
        "type",
        "ls",
        "list",
        "Length",
        "Output",
        "Input",
        "Includes",
        "Insert",
        "Remove",
        "TypeOf",
        "true",
        "false",
        "null",
        "loop",
        "break",
        "skip",
        "if",
        "otherwise",
        "return",
        "func",
        "function",
        "in",
        "import",
        "as",
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
    ModuleCall {
        module: Name,
        function: Name,
        args: Vec<Expr>,
    },
    Index(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone)]
struct Stmt {
    kind: Statement,
    span: Span,
}

#[derive(Debug, Clone)]
enum Statement {
    Import {
        path: String,
        alias: Name,
    },

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

    Break,
    Skip,

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
        &self.tokens[(self.pos + offset).min(self.tokens.len() - 1)].kind
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
        while matches!(&self.current().kind, TokenKind::Newline) || self.symbol(";") {
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

            _ => Err(Diagnostic::new(token.span, "Expected an identifier")),
        }
    }

    fn parse_element_type(&mut self) -> EResult<String> {
        self.expect_symbol("<")?;

        let inner = self.expect_name()?;

        if !ELEMENT_TYPES.contains(&inner.text.as_str()) {
            return Err(Diagnostic::new(inner.span, "Invalid list element type"));
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

            if !matches!(&self.current().kind, TokenKind::Eof | TokenKind::Newline)
                && !self.symbol(";")
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
                return Err(Diagnostic::new(self.current().span, "Unclosed block"));
            }

            statements.push(self.parse_statement()?);

            if !self.symbol("}")
                && !matches!(&self.current().kind, TokenKind::Newline)
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

    fn parse_function(&mut self) -> EResult<Statement> {
        self.advance(); // function
        let name = self.expect_name()?;
        check_reserved(&name)?;
        self.expect_symbol("(")?;
        self.skip_newlines();

        let mut parameters: Vec<Parameter> = Vec::new();
        if !self.symbol(")") {
            loop {
                let parameter_name = self.expect_name()?;
                check_reserved(&parameter_name)?;
                let parameter_type = self.parse_type()?;
                if !valid_type(&parameter_type.name)
                    || matches!(parameter_type.name.as_str(), "none" | "type")
                {
                    return Err(Diagnostic::new(
                        parameter_type.span,
                        "Invalid parameter type",
                    ));
                }
                if self.symbol("=") {
                    return Err(Diagnostic::new(
                        self.current().span,
                        "Default parameters are no longer supported",
                    ));
                }
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
                });
                self.skip_newlines();
                if self.symbol(",") {
                    self.advance();
                    self.skip_newlines();
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
            return Err(Diagnostic::new(return_type.span, "Invalid return type"));
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

    fn parse_loop(&mut self) -> EResult<Statement> {
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
        if self.identifier_is("import") {
            self.advance(); // 'import'

            let path_token = self.advance();
            let path = match path_token.kind {
                TokenKind::String(s) => s,
                _ => {
                    return Err(Diagnostic::new(
                        path_token.span,
                        "Expected a string literal for module path",
                    ));
                }
            };

            if !self.identifier_is("as") {
                return Err(Diagnostic::new(
                    self.current().span,
                    "import requires an alias: import \"...\" as name",
                ));
            }
            self.advance(); // 'as'

            let alias = self.expect_name()?;
            check_reserved(&alias)?;

            return Ok(Statement::Import { path, alias });
        }

        if self.identifier_is("function") {
            return self.parse_function();
        }

        if self.identifier_is("if") {
            return self.parse_if();
        }

        if self.identifier_is("loop") {
            return self.parse_loop();
        }

        if self.identifier_is("otherwise") {
            return Err(Diagnostic::new(
                self.current().span,
                "'otherwise' without a preceding 'if'",
            ));
        }

        if self.identifier_is("break") {
            self.advance();
            return Ok(Statement::Break);
        }

        if self.identifier_is("skip") {
            self.advance();
            return Ok(Statement::Skip);
        }

        if self.identifier_is("return") {
            self.advance();

            let starts_with_type = matches!(
                &self.current().kind,
                TokenKind::Identifier(s)
                    if matches!(
                        s.as_str(),
                        "int" | "flt" | "str" | "bool" | "none" | "type" | "ls"
                    )
            );

            if !starts_with_type {
                return Err(Diagnostic::new(
                    self.current().span,
                    "Expected a return type after 'return' (e.g. 'return int x')",
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
            let is_declaration = if let TokenKind::Identifier(t) = self.look(1) {
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
                    return Err(Diagnostic::new(declared_type.span, "Invalid variable type"));
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

                _ => Err(Diagnostic::new(target_span, "Invalid assignment target")),
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
                    if Self::precedence(s) >= min_prec && Self::precedence(s) > 0 =>
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
                ExprKind::Binary(Box::new(left), op, Box::new(right)),
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
            TokenKind::Integer(n) => ExprKind::Literal(Value::Int(n)),
            TokenKind::Float(n) => ExprKind::Literal(Value::Flt(n)),
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

                            if !matches!(&sub.current().kind, TokenKind::Eof) {
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
                self.skip_newlines();

                let mut elements = Vec::new();

                if !self.symbol("}") {
                    loop {
                        elements.push(self.parse_expression(0)?);
                        self.skip_newlines();

                        if self.symbol(",") {
                            self.advance();
                            self.skip_newlines();

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
                    "int" | "flt" | "str" | "bool" | "none" | "type"
                ) =>
            {
                ExprKind::Literal(Value::Type(name))
            }

            TokenKind::Identifier(name) if name == "ls" => {
                let inner = self.parse_element_type()?;
                ExprKind::Literal(Value::Type(format!("ls<{}>", inner)))
            }

            TokenKind::Identifier(name) if name == "true" => ExprKind::Literal(Value::Bool(true)),

            TokenKind::Identifier(name) if name == "false" => ExprKind::Literal(Value::Bool(false)),

            TokenKind::Identifier(name) if name == "null" => ExprKind::Literal(Value::Null),

            TokenKind::Identifier(name) => {
                if self.symbol(":") {
                    let colon_span = self.current().span;

                    if span.line != colon_span.line
                        || span.column + span.length != colon_span.column
                    {
                        return Err(Diagnostic::new(
                            colon_span,
                            "Module calls must not contain spaces around ':'",
                        ));
                    }

                    self.advance(); // ':'
                    let function = self.expect_name()?;

                    if function.span.line != colon_span.line
                        || colon_span.column + 1 != function.span.column
                    {
                        return Err(Diagnostic::new(
                            function.span,
                            "Module calls must not contain spaces around ':'",
                        ));
                    }

                    self.expect_symbol("(")?;
                    self.skip_newlines();

                    let mut args = Vec::new();

                    if !self.symbol(")") {
                        loop {
                            args.push(self.parse_expression(0)?);
                            self.skip_newlines();

                            if self.symbol(",") {
                                self.advance();
                                self.skip_newlines();

                                if self.symbol(")") {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    }

                    self.expect_symbol(")")?;
                    ExprKind::ModuleCall {
                        module: Name { text: name, span },
                        function,
                        args,
                    }
                } else if self.symbol("(") {
                    self.advance();
                    self.skip_newlines();

                    let mut args = Vec::new();

                    if !self.symbol(")") {
                        loop {
                            args.push(self.parse_expression(0)?);
                            self.skip_newlines();

                            if self.symbol(",") {
                                self.advance();
                                self.skip_newlines();

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
    params: Vec<String>,
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
    seen_non_import: bool,
    imported_paths: Vec<String>,
    loop_depth: usize,
}

impl Checker {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            function_order: Vec::new(),
            current_function: None,
            errors: Vec::new(),
            seen_non_import: false,
            imported_paths: Vec::new(),
            loop_depth: 0,
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

        self.errors.sort_by_key(|e| (e.span.line, e.span.column));

        self.errors
    }

    fn report_unused(&mut self, scope: HashMap<String, VarInfo>) {
        let mut unused: Vec<_> = scope.into_iter().filter(|(_, info)| !info.used).collect();

        unused.sort_by_key(|(_, info)| (info.span.line, info.span.column));

        for (name, info) in unused {
            let message = match (info.kind, &self.current_function) {
                ("parameter", Some((function, _))) => {
                    format!("Unused parameter '{}' in function '{}'", name, function)
                }

                (kind, _) => format!("Unused {} '{}'", kind, name),
            };

            self.err(info.span, message);
        }
    }

    fn declare(&mut self, name: &Name, ty: Option<String>, kind: &'static str) {
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
                format!("'{}' is already declared in an outer scope", name.text),
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

        self.err(name.span, format!("Undefined variable '{}'", name.text));

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
                if let Some((function, _)) = self.current_function.clone() {
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
        if let Statement::Import { path, alias } = &stmt.kind {
            if self.seen_non_import {
                self.err(
                    stmt.span,
                    "import statements must appear at the beginning of the file",
                );
            }
            if !matches!(path.as_str(), "strings" | "math" | "types") {
                self.err(
                    stmt.span,
                    format!(
                        "Unknown module '{}' (only strings, math and types are supported)",
                        path
                    ),
                );
                return;
            }
            if self.imported_paths.contains(path) {
                self.err(stmt.span, format!("Module '{}' is already imported", path));
                return;
            }
            self.declare(alias, Some(path.clone()), "module");
            self.imported_paths.push(path.clone());
            return;
        }

        self.seen_non_import = true;

        match &stmt.kind {
            Statement::Import { .. } => unreachable!(),

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

                self.declare(name, Some(declared_type.name.clone()), "variable");
            }

            Statement::Assign { name, value } => {
                let value_type = self.expr_type(value);
                let target_type = self.target_type(name);

                if let (Some(value_type), Some(target_type)) = (value_type, target_type) {
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

                        None => self.err(name.span, format!("Cannot index into {}", target_type)),
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
                    None => self.err(stmt.span, "return is only allowed inside a function"),

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

            Statement::Break | Statement::Skip => {
                if self.loop_depth == 0 {
                    let keyword = if matches!(&stmt.kind, Statement::Break) {
                        "break"
                    } else {
                        "skip"
                    };
                    self.err(
                        stmt.span,
                        format!("'{}' is only allowed inside a loop", keyword),
                    );
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
                self.expect_bool(condition, "loop");
                self.loop_depth += 1;
                self.check_scoped_block(body);
                self.loop_depth -= 1;
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
                                format!("loop .. in .. requires a list, got {}", ty),
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
                self.loop_depth += 1;
                self.check_block(body);
                self.loop_depth -= 1;

                let scope = self.scopes.pop().unwrap();
                self.report_unused(scope);
            }
        }
    }

    fn check_function(&mut self, definition: &FunctionDef) {
        let name = &definition.name;

        if self.current_function.is_some() || self.scopes.len() != 1 {
            self.err(name.span, "Functions can only be declared at the top level");
            return;
        }

        if self.functions.contains_key(&name.text) || self.scopes[0].contains_key(&name.text) {
            self.err(name.span, format!("'{}' is already declared", name.text));
            return;
        }

        self.functions.insert(
            name.text.clone(),
            FuncInfo {
                params: definition
                    .parameters
                    .iter()
                    .map(|p| p.declared_type.name.clone())
                    .collect(),
                return_type: definition.return_type.name.clone(),
                used: false,
                span: name.span,
            },
        );

        self.function_order.push(name.text.clone());

        self.current_function = Some((name.text.clone(), definition.return_type.name.clone()));

        let enclosing_loop_depth = self.loop_depth;
        self.loop_depth = 0;

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
        self.loop_depth = enclosing_loop_depth;
    }

    fn check_index_expr(&mut self, index: &Expr) {
        match &index.kind {
            ExprKind::Literal(Value::Int(n)) if *n <= 0 => {
                self.err(index.span, "Index must be positive and non-zero");
            }
            ExprKind::Unary(op, inner)
                if op == "-" && matches!(&inner.kind, ExprKind::Literal(Value::Int(_))) =>
            {
                self.err(index.span, "Index must be positive and non-zero");
            }
            _ => {}
        }
        if let Some(ty) = self.expr_type(index) {
            if ty != "int" {
                self.err(index.span, format!("Index must be int, got {}", ty));
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
                let mut found = None;

                for scope in self.scopes.iter_mut().rev() {
                    if let Some(info) = scope.get_mut(name) {
                        info.used = true;
                        found = Some((info.kind, info.ty.clone()));
                        break;
                    }
                }

                match found {
                    Some(("module", _)) => {
                        self.err(expr.span, format!("'{}' is a module, not a variable", name));
                        None
                    }

                    Some((_, ty)) => ty,

                    None => {
                        self.err(expr.span, format!("Undefined variable '{}'", name));
                        None
                    }
                }
            }

            ExprKind::Unary(op, operand) => {
                let ty = self.expr_type(operand)?;

                match (op.as_str(), ty.as_str()) {
                    ("-", "int" | "flt") => Some(ty),
                    ("!", "bool") => Some("bool".into()),
                    ("-", _) => {
                        self.err(
                            expr.span,
                            format!("Unary '-' requires int or flt, got {}", ty),
                        );
                        None
                    }
                    _ => {
                        self.err(expr.span, format!("Unary '!' requires bool, got {}", ty));
                        None
                    }
                }
            }

            ExprKind::Binary(left, op, right) => {
                let a = self.expr_type(left)?;
                let b = self.expr_type(right)?;
                let numeric = |t: &str| t == "int" || t == "flt";
                let result = match op.as_str() {
                    "+" | "-" | "*" | "%" | "^" if a == b && numeric(&a) => a.as_str(),
                    "/" if a == b && numeric(&a) => "flt",
                    "<" | ">" if a == b && numeric(&a) => "bool",
                    "&&" | "||" if a == "bool" && b == "bool" => "bool",
                    "==" | "!="
                        if a == b
                            && matches!(a.as_str(), "int" | "flt" | "str" | "bool" | "type") =>
                    {
                        "bool"
                    }
                    "==" | "!=" if list_inner(&a).is_some() && a == b => "bool",
                    _ => {
                        self.err(
                            expr.span,
                            format!("Operator '{}' cannot be applied to {} and {}", op, a, b),
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

                if array_type == "str" {
                    Some("str".into())
                } else {
                    match list_inner(&array_type) {
                        Some(inner) => Some(inner.to_string()),

                        None => {
                            self.err(array.span, format!("Cannot index into {}", array_type));
                            None
                        }
                    }
                }
            }

            ExprKind::Call(name, args) => self.call_type(name, args, expr.span),

            ExprKind::ModuleCall {
                module,
                function,
                args,
            } => {
                let mut found_path = None;

                for scope in self.scopes.iter_mut().rev() {
                    if let Some(info) = scope.get_mut(&module.text) {
                        info.used = true;

                        if info.kind == "module" {
                            found_path = info.ty.clone();
                        } else {
                            self.err(module.span, format!("'{}' is not a module", module.text));
                            return None;
                        }
                        break;
                    }
                }

                let path = match found_path {
                    Some(p) => p,
                    None => {
                        self.err(module.span, format!("Undefined module '{}'", module.text));
                        return None;
                    }
                };

                match path.as_str() {
                    "strings" => self.check_strings_module(module, function, args, expr.span),
                    "math" => self.check_math_module(module, function, args, expr.span),
                    "types" => self.check_types_module(module, function, args, expr.span),
                    _ => None,
                }
            }
        }
    }

    fn expect_count(&mut self, name: &str, args: &[Expr], count: usize, span: Span) -> bool {
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

    fn expect_count_mod(
        &mut self,
        full_name: &str,
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
                "{} expects {} argument(s), got {}",
                full_name,
                count,
                args.len()
            ),
        );

        for arg in args {
            self.expr_type(arg);
        }

        false
    }

    fn check_strings_module(
        &mut self,
        module: &Name,
        function: &Name,
        args: &[Expr],
        span: Span,
    ) -> Option<String> {
        let full_name = format!(
            "{}:{}() from module \"strings\"",
            module.text, function.text
        );

        let check_str = |checker: &mut Self, arg: &Expr| {
            if let Some(ty) = checker.expr_type(arg) {
                if ty != "str" {
                    checker.err(
                        arg.span,
                        format!("Argument to {} expects str, got {}", function.text, ty),
                    );
                }
            }
        };

        let check_str_not_empty = |checker: &mut Self, arg: &Expr, msg: &str| {
            if let Some(ty) = checker.expr_type(arg) {
                if ty != "str" {
                    checker.err(
                        arg.span,
                        format!("Argument to {} expects str, got {}", function.text, ty),
                    );
                } else if let ExprKind::Literal(Value::Str(text)) = &arg.kind {
                    if text.is_empty() {
                        checker.err(arg.span, msg);
                    }
                }
            }
        };

        match function.text.as_str() {
            "Trim" => {
                if self.expect_count_mod(&full_name, args, 2, span) {
                    check_str(self, &args[0]);
                    check_str_not_empty(
                        self,
                        &args[1],
                        "Trim() cannot use an empty string as a pattern",
                    );
                }
                Some("str".into())
            }

            "Replace" => {
                if self.expect_count_mod(&full_name, args, 3, span) {
                    check_str(self, &args[0]);
                    check_str_not_empty(
                        self,
                        &args[1],
                        "Replace() cannot use an empty string as a pattern",
                    );
                    check_str(self, &args[2]);
                }
                Some("str".into())
            }

            "Split" => {
                if self.expect_count_mod(&full_name, args, 2, span) {
                    check_str(self, &args[0]);
                    check_str_not_empty(
                        self,
                        &args[1],
                        "Split() cannot use an empty string as a separator",
                    );
                }
                Some("ls<str>".into())
            }

            "StartsWith" | "EndsWith" => {
                if self.expect_count_mod(&full_name, args, 2, span) {
                    check_str(self, &args[0]);
                    check_str_not_empty(
                        self,
                        &args[1],
                        &format!("{}() cannot search for an empty string", function.text),
                    );
                }
                Some("bool".into())
            }

            "ToLowerCase" | "ToUpperCase" | "Capitalize" => {
                if self.expect_count_mod(&full_name, args, 1, span) {
                    check_str(self, &args[0]);
                }
                Some("str".into())
            }

            "Repeat" => {
                if self.expect_count_mod(&full_name, args, 2, span) {
                    check_str(self, &args[0]);

                    if let Some(ty) = self.expr_type(&args[1]) {
                        if ty != "int" {
                            self.err(
                                args[1].span,
                                format!("Argument 2 of {} expects int, got {}", function.text, ty),
                            );
                        } else if let ExprKind::Literal(Value::Int(n)) = &args[1].kind {
                            if *n < 0 {
                                self.err(
                                    args[1].span,
                                    "Repeat() count must be a non-negative integer",
                                );
                            }
                        } else if let ExprKind::Unary(op, inner) = &args[1].kind {
                            if op == "-" && matches!(&inner.kind, ExprKind::Literal(Value::Int(_)))
                            {
                                self.err(
                                    args[1].span,
                                    "Repeat() count must be a non-negative integer",
                                );
                            }
                        }
                    }
                }
                Some("str".into())
            }

            _ => {
                self.err(
                    function.span,
                    format!("Unknown function '{}' in module 'strings'", function.text),
                );
                None
            }
        }
    }

    fn check_math_module(
        &mut self,
        module: &Name,
        function: &Name,
        args: &[Expr],
        span: Span,
    ) -> Option<String> {
        let name = function.text.as_str();
        let count = match name {
            "AbsoluteValue" | "SquareRoot" | "Floor" | "Ceil" | "Round" | "Sum" | "Average" => 1,
            "Minimum" | "Maximum" | "RandomInterval" => 2,
            "Clamp" => 3,
            _ => {
                self.err(
                    function.span,
                    format!("Unknown function '{}' in module 'math'", name),
                );
                for arg in args {
                    self.expr_type(arg);
                }
                return None;
            }
        };
        let full_name = format!("{}:{}()", module.text, name);
        if !self.expect_count_mod(&full_name, args, count, span) {
            return None;
        }
        let types: Vec<Option<String>> = args.iter().map(|a| self.expr_type(a)).collect();
        let first = types[0].as_deref();
        let numeric = |t: &str| t == "int" || t == "flt";

        match name {
            "RandomInterval" => {
                for (arg, ty) in args.iter().zip(&types) {
                    if let Some(ty) = ty {
                        if ty != "int" {
                            self.err(
                                arg.span,
                                format!("RandomInterval() expects int, got {}", ty),
                            );
                        }
                    }
                }
                if let (ExprKind::Literal(Value::Int(a)), ExprKind::Literal(Value::Int(b))) =
                    (&args[0].kind, &args[1].kind)
                {
                    if a > b {
                        self.err(span, "RandomInterval() minimum must not exceed maximum");
                    }
                }
                Some("int".into())
            }
            "SquareRoot" | "Floor" | "Ceil" | "Round" => {
                if let Some(ty) = first {
                    if ty != "flt" {
                        self.err(args[0].span, format!("{}() expects flt, got {}", name, ty));
                    }
                }
                Some("flt".into())
            }
            "AbsoluteValue" | "Minimum" | "Maximum" | "Clamp" => {
                if let Some(ty) = first {
                    if !numeric(ty) {
                        self.err(
                            args[0].span,
                            format!("{}() expects int or flt, got {}", name, ty),
                        );
                    }
                }
                for (arg, ty) in args.iter().zip(&types).skip(1) {
                    if let Some(ty) = ty {
                        if let Some(first) = first {
                            if ty != first || !numeric(first) {
                                self.err(
                                    arg.span,
                                    format!(
                                        "{}() expects matching int or flt arguments, got {} and {}",
                                        name, first, ty
                                    ),
                                );
                            }
                        } else if !numeric(ty) {
                            self.err(
                                arg.span,
                                format!("{}() expects int or flt, got {}", name, ty),
                            );
                        }
                    }
                }
                first.filter(|t| numeric(t)).map(str::to_string)
            }
            "Sum" | "Average" => {
                if let Some(ty) = first {
                    if ty != "ls<int>" && ty != "ls<flt>" {
                        self.err(
                            args[0].span,
                            format!("{}() expects ls<int> or ls<flt>, got {}", name, ty),
                        );
                    }
                }
                if name == "Average" {
                    Some("flt".into())
                } else {
                    first.and_then(|t| list_inner(t)).map(str::to_string)
                }
            }
            _ => unreachable!(),
        }
    }

    fn check_types_module(
        &mut self,
        module: &Name,
        function: &Name,
        args: &[Expr],
        span: Span,
    ) -> Option<String> {
        let name = function.text.as_str();
        if name != "Convert" && name != "IsConvertible" {
            self.err(
                function.span,
                format!("Unknown function '{}' in module 'types'", name),
            );
            for arg in args {
                self.expr_type(arg);
            }
            return None;
        }
        if !self.expect_count_mod(&format!("{}:{}()", module.text, name), args, 2, span) {
            return None;
        }
        let source = self.expr_type(&args[0]);
        self.expr_type(&args[1]);
        let target = match &args[1].kind {
            ExprKind::Literal(Value::Type(target)) if simple_convert_target(target) => target,
            _ => {
                self.err(
                    args[1].span,
                    "The conversion target must be a literal int, flt, str or bool",
                );
                return None;
            }
        };
        if name == "Convert" {
            if let Some(source) = source {
                if !supported_conversion(&source, target) {
                    self.err(
                        args[0].span,
                        format!("Convert() cannot convert {} to {}", source, target),
                    );
                }
            }
            Some(target.clone())
        } else {
            Some("bool".into())
        }
    }

    fn call_type(&mut self, name: &str, args: &[Expr], span: Span) -> Option<String> {
        match name {
            "Output" | "TypeOf" => {
                if self.expect_count(name, args, 1, span) {
                    self.expr_type(&args[0]);
                }

                Some(if name == "Output" { "none" } else { "type" }.into())
            }

            "Input" => {
                if self.expect_count(name, args, 1, span) {
                    if let Some(ty) = self.expr_type(&args[0]) {
                        if ty != "str" {
                            self.err(
                                args[0].span,
                                format!("Input() requires a str prompt, got {}", ty),
                            );
                        }
                    }
                }

                Some("str".into())
            }

            "Includes" => {
                if self.expect_count(name, args, 2, span) {
                    let haystack = self.expr_type(&args[0]);
                    let needle = self.expr_type(&args[1]);

                    if let (Some(haystack), Some(needle)) = (haystack, needle) {
                        match list_inner(&haystack) {
                            Some(inner) => {
                                if needle != inner {
                                    self.err(
                                        args[1].span,
                                        format!(
                                            "Includes() cannot search for {} inside {}",
                                            needle, haystack
                                        ),
                                    );
                                }
                            }

                            None if haystack == "str" => {
                                if needle != "str" {
                                    self.err(
                                        args[1].span,
                                        format!(
                                            "Includes() requires a str to search inside a str, got {}",
                                            needle
                                        ),
                                    );
                                } else if let ExprKind::Literal(Value::Str(pattern)) = &args[1].kind
                                {
                                    // Only substring search forbids the empty pattern.
                                    // Membership in ls<str> must allow an empty string.
                                    if pattern.is_empty() {
                                        self.err(
                                            args[1].span,
                                            "Includes() cannot search for an empty string",
                                        );
                                    }
                                }
                            }

                            None => self.err(
                                args[0].span,
                                format!("Includes() requires a string or list, got {}", haystack),
                            ),
                        }
                    }
                }

                Some("bool".into())
            }

            "Length" => {
                if self.expect_count(name, args, 1, span) {
                    if let Some(ty) = self.expr_type(&args[0]) {
                        if ty != "str" && list_inner(&ty).is_none() {
                            self.err(
                                args[0].span,
                                format!("Length() requires a string or list, got {}", ty),
                            );
                        }
                    }
                }

                Some("int".into())
            }

            "Insert" | "Remove" => {
                let count = if name == "Insert" { 3 } else { 2 };

                if !self.expect_count(name, args, count, span) {
                    return None;
                }

                let list_type = self.expr_type(&args[0]);
                self.check_index_expr(&args[1]);

                let value_type = if name == "Insert" {
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
                            format!("Cannot insert {} into {}", ty, list_type),
                        );
                    }
                }

                Some(list_type)
            }

            _ => {
                let arg_types: Vec<Option<String>> =
                    args.iter().map(|arg| self.expr_type(arg)).collect();

                let recursive = matches!(
                    &self.current_function,
                    Some((current, _)) if current == name
                );

                let (params, return_type) = match self.functions.get_mut(name) {
                    Some(info) => {
                        if !recursive {
                            info.used = true;
                        }

                        (info.params.clone(), info.return_type.clone())
                    }

                    None => {
                        self.err(span, format!("Unknown function '{}'", name));
                        return None;
                    }
                };

                if args.len() != params.len() {
                    self.err(
                        span,
                        format!(
                            "Function '{}' expects {} argument(s), got {}",
                            name,
                            params.len(),
                            args.len()
                        ),
                    );
                }

                for (index, ((arg, ty), expected)) in args
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

enum ExecFlow {
    Continue,
    Return(Value),
    Break,
    Skip,
}

struct Interpreter {
    scopes: Vec<HashMap<String, Variable>>,
    functions: HashMap<String, FunctionDef>,
    return_types: Vec<String>,
}

fn resolve_index(n: i64, length: usize, is_insert: bool, span: Span) -> EResult<usize> {
    if n <= 0 {
        return Err(Diagnostic::new(span, "Index must be positive and non-zero"));
    }
    let maximum = if is_insert {
        length
            .checked_add(1)
            .ok_or_else(|| Diagnostic::new(span, "List size out of range"))?
    } else {
        length
    };
    let index = usize::try_from(n).map_err(|_| Diagnostic::new(span, "Index out of bounds"))?;
    if index > maximum {
        return Err(Diagnostic::new(
            span,
            format!("Index {} out of bounds (max {})", n, maximum),
        ));
    }
    Ok(index - 1)
}

// Only exact quotients representable by binary64 are permitted for int / int.
// Cancelling common factors yields a dyadic rational when denominator is a power of 2.
fn exact_integer_quotient(a: i64, b: i64) -> Option<f64> {
    if b == 0 {
        return None;
    }
    if a == 0 {
        return Some(0.0);
    }
    let mut numerator = a.unsigned_abs();
    let mut denominator = b.unsigned_abs();
    let mut x = numerator;
    let mut y = denominator;
    while y != 0 {
        let rem = x % y;
        x = y;
        y = rem;
    }
    numerator /= x;
    denominator /= x;
    if !denominator.is_power_of_two() {
        return None;
    }
    let odd = numerator >> numerator.trailing_zeros();
    if (u64::BITS - odd.leading_zeros()) > f64::MANTISSA_DIGITS {
        return None;
    }
    // Use reduced operands: converting unreduced i64 values first can
    // round them even when their quotient is exactly representable.
    let magnitude = (numerator as f64) / (denominator as f64);
    let result = if (a < 0) != (b < 0) {
        -magnitude
    } else {
        magnitude
    };
    result.is_finite().then_some(result)
}

// Type conversion is explicit; conversion to/from lists and none is unsupported.
fn simple_convert_target(target: &str) -> bool {
    matches!(target, "int" | "flt" | "str" | "bool")
}

fn supported_conversion(source: &str, target: &str) -> bool {
    if !simple_convert_target(target) {
        return false;
    }
    match (source, target) {
        ("int" | "flt" | "str" | "bool", t) if t == source => true,
        ("int", "flt" | "str") => true,
        ("flt", "int" | "str") => true,
        ("str", "int" | "flt" | "bool") => true,
        ("bool", "str") => true,
        _ => false,
    }
}

fn convert_value(value: &Value, target: &str) -> Option<Value> {
    if !supported_conversion(&value.type_name(), target) {
        return None;
    }
    if value.type_name() == target {
        return Some(value.clone());
    }
    match (value, target) {
        (Value::Int(n), "flt") => {
            let float = *n as f64;
            ((float as i128) == (*n as i128)).then_some(Value::Flt(float))
        }
        (Value::Int(n), "str") => Some(Value::Str(n.to_string())),
        (Value::Flt(n), "int")
            if n.is_finite()
                && n.fract() == 0.0
                && *n >= i64::MIN as f64
                && *n < 9_223_372_036_854_775_808.0 =>
        {
            Some(Value::Int(*n as i64))
        }
        (Value::Flt(n), "str") => Some(Value::Str(n.to_string())),
        (Value::Str(s), "int") => s.parse::<i64>().ok().map(Value::Int),
        (Value::Str(s), "flt") => s
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .map(Value::Flt),
        (Value::Str(s), "bool") => match s.as_str() {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        },
        (Value::Bool(n), "str") => Some(Value::Str(n.to_string())),
        _ => None,
    }
}

// An integer average follows Stage 1 exact int/int division semantics.
// The reduced fraction must be exactly representable in f64.
fn exact_integer_average(sum: i128, count: usize) -> Option<f64> {
    if count == 0 {
        return None;
    }
    if sum == 0 {
        return Some(0.0);
    }
    let mut numerator = sum.unsigned_abs();
    let mut denominator = count as u128;
    let (mut a, mut b) = (numerator, denominator);
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    numerator /= a;
    denominator /= a;
    if !denominator.is_power_of_two() {
        return None;
    }
    let odd = numerator >> numerator.trailing_zeros();
    if u128::BITS - odd.leading_zeros() > f64::MANTISSA_DIGITS {
        return None;
    }
    let result = (numerator as f64) / (denominator as f64);
    if !result.is_finite() {
        return None;
    }
    Some(if sum < 0 { -result } else { result })
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
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn target_mut(&mut self, name: &Name) -> EResult<&mut Variable> {
        let index = self
            .scopes
            .iter()
            .rposition(|scope| scope.contains_key(&name.text))
            .ok_or_else(|| {
                Diagnostic::new(name.span, format!("Undefined variable '{}'", name.text))
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

        self.scopes[index].get_mut(&name.text).ok_or_else(|| {
            Diagnostic::new(name.span, format!("Undefined variable '{}'", name.text))
        })
    }

    fn run(&mut self, program: &[Stmt]) -> EResult<()> {
        self.execute_block(program)?;
        Ok(())
    }

    fn run_scoped(&mut self, body: &[Stmt], scope: HashMap<String, Variable>) -> EResult<ExecFlow> {
        self.scopes.push(scope);

        let result = self.execute_block(body);

        self.scopes.pop();
        result
    }

    fn execute_block(&mut self, statements: &[Stmt]) -> EResult<ExecFlow> {
        for stmt in statements {
            let flow = self.execute_stmt(stmt)?;
            if !matches!(flow, ExecFlow::Continue) {
                return Ok(flow);
            }
        }

        Ok(ExecFlow::Continue)
    }

    fn execute_stmt(&mut self, stmt: &Stmt) -> EResult<ExecFlow> {
        match &stmt.kind {
            Statement::Import { path, alias } => {
                let scope = self.scopes.last_mut().unwrap();
                scope.insert(
                    alias.text.clone(),
                    Variable {
                        declared_type: "module".into(),
                        value: Value::Type(path.clone()),
                    },
                );
            }

            Statement::Function(definition) => {
                if self.scopes.len() != 1 || !self.return_types.is_empty() {
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
                        format!("'{}' is already declared", definition.name.text),
                    ));
                }

                self.functions
                    .insert(definition.name.text.clone(), definition.clone());
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
                        format!("'{}' is already declared in this scope", name.text),
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
                            Value::Int(n) => n,

                            other => {
                                return Err(Diagnostic::new(
                                    index.span,
                                    format!("Index must be int, got {}", other.type_name()),
                                ));
                            }
                        };

                        let resolved = resolve_index(n, elements.len(), false, index.span)?;

                        elements[resolved] = new_value;
                    }

                    other => {
                        return Err(Diagnostic::new(
                            name.span,
                            format!("Cannot index into {}", other.type_name()),
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
                let expected = self.return_types.last().cloned().ok_or_else(|| {
                    Diagnostic::new(stmt.span, "return is only allowed inside a function")
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

                return Ok(ExecFlow::Return(result));
            }

            Statement::Break => return Ok(ExecFlow::Break),

            Statement::Skip => return Ok(ExecFlow::Skip),

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
                                format!("if condition must be bool, got {}", other.type_name()),
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
                            format!("loop condition must be bool, got {}", other.type_name()),
                        ));
                    }
                };

                if !keep_going {
                    break;
                }

                match self.run_scoped(body, HashMap::new())? {
                    ExecFlow::Continue => {}
                    ExecFlow::Return(value) => return Ok(ExecFlow::Return(value)),
                    ExecFlow::Break => break,
                    ExecFlow::Skip => continue,
                }
            },

            Statement::CycleIn {
                item_name,
                list_expr,
                body,
            } => {
                let (inner, elements) = match self.evaluate(list_expr)? {
                    Value::List(inner, elements) => (inner, elements),

                    other => {
                        return Err(Diagnostic::new(
                            list_expr.span,
                            format!("loop .. in .. requires a list, got {}", other.type_name()),
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

                    match self.run_scoped(body, scope)? {
                        ExecFlow::Continue => {}
                        ExecFlow::Return(value) => return Ok(ExecFlow::Return(value)),
                        ExecFlow::Break => break,
                        ExecFlow::Skip => continue,
                    }
                }
            }
        }

        Ok(ExecFlow::Continue)
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
                            result.push_str(&self.evaluate(inner)?.display());
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

            ExprKind::Variable(name) => match self.lookup(name) {
                Some(variable) if variable.declared_type == "module" => Err(Diagnostic::new(
                    expr.span,
                    format!("'{}' is a module, not a variable", name),
                )),

                Some(variable) => Ok(variable.value.clone()),

                None => Err(Diagnostic::new(
                    expr.span,
                    format!("Undefined variable '{}'", name),
                )),
            },

            ExprKind::Index(array, index) => {
                let array_value = self.evaluate(array)?;
                let index_value = self.evaluate(index)?;

                let n = match index_value {
                    Value::Int(n) => n,

                    other => {
                        return Err(Diagnostic::new(
                            index.span,
                            format!("Index must be int, got {}", other.type_name()),
                        ));
                    }
                };

                match array_value {
                    Value::List(_, elements) => {
                        let index = resolve_index(n, elements.len(), false, index.span)?;
                        Ok(elements[index].clone())
                    }

                    Value::Str(text) => {
                        let length = text.chars().count();
                        let index = resolve_index(n, length, false, index.span)?;
                        let character = text
                            .chars()
                            .nth(index)
                            .expect("Resolved string index must be in bounds");
                        Ok(Value::Str(character.to_string()))
                    }

                    other => Err(Diagnostic::new(
                        array.span,
                        format!("Cannot index into {}", other.type_name()),
                    )),
                }
            }

            ExprKind::Unary(op, operand) => {
                let value = self.evaluate(operand)?;

                match (op.as_str(), value) {
                    ("-", Value::Int(n)) => n.checked_neg().map(Value::Int).ok_or_else(|| {
                        Diagnostic::new(expr.span, "Integer overflow in unary minus")
                    }),
                    ("-", Value::Flt(n)) => Ok(Value::Flt(-n)),
                    ("!", Value::Bool(value)) => Ok(Value::Bool(!value)),
                    ("-", other) => Err(Diagnostic::new(
                        expr.span,
                        format!("Unary '-' requires int or flt, got {}", other.type_name()),
                    )),
                    (_, other) => Err(Diagnostic::new(
                        expr.span,
                        format!("Unary '!' requires bool, got {}", other.type_name()),
                    )),
                }
            }

            ExprKind::Binary(left, op, right) => {
                let a = self.evaluate(left)?;
                if op == "&&" || op == "||" {
                    match &a {
                        Value::Bool(false) if op == "&&" => return Ok(Value::Bool(false)),
                        Value::Bool(true) if op == "||" => return Ok(Value::Bool(true)),
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
                    (Value::Int(a), op @ ("+" | "-" | "*" | "%" | "^"), Value::Int(b)) => {
                        let checked = match op {
                            "+" => a.checked_add(b),
                            "-" => a.checked_sub(b),
                            "*" => a.checked_mul(b),
                            "%" => a.checked_rem(b),
                            "^" => {
                                if b < 0 {
                                    None
                                } else {
                                    u32::try_from(b)
                                        .ok()
                                        .and_then(|exponent| a.checked_pow(exponent))
                                }
                            }
                            _ => unreachable!(),
                        };
                        Value::Int(checked.ok_or_else(|| Diagnostic::new(
                            expr.span, "Integer arithmetic out of range (overflow, zero divisor or exponent)"
                        ))?)
                    }
                    (Value::Int(a), "/", Value::Int(b)) => {
                        if b == 0 {
                            return Err(Diagnostic::new(expr.span, "Division by zero"));
                        }
                        Value::Flt(exact_integer_quotient(a, b).ok_or_else(|| {
                            Diagnostic::new(
                                expr.span,
                                "Integer division would lose floating-point precision",
                            )
                        })?)
                    }
                    (Value::Flt(a), "+", Value::Flt(b)) => Value::Flt(a + b),
                    (Value::Flt(a), "-", Value::Flt(b)) => Value::Flt(a - b),
                    (Value::Flt(a), "*", Value::Flt(b)) => Value::Flt(a * b),
                    (Value::Flt(_), "/", Value::Flt(0.0)) => {
                        return Err(Diagnostic::new(expr.span, "Division by zero"));
                    }
                    (Value::Flt(_), "%", Value::Flt(0.0)) => {
                        return Err(Diagnostic::new(expr.span, "Modulo by zero"));
                    }
                    (Value::Flt(a), "/", Value::Flt(b)) => Value::Flt(a / b),
                    (Value::Flt(a), "%", Value::Flt(b)) => Value::Flt(a % b),
                    (Value::Flt(a), "^", Value::Flt(b)) => Value::Flt(a.powf(b)),
                    (Value::Int(a), "==", Value::Int(b)) => Value::Bool(a == b),
                    (Value::Int(a), "!=", Value::Int(b)) => Value::Bool(a != b),
                    (Value::Int(a), "<", Value::Int(b)) => Value::Bool(a < b),
                    (Value::Int(a), ">", Value::Int(b)) => Value::Bool(a > b),
                    (Value::Flt(a), "==", Value::Flt(b)) => Value::Bool(a == b),
                    (Value::Flt(a), "!=", Value::Flt(b)) => Value::Bool(a != b),
                    (Value::Flt(a), "<", Value::Flt(b)) => Value::Bool(a < b),
                    (Value::Flt(a), ">", Value::Flt(b)) => Value::Bool(a > b),
                    (Value::Str(a), "==", Value::Str(b)) => Value::Bool(a == b),
                    (Value::Str(a), "!=", Value::Str(b)) => Value::Bool(a != b),
                    (Value::Bool(a), "==", Value::Bool(b)) => Value::Bool(a == b),
                    (Value::Bool(a), "!=", Value::Bool(b)) => Value::Bool(a != b),
                    (Value::Bool(a), "&&", Value::Bool(b)) => Value::Bool(a && b),
                    (Value::Bool(a), "||", Value::Bool(b)) => Value::Bool(a || b),
                    (Value::Type(a), "==", Value::Type(b)) => Value::Bool(a == b),
                    (Value::Type(a), "!=", Value::Type(b)) => Value::Bool(a != b),
                    (Value::List(ta, a), "==", Value::List(tb, b)) => {
                        Value::Bool(ta == tb && a == b)
                    }
                    (Value::List(ta, a), "!=", Value::List(tb, b)) => {
                        Value::Bool(ta != tb || a != b)
                    }
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
                if let Value::Flt(n) = &result {
                    if !n.is_finite() {
                        return Err(Diagnostic::new(
                            expr.span,
                            "Floating-point result out of range",
                        ));
                    }
                }
                Ok(result)
            }

            ExprKind::Call(name, args) => self.call(name, args, expr.span),

            ExprKind::ModuleCall {
                module,
                function,
                args,
            } => {
                let path = match self.lookup(&module.text) {
                    Some(var) if var.declared_type == "module" => {
                        if let Value::Type(p) = &var.value {
                            p.clone()
                        } else {
                            unreachable!()
                        }
                    }
                    Some(_) => {
                        return Err(Diagnostic::new(
                            module.span,
                            format!("'{}' is not a module", module.text),
                        ));
                    }
                    None => {
                        return Err(Diagnostic::new(
                            module.span,
                            format!("Undefined module '{}'", module.text),
                        ));
                    }
                };

                match path.as_str() {
                    "strings" => self.call_strings_module(module, function, args, expr.span),
                    "math" => self.call_math_module(module, function, args, expr.span),
                    "types" => self.call_types_module(module, function, args, expr.span),
                    _ => Err(Diagnostic::new(
                        expr.span,
                        format!("Execution of module '{}' is not supported", path),
                    )),
                }
            }
        }
    }

    fn check_count(name: &str, actual: usize, expected: usize, span: Span) -> EResult<()> {
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

    fn call_strings_module(
        &mut self,
        module: &Name,
        function: &Name,
        args: &[Expr],
        span: Span,
    ) -> EResult<Value> {
        let mut vals = Vec::new();

        for arg in args {
            vals.push((self.evaluate(arg)?, arg.span));
        }

        let expected = match function.text.as_str() {
            "Trim" | "Split" | "StartsWith" | "EndsWith" | "Repeat" => 2,
            "Replace" => 3,
            "ToLowerCase" | "ToUpperCase" | "Capitalize" => 1,
            _ => {
                return Err(Diagnostic::new(
                    function.span,
                    format!("Unknown function '{}' in module 'strings'", function.text),
                ));
            }
        };

        if vals.len() != expected {
            return Err(Diagnostic::new(
                span,
                format!(
                    "{}:{}() from module \"strings\" expects {} argument(s), got {}",
                    module.text,
                    function.text,
                    expected,
                    vals.len()
                ),
            ));
        }

        let as_str = |val: &Value, span: Span| -> EResult<String> {
            if let Value::Str(s) = val {
                Ok(s.clone())
            } else {
                Err(Diagnostic::new(
                    span,
                    format!("Expected str, got {}", val.type_name()),
                ))
            }
        };

        let as_int = |val: &Value, span: Span| -> EResult<i64> {
            if let Value::Int(n) = val {
                Ok(*n)
            } else {
                Err(Diagnostic::new(
                    span,
                    format!("Expected int, got {}", val.type_name()),
                ))
            }
        };

        match function.text.as_str() {
            "Trim" => {
                let text = as_str(&vals[0].0, vals[0].1)?;
                let old = as_str(&vals[1].0, vals[1].1)?;

                if old.is_empty() {
                    return Err(Diagnostic::new(
                        vals[1].1,
                        "Trim() cannot use an empty string as a pattern",
                    ));
                }

                let mut res = text.as_str();

                loop {
                    let mut changed = false;

                    if let Some(stripped) = res.strip_prefix(old.as_str()) {
                        res = stripped;
                        changed = true;
                    }

                    if let Some(stripped) = res.strip_suffix(old.as_str()) {
                        res = stripped;
                        changed = true;
                    }

                    if !changed {
                        break;
                    }
                }

                Ok(Value::Str(res.to_string()))
            }

            "Replace" => {
                let text = as_str(&vals[0].0, vals[0].1)?;
                let old = as_str(&vals[1].0, vals[1].1)?;
                let new_val = as_str(&vals[2].0, vals[2].1)?;

                if old.is_empty() {
                    return Err(Diagnostic::new(
                        vals[1].1,
                        "Replace() cannot use an empty string as a pattern",
                    ));
                }

                Ok(Value::Str(text.replace(old.as_str(), &new_val)))
            }

            "Split" => {
                let text = as_str(&vals[0].0, vals[0].1)?;
                let sep = as_str(&vals[1].0, vals[1].1)?;

                if sep.is_empty() {
                    return Err(Diagnostic::new(
                        vals[1].1,
                        "Split() cannot use an empty string as a separator",
                    ));
                }

                let parts = text
                    .split(sep.as_str())
                    .map(|s| Value::Str(s.to_string()))
                    .collect();

                Ok(Value::List("str".to_string(), parts))
            }

            "StartsWith" | "EndsWith" => {
                let text = as_str(&vals[0].0, vals[0].1)?;
                let part = as_str(&vals[1].0, vals[1].1)?;

                if part.is_empty() {
                    return Err(Diagnostic::new(
                        vals[1].1,
                        format!("{}() cannot search for an empty string", function.text),
                    ));
                }

                if function.text == "StartsWith" {
                    Ok(Value::Bool(text.starts_with(part.as_str())))
                } else {
                    Ok(Value::Bool(text.ends_with(part.as_str())))
                }
            }

            "ToLowerCase" => {
                let text = as_str(&vals[0].0, vals[0].1)?;
                Ok(Value::Str(text.to_lowercase()))
            }

            "ToUpperCase" => {
                let text = as_str(&vals[0].0, vals[0].1)?;
                Ok(Value::Str(text.to_uppercase()))
            }

            "Capitalize" => {
                let text = as_str(&vals[0].0, vals[0].1)?;
                let mut result = String::new();
                let mut done = false;

                for ch in text.chars() {
                    if !done && ch.is_alphabetic() {
                        result.extend(ch.to_uppercase());
                        done = true;
                    } else {
                        result.push(ch);
                    }
                }

                Ok(Value::Str(result))
            }

            "Repeat" => {
                let text = as_str(&vals[0].0, vals[0].1)?;
                let count = as_int(&vals[1].0, vals[1].1)?;

                if count < 0 {
                    return Err(Diagnostic::new(
                        vals[1].1,
                        "Repeat() count must be a non-negative integer",
                    ));
                }

                let max_len: usize = 64 * 1024 * 1024;

                if count > max_len as i64 {
                    return Err(Diagnostic::new(
                        span,
                        "Resulting string is too large (exceeds 64 MiB limit)",
                    ));
                }

                let n = count as usize;

                match text.len().checked_mul(n) {
                    Some(total) if total <= max_len => Ok(Value::Str(text.repeat(n))),

                    _ => Err(Diagnostic::new(
                        span,
                        "Resulting string is too large (exceeds 64 MiB limit)",
                    )),
                }
            }

            _ => Err(Diagnostic::new(
                function.span,
                format!("Unknown function '{}' in module 'strings'", function.text),
            )),
        }
    }

    fn call_math_module(
        &mut self,
        module: &Name,
        function: &Name,
        args: &[Expr],
        span: Span,
    ) -> EResult<Value> {
        let name = function.text.as_str();
        let count = match name {
            "AbsoluteValue" | "SquareRoot" | "Floor" | "Ceil" | "Round" | "Sum" | "Average" => 1,
            "Minimum" | "Maximum" | "RandomInterval" => 2,
            "Clamp" => 3,
            _ => {
                return Err(Diagnostic::new(
                    function.span,
                    format!("Unknown function '{}' in module 'math'", name),
                ));
            }
        };
        Self::check_count(
            &format!("{}:{}", module.text, name),
            args.len(),
            count,
            span,
        )?;
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.evaluate(arg)?);
        }
        let mismatch = || {
            Diagnostic::new(
                span,
                format!("{}() arguments have invalid or incompatible types", name),
            )
        };
        let out_of_range =
            || Diagnostic::new(span, format!("{}() result out of range or invalid", name));

        match (name, values.as_slice()) {
            ("RandomInterval", [Value::Int(a), Value::Int(b)]) => {
                if a > b {
                    return Err(Diagnostic::new(
                        span,
                        "RandomInterval() minimum must not exceed maximum",
                    ));
                }
                use rand::Rng;
                Ok(Value::Int(rand::rng().random_range(*a..=*b)))
            }
            ("AbsoluteValue", [Value::Int(n)]) => {
                Ok(Value::Int(n.checked_abs().ok_or_else(out_of_range)?))
            }
            ("AbsoluteValue", [Value::Flt(n)]) => Ok(Value::Flt(n.abs())),
            ("SquareRoot", [Value::Flt(n)]) => {
                if *n < 0.0 {
                    return Err(Diagnostic::new(
                        span,
                        "SquareRoot() requires a non-negative value",
                    ));
                }
                Ok(Value::Flt(n.sqrt()))
            }
            ("Floor", [Value::Flt(n)]) => Ok(Value::Flt(n.floor())),
            ("Ceil", [Value::Flt(n)]) => Ok(Value::Flt(n.ceil())),
            ("Round", [Value::Flt(n)]) => Ok(Value::Flt(n.round())),
            ("Minimum", [Value::Int(a), Value::Int(b)]) => Ok(Value::Int((*a).min(*b))),
            ("Maximum", [Value::Int(a), Value::Int(b)]) => Ok(Value::Int((*a).max(*b))),
            ("Minimum", [Value::Flt(a), Value::Flt(b)]) => Ok(Value::Flt(a.min(*b))),
            ("Maximum", [Value::Flt(a), Value::Flt(b)]) => Ok(Value::Flt(a.max(*b))),
            ("Clamp", [Value::Int(n), Value::Int(lo), Value::Int(hi)]) => {
                if lo > hi {
                    return Err(Diagnostic::new(
                        span,
                        "Clamp() minimum must not exceed maximum",
                    ));
                }
                Ok(Value::Int((*n).clamp(*lo, *hi)))
            }
            ("Clamp", [Value::Flt(n), Value::Flt(lo), Value::Flt(hi)]) => {
                if lo > hi {
                    return Err(Diagnostic::new(
                        span,
                        "Clamp() minimum must not exceed maximum",
                    ));
                }
                Ok(Value::Flt((*n).max(*lo).min(*hi)))
            }
            ("Sum", [Value::List(inner, items)]) if inner == "int" => {
                let mut sum = 0_i64;
                for value in items {
                    if let Value::Int(n) = value {
                        sum = sum.checked_add(*n).ok_or_else(out_of_range)?;
                    } else {
                        return Err(mismatch());
                    }
                }
                Ok(Value::Int(sum))
            }
            ("Sum", [Value::List(inner, items)]) if inner == "flt" => {
                let mut sum = 0.0_f64;
                for value in items {
                    if let Value::Flt(n) = value {
                        sum += *n;
                        if !sum.is_finite() {
                            return Err(out_of_range());
                        }
                    } else {
                        return Err(mismatch());
                    }
                }
                Ok(Value::Flt(sum))
            }
            ("Average", [Value::List(inner, items)]) if inner == "int" => {
                let mut sum = 0_i128;
                for value in items {
                    if let Value::Int(n) = value {
                        sum = sum.checked_add(*n as i128).ok_or_else(out_of_range)?;
                    } else {
                        return Err(mismatch());
                    }
                }
                if items.is_empty() {
                    return Err(Diagnostic::new(span, "Average() requires a non-empty list"));
                }
                let result = exact_integer_average(sum, items.len()).ok_or_else(|| {
                    Diagnostic::new(
                        span,
                        "Average() of integers would lose floating-point precision",
                    )
                })?;
                Ok(Value::Flt(result))
            }
            ("Average", [Value::List(inner, items)]) if inner == "flt" => {
                if items.is_empty() {
                    return Err(Diagnostic::new(span, "Average() requires a non-empty list"));
                }
                // Scale each term before adding to avoid intermediate sum overflow.
                let divisor = items.len() as f64;
                let mut average = 0.0_f64;
                for value in items {
                    if let Value::Flt(n) = value {
                        average += *n / divisor;
                        if !average.is_finite() {
                            return Err(out_of_range());
                        }
                    } else {
                        return Err(mismatch());
                    }
                }
                Ok(Value::Flt(average))
            }
            _ => Err(mismatch()),
        }
    }

    fn call_types_module(
        &mut self,
        module: &Name,
        function: &Name,
        args: &[Expr],
        span: Span,
    ) -> EResult<Value> {
        let name = function.text.as_str();
        if name != "Convert" && name != "IsConvertible" {
            return Err(Diagnostic::new(
                function.span,
                format!("Unknown function '{}' in module 'types'", name),
            ));
        }
        Self::check_count(&format!("{}:{}", module.text, name), args.len(), 2, span)?;
        let source = self.evaluate(&args[0])?;
        let target = match &args[1].kind {
            ExprKind::Literal(Value::Type(target)) if simple_convert_target(target) => target,
            _ => {
                return Err(Diagnostic::new(
                    args[1].span,
                    "The conversion target must be a literal int, flt, str or bool",
                ));
            }
        };
        let converted = convert_value(&source, target);
        if name == "IsConvertible" {
            Ok(Value::Bool(converted.is_some()))
        } else {
            converted.ok_or_else(|| {
                Diagnostic::new(
                    span,
                    format!(
                        "Convert() cannot convert {} value to {}",
                        source.type_name(),
                        target
                    ),
                )
            })
        }
    }

    fn call(&mut self, name: &str, args: &[Expr], span: Span) -> EResult<Value> {
        match name {
            "Output" => {
                Self::check_count(name, args.len(), 1, span)?;

                let value = self.evaluate(&args[0])?;
                let text = value.display();

                let mut stdout = io::stdout().lock();

                write!(stdout, "{}", text)
                    .map_err(|e| Diagnostic::new(span, format!("Failed to write output: {}", e)))?;

                stdout
                    .flush()
                    .map_err(|e| Diagnostic::new(span, format!("Failed to flush output: {}", e)))?;

                if !text.is_empty() {
                    OUTPUT_LINE_OPEN.store(!text.ends_with('\n'), Ordering::Relaxed);
                }

                return Ok(Value::Null);
            }

            "Input" => {
                Self::check_count(name, args.len(), 1, span)?;

                let prompt = match self.evaluate(&args[0])? {
                    Value::Str(text) => text,

                    other => {
                        return Err(Diagnostic::new(
                            args[0].span,
                            format!("Input() requires a str prompt, got {}", other.type_name()),
                        ));
                    }
                };

                // Print the prompt exactly like Output() does,
                // so it is visible before the program starts waiting.
                {
                    let mut stdout = io::stdout().lock();

                    write!(stdout, "{}", prompt).map_err(|e| {
                        Diagnostic::new(span, format!("Failed to write output: {}", e))
                    })?;

                    stdout.flush().map_err(|e| {
                        Diagnostic::new(span, format!("Failed to flush output: {}", e))
                    })?;
                }

                if !prompt.is_empty() {
                    OUTPUT_LINE_OPEN.store(!prompt.ends_with('\n'), Ordering::Relaxed);
                }

                let mut line = String::new();

                let bytes_read = io::stdin()
                    .lock()
                    .read_line(&mut line)
                    .map_err(|e| Diagnostic::new(span, format!("Failed to read input: {}", e)))?;

                if bytes_read == 0 {
                    return Err(Diagnostic::new(
                        span,
                        "Input() reached the end of input: nothing left to read",
                    ));
                }

                // Remove only the Enter itself (\n or \r\n).
                // Everything the user typed stays untouched.
                if line.ends_with('\n') {
                    line.pop();

                    if line.ends_with('\r') {
                        line.pop();
                    }
                }

                // In a terminal the user's Enter already moved
                // the cursor to a new line. With piped input it did not.
                if io::stdin().is_terminal() {
                    OUTPUT_LINE_OPEN.store(false, Ordering::Relaxed);
                }

                return Ok(Value::Str(line));
            }

            "TypeOf" => {
                Self::check_count(name, args.len(), 1, span)?;

                let value = self.evaluate(&args[0])?;
                return Ok(Value::Type(value.type_name()));
            }

            "Includes" => {
                Self::check_count(name, args.len(), 2, span)?;

                let haystack = self.evaluate(&args[0])?;
                let needle = self.evaluate(&args[1])?;

                return match haystack {
                    Value::Str(text) => {
                        let pattern = match needle {
                            Value::Str(pattern) => pattern,

                            other => {
                                return Err(Diagnostic::new(
                                    args[1].span,
                                    format!(
                                        "Includes() requires a str to search inside a str, got {}",
                                        other.type_name()
                                    ),
                                ));
                            }
                        };

                        if pattern.is_empty() {
                            return Err(Diagnostic::new(
                                args[1].span,
                                "Includes() cannot search for an empty string",
                            ));
                        }

                        Ok(Value::Bool(text.contains(&pattern)))
                    }

                    Value::List(inner, elements) => {
                        if needle.type_name() != inner {
                            return Err(Diagnostic::new(
                                args[1].span,
                                format!(
                                    "Includes() cannot search for {} inside ls<{}>",
                                    needle.type_name(),
                                    inner
                                ),
                            ));
                        }

                        Ok(Value::Bool(elements.contains(&needle)))
                    }

                    other => Err(Diagnostic::new(
                        args[0].span,
                        format!(
                            "Includes() requires a string or list, got {}",
                            other.type_name()
                        ),
                    )),
                };
            }

            "Length" => {
                Self::check_count(name, args.len(), 1, span)?;

                return match self.evaluate(&args[0])? {
                    Value::Str(text) => Ok(Value::Int(
                        i64::try_from(text.chars().count())
                            .map_err(|_| Diagnostic::new(span, "String too long"))?,
                    )),

                    Value::List(_, elements) => Ok(Value::Int(
                        i64::try_from(elements.len())
                            .map_err(|_| Diagnostic::new(span, "List too long"))?,
                    )),

                    other => Err(Diagnostic::new(
                        args[0].span,
                        format!(
                            "Length() requires a string or list, got {}",
                            other.type_name()
                        ),
                    )),
                };
            }

            "Insert" => {
                Self::check_count(name, args.len(), 3, span)?;

                let list = self.evaluate(&args[0])?;
                let index = self.evaluate(&args[1])?;
                let element = self.evaluate(&args[2])?;

                let (inner, mut elements) = match list {
                    Value::List(inner, elements) => (inner, elements),

                    other => {
                        return Err(Diagnostic::new(
                            args[0].span,
                            format!("Insert() expects a list, got {}", other.type_name()),
                        ));
                    }
                };

                let n = match index {
                    Value::Int(n) => n,

                    other => {
                        return Err(Diagnostic::new(
                            args[1].span,
                            format!("Index must be int, got {}", other.type_name()),
                        ));
                    }
                };

                if element.type_name() != inner {
                    return Err(Diagnostic::new(
                        args[2].span,
                        format!("Cannot insert {} into ls<{}>", element.type_name(), inner),
                    ));
                }

                let index = resolve_index(n, elements.len(), true, args[1].span)?;

                elements.insert(index, element);

                return Ok(Value::List(inner, elements));
            }

            "Remove" => {
                Self::check_count(name, args.len(), 2, span)?;

                let list = self.evaluate(&args[0])?;
                let index = self.evaluate(&args[1])?;

                let (inner, mut elements) = match list {
                    Value::List(inner, elements) => (inner, elements),

                    other => {
                        return Err(Diagnostic::new(
                            args[0].span,
                            format!("Remove() expects a list, got {}", other.type_name()),
                        ));
                    }
                };

                let n = match index {
                    Value::Int(n) => n,

                    other => {
                        return Err(Diagnostic::new(
                            args[1].span,
                            format!("Index must be int, got {}", other.type_name()),
                        ));
                    }
                };

                let index = resolve_index(n, elements.len(), false, args[1].span)?;

                elements.remove(index);

                return Ok(Value::List(inner, elements));
            }

            _ => {}
        }

        let definition = self
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| Diagnostic::new(span, format!("Unknown function '{}'", name)))?;

        if args.len() != definition.parameters.len() {
            return Err(Diagnostic::new(
                span,
                format!(
                    "Function '{}' expects {} argument(s), got {}",
                    name,
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

        self.return_types.push(definition.return_type.name.clone());

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
            let (value, value_span) = values.next().ok_or_else(|| {
                Diagnostic::new(
                    parameter.name.span,
                    format!(
                        "Missing argument '{}' for '{}'",
                        parameter.name.text, definition.name.text
                    ),
                )
            })?;

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

        let value = match self.execute_block(&definition.body)? {
            ExecFlow::Return(value) => value,
            ExecFlow::Continue => {
                return Err(Diagnostic::new(
                    definition.name.span,
                    format!(
                        "Function '{}' finished without return",
                        definition.name.text
                    ),
                ));
            }
            ExecFlow::Break => {
                return Err(Diagnostic::new(
                    definition.name.span,
                    "'break' cannot escape a function",
                ));
            }
            ExecFlow::Skip => {
                return Err(Diagnostic::new(
                    definition.name.span,
                    "'skip' cannot escape a function",
                ));
            }
        };

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

    Parser::new(tokens).parse_program().map_err(Failure::Syntax)
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

    Interpreter::new().run(&program).map_err(Failure::Runtime)
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
        "tokens" => lex(&source).map_err(Failure::Syntax).map(|tokens| {
            for token in tokens {
                println!("{:?}", token);
            }
        }),

        "parse" => parse_source(&source).map(|program| {
            for stmt in program {
                println!("{:#?}", stmt);
            }
        }),

        "check" => check_source(&source).map(|_| println!("OK: no static errors")),

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
                print_diagnostic("Syntax error", path, &source, &diagnostic);
            }

            Failure::Static(diagnostics) => {
                for diagnostic in diagnostics {
                    print_diagnostic("Static error", path, &source, &diagnostic);
                }
            }

            Failure::Runtime(diagnostic) => {
                print_diagnostic("Runtime error", path, &source, &diagnostic);
            }
        }

        std::process::exit(1);
    }
}
#[cfg(test)]
mod stage1_tests {
    use super::*;

    #[test]
    fn numeric_literals_have_distinct_types() {
        let tokens = lex("5 5.0").unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Integer(5)));
        assert!(matches!(tokens[1].kind, TokenKind::Float(v) if v == 5.0));
    }

    #[test]
    fn function_syntax_parses_and_checks() {
        let program = "function Square(x int) int { return int x * x }\nOutput(Square(5))";
        assert!(check_source(program).is_ok());
    }

    #[test]
    fn default_parameters_are_rejected() {
        let source = "function Bad(x int = 1) int { return int x }";
        assert!(parse_source(source).is_err());
    }

    #[test]
    fn mixed_arithmetic_is_rejected() {
        assert!(check_source("Output(1 + 2.0)").is_err());
    }

    #[test]
    fn integer_quotient_exactness() {
        assert_eq!(exact_integer_quotient(5, 2), Some(2.5));
        assert_eq!(exact_integer_quotient(1, 4), Some(0.25));
        assert_eq!(exact_integer_quotient(1, 3), None);
        assert_eq!(exact_integer_quotient(1, 10), None);
        assert_eq!(exact_integer_quotient(0, 5), Some(0.0));
        assert_eq!(exact_integer_quotient(5, 0), None);
        assert_eq!(
            exact_integer_quotient(9_007_199_254_740_993, 3),
            Some(3_002_399_751_580_331.0)
        );
    }

    #[test]
    fn random_interval_inclusive_equal_bounds() {
        let source =
            "import \"math\" as math\nvalue int := math:RandomInterval(7, 7)\nOutput(value)";
        assert!(check_source(source).is_ok());
        assert!(execute(source).is_ok());
    }

    #[test]
    fn random_interval_rejects_inverted_bounds() {
        let source = "import \"math\" as math\nOutput(math:RandomInterval(8, 4))";
        assert!(check_source(source).is_err());
    }

    #[test]
    fn integer_index_and_length() {
        let source =
            "values ls<int> := list<int>{1, 2, 3}\nOutput(values[1])\nOutput(Length(values))";
        assert!(check_source(source).is_ok());
    }
}

#[cfg(test)]
mod stage2_tests {
    use super::*;

    fn evaluate_checked(source: &str) -> EResult<Value> {
        let program = match check_source(source) {
            Ok(program) => program,
            Err(Failure::Static(errors)) => panic!("Static errors: {:?}", errors),
            Err(Failure::Syntax(err)) => panic!("Syntax error: {:?}", err),
            Err(Failure::Runtime(_)) => unreachable!(),
        };
        let mut vm = Interpreter::new();
        let (last, previous) = program.split_last().expect("Non-empty program");
        vm.execute_block(previous)?;
        if let Statement::Expression(expr) = &last.kind {
            vm.evaluate(expr)
        } else {
            panic!("Last statement must be an expression");
        }
    }

    fn math(expr: &str) -> EResult<Value> {
        evaluate_checked(&format!("import \"math\" as math\n{}", expr))
    }

    fn types(expr: &str) -> EResult<Value> {
        evaluate_checked(&format!("import \"types\" as types\n{}", expr))
    }

    #[test]
    fn break_and_skip_control_while_loops() {
        let source = r#"
            i int := 0
            total int := 0
            loop i < 10 {
                i = i + 1
                if i == 2 {
                    skip
                }
                if i == 5 {
                    break
                }
                total = total + i
            }
            total
        "#;

        assert_eq!(evaluate_checked(source).unwrap(), Value::Int(8));
    }

    #[test]
    fn break_and_skip_control_for_in_loops() {
        let source = r#"
            values ls<int> := list<int>{1, 2, 3, 4}
            total int := 0
            loop item in values {
                if item == 2 {
                    skip
                }
                total = total + item
                if item == 3 {
                    break
                }
            }
            total
        "#;

        assert_eq!(evaluate_checked(source).unwrap(), Value::Int(4));
    }

    #[test]
    fn loop_control_only_affects_the_nearest_nested_loop() {
        let source = r#"
            outer int := 0
            total int := 0
            loop outer < 3 {
                outer = outer + 1
                inner int := 0
                loop inner < 4 {
                    inner = inner + 1
                    if inner == 2 {
                        break
                    }
                    total = total + 1
                }
            }
            total
        "#;

        assert_eq!(evaluate_checked(source).unwrap(), Value::Int(3));
    }

    #[test]
    fn loop_control_is_rejected_outside_loops() {
        assert!(matches!(check_source("break"), Err(Failure::Static(_))));
        assert!(matches!(check_source("skip"), Err(Failure::Static(_))));
    }

    #[test]
    fn function_cannot_control_a_callers_loop() {
        let source = r#"
            function Invalid() int {
                break
                return int 1
            }
            loop true {
                Output(Invalid())
                break
            }
        "#;

        assert!(matches!(check_source(source), Err(Failure::Static(_))));
    }

    #[test]
    fn loop_control_is_allowed_inside_a_functions_own_loop() {
        let source = r#"
            function First() int {
                loop true {
                    break
                }
                return int 1
            }
            Output(First())
        "#;

        assert!(check_source(source).is_ok());
        assert!(execute(source).is_ok());
    }

    #[test]
    fn absolute_value_and_square_root() {
        assert_eq!(math("math:AbsoluteValue((-17))").unwrap(), Value::Int(17));
        assert_eq!(math("math:AbsoluteValue((-2.5))").unwrap(), Value::Flt(2.5));
        assert_eq!(math("math:SquareRoot(81.0)").unwrap(), Value::Flt(9.0));
        assert!(math("math:SquareRoot((-1.0))").is_err());
        assert!(check_source("import \"math\" as math\nmath:SquareRoot(9)").is_err());
    }

    #[test]
    fn absolute_value_integer_minimum_rejected() {
        let source = "import \"math\" as math\nimport \"types\" as types\nmath:AbsoluteValue(types:Convert(\"-9223372036854775808\", int))";
        assert!(evaluate_checked(source).is_err());
    }

    #[test]
    fn rounding_away_from_zero() {
        assert_eq!(math("math:Round(2.5)").unwrap(), Value::Flt(3.0));
        assert_eq!(math("math:Round((-2.5))").unwrap(), Value::Flt(-3.0));
        assert_eq!(math("math:Floor((-2.1))").unwrap(), Value::Flt(-3.0));
        assert_eq!(math("math:Ceil((-2.1))").unwrap(), Value::Flt(-2.0));
    }

    #[test]
    fn min_max_clamp_typed_and_bounds_checked() {
        assert_eq!(math("math:Minimum(3, 8)").unwrap(), Value::Int(3));
        assert_eq!(math("math:Maximum(3.0, 8.0)").unwrap(), Value::Flt(8.0));
        assert_eq!(math("math:Clamp(15, 0, 10)").unwrap(), Value::Int(10));
        assert_eq!(
            math("math:Clamp((-2.0), 0.0, 10.0)").unwrap(),
            Value::Flt(0.0)
        );
        assert!(math("math:Clamp(1, 3, 2)").is_err());
        assert!(check_source("import \"math\" as math\nmath:Minimum(2, 3.0)").is_err());
    }

    #[test]
    fn sum_empty_and_overflow() {
        assert_eq!(math("math:Sum(list<int>{})").unwrap(), Value::Int(0));
        assert_eq!(math("math:Sum(list<int>{1, 2, 3})").unwrap(), Value::Int(6));
        assert_eq!(
            math("math:Sum(list<flt>{1.5, 2.5})").unwrap(),
            Value::Flt(4.0)
        );
        assert!(math("math:Sum(list<int>{9223372036854775807, 1})").is_err());
        assert!(check_source("import \"math\" as math\nmath:Sum(list<str>{\"x\"})").is_err());
    }

    #[test]
    fn average_exactness_int_and_floats() {
        assert_eq!(
            math("math:Average(list<int>{10, 20, 30})").unwrap(),
            Value::Flt(20.0)
        );
        assert_eq!(
            math("math:Average(list<int>{1, 2})").unwrap(),
            Value::Flt(1.5)
        );
        assert_eq!(
            math("math:Average(list<flt>{2.0, 4.0})").unwrap(),
            Value::Flt(3.0)
        );
        assert!(math("math:Average(list<int>{1, 2, 2})").is_err());
        assert!(math("math:Average(list<int>{})").is_err());
        assert!(math("math:Average(list<flt>{})").is_err());
    }

    #[test]
    fn random_interval_keeps_inclusive_endpoints() {
        // Equal bounds prove that both ends of an inclusive range are accepted.
        assert_eq!(math("math:RandomInterval(5, 5)").unwrap(), Value::Int(5));

        // Invalid literal bounds are rejected during static checking, not at runtime.
        let literal_bounds = "import \"math\" as math\nmath:RandomInterval(10, 1)";
        assert!(matches!(
            check_source(literal_bounds),
            Err(Failure::Static(_))
        ));

        // Non-literal bounds pass static checking but fail at runtime when inverted.
        let dynamic_bounds = "import \"math\" as math\nminimum int := 10\nmaximum int := 1\nmath:RandomInterval(minimum, maximum)";
        assert!(check_source(dynamic_bounds).is_ok());
        assert!(evaluate_checked(dynamic_bounds).is_err());

        // The numeric types of both boundaries are checked statically.
        assert!(check_source("import \"math\" as math\nmath:RandomInterval(1, 2.0)").is_err());
    }

    #[test]
    fn conversions_validate_values() {
        assert_eq!(
            types("types:Convert(\"123\", int)").unwrap(),
            Value::Int(123)
        );
        assert_eq!(
            types("types:Convert(\"3.25\", flt)").unwrap(),
            Value::Flt(3.25)
        );
        assert_eq!(types("types:Convert(3.0, int)").unwrap(), Value::Int(3));
        assert_eq!(
            types("types:Convert(42, str)").unwrap(),
            Value::Str("42".into())
        );
        assert_eq!(
            types("types:Convert(\"true\", bool)").unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            types("types:Convert(false, str)").unwrap(),
            Value::Str("false".into())
        );
        assert!(types("types:Convert(3.9, int)").is_err());
        assert!(types("types:Convert(9223372036854775807, flt)").is_err());
        assert!(types("types:Convert(9223372036854775808.0, int)").is_err());
        assert!(types("types:Convert(\"bad\", int)").is_err());
    }

    #[test]
    fn is_convertible_returns_false_without_error() {
        assert_eq!(
            types("types:IsConvertible(\"123\", int)").unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            types("types:IsConvertible(\"bad\", int)").unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            types("types:IsConvertible(3.9, int)").unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            types("types:IsConvertible(true, int)").unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            types("types:IsConvertible(list<int>{1}, str)").unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn convert_rejects_unsupported_pair_statically() {
        assert!(check_source("import \"types\" as types\ntypes:Convert(true, int)").is_err());
        assert!(
            check_source("import \"types\" as types\ntypes:Convert(list<int>{1}, str)").is_err()
        );
    }

    #[test]
    fn type_target_must_be_literal() {
        assert!(check_source("import \"types\" as types\ntypes:Convert(1, TypeOf(1))").is_err());
        assert!(
            check_source("import \"types\" as types\ntypes:IsConvertible(1, ls<int>)").is_err()
        );
    }

    #[test]
    fn string_indexing_is_one_based_and_returns_str() {
        assert_eq!(
            evaluate_checked(r#""hello"[1]"#).unwrap(),
            Value::Str("h".into())
        );
        assert_eq!(
            evaluate_checked(r#""hello"[5]"#).unwrap(),
            Value::Str("o".into())
        );
        assert_eq!(
            evaluate_checked(
                r#"text str := "hello"
text[2]"#
            )
            .unwrap(),
            Value::Str("e".into())
        );
    }

    #[test]
    fn string_indexing_uses_unicode_scalar_values() {
        assert_eq!(
            evaluate_checked(r#""Привет"[1]"#).unwrap(),
            Value::Str("П".into())
        );
        assert_eq!(
            evaluate_checked(r#""A🙂Б"[2]"#).unwrap(),
            Value::Str("🙂".into())
        );
        assert_eq!(
            evaluate_checked(r#""A🙂Б"[3]"#).unwrap(),
            Value::Str("Б".into())
        );
    }

    #[test]
    fn string_indexing_rejects_zero_and_out_of_bounds() {
        assert!(matches!(
            check_source(r#""hello"[0]"#),
            Err(Failure::Static(_))
        ));
        assert!(evaluate_checked(r#""hello"[6]"#).is_err());
    }

    #[test]
    fn string_indexing_requires_an_integer_index() {
        assert!(matches!(
            check_source(r#""hello"["1"]"#),
            Err(Failure::Static(_))
        ));
    }

    #[test]
    fn strings_are_not_mutable_through_index_assignment() {
        assert!(matches!(
            check_source(
                r#"text str := "hello"
text[1] = "H""#
            ),
            Err(Failure::Static(_))
        ));
    }

    #[test]
    fn list_indexing_still_works_after_string_indexing() {
        assert_eq!(
            evaluate_checked("values ls<int> := list<int>{10, 20, 30}\nvalues[2]").unwrap(),
            Value::Int(20)
        );
    }

    #[test]
    fn module_imports_and_invalid_arity() {
        assert!(check_source("import \"math\" as math\nimport \"types\" as types\nOutput(math:AbsoluteValue(types:Convert(\"-4\", int)))").is_ok());
        assert!(check_source("import \"math\" as math\nmath:AbsoluteValue(1, 2)").is_err());
        assert!(check_source("import \"types\" as types\ntypes:Convert(1)").is_err());
        assert!(check_source("import \"types\" as types\ntypes:Unknown(1)").is_err());
    }
}

#[cfg(test)]
mod stage3_tests {
    use super::*;

    #[test]
    fn includes_allows_empty_string_as_list_element() {
        let source = r#"items ls<str> := list<str>{"", "value"}
Output(Includes(items, ""))"#;
        assert!(
            check_source(source).is_ok(),
            "Empty strings are valid list elements"
        );
        let program = check_source(source).ok().unwrap();
        // The list membership operation itself must evaluate to true.
        let mut interpreter = Interpreter::new();
        let (last, preceding) = program.split_last().unwrap();
        interpreter.execute_block(preceding).unwrap();
        let Statement::Expression(Expr {
            kind: ExprKind::Call(_, args),
            ..
        }) = &last.kind
        else {
            panic!("Expected Output(...) expression");
        };
        assert_eq!(interpreter.evaluate(&args[0]).unwrap(), Value::Bool(true));
    }

    #[test]
    fn includes_rejects_empty_substring_statically() {
        assert!(matches!(
            check_source("Output(Includes(\"abc\", \"\"))"),
            Err(Failure::Static(_))
        ));
    }

    #[test]
    fn fractional_output_preserves_tiny_nonzero_values() {
        assert_eq!(
            Value::Flt(0.00000000000000001).display(),
            "0.00000000000000001"
        );
        assert_eq!(
            Value::Flt(1.2345678901234567).display(),
            "1.2345678901234567"
        );
    }

    #[test]
    fn integer_conversion_boundaries_are_checked() {
        assert_eq!(
            convert_value(&Value::Str("-9223372036854775808".into()), "int"),
            Some(Value::Int(i64::MIN))
        );
        assert_eq!(
            convert_value(&Value::Str("9223372036854775807".into()), "int"),
            Some(Value::Int(i64::MAX))
        );
        assert!(convert_value(&Value::Flt(9_223_372_036_854_775_808.0), "int").is_none());
        assert!(convert_value(&Value::Int(i64::MAX), "flt").is_none());
    }

    #[test]
    fn exact_division_rejects_non_dyadic_quotients() {
        assert_eq!(exact_integer_quotient(5, 2), Some(2.5));
        assert_eq!(exact_integer_quotient(1, 3), None);
        assert_eq!(exact_integer_quotient(0, 3), Some(0.0));
        assert_eq!(exact_integer_quotient(1, 0), None);
    }
}
