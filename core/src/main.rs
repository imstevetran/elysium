use clap::Parser;
use std::path::PathBuf;

use elysium::driver;
use elysium::driver::cli;
use elysium::epm::{init, install, migrate, publish, update};

fn main() {
    let cli = cli::Cli::parse();

    let result = match &cli.command {
        cli::Commands::Build { file, output, emit_ir, debug, env } => {
            let env = driver::resolve_env_alias(file, env);
            if driver::is_elyx_file(file) {
                driver::build_elyx(file, output.clone(), *emit_ir)
            } else {
                driver::compile_file(file, output.clone(), *emit_ir, *debug, &env)
            }
        }
        cli::Commands::Run { file, debug, emit_ir, env } => {
            let env = driver::resolve_env_alias(file, env);
            if driver::is_elyx_file(file) {
                driver::build_elyx(file, None, false)
            } else {
                driver::compile_and_run(file, *debug, *emit_ir, &env)
            }
        }
        cli::Commands::Check { file, env } => {
            let env = driver::resolve_env_alias(file, env);
            if driver::is_elyx_file(file) {
                driver::check_elyx(file)
            } else {
                driver::check_file(file, &env)
            }
        }
        cli::Commands::Highlight { file, format, output } => {
            driver::highlight_file(file, format, output)
        }
        cli::Commands::Lint { file, format } => driver::lint_file(file, format),
        cli::Commands::HighlightCss => {
            println!("{}", elysium::highlighter::css());
            Ok(())
        }
        cli::Commands::Repl => {
            let mut repl = elysium::debug::Repl::new();
            repl.run()
        }
        cli::Commands::Doc { file, output } => driver::doc_file(file, output),
        cli::Commands::DepGraph { file, format, output } => {
            driver::dep_graph_file(file, format, output)
        }
        cli::Commands::Test { path, dry_run, env } => driver::cmd_test(
            path,
            *dry_run,
            &driver::resolve_env_alias(
                &path
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| PathBuf::from("core/examples/")),
                env,
            ),
        ),
        cli::Commands::Update { package, apply, latest, force } => {
            update::cmd_update(package.as_deref(), *apply, *latest, *force)
        }
        cli::Commands::Migrate { file, check, dry_run, force } => {
            migrate::cmd_migrate(file.as_ref(), *check, *dry_run, *force)
        }
        cli::Commands::GenTest { file, output } => driver::gen_test_file(file, output),
        cli::Commands::Init { name, description, version, author, repository } => init::cmd_init(
            name,
            description.as_deref(),
            version,
            author.as_deref(),
            repository.as_deref(),
        ),
        cli::Commands::Install { package } => install::cmd_install(package.as_deref()),
        cli::Commands::Publish { path } => publish::cmd_publish(path.as_deref()),
        cli::Commands::Port { file, output, lang } => driver::port_file(file, output, lang),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e.message);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod integration_tests {
    use std::process::Command;

    fn assert_check_ok(relative_path: &str) {
        let output = Command::new("cargo")
            .args(["run", "--", "check", relative_path])
            .output()
            .expect("failed to run cargo check");
        assert!(
            output.status.success(),
            "check failed:\nstderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn assert_highlight_ok(relative_path: &str, format: &str) {
        let output = Command::new("cargo")
            .args(["run", "--", "highlight", relative_path, "--format", format])
            .output()
            .expect("failed to run cargo highlight");
        assert!(
            output.status.success(),
            "highlight {} failed:\nstderr: {}",
            format,
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn assert_lint_ok(relative_path: &str) {
        let output = Command::new("cargo")
            .args(["run", "--", "lint", relative_path, "--format", "text"])
            .output()
            .expect("failed to run cargo lint");
        assert!(
            output.status.success(),
            "lint failed:\nstderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn assert_doc_ok(relative_path: &str) {
        let output = Command::new("cargo")
            .args(["run", "--", "doc", relative_path])
            .output()
            .expect("failed to run cargo doc");
        assert!(
            output.status.success(),
            "doc failed:\nstderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn assert_dep_graph_ok(relative_path: &str) {
        let output = Command::new("cargo")
            .args(["run", "--", "dep-graph", relative_path, "--format", "dot"])
            .output()
            .expect("failed to run cargo dep-graph");
        assert!(
            output.status.success(),
            "dep-graph failed:\nstderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn assert_gen_test_ok(relative_path: &str) {
        let output = Command::new("cargo")
            .args(["run", "--", "gen-test", relative_path])
            .output()
            .expect("failed to run cargo gen-test");
        assert!(
            output.status.success(),
            "gen-test failed:\nstderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
    }

    #[test]
    #[ignore]
    fn test_integration_hello_check() {
        assert_check_ok("core/examples/hello.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_counter_check() {
        assert_check_ok("core/examples/counter.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_discount_check() {
        assert_check_ok("core/examples/discount.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_todo_question_check() {
        assert_check_ok("core/examples/todo_question.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_bench_check() {
        assert_check_ok("core/examples/bench.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_use_math_alias_check() {
        assert_check_ok("core/examples/use_math_alias.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_counter_elyx_check() {
        assert_check_ok("core/examples/counter.elyx");
    }

    #[test]
    #[ignore]
    fn test_integration_async_parallel_check() {
        assert_check_ok("core/examples/async_parallel.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_highlight_ansi() {
        assert_highlight_ok("core/examples/hello.ely", "ansi");
    }

    #[test]
    #[ignore]
    fn test_integration_highlight_html() {
        assert_highlight_ok("core/examples/hello.ely", "html");
    }

    #[test]
    #[ignore]
    fn test_integration_highlight_todo_question() {
        assert_highlight_ok("core/examples/todo_question.ely", "ansi");
    }

    #[test]
    #[ignore]
    fn test_integration_highlight_bench() {
        assert_highlight_ok("core/examples/bench.ely", "ansi");
    }

    #[test]
    #[ignore]
    fn test_integration_lint_hello() {
        assert_lint_ok("core/examples/hello.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_lint_todo_question() {
        assert_lint_ok("core/examples/todo_question.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_lint_bench() {
        assert_lint_ok("core/examples/bench.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_doc_hello() {
        assert_doc_ok("core/examples/hello.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_doc_discount() {
        assert_doc_ok("core/examples/discount.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_doc_todo_question() {
        assert_doc_ok("core/examples/todo_question.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_doc_bench() {
        assert_doc_ok("core/examples/bench.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_dep_graph_hello() {
        assert_dep_graph_ok("core/examples/hello.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_dep_graph_discount() {
        assert_dep_graph_ok("core/examples/discount.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_dep_graph_bench() {
        assert_dep_graph_ok("core/examples/bench.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_gen_test_hello() {
        assert_gen_test_ok("core/examples/hello.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_gen_test_discount() {
        assert_gen_test_ok("core/examples/discount.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_gen_test_counter() {
        assert_gen_test_ok("core/examples/counter.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_spec_keywords_check() {
        assert_check_ok("core/examples/spec_keywords.ely");
    }

    #[test]
    #[ignore]
    fn test_integration_import_alias_check() {
        assert_check_ok("core/examples/use_math_alias.ely");
    }
}
