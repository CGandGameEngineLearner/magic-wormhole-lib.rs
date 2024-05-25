

use color_eyre::eyre::{self, Ok};

use std::path::PathBuf;
use magic_wormhole::{transfer::{self, Offer}, transit::Abilities};
use eyre::ErrReport;
use magic_wormhole::{forwarding, transit, MailboxConnection, Wormhole};

pub async fn make_send_offer(
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

pub async fn try_send(paths_vec: Vec<PathBuf>, file_name_str: Option<String>, code_length: usize) -> eyre::Result<magic_wormhole::Code, ErrReport> {
    let offer= make_send_offer(paths_vec, file_name_str).await?;
    
    let transit_abilities: Abilities = transit::Abilities::ALL_ABILITIES;

    let mailbox_connection: MailboxConnection<transfer::AppVersion> = MailboxConnection::create(transfer::APP_CONFIG, code_length).await?;
    let wormhole_code: magic_wormhole::Code = mailbox_connection.code.clone();
    let wormhole = Wormhole::connect(mailbox_connection).await?;
    
    

   
    Ok(wormhole_code)
}


