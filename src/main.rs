use artefacta::{
    cli::{Cli, Command},
    ArtefactIndex,
};
use erreur::{Context, Help, Result};
use structopt::StructOpt;

#[tokio::main]
async fn main() -> Result<()> {
    erreur::install_panic_handler()?;

    let args = Cli::from_args();
    setup_logging(args.verbose);

    log::debug!("{:?}", args);
    let mut index = ArtefactIndex::new(&args.local_store, args.remote_store.clone())
        .await
        .context("open artifact store")
        .note("Always use absolute paths. This is serious business, there is no room for doubt.")?;

    match args.cmd {
        Command::Debug => {
            dbg!(index);
        }
        Command::Sync => {
            artefacta::sync(&index).await?;
        }
        Command::Install { version } => {
            let current = args.local_store.join("current");
            artefacta::install(&mut index, version, &current).await?;
        }
        Command::AddPackage { version, build } => {
            artefacta::add_package(&mut index, version, build).await?;
        }
        Command::CreatePatch { from, to } => {
            artefacta::create_patch(&mut index, from, to).await?;
        }
        Command::AutoPatch {
            repo_root,
            current,
            prefix,
        } => {
            artefacta::auto_patch(&mut index, repo_root.as_ref(), current, &prefix).await?;
        }
        Command::Add(build) => artefacta::add(&mut index, build).await?,
    }

    Ok(())
}

fn setup_logging(verbose: bool) {
    let mut log = pretty_env_logger::formatted_timed_builder();
    log.target(env_logger::Target::Stderr);

    if verbose {
        log.filter(None, log::LevelFilter::Info)
            .filter(Some("artefacta"), log::LevelFilter::Debug);
    } else {
        log.filter(None, log::LevelFilter::Warn)
            .filter(Some("artefacta"), log::LevelFilter::Info);
    }

    if let Ok(s) = std::env::var("RUST_LOG") {
        log.parse_filters(&s);
    }

    log.init();
}
