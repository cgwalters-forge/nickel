use nickel_lang_core::{
    error::Sink,
    eval::cache::lazy::CBNCache,
    program::{Program, ProgramBuilder},
};

// Regression test for https://github.com/tweag/nickel/issues/2362
#[test]
fn multiple_inputs_non_paths() {
    let mut prog: Program<CBNCache> = ProgramBuilder::new()
        .add_source_string("{}", "fst")
        .add_source_string("{} & {}", "snd")
        .with_trace(std::io::stderr())
        .with_reporter(Sink::default())
        .build()
        .unwrap();

    assert_eq!(&prog.eval_full_for_export().unwrap().to_string(), "{}");
}

#[cfg(all(feature = "cap-std", unix))]
mod root_dir {
    use super::*;
    use cap_std::{ambient_authority, fs::Dir};
    use std::{fs, os::unix::fs::symlink, path::Path};

    /// Evaluates `main` (a path relative to `root`), reading files only from within `root` if
    /// `confined`. Returns the exported value, or `None` on any error.
    fn eval(root: &Path, main: &str, confined: bool) -> Option<String> {
        let builder = if confined {
            let dir = Dir::open_ambient_dir(root, ambient_authority()).unwrap();
            ProgramBuilder::new().with_root_dir(dir).add_path(main)
        } else {
            ProgramBuilder::new().add_path(root.join(main))
        };
        let mut prog: Program<CBNCache> = builder.build().ok()?;
        prog.eval_full_for_export().ok().map(|v| v.to_string())
    }

    #[test]
    fn imports_confined_to_root_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tmp.path().join("outside.ncl");
        fs::write(&outside, "\"outside\"").unwrap();
        let root = tmp.path().join("root");
        fs::create_dir_all(root.join("lib")).unwrap();
        fs::write(root.join("lib/value.ncl"), "42").unwrap();
        fs::write(root.join("lib/nested.ncl"), "import \"value.ncl\"").unwrap();
        symlink("lib/value.ncl", root.join("inside-link.ncl")).unwrap();
        symlink("../outside.ncl", root.join("rel-link.ncl")).unwrap();
        symlink(&outside, root.join("abs-link.ncl")).unwrap();

        let allowed = [
            "lib/value.ncl",
            "./lib/../lib/value.ncl",
            "lib/nested.ncl",
            "inside-link.ncl",
        ];
        let outside = outside.to_str().unwrap();
        let escaping = ["../outside.ncl", outside, "rel-link.ncl", "abs-link.ncl"];

        for import in allowed {
            fs::write(root.join("main.ncl"), format!("import {import:?}")).unwrap();
            assert_eq!(
                eval(&root, "main.ncl", true).as_deref(),
                Some("42"),
                "{import}"
            );
        }
        for import in escaping {
            fs::write(root.join("main.ncl"), format!("import {import:?}")).unwrap();
            // Only the root dir is what makes the import fail.
            assert!(eval(&root, "main.ncl", false).is_some(), "{import}");
            assert_eq!(eval(&root, "main.ncl", true), None, "{import}");
        }
        // The main file is confined too.
        assert_eq!(eval(&root, "../outside.ncl", true), None);
        assert_eq!(eval(&root, "lib/value.ncl", true).as_deref(), Some("42"));
    }
}
