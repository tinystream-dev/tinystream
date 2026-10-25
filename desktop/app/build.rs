// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;
use std::{env, fs};

use graphql_client_codegen::{CodegenMode, GraphQLClientCodegenOptions, generate_module_token_stream_from_string};
use graphql_parser::query::Definition;

fn main() {
    let logo = "../../packages/shared/src/logo.ts";
    println!("cargo:rerun-if-changed={logo}");
    let source = fs::read_to_string(logo).expect("shared logo");
    let cut = source.split("export const CUT = '").nth(1).expect("logo path").split('\'').next().expect("logo path");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    fs::write(output.join("logo.rs"), format!("const LOGO: &str = {cut:?};")).expect("write logo");

    let schema = "../../packages/shared/schema.graphql";
    let fragments = "../../packages/shared/fragments.graphql";
    let query = "graphql/session.graphql";
    for path in [schema, fragments, query] {
        println!("cargo:rerun-if-changed={path}");
    }

    let shared = fs::read_to_string(fragments).expect("shared fragments");
    let mut shared = graphql_parser::parse_query::<String>(&shared).expect("valid shared fragments");
    shared.definitions.retain(|definition| {
        matches!(definition, Definition::Fragment(fragment) if matches!(fragment.name.as_str(), "Person" | "NotificationFields"))
    });
    let document = format!("{}\n{shared}", fs::read_to_string(query).expect("session operations"));
    let mut options = GraphQLClientCodegenOptions::new(CodegenMode::Cli);
    options.set_module_visibility(syn::parse_str("pub").expect("public visibility"));
    options.set_response_derives("Clone, Serialize".into());
    options.set_fragments_other_variant(true);
    let generated = generate_module_token_stream_from_string(&document, &PathBuf::from(schema), options)
        .expect("session operations match the shared schema");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    fs::write(output.join("graphql.rs"), generated.to_string()).expect("write generated API types");
}
