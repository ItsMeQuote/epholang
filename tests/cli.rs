use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);

struct Program {
    directory: PathBuf,
    path: PathBuf,
}

impl Program {
    fn new(source: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "epholang-cli-{}-{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("example.epho");
        fs::write(&path, source).unwrap();
        Self { directory, path }
    }

    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_epholang"))
            .args(args)
            .arg(&self.path)
            .output()
            .unwrap()
    }

    fn read(&self) -> String {
        fs::read_to_string(&self.path).unwrap()
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn formatting_and_execution_agree_for_nested_interpolation_and_lists() {
    let source = concat!(
        "import \"strings\" as strings\n",
        "str name := \"Hexer\"\n",
        "ls<int> items := list<int>{\n\t7\n}\n",
        r##"Output("Result: #{strings:Repeat("Hi #{name}", items[1] - 5)} \"!\"\n")"##,
    );
    let program = Program::new(source);
    let initial = program.command(&["run"]);
    assert!(
        initial.status.success(),
        "{}",
        String::from_utf8_lossy(&initial.stderr)
    );
    assert_eq!(initial.stdout, b"Result: Hi HexerHi Hexer \"!\"\n");

    assert!(!program.command(&["format", "--check"]).status.success());
    assert_eq!(program.read(), source);
    assert!(program.command(&["format"]).status.success());
    let formatted = program.read();
    assert!(formatted.contains("\t7,\n"));
    assert_eq!(program.command(&["run"]).stdout, initial.stdout);
    assert!(program.command(&["format", "--check"]).status.success());
    assert!(program.command(&["format"]).status.success());
    assert_eq!(program.read(), formatted);
}

#[test]
fn invalid_syntax_gets_a_position_and_leaves_the_file_untouched() {
    let source = "if true:\r\n  Output(1)\r\n";
    let program = Program::new(source);
    for command in [&["format"][..], &["format", "--check"][..]] {
        let result = program.command(command);
        assert!(!result.status.success());
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(error.contains("Indentation must use tabs"), "{error}");
        assert!(error.contains("example.epho:2:1"), "{error}");
        assert_eq!(program.read(), source);
    }
}
