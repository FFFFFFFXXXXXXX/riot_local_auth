use crate::credentials::*;
use crate::error::*;
use std::thread;
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use sysinfo::{Process, ProcessRefreshKind, System};

#[cfg(target_os = "windows")]
const TARGET_PROCESS: &str = "LeagueClientUx.exe";
#[cfg(target_os = "macos")]
const TARGET_PROCESS: &str = "LeagueClientUx";

static PROCESS_INFO: Mutex<Option<System>> = Mutex::new(None);

pub fn try_get_credentials() -> Result<Credentials> {
    match PROCESS_INFO.lock() {
        Ok(mut p_info) => get_processes(p_info.get_or_insert_with(System::new)),
        Err(e) => get_processes(e.into_inner().insert(System::new())),
    }
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
            result @ _ => return result,
        }

        thread::sleep(Duration::from_secs(1));
    }

    Err(Error::Timeout)
}

fn get_processes(process_info: &mut System) -> Result<Credentials> {
    process_info.refresh_processes_specifics(
        ProcessRefreshKind::new().with_cmd(sysinfo::UpdateKind::Always),
    );

    let credentials = process_info
        .processes_by_exact_name(TARGET_PROCESS)
        .map(Process::cmd)
        .find_map(|cmd| {
            let port = cmd
                .iter()
                .find_map(|arg| arg.strip_prefix("--app-port="))
                .and_then(|port| port.parse::<u16>().ok())?;
            let token = cmd
                .iter()
                .find_map(|arg| arg.strip_prefix("--remoting-auth-token="))
                .map(str::to_string)?;

            Some(Credentials { token, port })
        });

    credentials.ok_or(Error::ApiNotRunning)
}
