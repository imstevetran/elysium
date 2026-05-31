//! `.elyx` UI component build and check.

use std::path::PathBuf;

use crate::ast;
use crate::elyx;
use crate::error;
use crate::ownership;
use crate::type_checker;

use super::source::read_source;

pub fn build_elyx(file: &PathBuf, _output: Option<PathBuf>, _emit_ir: bool) -> error::Result<()> {
    let source = read_source(file)?;

    let elyx_file = elyx::parse_elyx(&source)?;
    let component = &elyx_file.component;

    let component_name = component.value.name.clone();
    println!("Parsed .elyx component: {}", component_name);

    let program = ast::Program {
        items: vec![ast::Node::new(
            ast::Item::Component(component.value.clone()),
            component.span.clone(),
        )],
    };

    let mut type_checker = type_checker::TypeChecker::new();
    type_checker.check_program(&program)?;

    let mut ownership = ownership::OwnershipChecker::new();
    ownership.check_program(&program)?;

    println!("Type check passed for .elyx component: {}", component_name);
    Ok(())
}

pub fn check_elyx(file: &PathBuf) -> error::Result<()> {
    let source = read_source(file)?;
    let elyx_file = elyx::parse_elyx(&source)?;
    let component = &elyx_file.component;

    let component_name = component.value.name.clone();

    let program = ast::Program {
        items: vec![ast::Node::new(
            ast::Item::Component(component.value.clone()),
            component.span.clone(),
        )],
    };

    let mut type_checker = type_checker::TypeChecker::new();
    type_checker.check_program(&program)?;

    let mut ownership = ownership::OwnershipChecker::new();
    ownership.check_program(&program)?;

    println!("Type check passed for .elyx component: {}", component_name);
    Ok(())
}
