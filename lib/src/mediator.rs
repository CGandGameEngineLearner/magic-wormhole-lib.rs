

use color_eyre::eyre::{self, Ok, Context};
use futures::future::BoxFuture;
use std::path::PathBuf;
use magic_wormhole::{transfer::{self}, transit::Abilities};
use eyre::ErrReport;
use std::sync::Arc;
use indicatif::{ProgressBar};
use magic_wormhole::{ transit, MailboxConnection, Wormhole};

pub async fn try_send(paths_vec: Vec<PathBuf>, file_name_str: Option<String>, code_length: usize) 
-> eyre::Result<String, ErrReport> 
{
    let offer = make_send_offer(paths_vec, file_name_str).await?;
    
    let transit_abilities: Abilities = transit::Abilities::ALL_ABILITIES;

    let mailbox_connection: MailboxConnection<transfer::AppVersion> 
    = MailboxConnection::create(transfer::APP_CONFIG, code_length).await?;

    let wormhole_code: magic_wormhole::Code = mailbox_connection.code.clone();
    let wormhole: Wormhole = Wormhole::connect(mailbox_connection).await?;
    let mut relay_hints: Vec<transit::RelayHint> = Vec::new();
    relay_hints.push(transit::RelayHint::from_urls(
        None,
        [magic_wormhole::transit::DEFAULT_RELAY_SERVER
            .parse()
            .unwrap()],
    )?);
    
    let fn_cancel = cancel();

    let pb = create_progress_bar(0);
    let pb2 = pb.clone();
    transfer::send(
        wormhole,
        relay_hints,
        transit_abilities,
        offer,
        &transit::log_transit_connection,
        create_progress_handler(pb),
        fn_cancel(),
    )
    .await
    .context("Send process failed")?;
    pb2.finish();
   
    Ok(wormhole_code.0)
}

async fn make_send_offer(
    mut files: Vec<PathBuf>,
    file_name: Option<String>,
) -> eyre::Result<transfer::OfferSend> {
    for file in &files {
        eyre::ensure!(
            async_std::path::Path::new(&file).exists().await,
            "{} does not exist",
            file.display()
        );
    }
    log::trace!("Making send offer in {files:?}, with name {file_name:?}");

    match (files.len(), file_name) {
        (0, _) => unreachable!("Already checked by CLI parser"),
        (1, Some(file_name)) => {
            let file = files.remove(0);
            Ok(transfer::OfferSend::new_file_or_folder(file_name, file).await?)
        },
        (1, None) => {
            let file = files.remove(0);
            let file_name = file
                .file_name()
                .ok_or_else(|| {
                    eyre::format_err!("You can't send a file without a name. Maybe try --rename")
                })?
                .to_str()
                .ok_or_else(|| eyre::format_err!("File path must be a valid UTF-8 string"))?
                .to_owned();
            Ok(transfer::OfferSend::new_file_or_folder(file_name, file).await?)
        },
        (_, Some(_)) => Err(eyre::format_err!(
            "Can't customize file name when sending multiple files"
        )),
        (_, None) => {
            let mut names = std::collections::BTreeMap::new();
            for path in &files {
                eyre::ensure!(path.file_name().is_some(), "'{}' has no name. You need to send it separately and use the --rename flag, or rename it on the file system", path.display());
                if let Some(old) = names.insert(path.file_name(), path) {
                    eyre::bail!(
                        "'{}' and '{}' have the same file name. Rename one of them on disk, or send them in separate transfers", old.display(), path.display(),
                    );
                }
            }
            Ok(transfer::OfferSend::new_paths(files).await?)
        },
    }
}

fn cancel() -> impl Fn() -> BoxFuture<'static, ()> + Clone {
    let func = Arc::new(|| {
        Box::pin(async {
            // 什么都不做
        }) as BoxFuture<'static, ()>
    });

    move || {
        let func = func.clone();
        (func)()
    }
}

fn create_progress_bar(file_size: u64) -> ProgressBar {
    use indicatif::ProgressStyle;

    let pb = ProgressBar::new(file_size);
    pb.set_style(
        ProgressStyle::default_bar()
            // .template("[{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .template("[{elapsed_precise}] [{wide_bar}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb
}

fn create_progress_handler(pb: ProgressBar) -> impl FnMut(u64, u64) {
    move |sent, total| {
        if sent == 0 {
            pb.reset_elapsed();
            pb.set_length(total);
            pb.enable_steady_tick(std::time::Duration::from_millis(250));
        }
        pb.set_position(sent);
    }
}