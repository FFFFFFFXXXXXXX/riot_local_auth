use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use std::{fs, thread};

use ureq::config::Config;
use ureq::tls::{Certificate, RootCerts, TlsConfig};
use ureq::{Agent, BodyReader};

use crate::error::{Error, Result};
use crate::{riot, Credentials};

static UREQ_AGENT: OnceLock<Agent> = OnceLock::new();

pub fn try_get_credentials() -> Result<Credentials> {
    let riot_credentials = riot::try_get_credentials()?;

    let ureq_agent = UREQ_AGENT.get_or_init(create_ureq_agent);
    let request = ureq_agent
        .get(&format!(
            "https://127.0.0.1:{}/patch/v1/installs/league_of_legends.live",
            riot_credentials.port,
        ))
        .header("Authorization", &riot_credentials.basic_auth());

    #[derive(serde::Deserialize)]
    struct InstallInfo {
        path: PathBuf,
    }

    let response = request.call().map_err(Box::new)?;
    let install_path =
        serde_json::from_reader::<BodyReader, InstallInfo>(response.into_body().into_reader())
            .map(|install_info| install_info.path)
            .map_err(Error::InstallInfoParse)?;

    let lockfile_content = fs::read_to_string(install_path.join("lockfile"))?;
    Credentials::try_from(lockfile_content)
}

pub fn get_credentials_blocking() -> Result<Credentials> {
    get_credentials_interal(None)
}

pub fn get_credentials_timeout(timeout: Duration) -> Result<Credentials> {
    get_credentials_interal(Some(timeout))
}

fn get_credentials_interal(timeout: Option<Duration>) -> Result<Credentials> {
    let timeout = timeout.unwrap_or(Duration::MAX);

    let now = Instant::now();
    while now.elapsed() < timeout {
        match try_get_credentials() {
            Err(Error::ApiNotRunning) => {}
            result => return result,
        }

        thread::sleep(Duration::from_secs(1));
    }

    Err(Error::Timeout)
}

fn create_ureq_agent() -> Agent {
    let cert = Certificate::from_pem(include_bytes!("../riotgames.pem").as_slice()).unwrap();
    let client_config = TlsConfig::builder()
        .root_certs(RootCerts::new_with_certs(&[cert]))
        .build();
    ureq::Agent::new_with_config(
        Config::builder()
            .https_only(true)
            .tls_config(client_config)
            .timeout_global(Some(Duration::from_millis(250)))
            .build(),
    )
}
