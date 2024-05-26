use std::sync::Arc;
use futures::FutureExt;
use std::path::PathBuf;
use indicatif::ProgressBar;
use color_eyre::eyre::{self, Ok, Context,ErrReport};
use magic_wormhole::{ transfer::{self}, transit::Abilities,transit, MailboxConnection, Wormhole};

pub async fn try_send(paths_vec: Vec<PathBuf>, new_name_str: Option<String>, code_length: usize) 
-> eyre::Result<String, ErrReport> 
{
    let offer = make_send_offer(paths_vec, new_name_str).await?;
    
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
    

    let pb = create_progress_bar(0);
    let pb2 = pb.clone();
    transfer::send(
        wormhole,
        relay_hints,
        transit_abilities,
        offer,
        &transit::log_transit_connection,
        create_progress_handler(pb),
        do_nothing(),
    )
    .await
    .context("Send process failed")?;
    pb2.finish();
   
    Ok(wormhole_code.0)
}


pub async fn  try_recieve(wormhole_code:String, save_path: PathBuf)-> eyre::Result<bool,ErrReport> 
{
    let transit_abilities: Abilities = transit::Abilities::ALL_ABILITIES;
    
    
    let code = Some(wormhole_code)
        .map(Result::Ok)
        .or_else(|| (true).then(enter_code))
        .transpose()?
        .map(magic_wormhole::Code);

    match code {
        Some(code)=>{
            let mailbox_connection: MailboxConnection<transfer::AppVersion> 
            = MailboxConnection::connect(transfer::APP_CONFIG, code, true).await?;


            
            let wormhole: Wormhole = Wormhole::connect(mailbox_connection).await?;
            let mut relay_hints: Vec<transit::RelayHint> = Vec::new();
            relay_hints.push(transit::RelayHint::from_urls(
                None,
                [magic_wormhole::transit::DEFAULT_RELAY_SERVER
                    .parse()
                    .unwrap()],
            )?);
          
            let req = transfer::request(wormhole, relay_hints, transit_abilities, do_nothing()) .await
            .context("Could not get an offer")?;
            let ctrl_c = install_ctrlc_handler()?;
            match req {
                Some(transfer::ReceiveRequest::V1(req)) => {
                    receive_inner_v1(req, &save_path, true, ctrl_c).await?
                },
                Some(transfer::ReceiveRequest::V2(req)) => {
                    receive_inner_v2(req,&save_path,  ctrl_c).await?
                },
                None => {
                    return Ok(false);
                }
            }
        }
        None =>{
            return Ok(false);
        }
    }
    
    Ok(true)
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

async fn do_nothing() {}

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

fn enter_code() -> eyre::Result<String> {
    use dialoguer::Input;

    Input::new()
        .with_prompt("Enter code")
        .interact_text()
        .map_err(From::from)
}


async fn receive_inner_v1(
    req: transfer::ReceiveRequestV1,
    target_dir: &std::path::Path,
    noconfirm: bool,
    ctrl_c: impl Fn() -> futures::future::BoxFuture<'static, ()>,
) -> eyre::Result<()> {
    use async_std::fs::OpenOptions;

    /*
     * Control flow is a bit tricky here:
     * - First of all, we ask if we want to receive the file at all
     * - Then, we check if the file already exists
     * - If it exists, ask whether to overwrite and act accordingly
     * - If it doesn't, directly accept, but DON'T overwrite any files
     */ 

    // TODO validate untrusted input here
    let file_path = std::path::Path::new(target_dir).join(&req.filename);

    let pb = create_progress_bar(req.filesize);

    /* Then, accept if the file exists */
    if !file_path.exists() || noconfirm {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file_path)
            .await
            .context("Failed to create destination file")?;
        return req
            .accept(
                &transit::log_transit_connection,
                &mut file,
                create_progress_handler(pb),
                ctrl_c(),
            )
            .await
            .context("Receive process failed");
    }

    // /* If there is a collision, ask whether to overwrite */
    // if !util::ask_user(
    //     format!("Override existing file {}?", file_path.display()),
    //     false,
    // )
    // .await
    // {
    //     return req.reject().await.context("Could not reject offer");
    // }

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&file_path)
        .await?;
    req.accept(
        &transit::log_transit_connection,
        &mut file,
        create_progress_handler(pb),
        ctrl_c(),
    )
    .await
    .context("Receive process failed")
}

async fn receive_inner_v2(
    req: transfer::ReceiveRequestV2,
    target_dir: &std::path::Path,
    ctrl_c: impl Fn() -> futures::future::BoxFuture<'static, ()>,
) -> eyre::Result<()> {
    let offer = req.offer();
    let file_size = offer.total_size();

    

    let pb = create_progress_bar(file_size);

    let on_progress = move |received, _total| {
        pb.set_position(received);
    };

    /* Create a temporary directory for receiving */
    use rand::Rng;
    let tmp_dir = target_dir.join(format!(
        "wormhole-tmp-{:06}",
        rand::thread_rng().gen_range(0..1_000_000)
    ));
    async_std::fs::create_dir_all(&tmp_dir)
        .await
        .context("Failed to create temporary directory for receiving")?;

    /* Prepare the receive by creating all directories */
    offer.create_directories(&tmp_dir).await?;

    /* Accept the offer and receive it */
    let answer = offer.accept_all(&tmp_dir);
    req.accept(
        &transit::log_transit_connection,
        answer,
        on_progress,
        ctrl_c(),
    )
    .await
    .context("Receive process failed")?;

    // /* Put in all the symlinks last, this greatly reduces the attack surface */
    // offer.create_symlinks(&tmp_dir).await?;

    /* TODO walk the output directory and delete things we did not accept; this will be important for resumption */

    /* Move the received files to their target location */
    use futures::TryStreamExt;
    async_std::fs::read_dir(&tmp_dir)
    .await?
    .map_err(Into::into)
    .and_then(|file| {
        let tmp_dir = tmp_dir.clone();
        async move {
            let path = file.path();
            let name = path.file_name().expect("Internal error: this should never happen");
            let target_path = target_dir.join(name);

            /* This suffers some TOCTTOU, sorry about that: https://internals.rust-lang.org/t/rename-file-without-overriding-existing-target/17637 */
            if async_std::path::Path::new(&target_path).exists().await {
                eyre::bail!(
                    "Target destination {} exists, you can manually extract the file from {}",
                    target_path.display(),
                    tmp_dir.display(),
                );
            } else {
                async_std::fs::rename(&path, &target_path).await?;
            }
            Ok(())
        }})
    .try_collect::<()>()
    .await?;

    /* Delete the temporary directory */
    async_std::fs::remove_dir_all(&tmp_dir)
        .await
        .context(format!(
            "Failed to delete {}, please do it manually",
            tmp_dir.display()
        ))?;

    Ok(())
}

fn install_ctrlc_handler(
) -> eyre::Result<impl Fn() -> futures::future::BoxFuture<'static, ()> + Clone> {
    use async_std::sync::{Condvar, Mutex};

    let notifier = Arc::new((Mutex::new(false), Condvar::new()));

    /* Register the handler */
    let notifier2 = notifier.clone();
    ctrlc::set_handler(move || {
        futures::executor::block_on(async {
            let mut has_notified = notifier2.0.lock().await;
            if *has_notified {
                /* Second signal. Exit */
                log::debug!("Exit.");
                std::process::exit(130);
            }
            /* First signal. */
            log::info!("Got Ctrl-C event. Press again to exit immediately");
            *has_notified = true;
            notifier2.1.notify_all();
        })
    })
    .context("Error setting Ctrl-C handler")?;

    Ok(move || {
        /* Transform the notification into a future that waits */
        let notifier = notifier.clone();
        async move {
            let (lock, cvar) = &*notifier;
            let mut started = lock.lock().await;
            while !*started {
                started = cvar.wait(started).await;
            }
        }
        .boxed()
    })
}