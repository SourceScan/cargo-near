use std::process::Command;

use color_eyre::{eyre::WrapErr, owo_colors::OwoColorize};

pub mod build;

#[derive(Debug, Default, Clone, interactive_clap::InteractiveClap)]
#[interactive_clap(input_context = near_cli_rs::GlobalContext)]
#[interactive_clap(output_context = BuildCommandlContext)]
pub struct BuildCommand {
    /// Build contract without SourceScan verification
    #[interactive_clap(long)]
    pub no_docker: bool,
    /// Build contract in debug mode, without optimizations and bigger is size
    #[interactive_clap(long)]
    pub no_release: bool,
    /// Do not generate ABI for the contract
    #[interactive_clap(long)]
    pub no_abi: bool,
    /// Do not embed the ABI in the contract binary
    #[interactive_clap(long)]
    pub no_embed_abi: bool,
    /// Do not include rustdocs in the embedded ABI
    #[interactive_clap(long)]
    pub no_doc: bool,
    /// Copy final artifacts to this directory
    #[interactive_clap(long)]
    #[interactive_clap(skip_interactive_input)]
    pub out_dir: Option<crate::types::utf8_path_buf::Utf8PathBuf>,
    /// Path to the `Cargo.toml` of the contract to build
    #[interactive_clap(long)]
    #[interactive_clap(skip_interactive_input)]
    pub manifest_path: Option<crate::types::utf8_path_buf::Utf8PathBuf>,
    /// Coloring: auto, always, never
    #[interactive_clap(long)]
    #[interactive_clap(value_enum)]
    #[interactive_clap(skip_interactive_input)]
    pub color: Option<crate::common::ColorPreference>,
}

#[derive(Debug, Clone)]
pub struct BuildCommandlContext;

impl BuildCommandlContext {
    pub fn from_previous_context(
        _previous_context: near_cli_rs::GlobalContext,
        scope: &<BuildCommand as interactive_clap::ToInteractiveClapContextScope>::InteractiveClapContextScope,
    ) -> color_eyre::eyre::Result<Self> {
        let args = BuildCommand {
            no_docker: scope.no_docker,
            no_release: scope.no_release,
            no_abi: scope.no_abi,
            no_embed_abi: scope.no_embed_abi,
            no_doc: scope.no_doc,
            out_dir: scope.out_dir.clone(),
            manifest_path: scope.manifest_path.clone(),
            color: scope.color.clone(),
        };
        if args.no_docker {
            self::build::run(args)?;
        } else {
            docker_run(args)?;
        }
        Ok(Self)
    }
}

pub fn docker_run(args: BuildCommand) -> color_eyre::eyre::Result<camino::Utf8PathBuf> {
    let mut cargo_args = vec![];
    // Use this in new release version:
    // let mut cargo_args = vec!["--no-docker"];

    if args.no_abi {
        cargo_args.push("--no-abi")
    }
    if args.no_embed_abi {
        cargo_args.push("--no-embed-abi")
    }
    if args.no_doc {
        cargo_args.push("--no-doc")
    }
    let color = args
        .color
        .clone()
        .unwrap_or(crate::common::ColorPreference::Auto)
        .to_string();
    cargo_args.extend(&["--color", &color]);

    let mut contract_path: camino::Utf8PathBuf = if let Some(manifest_path) = &args.manifest_path {
        manifest_path.into()
    } else {
        camino::Utf8PathBuf::from_path_buf(std::env::current_dir()?).map_err(|err| {
            color_eyre::eyre::eyre!("failed to convert path {}", err.to_string_lossy())
        })?
    };

    let volume = format!("{contract_path}:/host");
    let mut docker_args = vec!["--name", "cargo-near-container", "-v", &volume];
    docker_args.extend(&[
        "--rm",
        "-it",
        "sourcescan/cargo-near:0.6.0", //XXX need to fix version!!!
        "bash",
        "-c",
        "cd /host && cargo near build",
    ]);
    docker_args.extend(&cargo_args);

    let mut docker_cmd = Command::new("docker");
    docker_cmd.arg("run");
    docker_cmd.args(docker_args);

    let status = match docker_cmd.status() {
        Ok(exit_status) => exit_status,
        Err(_) => {
            println!("Error executing SourceScan command `{:?}`", docker_cmd);
            println!(
                "{}",
                "WARNING! Compilation without SourceScan verification".red()
            );
            return Ok(self::build::run(args)?.path);
        }
    };

    if status.success() {
        contract_path.push("target");
        contract_path.push("near");

        let dir = contract_path
            .read_dir()
            .wrap_err_with(|| format!("No artifacts directory found: `{contract_path:?}`."))?;

        for entry in dir.flatten() {
            if entry.path().extension().unwrap().to_str().unwrap() == "wasm" {
                return camino::Utf8PathBuf::from_path_buf(entry.path()).map_err(|err| {
                    color_eyre::eyre::eyre!("failed to convert path {}", err.to_string_lossy())
                });
            }
        }

        Err(color_eyre::eyre::eyre!(
            "Wasm file not found in directory: `{contract_path:?}`."
        ))
    } else {
        println!(
            "SourceScan command `{:?}` failed with exit status: {status}",
            docker_cmd
        );
        println!(
            "{}",
            "WARNING! Compilation without SourceScan verification".red()
        );
        Ok(self::build::run(args)?.path)
    }
}
