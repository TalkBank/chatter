//! Unit tests for `args::core`.

use std::path::PathBuf;

use clap::{CommandFactory, Parser};

use super::Commands;
use crate::cli::args::{Cli, DebugCommands};

fn run_with_large_stack(test: impl FnOnce() + Send + 'static) {
    let join_result = std::thread::Builder::new()
        .name("core-args-test".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(test)
        .expect("spawn core args test thread")
        .join();
    match join_result {
        Ok(()) => {}
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

#[test]
fn to_xml_parses_output_flag() {
    run_with_large_stack(|| {
        let parsed = Cli::parse_from(["chatter", "to-xml", "sample.cha", "--output", "sample.xml"]);

        let Commands::ToXml {
            input,
            output,
            skip_alignment,
        } = parsed.command
        else {
            panic!("expected to-xml command");
        };

        assert_eq!(input, PathBuf::from("sample.cha"));
        assert_eq!(output, Some(PathBuf::from("sample.xml")));
        assert!(!skip_alignment);
    });
}

#[test]
fn help_shows_to_xml_and_not_from_xml() {
    run_with_large_stack(|| {
        let mut command = Cli::command();
        let mut help = Vec::new();
        command.write_long_help(&mut help).expect("render help");
        let rendered = String::from_utf8(help).expect("utf8 help");

        assert!(rendered.contains("to-xml"));
        assert!(!rendered.contains("from-xml"));
    });
}

#[test]
fn debug_fix_s_parses_path_arguments() {
    run_with_large_stack(|| {
        let parsed = Cli::parse_from(["chatter", "debug", "fix-s", "sample.cha"]);

        let Commands::Debug {
            command: DebugCommands::FixS { path },
        } = parsed.command
        else {
            panic!("expected debug fix-s command");
        };

        assert_eq!(path, vec![PathBuf::from("sample.cha")]);
    });
}
